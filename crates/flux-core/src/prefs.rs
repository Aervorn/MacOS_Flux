//! Réglages utilisateur et leurs valeurs par défaut.
//!
//! Les valeurs par défaut reprennent `Prefs.defaults` de l'app Swift
//! (`Flux/Settings.swift`). L'import des réglages (étape 2) applique ces
//! mêmes valeurs pour toute clé absente de l'export `defaults`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prefs {
    pub refresh_minutes: u32,
    pub retention_days: u32,
    pub mark_read_delay: u32,
    pub image_cache_limit_mb: u32,
    pub reader_width: String,
    pub show_summary: bool,
    pub reader_font_size: u32,
    pub archive_favorites: bool,
    pub group_duplicates: bool,
    pub duplicate_sensitivity: u32,
    pub hide_read: bool,
    pub hide_tagged_favorites: bool,
    pub backup_enabled: bool,
    pub backup_keep: u32,
    pub backup_include_archive: bool,
    pub auto_mark_read_enabled: bool,
    pub auto_mark_read_days: u32,
    pub offline_enabled: bool,
    pub appearance: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            refresh_minutes: 30,
            retention_days: 60,
            mark_read_delay: 0,
            image_cache_limit_mb: 1024,
            reader_width: "narrow".into(),
            show_summary: true,
            reader_font_size: 17,
            archive_favorites: true,
            group_duplicates: true,
            duplicate_sensitivity: 1,
            hide_read: false,
            hide_tagged_favorites: false,
            backup_enabled: false,
            backup_keep: 7,
            backup_include_archive: true,
            auto_mark_read_enabled: false,
            auto_mark_read_days: 7,
            offline_enabled: false,
            appearance: "system".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_swift_app() {
        let p = Prefs::default();
        assert_eq!(p.refresh_minutes, 30);
        assert_eq!(p.retention_days, 60);
        assert_eq!(p.image_cache_limit_mb, 1024);
        assert_eq!(p.reader_width, "narrow");
        assert!(p.archive_favorites && p.group_duplicates);
    }

    #[test]
    fn serializes_in_camel_case() {
        let json = serde_json::to_string(&Prefs::default()).unwrap();
        assert!(json.contains("\"refreshMinutes\":30"));
    }
}
