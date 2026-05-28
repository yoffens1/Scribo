//! # Pack Module
//!
//! Packers aggregate a stream of [`Atom`](crate::fragmenter::segment::Atom)s into
//! [`RawFragment`]s — the pre-cleaning intermediate representation.
//!
//! ## Available Packers
//!
//! | Module | Configured by | Strategy |
//! |---|---|---|
//! | [`token_budget`] | `Packer::TokenBudget` | Greedy sliding window by token count, with configurable overlap |
//! | [`char_budget`]  | `Packer::CharBudget`  | Greedy sliding window by character count |
//! | [`passthrough`]  | `Packer::Passthrough` | Identity — one atom → one fragment |
//! | [`tables`]       | (internal)            | Table restore and linearisation helpers |

pub mod token_budget;
pub mod char_budget;
pub mod passthrough;
pub mod tables;

use crate::fragmenter::output::FragmentMeta;

/// An intermediate fragment produced by the packer before cleaning is applied.
/// The `text` field still contains raw markdown (including table syntax, links, etc.).
/// [`CleanProfile`](crate::fragmenter::config::CleanProfile) is applied in the final pipeline step.
#[derive(Debug, Clone)]
pub struct RawFragment {
    /// Raw markdown text — no cleaning applied yet.
    pub text: String,
    /// Structural metadata carried forward from the source atoms.
    pub meta: FragmentMeta,
}
