//! # Database Module
//!
//! Provides the full database layer for Scribo: connection state, schema management,
//! content hashing, time utilities, and repository functions.
//!
//! ## Architecture
//!
//! ```text
//!  ┌──────────────────────────────────────────────────────┐
//!  │                      DbState                         │
//!  │  Arc<RwLock<Option<Pool>>>   ← connection pool       │
//!  │  Arc<RwLock<Option<LLM>>>    ← cached LLM service    │
//!  │  Arc<RwLock<Option<String>>> ← cached vault language │
//!  │  Mutex<()>                   ← single-writer guard   │
//!  └──────────────────────────────────────────────────────┘
//!           │ with_conn / with_write
//!           ▼
//!  ┌──────────────┐   ┌──────────────┐
//!  │   repos/     │   │   schema/    │
//!  │  notes       │   │  initialize  │
//!  │  fragments   │   │  migrations  │
//!  │  sections    │   │  helpers     │
//!  │  cards       │   └──────────────┘
//!  │  schedules   │
//!  │  review_logs │
//!  │  tags        │
//!  └──────────────┘
//! ```
//!
//! ## Modules
//!
//! - [`hash`]   — BLAKE3 content hashing used for change detection.
//! - [`time`]   — Unix-second timestamp helper.
//! - [`state`]  — `DbState`: the Tauri-managed global database handle.
//! - [`schema`] — Schema creation and incremental migration (v1 → v15).
//! - [`repos`]  — Repository functions (CRUD) for each domain entity.

pub mod hash;
pub mod state;
pub mod schema;
pub mod repos;
pub mod time;

pub use hash::content_hash;
pub use state::DbState;
