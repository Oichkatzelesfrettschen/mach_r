//! Locking Primitives
//!
//! Based on Mach4 kern/lock.h/c by Avadis Tevanian, Jr. and Michael Wayne Young
//!
//! Provides:
//! - Simple spin locks for short critical sections
//! - Read/write locks for multiple-reader, single-writer scenarios
//! - Mutex locks for sleeping
//!
//! Rust's ownership model provides many of the guarantees that Mach's locks
//! were designed to provide. These primitives are provided for compatibility
//! and for cases where the spin-based approach is more appropriate.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

// ============================================================================
// Simple Lock (Spin Lock)
// ============================================================================

/// A simple spin lock
///
/// This is the basic locking primitive. It spins waiting for the lock
/// to become available. Should only be used for very short critical sections.
#[repr(C)]
pub struct SimpleLock {
    lock_data: AtomicBool,
}

impl core::fmt::Debug for SimpleLock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SimpleLock")
            .field("locked", &self.is_locked())
            .finish()
    }
}

impl SimpleLock {
    /// Create a new unlocked simple lock
    pub const fn new() -> Self {
        Self {
            lock_data: AtomicBool::new(false),
        }
    }

    /// Initialize the lock
    pub fn init(&self) {
        self.lock_data.store(false, Ordering::Release);
    }

    /// Acquire the lock, spinning until available
    pub fn lock(&self) {
        while self
            .lock_data
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Spin with a hint to the CPU
            while self.lock_data.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
    }

    /// Release the lock
    pub fn unlock(&self) {
        self.lock_data.store(false, Ordering::Release);
    }

    /// Try to acquire the lock without blocking
    pub fn try_lock(&self) -> bool {
        self.lock_data
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    /// Check if the lock is held
    pub fn is_locked(&self) -> bool {
        self.lock_data.load(Ordering::Relaxed)
    }
}

impl Default for SimpleLock {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for SimpleLock {}
unsafe impl Sync for SimpleLock {}

// ============================================================================
// Mutex (for compatibility)
// ============================================================================

/// Mutex type alias for simple lock
pub type Mutex = SimpleLock;

impl Mutex {
    pub fn mutex_init(&self) {
        self.init();
    }

    pub fn mutex_lock(&self) {
        self.lock();
    }

    pub fn mutex_unlock(&self) {
        self.unlock();
    }

    pub fn mutex_try(&self) -> bool {
        self.try_lock()
    }
}

// ============================================================================
// Read/Write Lock
// ============================================================================

/// Lock state constants
const LOCK_FREE: u32 = 0;
const LOCK_WRITE: u32 = 0x80000000;
const LOCK_READ_MASK: u32 = 0x7FFFFFFF;

/// A read/write lock
///
/// Allows multiple concurrent readers or a single writer.
/// Writers have priority to prevent starvation.
#[repr(C)]
pub struct RwLock {
    /// Lock state: high bit = write lock, low 31 bits = reader count
    state: AtomicU32,

    /// Writer waiting flag
    want_write: AtomicBool,

    /// Upgrade waiting flag
    want_upgrade: AtomicBool,

    /// Can this lock sleep?
    can_sleep: AtomicBool,

    /// Is someone waiting to be woken?
    waiting: AtomicBool,

    /// Recursion depth for recursive locking
    recursion_depth: AtomicU32,

    /// Thread holding write lock (for recursive locking)
    holder: AtomicUsize,

    /// Interlock for modifying the lock structure
    interlock: SimpleLock,
}

impl RwLock {
    /// Create a new unlocked read/write lock
    pub const fn new() -> Self {
        Self {
            state: AtomicU32::new(LOCK_FREE),
            want_write: AtomicBool::new(false),
            want_upgrade: AtomicBool::new(false),
            can_sleep: AtomicBool::new(true),
            waiting: AtomicBool::new(false),
            recursion_depth: AtomicU32::new(0),
            holder: AtomicUsize::new(0),
            interlock: SimpleLock::new(),
        }
    }

    /// Initialize the lock
    pub fn init(&self, can_sleep: bool) {
        self.state.store(LOCK_FREE, Ordering::Release);
        self.want_write.store(false, Ordering::Release);
        self.want_upgrade.store(false, Ordering::Release);
        self.can_sleep.store(can_sleep, Ordering::Release);
        self.waiting.store(false, Ordering::Release);
        self.recursion_depth.store(0, Ordering::Release);
        self.holder.store(0, Ordering::Release);
        self.interlock.init();
    }

    /// Set whether the lock can sleep
    pub fn set_sleepable(&self, can_sleep: bool) {
        self.can_sleep.store(can_sleep, Ordering::Release);
    }

    /// Acquire the lock for writing (exclusive access)
    pub fn write(&self) {
        self.interlock.lock();

        loop {
            let state = self.state.load(Ordering::Relaxed);

            // If lock is free, acquire for writing
            if state == LOCK_FREE {
                self.state.store(LOCK_WRITE, Ordering::Release);
                self.interlock.unlock();
                return;
            }

            // Mark that a writer is waiting
            self.want_write.store(true, Ordering::Release);
            self.interlock.unlock();

            // Spin waiting
            while self.state.load(Ordering::Relaxed) != LOCK_FREE {
                core::hint::spin_loop();
            }

            self.interlock.lock();
        }
    }

    /// Acquire the lock for reading (shared access)
    pub fn read(&self) {
        self.interlock.lock();

        loop {
            let state = self.state.load(Ordering::Relaxed);

            // Can acquire for read if:
            // - No write lock held
            // - No writer waiting (to prevent starvation)
            if (state & LOCK_WRITE) == 0 && !self.want_write.load(Ordering::Relaxed) {
                self.state.fetch_add(1, Ordering::AcqRel);
                self.interlock.unlock();
                return;
            }

            self.interlock.unlock();

            // Spin waiting
            while self.state.load(Ordering::Relaxed) & LOCK_WRITE != 0
                || self.want_write.load(Ordering::Relaxed)
            {
                core::hint::spin_loop();
            }

            self.interlock.lock();
        }
    }

    /// Release the lock (works for both read and write)
    pub fn done(&self) {
        self.interlock.lock();

        let state = self.state.load(Ordering::Relaxed);

        if state & LOCK_WRITE != 0 {
            // Releasing write lock
            self.state.store(LOCK_FREE, Ordering::Release);
            self.want_write.store(false, Ordering::Release);
            self.holder.store(0, Ordering::Release);
        } else if state > 0 {
            // Releasing read lock
            self.state.fetch_sub(1, Ordering::AcqRel);
        }

        self.interlock.unlock();
    }

    /// Try to acquire the lock for writing without blocking
    pub fn try_write(&self) -> bool {
        if !self.interlock.try_lock() {
            return false;
        }

        let state = self.state.load(Ordering::Relaxed);

        if state == LOCK_FREE {
            self.state.store(LOCK_WRITE, Ordering::Release);
            self.interlock.unlock();
            true
        } else {
            self.interlock.unlock();
            false
        }
    }

    /// Try to acquire the lock for reading without blocking
    pub fn try_read(&self) -> bool {
        if !self.interlock.try_lock() {
            return false;
        }

        let state = self.state.load(Ordering::Relaxed);

        if (state & LOCK_WRITE) == 0 && !self.want_write.load(Ordering::Relaxed) {
            self.state.fetch_add(1, Ordering::AcqRel);
            self.interlock.unlock();
            true
        } else {
            self.interlock.unlock();
            false
        }
    }

    /// Upgrade from read lock to write lock
    ///
    /// Returns true if upgrade was successful, false if someone else
    /// is already trying to upgrade (in which case you must release
    /// the read lock and acquire a write lock normally).
    pub fn read_to_write(&self) -> bool {
        self.interlock.lock();

        // Check if someone is already upgrading
        if self.want_upgrade.load(Ordering::Relaxed) {
            self.interlock.unlock();
            return false;
        }

        // Mark that we want to upgrade
        self.want_upgrade.store(true, Ordering::Release);

        // Release our read lock
        self.state.fetch_sub(1, Ordering::AcqRel);

        // Wait for all other readers to leave
        loop {
            let state = self.state.load(Ordering::Relaxed);

            if state == LOCK_FREE {
                self.state.store(LOCK_WRITE, Ordering::Release);
                self.want_upgrade.store(false, Ordering::Release);
                self.interlock.unlock();
                return true;
            }

            self.interlock.unlock();

            while self.state.load(Ordering::Relaxed) != LOCK_FREE {
                core::hint::spin_loop();
            }

            self.interlock.lock();
        }
    }

    /// Downgrade from write lock to read lock
    pub fn write_to_read(&self) {
        self.interlock.lock();

        // Convert write lock to read lock
        self.state.store(1, Ordering::Release);
        self.want_write.store(false, Ordering::Release);

        self.interlock.unlock();
    }

    /// Get the current reader count
    pub fn read_count(&self) -> u32 {
        self.state.load(Ordering::Relaxed) & LOCK_READ_MASK
    }

    /// Check if the lock is held for writing
    pub fn is_write_locked(&self) -> bool {
        self.state.load(Ordering::Relaxed) & LOCK_WRITE != 0
    }

    /// Check if the lock is held for reading
    pub fn is_read_locked(&self) -> bool {
        let state = self.state.load(Ordering::Relaxed);
        state != LOCK_FREE && (state & LOCK_WRITE) == 0
    }
}

impl Default for RwLock {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for RwLock {}
unsafe impl Sync for RwLock {}

// ============================================================================
// Lock Guard Types
// ============================================================================

/// RAII guard for simple lock
pub struct SimpleLockGuard<'a> {
    lock: &'a SimpleLock,
}

impl<'a> SimpleLockGuard<'a> {
    pub fn new(lock: &'a SimpleLock) -> Self {
        lock.lock();
        Self { lock }
    }
}

impl<'a> Drop for SimpleLockGuard<'a> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}

/// RAII guard for write lock
pub struct WriteLockGuard<'a> {
    lock: &'a RwLock,
}

impl<'a> WriteLockGuard<'a> {
    pub fn new(lock: &'a RwLock) -> Self {
        lock.write();
        Self { lock }
    }
}

impl<'a> Drop for WriteLockGuard<'a> {
    fn drop(&mut self) {
        self.lock.done();
    }
}

/// RAII guard for read lock
pub struct ReadLockGuard<'a> {
    lock: &'a RwLock,
}

impl<'a> ReadLockGuard<'a> {
    pub fn new(lock: &'a RwLock) -> Self {
        lock.read();
        Self { lock }
    }
}

impl<'a> Drop for ReadLockGuard<'a> {
    fn drop(&mut self) {
        self.lock.done();
    }
}

// ============================================================================
// Compatibility Macros/Functions
// ============================================================================

/// Initialize a simple lock
pub fn simple_lock_init(lock: &SimpleLock) {
    lock.init();
}

/// Acquire a simple lock
pub fn simple_lock(lock: &SimpleLock) {
    lock.lock();
}

/// Release a simple lock
pub fn simple_unlock(lock: &SimpleLock) {
    lock.unlock();
}

/// Try to acquire a simple lock
pub fn simple_lock_try(lock: &SimpleLock) -> bool {
    lock.try_lock()
}

/// Check if simple lock is held
pub fn simple_lock_taken(lock: &SimpleLock) -> bool {
    lock.is_locked()
}

/// Initialize a read/write lock
pub fn lock_init(lock: &RwLock, can_sleep: bool) {
    lock.init(can_sleep);
}

/// Set whether a lock can sleep
pub fn lock_sleepable(lock: &RwLock, can_sleep: bool) {
    lock.set_sleepable(can_sleep);
}

/// Acquire for writing
pub fn lock_write(lock: &RwLock) {
    lock.write();
}

/// Acquire for reading
pub fn lock_read(lock: &RwLock) {
    lock.read();
}

/// Release the lock
pub fn lock_done(lock: &RwLock) {
    lock.done();
}

/// Upgrade from read to write
pub fn lock_read_to_write(lock: &RwLock) -> bool {
    lock.read_to_write()
}

/// Downgrade from write to read
pub fn lock_write_to_read(lock: &RwLock) {
    lock.write_to_read();
}

/// Try to acquire for writing
pub fn lock_try_write(lock: &RwLock) -> bool {
    lock.try_write()
}

/// Try to acquire for reading
pub fn lock_try_read(lock: &RwLock) -> bool {
    lock.try_read()
}

// ============================================================================
// Spin Lock with Data (like spin::Mutex)
// ============================================================================

/// A spin lock that protects data (similar to spin::Mutex)
pub struct SpinLock<T> {
    lock: SimpleLock,
    data: UnsafeCell<T>,
}

impl<T> SpinLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            lock: SimpleLock::new(),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        self.lock.lock();
        SpinLockGuard { lock: self }
    }

    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        if self.lock.try_lock() {
            Some(SpinLockGuard { lock: self })
        } else {
            None
        }
    }

    pub fn is_locked(&self) -> bool {
        self.lock.is_locked()
    }
}

unsafe impl<T: Send> Send for SpinLock<T> {}
unsafe impl<T: Send> Sync for SpinLock<T> {}

pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<'a, T> core::ops::Deref for SpinLockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> core::ops::DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T> Drop for SpinLockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.lock.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_lock() {
        let lock = SimpleLock::new();

        assert!(!lock.is_locked());

        lock.lock();
        assert!(lock.is_locked());

        lock.unlock();
        assert!(!lock.is_locked());
    }

    #[test]
    fn test_simple_lock_try() {
        let lock = SimpleLock::new();

        assert!(lock.try_lock());
        assert!(!lock.try_lock());

        lock.unlock();
        assert!(lock.try_lock());
        lock.unlock();
    }

    #[test]
    fn test_rwlock_read() {
        let lock = RwLock::new();

        lock.read();
        assert!(lock.is_read_locked());
        assert!(!lock.is_write_locked());

        // Can acquire multiple read locks
        lock.read();
        assert_eq!(lock.read_count(), 2);

        lock.done();
        lock.done();
        assert!(!lock.is_read_locked());
    }

    #[test]
    fn test_rwlock_write() {
        let lock = RwLock::new();

        lock.write();
        assert!(lock.is_write_locked());
        assert!(!lock.is_read_locked());

        lock.done();
        assert!(!lock.is_write_locked());
    }

    #[test]
    fn test_spinlock_guard() {
        let lock = SpinLock::new(42);

        {
            let mut guard = lock.lock();
            assert_eq!(*guard, 42);
            *guard = 100;
        }

        let guard = lock.lock();
        assert_eq!(*guard, 100);
    }
}
