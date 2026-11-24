//! Cœur de Flux : tout ce qui ne dépend pas de Tauri.
//!
//! Déjà présents : `db` (schéma SQLite), `import` (reprise des données de
//! l'app Swift), `prefs` (réglages). Viendront ensuite : `feed`, `ingest`,
//! `cluster`, `images`, `sync`, `backup`, `opml`.

pub mod db;
pub mod error;
pub mod import;
pub mod prefs;

pub use error::{Error, Result};

/// Version du cœur (celle du workspace).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
