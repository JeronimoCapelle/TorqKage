//! High-performance non-cryptographic hasher and `SwissTable`-backed [`FastHashMap`] and [`FastHashSet`]
//! backed by the official compiler team's [`rustc_hash`] crate.

#![allow(
    clippy::module_name_repetitions,
    reason = "Explicit FastHashMap/FastHashSet/FastHasher names for clear unqualified imports"
)]

/// Fast `BuildHasher` using `rustc_hash::FxBuildHasher`.
pub use rustc_hash::FxBuildHasher as FastBuildHasher;
/// High-performance non-cryptographic hasher backed by official `rustc_hash`.
pub use rustc_hash::FxHasher as FastHasher;
/// SwissTable-backed `HashMap` using the ultra-fast `FastHasher`.
pub type FastHashMap<K, V> = rustc_hash::FxHashMap<K, V>;
/// SwissTable-backed `HashSet` using the ultra-fast `FastHasher`.
pub type FastHashSet<T> = rustc_hash::FxHashSet<T>;

/// Constructs an empty `FastHashMap`.
#[must_use]
pub fn fast_map_new<K, V>() -> FastHashMap<K, V> {
    FastHashMap::default()
}

/// Constructs an empty `FastHashMap` with the specified initial capacity.
#[must_use]
pub fn fast_map_with_capacity<K, V>(capacity: usize) -> FastHashMap<K, V> {
    FastHashMap::with_capacity_and_hasher(capacity, FastBuildHasher)
}

/// Constructs an empty `FastHashSet`.
#[must_use]
pub fn fast_set_new<T>() -> FastHashSet<T> {
    FastHashSet::default()
}

/// Constructs an empty `FastHashSet` with the specified initial capacity.
#[must_use]
pub fn fast_set_with_capacity<T>(capacity: usize) -> FastHashSet<T> {
    FastHashSet::with_capacity_and_hasher(capacity, FastBuildHasher)
}

#[cfg(test)]
mod tests {
    use std::hash::{Hash, Hasher};

    use super::*;

    fn compute_hash<T: Hash>(val: &T) -> u64 {
        let mut hasher = FastHasher::default();
        val.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn test_fast_hasher_distinct_hashes() {
        let h1 = compute_hash(&"quick_check");
        let h2 = compute_hash(&"cargo_check");
        let h3 = compute_hash(&"cargo_clippy");
        assert_ne!(h1, h2);
        assert_ne!(h2, h3);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_fast_hash_map_crud() {
        let mut map = fast_map_new::<String, i32>();
        assert!(map.is_empty());

        let _ = map.insert("alpha".to_owned(), 100);
        let _ = map.insert("beta".to_owned(), 200);

        assert_eq!(map.len(), 2);
        assert_eq!(map.get("alpha"), Some(&100));
        assert_eq!(map.get("beta"), Some(&200));
        assert_eq!(map.get("gamma"), None);

        assert_eq!(map.remove("alpha"), Some(100));
        assert_eq!(map.get("alpha"), None);
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_fast_hash_map_with_capacity() {
        let mut map = fast_map_with_capacity::<usize, usize>(64);
        for i in 0..64 {
            let _ = map.insert(i, i.wrapping_mul(2));
        }
        assert_eq!(map.len(), 64);
        assert_eq!(map.get(&10), Some(&20));
    }

    #[test]
    fn test_fast_hash_set_crud() {
        let mut set = fast_set_new::<String>();
        assert!(set.is_empty());

        assert!(set.insert("tool_a".to_owned()));
        assert!(set.insert("tool_b".to_owned()));
        assert!(!set.insert("tool_a".to_owned())); // duplicate

        assert_eq!(set.len(), 2);
        assert!(set.contains("tool_a"));
        assert!(set.contains("tool_b"));
        assert!(!set.contains("tool_c"));

        assert!(set.remove("tool_a"));
        assert!(!set.contains("tool_a"));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_fast_hash_set_with_capacity() {
        let mut set = fast_set_with_capacity::<u64>(128);
        for i in 0..128 {
            let _ = set.insert(i);
        }
        assert_eq!(set.len(), 128);
        assert!(set.contains(&42));
    }
}
