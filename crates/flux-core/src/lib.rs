//! Cœur de Flux : tout ce qui ne dépend pas de Tauri.
//!
//! Déjà présents : `db` (schéma SQLite), `query` (barre latérale et listes),
//! `images` (noms du cache d'images), `import` (reprise des données de l'app
//! Swift), `prefs` (réglages). Viendront ensuite : `feed`, `ingest`,
//! `cluster`, `images`, `sync`, `backup`, `opml`.

pub mod actions;
pub mod articles;
pub mod cluster;
pub mod db;
pub mod error;
pub mod feed;
pub mod feeds;
pub mod filters;
pub mod html;
pub mod images;
pub mod import;
pub mod manage;
pub mod opml;
pub mod refresh;
pub mod net;
pub mod prefs;
pub mod query;

pub use error::{Error, Result};

/// Version du cœur (celle du workspace).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
