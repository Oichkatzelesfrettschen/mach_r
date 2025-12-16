//! Page Daemon (Pageout)
//!
//! Based on Mach4 vm/vm_pageout.h/c by Avadis Tevanian, Jr. (1986)
//!
//! The pageout daemon runs in the background to maintain free memory:
//! - Scans inactive pages and reclaims them
//! - Moves unreferenced active pages to inactive queue
//! - Writes dirty pages to their backing store (external pager)
//! - Handles memory pressure situations
//!
//! The daemon uses a two-handed clock algorithm:
//! - Front hand clears reference bits
//! - Back hand reclaims unreferenced pages

use alloc::collections::VecDeque;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

use crate::ipc::PortName;
use crate::mach_vm::vm_object::VmObjectId;
use crate::mach_vm::vm_page::{page_manager, PageFlags, PageManager, VmPage};

// ============================================================================
// Pageout Constants
// ============================================================================

/// Number of pages to scan per pageout iteration
pub const PAGEOUT_BURST: u32 = 32;

/// Minimum interval between pageout scans (ms)
pub const PAGEOUT_INTERVAL_MIN: u32 = 10;

/// Maximum interval between pageout scans (ms)
pub const PAGEOUT_INTERVAL_MAX: u32 = 1000;

/// Target inactive ratio (pages)
pub const INACTIVE_TARGET_RATIO: u32 = 3;

/// Laundry limit (pages being cleaned)
pub const LAUNDRY_MAX: u32 = 64;

// ============================================================================
// Pageout Statistics
// ============================================================================

/// Pageout statistics
#[derive(Debug, Default)]
pub struct PageoutStats {
    /// Pages scanned
    pub scanned: AtomicU64,
    /// Pages reclaimed (freed)
    pub reclaimed: AtomicU64,
    /// Pages written out (dirty)
    pub cleaned: AtomicU64,
    /// Pages moved to inactive
    pub deactivated: AtomicU64,
    /// Pages reactivated (referenced while inactive)
    pub reactivated: AtomicU64,
    /// Pages skipped (busy/wired)
    pub skipped: AtomicU64,
    /// Daemon wakeups
    pub wakeups: AtomicU64,
    /// Times memory was critical
    pub critical_events: AtomicU64,
}

impl PageoutStats {
    pub fn new() -> Self {
        Self::default()
    }
}

// ============================================================================
// Pageout Request
// ============================================================================

/// A page that needs to be written out
#[derive(Debug)]
pub struct PageoutRequest {
    /// Page number
    pub page_num: u32,
    /// Object the page belongs to
    pub object_id: VmObjectId,
    /// Offset in object
    pub offset: u64,
    /// Pager port to write to
    pub pager_port: PortName,
    /// Is page precious?
    pub precious: bool,
}

impl PageoutRequest {
    pub fn new(page_num: u32, object_id: VmObjectId, offset: u64, pager_port: PortName) -> Self {
        Self {
            page_num,
            object_id,
            offset,
            pager_port,
            precious: false,
        }
    }
}

// ============================================================================
// Laundry Queue
// ============================================================================

/// Queue of pages being cleaned (written to pager)
#[derive(Debug)]
pub struct LaundryQueue {
    /// Pages in laundry
    pages: VecDeque<PageoutRequest>,
    /// Maximum size
    max_size: u32,
    /// Currently cleaning count
    cleaning: AtomicU32,
}

impl LaundryQueue {
    pub fn new(max_size: u32) -> Self {
        Self {
            pages: VecDeque::new(),
            max_size,
            cleaning: AtomicU32::new(0),
        }
    }

    /// Add page to laundry
    pub fn add(&mut self, request: PageoutRequest) -> bool {
        if self.pages.len() as u32 >= self.max_size {
            return false;
        }
        self.pages.push_back(request);
        true
    }

    /// Get next page to clean
    pub fn get_next(&mut self) -> Option<PageoutRequest> {
        self.pages.pop_front()
    }

    /// Mark cleaning started
    pub fn start_cleaning(&self) {
        self.cleaning.fetch_add(1, Ordering::Relaxed);
    }

    /// Mark cleaning completed
    pub fn done_cleaning(&self) {
        self.cleaning.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get number of pages being cleaned
    pub fn cleaning_count(&self) -> u32 {
        self.cleaning.load(Ordering::Relaxed)
    }

    /// Get queue length
    pub fn len(&self) -> usize {
        self.pages.len()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// Check if laundry is full
    pub fn is_full(&self) -> bool {
        self.pages.len() as u32 >= self.max_size
    }
}

impl Default for LaundryQueue {
    fn default() -> Self {
        Self::new(LAUNDRY_MAX)
    }
}

// ============================================================================
// Pageout Daemon State
// ============================================================================

/// Pageout daemon state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum DaemonState {
    /// Daemon not started
    Stopped = 0,
    /// Daemon sleeping (waiting for work)
    Sleeping = 1,
    /// Daemon running (scanning pages)
    Running = 2,
    /// Daemon paused
    Paused = 3,
}

// ============================================================================
// Pageout Daemon
// ============================================================================

/// The pageout daemon
pub struct PageoutDaemon {
    /// Daemon state
    state: Mutex<DaemonState>,

    /// Should daemon run?
    should_run: AtomicBool,

    /// Current scan interval (ms)
    interval: AtomicU32,

    /// Laundry queue
    laundry: Mutex<LaundryQueue>,

    /// Statistics
    pub stats: PageoutStats,

    /// Free pages target
    free_target: AtomicU32,

    /// Free pages minimum (critical threshold)
    free_min: AtomicU32,

    /// Inactive pages target
    inactive_target: AtomicU32,

    /// Last scan position in inactive queue
    scan_position: AtomicU32,
}

impl PageoutDaemon {
    /// Create a new pageout daemon
    pub fn new() -> Self {
        Self {
            state: Mutex::new(DaemonState::Stopped),
            should_run: AtomicBool::new(false),
            interval: AtomicU32::new(PAGEOUT_INTERVAL_MAX),
            laundry: Mutex::new(LaundryQueue::default()),
            stats: PageoutStats::new(),
            free_target: AtomicU32::new(0),
            free_min: AtomicU32::new(0),
            inactive_target: AtomicU32::new(0),
            scan_position: AtomicU32::new(0),
        }
    }

    /// Configure thresholds based on memory size
    pub fn configure(&self, total_pages: u32) {
        // Target 5% free
        self.free_target.store(total_pages / 20, Ordering::Relaxed);
        // Minimum 2% free (critical)
        self.free_min.store(total_pages / 50, Ordering::Relaxed);
        // Target inactive = 1/3 of non-wired pages
        self.inactive_target
            .store(total_pages / INACTIVE_TARGET_RATIO, Ordering::Relaxed);
    }

    /// Start the daemon
    pub fn start(&self) {
        self.should_run.store(true, Ordering::Release);
        *self.state.lock() = DaemonState::Sleeping;
    }

    /// Stop the daemon
    pub fn stop(&self) {
        self.should_run.store(false, Ordering::Release);
        *self.state.lock() = DaemonState::Stopped;
    }

    /// Wake up the daemon
    pub fn wakeup(&self) {
        self.stats.wakeups.fetch_add(1, Ordering::Relaxed);
        // In a real implementation, would signal the daemon thread
    }

    /// Check if daemon should run
    pub fn should_run(&self) -> bool {
        self.should_run.load(Ordering::Acquire)
    }

    /// Get current state
    pub fn state(&self) -> DaemonState {
        *self.state.lock()
    }

    /// Set scan interval
    pub fn set_interval(&self, ms: u32) {
        let ms = ms.clamp(PAGEOUT_INTERVAL_MIN, PAGEOUT_INTERVAL_MAX);
        self.interval.store(ms, Ordering::Relaxed);
    }

    /// Get scan interval
    pub fn get_interval(&self) -> u32 {
        self.interval.load(Ordering::Relaxed)
    }

    /// Check if we need to reclaim pages
    pub fn needs_pages(&self) -> bool {
        let pm = page_manager();
        let mgr = pm.lock();
        let stats = mgr.stats();
        stats.free < self.free_target.load(Ordering::Relaxed)
    }

    /// Check if memory is critically low
    pub fn is_critical(&self) -> bool {
        let pm = page_manager();
        let mgr = pm.lock();
        let stats = mgr.stats();
        stats.free < self.free_min.load(Ordering::Relaxed)
    }

    /// Run one iteration of the pageout daemon
    ///
    /// Returns the number of pages reclaimed
    pub fn run_iteration(&self) -> u32 {
        if !self.should_run() {
            return 0;
        }

        *self.state.lock() = DaemonState::Running;

        let mut reclaimed = 0;
        let pm = page_manager();

        // Check if we need to do work
        if !self.needs_pages() && !self.needs_inactive() {
            *self.state.lock() = DaemonState::Sleeping;
            return 0;
        }

        if self.is_critical() {
            self.stats.critical_events.fetch_add(1, Ordering::Relaxed);
            // Adjust interval for faster scanning
            self.set_interval(PAGEOUT_INTERVAL_MIN);
        }

        // PHASE 1: Run the clock hand over active pages to clear reference bits
        // This is the "front hand" of the two-handed clock
        let active_to_scan = PAGEOUT_BURST / 2;
        reclaimed += self.scan_active_pages(&pm, active_to_scan);

        // PHASE 2: Scan inactive pages and reclaim unreferenced ones
        // This is the "back hand" of the two-handed clock
        let pages_to_scan = PAGEOUT_BURST.min(self.shortage());
        reclaimed += self.scan_inactive_pages(&pm, pages_to_scan);

        // PHASE 3: Process laundry (pages that were written)
        let _cleaned = self.process_laundry();

        // PHASE 4: Move some active pages to inactive if needed
        if self.needs_inactive() {
            self.balance_queues();
        }

        // Adjust interval based on pressure
        self.adjust_interval();

        self.stats
            .reclaimed
            .fetch_add(reclaimed as u64, Ordering::Relaxed);
        *self.state.lock() = DaemonState::Sleeping;

        reclaimed
    }

    /// Scan active pages using clock algorithm (front hand)
    ///
    /// Clears reference bits on active pages. Pages that are not referenced
    /// when scanned twice will be moved to inactive.
    fn scan_active_pages(&self, pm: &Mutex<PageManager>, count: u32) -> u32 {
        let mut deactivated = 0;

        for _ in 0..count {
            let mut mgr = pm.lock();
            let stats = mgr.stats();

            if stats.active == 0 {
                break;
            }

            // Get the next page from active queue (clock hand position)
            let page_num = match mgr.dequeue_active() {
                Some(num) => num,
                None => break,
            };

            // Check the page's reference bit
            if let Some(page) = mgr.get_page(page_num) {
                self.stats.scanned.fetch_add(1, Ordering::Relaxed);

                // Check if page can be reclaimed
                if page.is_wired() || page.is_busy() {
                    // Cannot touch wired or busy pages - put back at end
                    mgr.enqueue_active(page_num);
                    self.stats.skipped.fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                if page.is_referenced() {
                    // Page was referenced - clear bit and put back at end (second chance)
                    page.clear_referenced();
                    mgr.enqueue_active(page_num);
                } else {
                    // Page not referenced - move to inactive
                    drop(mgr); // Release lock before deactivate
                    page_manager().lock().deactivate(page_num);
                    deactivated += 1;
                    self.stats.deactivated.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        deactivated
    }

    /// Scan inactive pages and reclaim unreferenced ones (back hand)
    fn scan_inactive_pages(&self, pm: &Mutex<PageManager>, count: u32) -> u32 {
        let mut reclaimed = 0;

        for _ in 0..count {
            let mut mgr = pm.lock();
            let stats = mgr.stats();

            if stats.inactive == 0 {
                break;
            }

            // Get the next page from inactive queue
            let page_num = match mgr.dequeue_inactive() {
                Some(num) => num,
                None => break,
            };

            // Process this page
            if let Some(page) = mgr.get_page(page_num) {
                self.stats.scanned.fetch_add(1, Ordering::Relaxed);

                // Skip wired or busy pages
                if page.is_wired() || page.is_busy() {
                    mgr.enqueue_inactive(page_num);
                    self.stats.skipped.fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                // Check if page was referenced while inactive (reactivate it)
                if page.is_referenced() {
                    page.clear_referenced();
                    drop(mgr);
                    page_manager().lock().activate(page_num);
                    self.stats.reactivated.fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                // Page is a candidate for reclamation
                if page.is_dirty() {
                    // Dirty page - needs to be written out first
                    if let Some(request) = self.setup_pageout(page) {
                        if self.queue_pageout(request) {
                            // Page is now in laundry, will be freed when clean completes
                            self.stats.cleaned.fetch_add(1, Ordering::Relaxed);
                        } else {
                            // Laundry full - put back
                            mgr.enqueue_inactive(page_num);
                        }
                    } else {
                        // Cannot pageout - put back
                        mgr.enqueue_inactive(page_num);
                    }
                } else {
                    // Clean page - can be reclaimed immediately
                    drop(mgr);
                    page_manager().lock().free(page_num);
                    reclaimed += 1;
                }
            }
        }

        reclaimed
    }

    /// Calculate page shortage
    fn shortage(&self) -> u32 {
        let pm = page_manager();
        let mgr = pm.lock();
        let stats = mgr.stats();
        let target = self.free_target.load(Ordering::Relaxed);

        target.saturating_sub(stats.free)
    }

    /// Check if we need more inactive pages
    fn needs_inactive(&self) -> bool {
        let pm = page_manager();
        let mgr = pm.lock();
        let stats = mgr.stats();
        stats.inactive < self.inactive_target.load(Ordering::Relaxed)
    }

    /// Balance active/inactive queues
    fn balance_queues(&self) {
        let inactive_target = self.inactive_target.load(Ordering::Relaxed);
        let pm = page_manager();
        let mgr = pm.lock();
        let stats = mgr.stats();

        if stats.inactive >= inactive_target {
            return;
        }

        let needed = inactive_target - stats.inactive;
        let to_deactivate = needed.min(stats.active / 10); // Move at most 10% at a time

        // In a real implementation, would move pages from active to inactive
        // based on reference bits

        self.stats
            .deactivated
            .fetch_add(to_deactivate as u64, Ordering::Relaxed);
    }

    /// Process laundry queue (completed writes)
    fn process_laundry(&self) -> u32 {
        let mut laundry = self.laundry.lock();
        let mut processed = 0;

        while let Some(_request) = laundry.get_next() {
            // In real implementation:
            // 1. Check if write completed successfully
            // 2. Clear dirty bit
            // 3. Move page to free queue
            self.stats.cleaned.fetch_add(1, Ordering::Relaxed);
            processed += 1;
        }

        processed
    }

    /// Adjust scan interval based on memory pressure
    fn adjust_interval(&self) {
        let shortage = self.shortage();

        let interval = if shortage == 0 {
            PAGEOUT_INTERVAL_MAX
        } else if self.is_critical() {
            PAGEOUT_INTERVAL_MIN
        } else {
            // Linear interpolation
            let target = self.free_target.load(Ordering::Relaxed);
            let ratio = (shortage * 100) / target.max(1);
            let range = PAGEOUT_INTERVAL_MAX - PAGEOUT_INTERVAL_MIN;
            PAGEOUT_INTERVAL_MAX - (range * ratio.min(100)) / 100
        };

        self.set_interval(interval);
    }

    /// Setup a page for pageout
    ///
    /// Prepares a page to be written to its pager
    pub fn setup_pageout(&self, page: &VmPage) -> Option<PageoutRequest> {
        // Check if page can be paged out
        if page.is_wired() || page.is_busy() {
            self.stats.skipped.fetch_add(1, Ordering::Relaxed);
            return None;
        }

        // Lock the page
        if !page.lock() {
            self.stats.skipped.fetch_add(1, Ordering::Relaxed);
            return None;
        }

        // Get object info
        let object_id = page.get_object()?;
        let offset = page.offset.load(Ordering::Relaxed);

        // Mark page as being laundered
        page.set_flags(PageFlags::LAUNDRY);

        // In a real implementation, would look up the pager port
        // from the object's memory_object
        let pager_port = PortName(0); // Placeholder

        let request = PageoutRequest::new(page.page_num, object_id, offset, pager_port);

        Some(request)
    }

    /// Queue a page for pageout
    pub fn queue_pageout(&self, request: PageoutRequest) -> bool {
        let mut laundry = self.laundry.lock();
        laundry.add(request)
    }

    /// Get daemon statistics
    pub fn get_stats(&self) -> PageoutSnapshot {
        PageoutSnapshot {
            state: self.state(),
            interval: self.get_interval(),
            scanned: self.stats.scanned.load(Ordering::Relaxed),
            reclaimed: self.stats.reclaimed.load(Ordering::Relaxed),
            cleaned: self.stats.cleaned.load(Ordering::Relaxed),
            deactivated: self.stats.deactivated.load(Ordering::Relaxed),
            reactivated: self.stats.reactivated.load(Ordering::Relaxed),
            skipped: self.stats.skipped.load(Ordering::Relaxed),
            wakeups: self.stats.wakeups.load(Ordering::Relaxed),
            critical_events: self.stats.critical_events.load(Ordering::Relaxed),
            laundry_size: self.laundry.lock().len() as u32,
            free_target: self.free_target.load(Ordering::Relaxed),
            free_min: self.free_min.load(Ordering::Relaxed),
        }
    }
}

impl Default for PageoutDaemon {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of pageout daemon state
#[derive(Debug, Clone)]
pub struct PageoutSnapshot {
    pub state: DaemonState,
    pub interval: u32,
    pub scanned: u64,
    pub reclaimed: u64,
    pub cleaned: u64,
    pub deactivated: u64,
    pub reactivated: u64,
    pub skipped: u64,
    pub wakeups: u64,
    pub critical_events: u64,
    pub laundry_size: u32,
    pub free_target: u32,
    pub free_min: u32,
}

// ============================================================================
// Global State
// ============================================================================

static PAGEOUT_DAEMON: spin::Once<PageoutDaemon> = spin::Once::new();

fn pageout_daemon() -> &'static PageoutDaemon {
    PAGEOUT_DAEMON.call_once(PageoutDaemon::new);
    PAGEOUT_DAEMON.get().unwrap()
}

/// Initialize pageout daemon
pub fn init() {
    let _ = pageout_daemon();
}

/// Configure pageout daemon with memory size
pub fn configure(total_pages: u32) {
    pageout_daemon().configure(total_pages);
}

/// Start the pageout daemon
pub fn start() {
    pageout_daemon().start();
}

/// Stop the pageout daemon
pub fn stop() {
    pageout_daemon().stop();
}

/// Wake up the pageout daemon
pub fn wakeup() {
    pageout_daemon().wakeup();
}

/// Check if daemon is running
pub fn is_running() -> bool {
    pageout_daemon().state() == DaemonState::Running
}

/// Check if memory is critically low
pub fn memory_critical() -> bool {
    pageout_daemon().is_critical()
}

/// Run one pageout iteration (for testing/debugging)
pub fn run_once() -> u32 {
    pageout_daemon().run_iteration()
}

/// Get pageout statistics
pub fn stats() -> PageoutSnapshot {
    pageout_daemon().get_stats()
}

/// Setup page for pageout
pub fn vm_pageout_setup(page: &VmPage) -> Option<PageoutRequest> {
    pageout_daemon().setup_pageout(page)
}

/// Queue page for pageout
pub fn vm_pageout_page(request: PageoutRequest) -> bool {
    pageout_daemon().queue_pageout(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_laundry_queue() {
        let mut laundry = LaundryQueue::new(4);

        assert!(laundry.is_empty());
        assert!(!laundry.is_full());

        for i in 0..4 {
            let req = PageoutRequest::new(i, VmObjectId(1), i as u64 * 4096, PortName(100));
            assert!(laundry.add(req));
        }

        assert!(laundry.is_full());

        let req = PageoutRequest::new(99, VmObjectId(1), 0, PortName(100));
        assert!(!laundry.add(req)); // Queue full

        assert_eq!(laundry.get_next().map(|r| r.page_num), Some(0));
        assert_eq!(laundry.len(), 3);
    }

    #[test]
    fn test_pageout_daemon() {
        let daemon = PageoutDaemon::new();

        assert_eq!(daemon.state(), DaemonState::Stopped);
        assert!(!daemon.should_run());

        daemon.configure(1000);
        daemon.start();

        assert_eq!(daemon.state(), DaemonState::Sleeping);
        assert!(daemon.should_run());

        daemon.stop();
        assert!(!daemon.should_run());
    }

    #[test]
    fn test_interval_clamping() {
        let daemon = PageoutDaemon::new();

        daemon.set_interval(5); // Too low
        assert_eq!(daemon.get_interval(), PAGEOUT_INTERVAL_MIN);

        daemon.set_interval(5000); // Too high
        assert_eq!(daemon.get_interval(), PAGEOUT_INTERVAL_MAX);

        daemon.set_interval(500); // Valid
        assert_eq!(daemon.get_interval(), 500);
    }
}
