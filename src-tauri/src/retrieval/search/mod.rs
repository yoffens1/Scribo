//! # Search Phase
//!
//! Executes concurrent FTS5 and vector searches across all query variants,
//! fuses results with RRF, and detects the vault's dominant language.

pub mod executor;
pub mod fusion;
pub mod vault_lang;

pub use executor::{retrieve_per_variant, build_variant_embeddings};
pub use fusion::{rrf, apply_term_boost};
pub use vault_lang::get_vault_language;
