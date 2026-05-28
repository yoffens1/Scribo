//! # Services Module
//!
//! Application-level services that coordinate domain logic, database access, and AI operations.
//! Each service corresponds to a distinct capability of the Scribo backend.
//!
//! ## Inventory
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`indexer`]    | Fragment extraction, deduplication, and persistence into the `chunks` table |
//! | [`scheduler`]  | Background async queue for triggering re-indexing of changed notes |
//! | [`validation`] | Pre-flight checks to determine whether a note needs re-indexing |
//! | [`notesearch`] | Fuzzy note-title search backed by `skim` |
//! | [`reviewer`]   | FSRS-driven spaced repetition scheduling and review logging |
//! | [`import`]     | Bulk markdown-file import into the note store |
//! | [`distribute`] | Draft-to-note distribution pipeline (chunk → retrieve → classify → apply) |
//! | [`reindex`]    | Detects model drift and re-queues notes for vectorization |

pub mod indexer;
pub mod scheduler;
pub mod validation;
pub mod notesearch;
pub mod reviewer;
pub mod import;
pub mod distribute;
pub mod reindex;
