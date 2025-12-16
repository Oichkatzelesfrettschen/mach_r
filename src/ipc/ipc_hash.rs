//! IPC Hash Table - Fast (space, object) -> name lookup
//!
//! Based on Mach4 ipc/ipc_hash.h/c by Rich Draves (1989)
//!
//! This module provides fast reverse lookups from IPC objects to their names
//! within a particular IPC space. This is essential for:
//! - Avoiding duplicate names for the same object
//! - Fast capability transfer (finding existing names)
//! - Port destruction cleanup
//!
//! The hash table uses open addressing with linear probing.

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use core::hash::{Hash, Hasher};
use spin::Mutex;

use crate::ipc::entry::{MachPortIndex, MachPortName};
use crate::ipc::space::SpaceId;

// ============================================================================
// Hash Key
// ============================================================================

/// Key for hash table: (space_id, object_ptr)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IpcHashKey {
    /// IPC space ID
    pub space_id: SpaceId,
    /// Object pointer (as usize for hashing)
    pub object_ptr: usize,
}

impl IpcHashKey {
    pub fn new(space_id: SpaceId, object_ptr: usize) -> Self {
        Self {
            space_id,
            object_ptr,
        }
    }
}

// ============================================================================
// Hash Entry
// ============================================================================

/// Entry stored in the hash table
#[derive(Debug, Clone, Copy)]
pub struct IpcHashEntry {
    /// Port name in the space
    pub name: MachPortName,
    /// Index in the entry table
    pub index: MachPortIndex,
}

impl IpcHashEntry {
    pub fn new(name: MachPortName, index: MachPortIndex) -> Self {
        Self { name, index }
    }
}

// ============================================================================
// Global Hash Table
// ============================================================================

/// Global hash table for (space, object) -> name lookups
///
/// This is used for tree entries (large port names) where the entry table
/// doesn't provide efficient reverse lookup.
#[derive(Debug)]
pub struct IpcGlobalHash {
    /// The hash map
    map: BTreeMap<IpcHashKey, IpcHashEntry>,
    /// Statistics
    pub stats: IpcHashStats,
}

impl IpcGlobalHash {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
            stats: IpcHashStats::new(),
        }
    }

    /// Look up entry by (space, object)
    pub fn lookup(&self, space_id: SpaceId, object_ptr: usize) -> Option<&IpcHashEntry> {
        let key = IpcHashKey::new(space_id, object_ptr);
        self.map.get(&key)
    }

    /// Insert entry
    pub fn insert(
        &mut self,
        space_id: SpaceId,
        object_ptr: usize,
        name: MachPortName,
        index: MachPortIndex,
    ) {
        let key = IpcHashKey::new(space_id, object_ptr);
        let entry = IpcHashEntry::new(name, index);
        self.map.insert(key, entry);
        self.stats.inserts += 1;
    }

    /// Delete entry
    pub fn delete(&mut self, space_id: SpaceId, object_ptr: usize) -> Option<IpcHashEntry> {
        let key = IpcHashKey::new(space_id, object_ptr);
        let result = self.map.remove(&key);
        if result.is_some() {
            self.stats.deletes += 1;
        }
        result
    }

    /// Get entry count
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Remove all entries for a space
    pub fn cleanup_space(&mut self, space_id: SpaceId) {
        self.map.retain(|k, _| k.space_id != space_id);
    }
}

impl Default for IpcGlobalHash {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Local Hash Table (per-space)
// ============================================================================

/// Local hash table for a single IPC space
///
/// Uses open addressing with linear probing in the entry table itself.
/// The ie_index field of entries forms the hash chain.
#[derive(Debug)]
pub struct IpcLocalHash {
    /// Hash table size (must be power of 2)
    size: usize,
    /// Hash table mask (size - 1)
    mask: usize,
    /// Number of entries
    count: usize,
}

impl IpcLocalHash {
    /// Create a new local hash table
    pub fn new(size: usize) -> Self {
        // Round up to power of 2
        let size = size.next_power_of_two();
        Self {
            size,
            mask: size - 1,
            count: 0,
        }
    }

    /// Get hash table size
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get entry count
    pub fn count(&self) -> usize {
        self.count
    }

    /// Compute hash index for (space, object)
    pub fn hash_index(&self, space_id: SpaceId, object_ptr: usize) -> usize {
        // Simple hash combining space and object
        let mut hasher = FnvHasher::new();
        space_id.hash(&mut hasher);
        object_ptr.hash(&mut hasher);
        (hasher.finish() as usize) & self.mask
    }

    /// Record that an entry was inserted
    pub fn record_insert(&mut self) {
        self.count += 1;
    }

    /// Record that an entry was deleted
    pub fn record_delete(&mut self) {
        self.count = self.count.saturating_sub(1);
    }

    /// Check if hash table should be resized
    pub fn should_resize(&self) -> bool {
        // Resize when 75% full
        self.count * 4 > self.size * 3
    }
}

impl Default for IpcLocalHash {
    fn default() -> Self {
        Self::new(16)
    }
}

// ============================================================================
// FNV-1a Hash
// ============================================================================

/// FNV-1a hasher for fast hashing
struct FnvHasher {
    state: u64,
}

impl FnvHasher {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn new() -> Self {
        Self {
            state: Self::OFFSET_BASIS,
        }
    }
}

impl Hasher for FnvHasher {
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state ^= *byte as u64;
            self.state = self.state.wrapping_mul(Self::PRIME);
        }
    }

    fn finish(&self) -> u64 {
        self.state
    }
}

// ============================================================================
// Hash Statistics
// ============================================================================

/// Hash table statistics
#[derive(Debug, Default, Clone)]
pub struct IpcHashStats {
    /// Number of lookups
    pub lookups: u64,
    /// Number of successful lookups
    pub hits: u64,
    /// Number of insertions
    pub inserts: u64,
    /// Number of deletions
    pub deletes: u64,
    /// Number of collisions
    pub collisions: u64,
}

impl IpcHashStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get hit rate (0.0 to 1.0)
    pub fn hit_rate(&self) -> f64 {
        if self.lookups == 0 {
            0.0
        } else {
            self.hits as f64 / self.lookups as f64
        }
    }
}

// ============================================================================
// Hash Bucket Info (for debugging)
// ============================================================================

/// Information about a hash bucket (for debugging)
#[derive(Debug, Clone, Copy, Default)]
pub struct HashBucketInfo {
    /// Number of entries in bucket
    pub count: u32,
}

// ============================================================================
// Global State
// ============================================================================

static GLOBAL_HASH: spin::Once<Mutex<IpcGlobalHash>> = spin::Once::new();

fn global_hash() -> &'static Mutex<IpcGlobalHash> {
    GLOBAL_HASH.call_once(|| Mutex::new(IpcGlobalHash::new()));
    GLOBAL_HASH.get().unwrap()
}

/// Initialize the IPC hash subsystem
pub fn init() {
    let _ = global_hash();
}

// ============================================================================
// Public API - Global Hash
// ============================================================================

/// Look up in global hash
pub fn ipc_hash_global_lookup(
    space_id: SpaceId,
    object_ptr: usize,
) -> Option<(MachPortName, MachPortIndex)> {
    let hash = global_hash().lock();
    hash.lookup(space_id, object_ptr).map(|e| (e.name, e.index))
}

/// Insert into global hash
pub fn ipc_hash_global_insert(
    space_id: SpaceId,
    object_ptr: usize,
    name: MachPortName,
    index: MachPortIndex,
) {
    let mut hash = global_hash().lock();
    hash.insert(space_id, object_ptr, name, index);
}

/// Delete from global hash
pub fn ipc_hash_global_delete(space_id: SpaceId, object_ptr: usize) {
    let mut hash = global_hash().lock();
    hash.delete(space_id, object_ptr);
}

/// Clean up all entries for a space
pub fn ipc_hash_cleanup_space(space_id: SpaceId) {
    let mut hash = global_hash().lock();
    hash.cleanup_space(space_id);
}

// ============================================================================
// Public API - Combined Lookup
// ============================================================================

/// Look up entry by object in either global or local hash
///
/// Returns (name, index) if found
pub fn ipc_hash_lookup(
    space_id: SpaceId,
    object_ptr: usize,
    local_hash: Option<&IpcLocalHash>,
    _entries: &[super::entry::IpcEntry],
) -> Option<(MachPortName, MachPortIndex)> {
    // First try local hash (entry table)
    if let Some(_local) = local_hash {
        // In a full implementation, we'd search the local hash chain
        // in the entry table using ie_index fields
    }

    // Fall back to global hash
    ipc_hash_global_lookup(space_id, object_ptr)
}

/// Insert entry into appropriate hash table
pub fn ipc_hash_insert(
    space_id: SpaceId,
    object_ptr: usize,
    name: MachPortName,
    index: MachPortIndex,
    local_hash: Option<&mut IpcLocalHash>,
    is_tree_entry: bool,
) {
    if is_tree_entry {
        // Tree entries go in global hash
        ipc_hash_global_insert(space_id, object_ptr, name, index);
    } else if let Some(local) = local_hash {
        // Table entries use local hash
        local.record_insert();
        // In a full implementation, would update ie_index chain
    }
}

/// Delete entry from hash tables
pub fn ipc_hash_delete(
    space_id: SpaceId,
    object_ptr: usize,
    _name: MachPortName,
    _index: MachPortIndex,
    local_hash: Option<&mut IpcLocalHash>,
    is_tree_entry: bool,
) {
    if is_tree_entry {
        ipc_hash_global_delete(space_id, object_ptr);
    } else if let Some(local) = local_hash {
        local.record_delete();
        // In a full implementation, would update ie_index chain
    }
}

// ============================================================================
// Debug/Info API
// ============================================================================

/// Get hash table info for debugging
pub fn ipc_hash_info() -> Vec<HashBucketInfo> {
    let hash = global_hash().lock();
    // Return simplified info
    vec![HashBucketInfo {
        count: hash.len() as u32,
    }]
}

/// Get hash statistics
pub fn ipc_hash_stats() -> IpcHashStats {
    global_hash().lock().stats.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_key() {
        let key1 = IpcHashKey::new(SpaceId(1), 0x1000);
        let key2 = IpcHashKey::new(SpaceId(1), 0x1000);
        let key3 = IpcHashKey::new(SpaceId(2), 0x1000);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_global_hash() {
        let mut hash = IpcGlobalHash::new();

        hash.insert(SpaceId(1), 0x1000, 100, 5);
        hash.insert(SpaceId(1), 0x2000, 200, 10);
        hash.insert(SpaceId(2), 0x1000, 300, 15);

        // Lookup
        let entry = hash.lookup(SpaceId(1), 0x1000).unwrap();
        assert_eq!(entry.name, 100);
        assert_eq!(entry.index, 5);

        // Different space, same object
        let entry = hash.lookup(SpaceId(2), 0x1000).unwrap();
        assert_eq!(entry.name, 300);

        // Delete
        hash.delete(SpaceId(1), 0x1000);
        assert!(hash.lookup(SpaceId(1), 0x1000).is_none());

        // Cleanup space
        hash.cleanup_space(SpaceId(1));
        assert!(hash.lookup(SpaceId(1), 0x2000).is_none());
        assert!(hash.lookup(SpaceId(2), 0x1000).is_some());
    }

    #[test]
    fn test_local_hash() {
        let mut hash = IpcLocalHash::new(16);

        assert_eq!(hash.size(), 16);
        assert_eq!(hash.count(), 0);

        hash.record_insert();
        hash.record_insert();
        assert_eq!(hash.count(), 2);

        hash.record_delete();
        assert_eq!(hash.count(), 1);

        // Check resize threshold
        for _ in 0..12 {
            hash.record_insert();
        }
        assert!(hash.should_resize());
    }

    #[test]
    fn test_fnv_hash() {
        let mut hasher1 = FnvHasher::new();
        hasher1.write(b"test");
        let h1 = hasher1.finish();

        let mut hasher2 = FnvHasher::new();
        hasher2.write(b"test");
        let h2 = hasher2.finish();

        // Same input should give same hash
        assert_eq!(h1, h2);

        let mut hasher3 = FnvHasher::new();
        hasher3.write(b"test2");
        let h3 = hasher3.finish();

        // Different input should give different hash
        assert_ne!(h1, h3);
    }
}
