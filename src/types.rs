//! Common types used across Mach_R
//!
//! This module defines shared types to avoid circular dependencies.

use core::sync::atomic::{AtomicU64, Ordering};

/// Task identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TaskId(pub u64);

impl TaskId {
    /// Create a new task ID with a unique auto-incremented value
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        TaskId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

/// Thread identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ThreadId(pub u64);

impl ThreadId {
    /// Create a new thread ID with a unique auto-incremented value
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        ThreadId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

/// Unique port identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct PortId(pub u64);

impl PortId {
    /// Generate a new unique port ID with an auto-incremented value
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        PortId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}
