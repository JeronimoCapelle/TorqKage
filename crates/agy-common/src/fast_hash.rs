//! Ultra-fast non-cryptographic hasher and `SwissTable`-backed [`FastHashMap`] and [`FastHashSet`].
//!
//! # Architecture & Rationale
//!
//! The standard library's `std::collections::HashMap` uses `hashbrown`'s `SwissTable`
//! algorithm under the hood, but defaults to `SipHash` 1-3 (`RandomState`) for `DoS` protection
//! against untrusted network hash flooding.
//!
//! For internal systems tooling, compiler AST nodes, tool tables, and lifecycle hooks,
//! `SipHash` introduces unnecessary overhead (~3x to 5x slower than non-cryptographic hashers).
//! `FastHasher` implements the multiplicative rotating hash (`FxHash` algorithm) used
//! internally by `rustc`, processing 8-byte and 4-byte chunks with zero indexing and
//! zero casting for maximum hardware throughput.

#![allow(clippy::module_name_repetitions, reason = "Explicit FastHashMap/FastHashSet/FastHasher names for clear unqualified imports")]

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// Golden-ratio multiplicative constant used in rustc-hash and `FxHash`.
const K: u64 = 0x517c_c1b7_2722_0a95;

/// High-performance non-cryptographic hasher for internal systems and compiler identifiers.
#[derive(Debug, Clone, Default)]
pub struct FastHasher {
    hash: u64,
}

impl Hasher for FastHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let (chunks8, remainder) = bytes.as_chunks::<8>();
        for chunk in chunks8 {
            let val = u64::from_ne_bytes(*chunk);
            self.write_u64(val);
        }

        let (chunks4, trailing) = remainder.as_chunks::<4>();
        for chunk in chunks4 {
            let val = u32::from_ne_bytes(*chunk);
            self.write_u32(val);
        }

        for &byte in trailing {
            self.write_u8(byte);
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.write_u64(u64::from(i));
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.write_u64(u64::from(i));
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.write_u64(u64::from(i));
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.hash = (self.hash.rotate_left(5) ^ i).wrapping_mul(K);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.write_u64(u64::try_from(i).unwrap_or(0));
    }
}

/// Fast `BuildHasher` using `BuildHasherDefault<FastHasher>`.
pub type FastBuildHasher = BuildHasherDefault<FastHasher>;

/// SwissTable-backed `HashMap` using the ultra-fast `FastHasher`.
pub type FastHashMap<K, V> = HashMap<K, V, FastBuildHasher>;

/// SwissTable-backed `HashSet` using the ultra-fast `FastHasher`.
pub type FastHashSet<T> = HashSet<T, FastBuildHasher>;

/// Constructs an empty `FastHashMap`.
#[must_use]
pub fn fast_map_new<K, V>() -> FastHashMap<K, V> {
    FastHashMap::default()
}

/// Constructs an empty `FastHashMap` with the specified initial capacity.
#[must_use]
pub fn fast_map_with_capacity<K, V>(capacity: usize) -> FastHashMap<K, V> {
    FastHashMap::with_capacity_and_hasher(capacity, FastBuildHasher::default())
}

/// Constructs an empty `FastHashSet`.
#[must_use]
pub fn fast_set_new<T>() -> FastHashSet<T> {
    FastHashSet::default()
}

/// Constructs an empty `FastHashSet` with the specified initial capacity.
#[must_use]
pub fn fast_set_with_capacity<T>(capacity: usize) -> FastHashSet<T> {
    FastHashSet::with_capacity_and_hasher(capacity, FastBuildHasher::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::Hash;

    fn compute_hash<T: Hash>(val: &T) -> u64 {
        let mut hasher = FastHasher::default();
        val.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn test_fast_hasher_distinct_hashes() {
        let h1 = compute_hash(&"run_quick_check");
        let h2 = compute_hash(&"run_cargo_verification");
        let h3 = compute_hash(&"run_cargo_clippy");
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
