//! Reprise des données de l'app Swift (lecture seule de l'existant).

pub mod nskeyed;
pub mod settings;
pub mod swift_store;

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::db;
use crate::error::Result;

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub folders: usize,
    pub feeds: usize,
    pub articles: usize,
    pub favorite_tags: usize,
    pub article_tags: usize,
    pub filter_rules: usize,
    pub settings_imported: bool,
    /// Langue forcée dans l'app Swift ("fr", "en"), si elle l'était.
    pub language: Option<String>,
    /// Points d'attention (lignes ignorées, blob illisible…).
    pub warnings: Vec<String>,
}

/// Importe le store Swift, puis les réglages si un plist est donné et existe.
pub fn import_all(store: &Path, prefs_plist: Option<&Path>, dst: &mut Connection) -> Result<ImportReport> {
    let mut report = swift_store::import_store(store, dst)?;
    match prefs_plist {
        Some(path) if path.exists() => {
            let imported = settings::read_settings_plist(path)?;
            db::save_prefs(dst, &imported.prefs)?;
            if let Some(lang) = &imported.language {
                db::save_setting(dst, "language", lang)?;
            }
            report.settings_imported = true;
            report.language = imported.language;
        }
        Some(path) => report
            .warnings
            .push(format!("fichier de réglages introuvable : {}", path.display())),
        None => {}
    }
    Ok(report)
}
