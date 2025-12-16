//! Thread Priority System
//!
//! Based on Mach4 kern/sched.h priority definitions (CMU 1988)
//!
//! This module defines the priority system used by the Mach scheduler.
//! Priorities range from 0 (highest) to 31 (lowest), with various bands
//! allocated for system threads, realtime, timeshare, and idle threads.
//!
//! ## Priority Bands
//!
//! ```text
//! 0-7:   Highest priority - interrupt handlers, critical paths
//! 8-15:  High priority - system threads
//! 16-23: Normal priority - user timeshare threads
//! 24-31: Low priority - background/idle threads
//! ```
//!
//! ## Priority Aging
//!
//! User priorities are aged based on CPU usage. A thread that uses its
//! entire quantum gets its priority raised (numerically higher = lower
//! priority). Threads that block frequently get priority boosts.

use core::cmp;

// ============================================================================
// Priority Constants
// ============================================================================

/// Number of priority levels (0-31)
pub const NRQS: usize = 32;

/// Number of run queue heads (for multi-level feedback queue)
pub const NRQS_MAX: usize = 128;

/// Minimum priority value (highest priority)
pub const MINPRI: i32 = 0;

/// Maximum priority value (lowest priority)
pub const MAXPRI: i32 = 31;

/// Default priority for new threads
pub const BASEPRI_DEFAULT: i32 = 16;

/// Highest timeshare priority
pub const BASEPRI_HIGHEST: i32 = 0;

/// Lowest timeshare priority
pub const BASEPRI_LOWEST: i32 = 31;

/// Priority for the null/idle thread
pub const IDLEPRI: i32 = 31;

/// Default user thread priority
pub const BASEPRI_USER: i32 = 16;

/// Priority for system threads
pub const BASEPRI_SYSTEM: i32 = 8;

/// Priority for kernel threads
pub const BASEPRI_KERNEL: i32 = 4;

/// Priority for realtime threads (highest band)
pub const BASEPRI_RTQUEUES: i32 = 0;

/// Priority for preemption threads
pub const BASEPRI_PREEMPT: i32 = 2;

// ============================================================================
// Priority Policy Constants
// ============================================================================

/// Standard timesharing policy
pub const POLICY_TIMESHARE: i32 = 1;

/// Round-robin policy
pub const POLICY_RR: i32 = 2;

/// First-in-first-out (FIFO) policy
pub const POLICY_FIFO: i32 = 4;

/// Fixed priority policy
pub const POLICY_FIXED: i32 = 3;

// ============================================================================
// Scheduler Constants
// ============================================================================

/// Ticks per second for scheduling decisions
pub const SCHED_TICK_RATE: u32 = 100;

/// Aging decay factor (percentage, e.g., 50 = 50%)
pub const SCHED_DECAY_FACTOR: u32 = 50;

/// Maximum CPU usage percentage before priority aging
pub const SCHED_MAX_CPU: u32 = 100;

/// Priority shift for each usage accumulation
pub const SCHED_SHIFT: u32 = 1;

/// Number of scheduler ticks before recomputing priorities
pub const SCHED_RECOMPUTE_TICKS: u32 = 4;

// ============================================================================
// Priority Type
// ============================================================================

/// A thread priority value
///
/// Priorities are in range 0-31, where 0 is highest priority and 31 is lowest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct Priority(i32);

impl Priority {
    /// Create a new priority value, clamped to valid range
    pub const fn new(value: i32) -> Self {
        let clamped = if value < MINPRI {
            MINPRI
        } else if value > MAXPRI {
            MAXPRI
        } else {
            value
        };
        Self(clamped)
    }

    /// Create priority without bounds checking
    ///
    /// # Safety
    /// The value must be in range [MINPRI, MAXPRI]
    pub const unsafe fn new_unchecked(value: i32) -> Self {
        Self(value)
    }

    /// Get the raw priority value
    pub const fn value(self) -> i32 {
        self.0
    }

    /// Create the highest priority (0)
    pub const fn highest() -> Self {
        Self(MINPRI)
    }

    /// Create the lowest priority (31)
    pub const fn lowest() -> Self {
        Self(MAXPRI)
    }

    /// Create idle priority
    pub const fn idle() -> Self {
        Self(IDLEPRI)
    }

    /// Create default user priority
    pub const fn default_user() -> Self {
        Self(BASEPRI_USER)
    }

    /// Create default system priority
    pub const fn system() -> Self {
        Self(BASEPRI_SYSTEM)
    }

    /// Create default kernel priority
    pub const fn kernel() -> Self {
        Self(BASEPRI_KERNEL)
    }

    /// Create realtime priority
    pub const fn realtime() -> Self {
        Self(BASEPRI_RTQUEUES)
    }

    /// Check if this is a system priority (0-15)
    pub const fn is_system(self) -> bool {
        self.0 < BASEPRI_USER
    }

    /// Check if this is a user priority (16-31)
    pub const fn is_user(self) -> bool {
        self.0 >= BASEPRI_USER
    }

    /// Check if this is idle priority
    pub const fn is_idle(self) -> bool {
        self.0 == IDLEPRI
    }

    /// Check if this is realtime priority
    pub const fn is_realtime(self) -> bool {
        self.0 <= BASEPRI_RTQUEUES
    }

    /// Lower priority (increase value, up to MAXPRI)
    pub fn lower(self) -> Self {
        Self::new(self.0 + 1)
    }

    /// Raise priority (decrease value, down to MINPRI)
    pub fn raise(self) -> Self {
        Self::new(self.0 - 1)
    }

    /// Lower priority by amount
    pub fn lower_by(self, amount: i32) -> Self {
        Self::new(self.0 + amount)
    }

    /// Raise priority by amount
    pub fn raise_by(self, amount: i32) -> Self {
        Self::new(self.0 - amount)
    }

    /// Get the run queue index for this priority
    pub const fn queue_index(self) -> usize {
        self.0 as usize
    }

    /// Compare priorities (returns true if self should run before other)
    pub fn should_preempt(self, other: Self) -> bool {
        // Lower numeric value = higher priority = should preempt
        self.0 < other.0
    }
}

impl From<i32> for Priority {
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

impl From<Priority> for i32 {
    fn from(pri: Priority) -> Self {
        pri.0
    }
}

// ============================================================================
// Scheduling Policy
// ============================================================================

/// Scheduling policy for a thread
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
pub enum SchedulingPolicy {
    /// Standard timesharing - dynamic priority based on CPU usage
    #[default]
    Timeshare = POLICY_TIMESHARE,
    /// Round-robin - fixed quantum rotation
    RoundRobin = POLICY_RR,
    /// FIFO - runs until blocks (no preemption by same-priority)
    Fifo = POLICY_FIFO,
    /// Fixed priority - no priority aging
    Fixed = POLICY_FIXED,
}

impl SchedulingPolicy {
    /// Create from raw policy value
    pub fn from_raw(value: i32) -> Option<Self> {
        match value {
            POLICY_TIMESHARE => Some(Self::Timeshare),
            POLICY_RR => Some(Self::RoundRobin),
            POLICY_FIFO => Some(Self::Fifo),
            POLICY_FIXED => Some(Self::Fixed),
            _ => None,
        }
    }

    /// Get raw policy value
    pub fn to_raw(self) -> i32 {
        self as i32
    }

    /// Check if this policy uses priority aging
    pub fn has_aging(self) -> bool {
        matches!(self, Self::Timeshare)
    }

    /// Check if this policy uses round-robin
    pub fn is_round_robin(self) -> bool {
        matches!(self, Self::Timeshare | Self::RoundRobin)
    }

    /// Check if this is a realtime policy
    pub fn is_realtime(self) -> bool {
        matches!(self, Self::Fifo | Self::RoundRobin)
    }
}

impl From<i32> for SchedulingPolicy {
    fn from(value: i32) -> Self {
        Self::from_raw(value).unwrap_or(Self::Timeshare)
    }
}

// ============================================================================
// Priority Info
// ============================================================================

/// Complete priority information for a thread
#[derive(Debug, Clone, Copy, Default)]
pub struct PriorityInfo {
    /// Base priority (set by user/system)
    pub base_priority: Priority,
    /// Current scheduled priority (after aging)
    pub sched_priority: Priority,
    /// Maximum priority allowed
    pub max_priority: Priority,
    /// Scheduling policy
    pub policy: SchedulingPolicy,
    /// Depressed priority (for priority inversion handling)
    pub depress_priority: Option<Priority>,
    /// Whether thread is depressed
    pub depressed: bool,
    /// CPU usage (0-100, for aging calculation)
    pub cpu_usage: u32,
    /// Ticks since last priority recomputation
    pub recompute_ticks: u32,
}

impl PriorityInfo {
    /// Create new priority info with defaults
    pub fn new() -> Self {
        Self {
            base_priority: Priority::default_user(),
            sched_priority: Priority::default_user(),
            max_priority: Priority::highest(),
            policy: SchedulingPolicy::Timeshare,
            depress_priority: None,
            depressed: false,
            cpu_usage: 0,
            recompute_ticks: 0,
        }
    }

    /// Create priority info for a kernel thread
    pub fn kernel_thread() -> Self {
        Self {
            base_priority: Priority::kernel(),
            sched_priority: Priority::kernel(),
            max_priority: Priority::highest(),
            policy: SchedulingPolicy::Fixed,
            depress_priority: None,
            depressed: false,
            cpu_usage: 0,
            recompute_ticks: 0,
        }
    }

    /// Create priority info for a system thread
    pub fn system_thread() -> Self {
        Self {
            base_priority: Priority::system(),
            sched_priority: Priority::system(),
            max_priority: Priority::highest(),
            policy: SchedulingPolicy::Fixed,
            depress_priority: None,
            depressed: false,
            cpu_usage: 0,
            recompute_ticks: 0,
        }
    }

    /// Create priority info for idle thread
    pub fn idle_thread() -> Self {
        Self {
            base_priority: Priority::idle(),
            sched_priority: Priority::idle(),
            max_priority: Priority::idle(),
            policy: SchedulingPolicy::Fixed,
            depress_priority: None,
            depressed: false,
            cpu_usage: 0,
            recompute_ticks: 0,
        }
    }

    /// Get effective priority (accounting for depression)
    pub fn effective_priority(&self) -> Priority {
        if self.depressed {
            self.depress_priority.unwrap_or(Priority::lowest())
        } else {
            self.sched_priority
        }
    }

    /// Set base priority and recalculate scheduled priority
    pub fn set_base_priority(&mut self, priority: Priority) {
        // Clamp to max allowed
        let priority = Priority::new(cmp::max(priority.value(), self.max_priority.value()));
        self.base_priority = priority;

        // If not depressed and not using aging, update sched priority immediately
        if !self.depressed && !self.policy.has_aging() {
            self.sched_priority = priority;
        }
    }

    /// Set maximum priority
    pub fn set_max_priority(&mut self, max: Priority) {
        self.max_priority = max;

        // Clamp base priority if needed
        if self.base_priority.value() < max.value() {
            self.base_priority = max;
        }
    }

    /// Depress priority (for priority inversion handling)
    pub fn depress(&mut self, depress_to: Priority) {
        if !self.depressed {
            self.depress_priority = Some(depress_to);
            self.depressed = true;
        }
    }

    /// Un-depress priority
    pub fn undepress(&mut self) {
        self.depressed = false;
        self.depress_priority = None;
    }

    /// Record CPU usage tick
    pub fn tick(&mut self) {
        if self.cpu_usage < SCHED_MAX_CPU {
            self.cpu_usage += 1;
        }
        self.recompute_ticks += 1;
    }

    /// Decay CPU usage (called periodically)
    pub fn decay_usage(&mut self) {
        self.cpu_usage = (self.cpu_usage * SCHED_DECAY_FACTOR) / 100;
    }

    /// Check if priority should be recomputed
    pub fn should_recompute(&self) -> bool {
        self.recompute_ticks >= SCHED_RECOMPUTE_TICKS
    }

    /// Recompute scheduled priority based on CPU usage
    pub fn recompute(&mut self) {
        if !self.policy.has_aging() {
            // Fixed priority policies don't age
            self.sched_priority = self.base_priority;
            return;
        }

        // Compute priority shift based on CPU usage
        let usage_shift = (self.cpu_usage >> SCHED_SHIFT) as i32;
        let new_pri = self.base_priority.value() + usage_shift;

        // Clamp to valid range
        self.sched_priority = Priority::new(new_pri);
        self.recompute_ticks = 0;
    }

    /// Give a priority boost (e.g., after blocking)
    pub fn boost(&mut self) {
        // Reduce CPU usage to improve priority
        self.cpu_usage = self.cpu_usage.saturating_sub(10);
        self.recompute();
    }
}

// ============================================================================
// Priority Conversion Helpers
// ============================================================================

/// Convert Unix nice value (-20 to 19) to Mach priority
pub fn nice_to_priority(nice: i32) -> Priority {
    // nice -20 = high priority (around 8)
    // nice 0 = normal priority (16)
    // nice 19 = low priority (around 28)
    let pri = BASEPRI_USER + ((nice * 12) / 20);
    Priority::new(pri)
}

/// Convert Mach priority to Unix nice value
pub fn priority_to_nice(pri: Priority) -> i32 {
    // Reverse of nice_to_priority
    let pri = pri.value();
    ((pri - BASEPRI_USER) * 20) / 12
}

/// Convert POSIX priority (higher = better) to Mach priority
pub fn posix_to_priority(posix_pri: i32, min: i32, max: i32) -> Priority {
    if max <= min {
        return Priority::default_user();
    }

    // Normalize to 0..1 then scale to Mach range
    let range = max - min;
    let normalized = ((posix_pri - min) * (MAXPRI - MINPRI)) / range;
    Priority::new(MAXPRI - normalized) // Invert: high POSIX = low Mach value
}

/// Convert Mach priority to POSIX priority
pub fn priority_to_posix(pri: Priority, min: i32, max: i32) -> i32 {
    if max <= min {
        return min;
    }

    let range = max - min;
    let normalized = ((MAXPRI - pri.value()) * range) / (MAXPRI - MINPRI);
    min + normalized
}

// ============================================================================
// Run Queue Index Calculation
// ============================================================================

/// Get run queue bucket for a priority
///
/// Groups similar priorities into the same bucket for run queue
/// implementation efficiency.
pub fn priority_to_bucket(pri: Priority) -> usize {
    // Simple 1:1 mapping for 32 queues
    pri.queue_index()
}

/// Find highest priority (lowest value) set in bitmap
pub fn find_first_runnable(bitmap: u32) -> Option<usize> {
    if bitmap == 0 {
        None
    } else {
        Some(bitmap.trailing_zeros() as usize)
    }
}

/// Set bit for priority in bitmap
pub fn set_runnable(bitmap: &mut u32, pri: Priority) {
    *bitmap |= 1 << pri.queue_index();
}

/// Clear bit for priority in bitmap
pub fn clear_runnable(bitmap: &mut u32, pri: Priority) {
    *bitmap &= !(1 << pri.queue_index());
}

// ============================================================================
// Statistics
// ============================================================================

/// Priority-related statistics
#[derive(Debug, Clone, Default)]
pub struct PriorityStats {
    /// Number of priority changes
    pub priority_changes: u64,
    /// Number of priority depressions
    pub depressions: u64,
    /// Number of priority boosts
    pub boosts: u64,
    /// Number of recomputations
    pub recomputations: u64,
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the priority subsystem
pub fn init() {
    // Priority is pure computation, no global state to initialize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_creation() {
        let pri = Priority::new(16);
        assert_eq!(pri.value(), 16);

        // Test clamping
        let too_low = Priority::new(-5);
        assert_eq!(too_low.value(), MINPRI);

        let too_high = Priority::new(100);
        assert_eq!(too_high.value(), MAXPRI);
    }

    #[test]
    fn test_priority_comparison() {
        let high = Priority::new(4);
        let low = Priority::new(20);

        assert!(high.should_preempt(low));
        assert!(!low.should_preempt(high));
    }

    #[test]
    fn test_priority_classification() {
        assert!(Priority::kernel().is_system());
        assert!(Priority::system().is_system());
        assert!(Priority::default_user().is_user());
        assert!(Priority::idle().is_idle());
        assert!(Priority::realtime().is_realtime());
    }

    #[test]
    fn test_priority_modify() {
        let pri = Priority::new(16);

        let lower = pri.lower();
        assert_eq!(lower.value(), 17);

        let higher = pri.raise();
        assert_eq!(higher.value(), 15);

        // Test bounds
        let max = Priority::lowest();
        assert_eq!(max.lower().value(), MAXPRI); // Can't go lower

        let min = Priority::highest();
        assert_eq!(min.raise().value(), MINPRI); // Can't go higher
    }

    #[test]
    fn test_scheduling_policy() {
        assert_eq!(
            SchedulingPolicy::from_raw(POLICY_TIMESHARE),
            Some(SchedulingPolicy::Timeshare)
        );
        assert!(SchedulingPolicy::Timeshare.has_aging());
        assert!(!SchedulingPolicy::Fixed.has_aging());
        assert!(SchedulingPolicy::Fifo.is_realtime());
    }

    #[test]
    fn test_priority_info() {
        let mut info = PriorityInfo::new();
        assert_eq!(info.base_priority.value(), BASEPRI_USER);

        info.set_base_priority(Priority::new(8));
        assert_eq!(info.base_priority.value(), 8);

        info.depress(Priority::lowest());
        assert!(info.depressed);
        assert_eq!(info.effective_priority().value(), MAXPRI);

        info.undepress();
        assert!(!info.depressed);
    }

    #[test]
    fn test_priority_aging() {
        let mut info = PriorityInfo::new();
        info.cpu_usage = 50;
        info.recompute();

        // With 50% usage and shift of 1, priority should be lowered
        assert!(info.sched_priority.value() > info.base_priority.value());
    }

    #[test]
    fn test_nice_conversion() {
        let nice_0 = nice_to_priority(0);
        assert_eq!(nice_0.value(), BASEPRI_USER);

        let nice_neg = nice_to_priority(-20);
        assert!(nice_neg.value() < BASEPRI_USER);

        let nice_pos = nice_to_priority(19);
        assert!(nice_pos.value() > BASEPRI_USER);
    }

    #[test]
    fn test_bitmap_operations() {
        let mut bitmap: u32 = 0;

        set_runnable(&mut bitmap, Priority::new(5));
        set_runnable(&mut bitmap, Priority::new(10));
        set_runnable(&mut bitmap, Priority::new(15));

        assert_eq!(find_first_runnable(bitmap), Some(5)); // Lowest index = highest priority

        clear_runnable(&mut bitmap, Priority::new(5));
        assert_eq!(find_first_runnable(bitmap), Some(10));
    }
}
