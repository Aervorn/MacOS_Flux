//! Cœur de Flux : tout ce qui ne dépend pas de Tauri.
//!
//! Les modules `db`, `feed`, `ingest`, `cluster`, `images`, `sync`, `backup`,
//! `opml` et `import` seront ajoutés aux étapes suivantes.

pub mod prefs;

/// Version du cœur (celle du workspace).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
