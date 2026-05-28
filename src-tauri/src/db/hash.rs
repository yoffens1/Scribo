//! # Content Hashing
//!
//! Provides a fast, cryptographically strong content hash used throughout the indexing pipeline
//! to detect whether a fragment's text has changed since the last indexing run.
//!
//! Uses **BLAKE3** — a highly parallel, hardware-accelerated hash function that is:
//! - Faster than SHA-256 and MD5 on modern hardware.
//! - Collision-resistant for content deduplication.
//! - Deterministic: identical UTF-8 input always produces the same 64-character hex string.

/// Computes a BLAKE3 hash of `content` and returns it as a 64-character lowercase hex string.
///
/// Used by the indexer to compare stored `clean_hash` / `raw_hash` values against
/// newly generated fragments to decide whether a DB row needs updating.
pub fn content_hash(content: &str) -> String {
    blake3::hash(content.as_bytes()).to_hex().to_string()
}
