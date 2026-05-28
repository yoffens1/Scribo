//! # Repository Module
//!
//! Domain-specific CRUD functions grouped by entity. Each sub-module wraps raw `rusqlite`
//! queries behind a typed Rust API, mapping rows to domain structs and errors to `AppError`.
//!
//! ## Chunk Level Convention
//!
//! All fragment and section data lives in the unified `chunks` table.
//! The `level` column distinguishes between row types:
//!
//! | `level` | Domain type | Module |
//! |---|---|---|
//! | `0` | `Section` (heading block, raw markdown) | [`sections`] |
//! | `1` | `Fragment` (clean text + embedding blob) | [`fragments`] |
//!
//! ## Modules
//!
//! | Module | Table(s) | Responsibilities |
//! |---|---|---|
//! | [`notes`]       | `notes`        | Full note lifecycle: insert, update, rename, move, soft/hard delete |
//! | [`fragments`]   | `chunks` (l=1) | Fragment CRUD, FTS5 keyword search, cosine vector search |
//! | [`sections`]    | `chunks` (l=0) | Section (heading block) CRUD with byte-offset tracking |
//! | [`cards`]       | `cards`        | SRS card lifecycle; auto-creates schedule on insert |
//! | [`schedules`]   | `schedules`    | FSRS schedule CRUD and due-card queries |
//! | [`review_logs`] | `review_logs`  | Append-only review history |
//! | [`tags`]        | `tags`, `tag_closure`, `note_tags`, `chunk_tags` | Hierarchical tag management |

pub mod cards;
pub mod fragments;
pub mod notes;
pub mod review_logs;
pub mod schedules;
pub mod sections;
pub mod tags;
