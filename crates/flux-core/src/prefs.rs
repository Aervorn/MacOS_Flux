//! Réglages utilisateur et leurs valeurs par défaut.
//!
//! Les valeurs par défaut reprennent `Prefs.defaults` de l'app Swift
//! (`Flux/Settings.swift`). L'import des réglages applique ces mêmes valeurs
//! pour toute clé absente de l'export `defaults`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    pub refresh_minutes: u32,
    pub retention_days: u32,
    /// Secondes avant de marquer l'article ouvert comme lu (0 = tout de suite, -1 = jamais).
    pub mark_read_delay: i32,
    pub image_cache_limit_mb: u32,
    /// "narrow", "wide" ou "full"
    pub reader_width: String,
    /// Lignes du début de l'article affichées sous le titre en mode liste (0 à 4, 2 par défaut).
    pub summary_lines: u32,
    /// La recherche porte sur le titre des articles.
    pub search_in_title: bool,
    /// La recherche porte sur le corps des articles (résumé et texte).
    pub search_in_body: bool,
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
    /// Synchronisation entre Mac par iCloud Drive.
    pub sync_enabled: bool,
    /// "system", "light" ou "dark"
    pub appearance: String,
    /// "list", "grid" ou "gridFull"
    pub article_layout: String,
    pub article_list_width: f64,
    /// Largeur de la barre latérale, en pixels.
    pub sidebar_width: f64,
    pub favorites_expanded: bool,
    /// Police de l'interface : "inter", "geist" ou "system".
    pub ui_font: String,
    /// Police de l'article : "classic", "geist", "literata", "sourceSerif", "merriweather",
    /// "atkinson", "systemSerif" ou "mono".
    pub article_font: String,
    /// Espacement de la barre latérale : "compact", "normal" ou "spacious".
    pub sidebar_density: String,
    /// La découverte de Flux a déjà été vue (ou n'a plus lieu d'être).
    pub onboarding_seen: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            refresh_minutes: 30,
            retention_days: 60,
            mark_read_delay: 0,
            image_cache_limit_mb: 1024,
            reader_width: "narrow".into(),
            summary_lines: 2,
            search_in_title: true,
            search_in_body: true,
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
            sync_enabled: false,
            appearance: "system".into(),
            article_layout: "list".into(),
            article_list_width: 400.0,
            sidebar_width: 260.0,
            favorites_expanded: true,
            ui_font: "inter".into(),
            article_font: "classic".into(),
            sidebar_density: "normal".into(),
            onboarding_seen: false,
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
        assert_eq!(p.article_layout, "list");
        assert_eq!(p.sidebar_width, 260.0);
        assert!(p.archive_favorites && p.group_duplicates);
        assert_eq!(p.summary_lines, 2);
        assert!(p.search_in_title && p.search_in_body);
        assert_eq!((p.ui_font.as_str(), p.article_font.as_str(), p.sidebar_density.as_str()), ("inter", "classic", "normal"));
    }

    #[test]
    fn serializes_in_camel_case() {
        let json = serde_json::to_string(&Prefs::default()).unwrap();
        assert!(json.contains("\"refreshMinutes\":30"));
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let p: Prefs = serde_json::from_str("{\"retentionDays\":14}").unwrap();
        assert_eq!(p.retention_days, 14);
        assert_eq!(p.refresh_minutes, 30);
    }
}
