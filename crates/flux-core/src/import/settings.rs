//! Import des réglages de l'app Swift (UserDefaults, plist exporté par `defaults export`
//! ou fichier ~/Library/Preferences/io.github.aervorn.flux.plist).
//!
//! Seules les clés modifiées par l'utilisateur figurent dans le plist : pour les autres,
//! on garde les valeurs par défaut de `Prefs::default()`. Ne sont pas importés :
//! sauvegarde (`lastBackup`), version vue, découverte vue, positions de fenêtres,
//! clés Sparkle (`SU…`) et clés inconnues.

use std::path::Path;

use plist::{Dictionary, Value};

use crate::error::{Error, Result};
use crate::prefs::Prefs;

#[derive(Debug, Clone)]
pub struct ImportedSettings {
    pub prefs: Prefs,
    /// Langue forcée dans l'app Swift (`AppleLanguages`), réduite au code court ("fr").
    pub language: Option<String>,
}

fn real(d: &Dictionary, key: &str) -> Option<f64> {
    let v = d.get(key)?;
    v.as_real().or_else(|| v.as_signed_integer().map(|i| i as f64))
}

fn int(d: &Dictionary, key: &str) -> Option<i64> {
    real(d, key).map(|f| f.round() as i64)
}

fn uint(d: &Dictionary, key: &str) -> Option<u32> {
    int(d, key).and_then(|i| u32::try_from(i).ok())
}

fn flag(d: &Dictionary, key: &str) -> Option<bool> {
    let v = d.get(key)?;
    v.as_boolean().or_else(|| int(d, key).map(|i| i != 0))
}

fn text(d: &Dictionary, key: &str) -> Option<String> {
    d.get(key)?.as_string().map(str::to_string)
}

/// Texte, seulement s'il fait partie des valeurs connues.
fn one_of(d: &Dictionary, key: &str, allowed: &[&str]) -> Option<String> {
    text(d, key).filter(|s| allowed.contains(&s.as_str()))
}

pub fn parse_settings(d: &Dictionary) -> ImportedSettings {
    let mut p = Prefs::default();

    if let Some(v) = uint(d, "refreshMinutes") { p.refresh_minutes = v; }
    if let Some(v) = uint(d, "retentionDays") { p.retention_days = v; }
    if let Some(v) = int(d, "markReadDelay").and_then(|i| i32::try_from(i).ok()) { p.mark_read_delay = v; }
    if let Some(v) = uint(d, "imageCacheLimitMB") { p.image_cache_limit_mb = v; }
    if let Some(v) = one_of(d, "readerWidth", &["narrow", "wide", "full"]) { p.reader_width = v; }
    if let Some(v) = flag(d, "showSummary") { p.summary_lines = if v { 2 } else { 0 }; }
    if let Some(v) = uint(d, "readerFontSize") { p.reader_font_size = v.clamp(13, 28); }
    if let Some(v) = flag(d, "archiveFavorites") { p.archive_favorites = v; }
    if let Some(v) = flag(d, "groupDuplicates") { p.group_duplicates = v; }
    if let Some(v) = uint(d, "duplicateSensitivity") { p.duplicate_sensitivity = v; }
    if let Some(v) = flag(d, "hideRead") { p.hide_read = v; }
    if let Some(v) = flag(d, "hideTaggedFavorites") { p.hide_tagged_favorites = v; }
    if let Some(v) = flag(d, "backupEnabled") { p.backup_enabled = v; }
    if let Some(v) = uint(d, "backupKeep") { p.backup_keep = v; }
    if let Some(v) = flag(d, "backupIncludeArchive") { p.backup_include_archive = v; }
    if let Some(v) = flag(d, "autoMarkReadEnabled") { p.auto_mark_read_enabled = v; }
    if let Some(v) = uint(d, "autoMarkReadDays") { p.auto_mark_read_days = v; }
    if let Some(v) = flag(d, "offlineEnabled") { p.offline_enabled = v; }
    if let Some(v) = one_of(d, "appearance", &["system", "light", "dark"]) { p.appearance = v; }
    if let Some(v) = one_of(d, "articleLayout", &["list", "grid", "gridFull"]) { p.article_layout = v; }
    if let Some(v) = real(d, "articleListWidth") { p.article_list_width = v; }
    if let Some(v) = flag(d, "favoritesExpanded") { p.favorites_expanded = v; }

    // Ancien réglage « vignettes en plein écran » : devenu articleLayout = "gridFull"
    // (même migration que Prefs.register() côté Swift).
    if flag(d, "gridFullWidth") == Some(true) && p.article_layout == "grid" {
        p.article_layout = "gridFull".into();
    }

    let language = d
        .get("AppleLanguages")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(Value::as_string)
        .and_then(|s| s.split(['-', '_']).next())
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase);

    ImportedSettings { prefs: p, language }
}

pub fn read_settings_plist(path: &Path) -> Result<ImportedSettings> {
    let value = Value::from_file(path)?;
    let dict = value
        .as_dictionary()
        .ok_or_else(|| Error::Other("le fichier de réglages n'est pas un dictionnaire".into()))?;
    Ok(parse_settings(dict))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict(entries: Vec<(&str, Value)>) -> Dictionary {
        let mut d = Dictionary::new();
        for (k, v) in entries {
            d.insert(k.to_string(), v);
        }
        d
    }

    #[test]
    fn absent_keys_keep_defaults() {
        let s = parse_settings(&Dictionary::new());
        assert_eq!(s.prefs, Prefs::default());
        assert_eq!(s.language, None);
    }

    #[test]
    fn imports_user_values_and_ignores_the_rest() {
        let d = dict(vec![
            ("retentionDays", Value::Integer(14i64.into())),
            ("markReadDelay", Value::Integer(5i64.into())),
            ("hideRead", Value::Boolean(true)),
            ("readerWidth", Value::String("full".into())),
            ("articleListWidth", Value::Real(405.76)),
            ("appearance", Value::String("n'importe quoi".into())),
            ("lastBackup", Value::Real(1.0)),
            ("SUHasLaunchedBefore", Value::Boolean(true)),
            ("AppleLanguages", Value::Array(vec![Value::String("fr-FR".into())])),
        ]);
        let s = parse_settings(&d);
        assert_eq!(s.prefs.retention_days, 14);
        assert_eq!(s.prefs.mark_read_delay, 5);
        assert!(s.prefs.hide_read);
        assert_eq!(s.prefs.reader_width, "full");
        assert!((s.prefs.article_list_width - 405.76).abs() < 1e-9);
        assert_eq!(s.prefs.appearance, "system"); // valeur invalide : défaut conservé
        assert_eq!(s.prefs.refresh_minutes, 30); // absente : défaut
        assert_eq!(s.language.as_deref(), Some("fr"));
    }

    #[test]
    fn never_mark_read_delay_is_kept() {
        let d = dict(vec![("markReadDelay", Value::Integer((-1i64).into()))]);
        assert_eq!(parse_settings(&d).prefs.mark_read_delay, -1);
    }

    #[test]
    fn legacy_grid_full_width_is_migrated() {
        let d = dict(vec![
            ("articleLayout", Value::String("grid".into())),
            ("gridFullWidth", Value::Boolean(true)),
        ]);
        assert_eq!(parse_settings(&d).prefs.article_layout, "gridFull");
    }

    #[test]
    fn reads_an_xml_plist_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.plist");
        let d = dict(vec![("retentionDays", Value::Integer(21i64.into()))]);
        plist::to_file_xml(&path, &Value::Dictionary(d)).unwrap();
        assert_eq!(read_settings_plist(&path).unwrap().prefs.retention_days, 21);
    }
}
