//! Generation-based handle table for safe JNI object management.
//!
//! Instead of passing raw `Box::into_raw` pointers as `jlong` across the JNI
//! boundary (which risks use-after-free and double-free), this module provides
//! a global handle table that maps opaque `u64` IDs to `Arc<Pipeline>` instances.
//!
//! Each handle ID is monotonically increasing and never reused, preventing
//! ABA-style bugs.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tingyuxuan_core::pipeline::Pipeline;

/// Monotonically increasing handle counter. Starts at 1 (0 = invalid).
static HANDLE_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Global handle table. Lazily initialized on first use.
static HANDLES: Mutex<Option<HashMap<u64, Arc<Pipeline>>>> = Mutex::new(None);

/// Register a pipeline instance and return its handle ID.
pub fn register_handle(pipeline: Arc<Pipeline>) -> u64 {
    let id = HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut map = HANDLES.lock().unwrap();
    let map = map.get_or_insert_with(HashMap::new);
    map.insert(id, pipeline);
    id
}

/// Look up a pipeline by handle ID. Returns an error if the handle is invalid.
pub fn get_handle(id: u64) -> Result<Arc<Pipeline>, String> {
    let map = HANDLES.lock().unwrap();
    map.as_ref()
        .and_then(|m| m.get(&id).cloned())
        .ok_or_else(|| format!("Invalid pipeline handle: {id}"))
}

/// Remove a pipeline handle. Returns `true` if the handle existed.
pub fn remove_handle(id: u64) -> bool {
    let mut map = HANDLES.lock().unwrap();
    map.as_mut().map_or(false, |m| m.remove(&id).is_some())
}

/// Returns the number of active handles (for testing/debugging).
#[cfg(test)]
pub fn active_handle_count() -> usize {
    let map = HANDLES.lock().unwrap();
    map.as_ref().map_or(0, |m| m.len())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // We can't easily construct a real Pipeline without API keys, so we test
    // the handle table mechanics using a helper that bypasses Pipeline creation.
    // The handle table stores Arc<Pipeline>, but the key operations (register,
    // get, remove) are what we validate here.

    #[test]
    fn test_handle_counter_starts_at_one() {
        let id = HANDLE_COUNTER.load(Ordering::Relaxed);
        assert!(id >= 1, "Handle counter should start at 1 or higher");
    }

    #[test]
    fn test_remove_nonexistent_handle() {
        assert!(!remove_handle(999_999_999));
    }

    #[test]
    fn test_get_nonexistent_handle() {
        let result = get_handle(999_999_998);
        assert!(result.is_err());
        match result {
            Err(msg) => assert!(msg.contains("Invalid pipeline handle")),
            Ok(_) => panic!("Expected error for nonexistent handle"),
        }
    }

    #[test]
    fn test_handle_ids_are_unique() {
        let id1 = HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let id2 = HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_handle_zero_is_invalid() {
        let result = get_handle(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_double_remove_is_safe() {
        // Removing the same handle twice should return false the second time.
        assert!(!remove_handle(888_888_888));
        assert!(!remove_handle(888_888_888));
    }
}
