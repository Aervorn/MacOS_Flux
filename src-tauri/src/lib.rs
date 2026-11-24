mod pdf;

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use flux_core::actions::TagInfo;
use flux_core::articles::ArticleDetail;
use flux_core::images::ImageDirs;
use flux_core::import::ImportReport;
use flux_core::refresh::{AddedFeed, RefreshReport};
use flux_core::net::FetchedPage;
use flux_core::prefs::Prefs;
use flux_core::query::{ArticleRow, ListQuery, Selection, Sidebar};
use rusqlite::Connection;
use serde::Serialize;
use tauri::{Emitter, Manager, State};

struct AppState {
    db: Mutex<Connection>,
    db_path: PathBuf,
    /// Images des articles : notre dossier, puis ceux de l'app Swift (lecture seule).
    images: ImageDirs,
    /// Une actualisation à la fois.
    refreshing: AtomicBool,
    /// Archivage des favoris en cours.
    archiving: AtomicBool,
    /// Préparation hors ligne en cours.
    preparing: AtomicBool,
    /// État de la synchronisation iCloud (affiché dans les réglages).
    sync: Mutex<SyncStatus>,
    /// État de la sauvegarde iCloud (affiché dans les réglages).
    backup: Mutex<BackupStatus>,
}

#[derive(Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackupStatus {
    working: bool,
    /// Code d'erreur (`icloud_unavailable`) ou message
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BackupInfo {
    working: bool,
    error: Option<String>,
    /// Secondes Unix de la dernière sauvegarde réussie
    last_backup: Option<f64>,
    icloud_available: bool,
    backups: Vec<flux_core::backup::BackupFile>,
}

#[derive(Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncStatus {
    syncing: bool,
    /// Secondes Unix de la dernière synchro réussie
    last_sync: Option<f64>,
    /// Code d'erreur (`icloud_unavailable`) ou message
    error: Option<String>,
    others: Vec<flux_core::sync::OtherDevice>,
    icloud_available: bool,
}

fn lock<'a>(state: &'a State<'_, AppState>) -> Result<std::sync::MutexGuard<'a, Connection>, String> {
    state.db.lock().map_err(|_| "base de données verrouillée".to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    core_version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    prefs: Prefs,
    /// Langue forcée dans l'app Swift, reprise à l'import (None si absente).
    language: Option<String>,
}

#[tauri::command]
fn app_info(app: tauri::AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        core_version: flux_core::VERSION.to_string(),
    }
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let conn = lock(&state)?;
    let prefs = flux_core::db::load_prefs(&conn).map_err(|e| e.to_string())?;
    let language = flux_core::db::load_setting::<String>(&conn, "language").map_err(|e| e.to_string())?;
    Ok(Settings { prefs, language })
}

#[tauri::command]
fn save_prefs(state: State<'_, AppState>, prefs: Prefs) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::db::save_prefs(&conn, &prefs).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_sidebar(state: State<'_, AppState>) -> Result<Sidebar, String> {
    let conn = lock(&state)?;
    flux_core::query::sidebar(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_articles(state: State<'_, AppState>, query: ListQuery) -> Result<Vec<ArticleRow>, String> {
    let conn = lock(&state)?;
    flux_core::query::list_articles(&conn, &query).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_article(state: State<'_, AppState>, key: String) -> Result<Option<ArticleDetail>, String> {
    let conn = lock(&state)?;
    flux_core::articles::get_article(&conn, &key).map_err(|e| e.to_string())
}

/// Y a-t-il une connexion à Internet ?
#[tauri::command]
async fn is_online() -> bool {
    flux_core::net::is_online().await
}

/// Télécharge la page d'un article (pour en extraire le texte complet côté interface).
#[tauri::command]
async fn fetch_page(url: String) -> Result<FetchedPage, String> {
    flux_core::net::fetch_page(&url).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn save_full_html(
    state: State<'_, AppState>,
    key: String,
    html: String,
    search_text: String,
    word_count: i64,
    image_url: Option<String>,
) -> Result<bool, String> {
    let conn = lock(&state)?;
    flux_core::articles::save_full_html(&conn, &key, &html, &search_text, word_count, image_url.as_deref())
        .map_err(|e| e.to_string())
}

/// Ouvre une adresse dans le navigateur par défaut (http, https ou mailto seulement).
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let lower = url.trim().to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")) {
        return Err("adresse non prise en charge".into());
    }
    std::process::Command::new("open")
        .arg(url.trim())
        .status()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_read(state: State<'_, AppState>, keys: Vec<String>, read: bool) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::actions::set_read(&conn, &keys, read).map_err(|e| e.to_string())
}

#[tauri::command]
fn mark_all_read(state: State<'_, AppState>, selection: Selection, hide_tagged_favorites: bool) -> Result<usize, String> {
    let conn = lock(&state)?;
    flux_core::actions::mark_all_read(&conn, &selection, hide_tagged_favorites).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_starred(state: State<'_, AppState>, keys: Vec<String>, starred: bool) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::actions::set_starred(&conn, &keys, starred).map_err(|e| e.to_string())?;
    if !starred {
        // Les images de ces articles quittent l'archive (sauf si un autre favori les utilise)
        flux_core::archive::release(&conn, &state.images, &keys).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveInfo {
    pending: u32,
    archived: u32,
    bytes: u64,
    working: bool,
}

#[tauri::command]
fn archive_status(state: State<'_, AppState>) -> Result<ArchiveInfo, String> {
    let conn = lock(&state)?;
    let st = flux_core::archive::status(&conn, &state.images).map_err(|e| e.to_string())?;
    Ok(ArchiveInfo {
        pending: st.pending,
        archived: st.archived,
        bytes: st.bytes,
        working: state.archiving.load(Ordering::SeqCst),
    })
}

/// Archive les favoris pas encore archivés (images). Sans effet si l'option est désactivée
/// ou si un archivage est déjà en cours.
#[tauri::command]
async fn archive_pending(state: State<'_, AppState>) -> Result<usize, String> {
    if !current_prefs(&state)?.archive_favorites || state.archiving.swap(true, Ordering::SeqCst) {
        return Ok(0);
    }
    let result = flux_core::archive::run(&state.db, &state.images).await;
    state.archiving.store(false, Ordering::SeqCst);
    result.map_err(|e| e.to_string())
}

#[tauri::command]
fn create_tag(state: State<'_, AppState>, name: String) -> Result<Option<TagInfo>, String> {
    let conn = lock(&state)?;
    flux_core::actions::create_tag(&conn, &name).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_tag(state: State<'_, AppState>, uid: String, on: bool, keys: Vec<String>) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::actions::set_tag(&conn, &uid, on, &keys).map_err(|e| e.to_string())
}

#[tauri::command]
fn record_opened(state: State<'_, AppState>, key: String) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::actions::record_opened(&conn, &key).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_reading_seconds(state: State<'_, AppState>, key: String, seconds: f64) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::actions::add_reading_seconds(&conn, &key, seconds).map_err(|e| e.to_string())
}

#[tauri::command]
fn reading_stats(state: State<'_, AppState>, days: u32, tz_offset: i64) -> Result<flux_core::stats::Stats, String> {
    let conn = lock(&state)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    flux_core::stats::compute(&conn, days, now, tz_offset).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OfflineInfo {
    cache_bytes: u64,
    working: bool,
}

#[tauri::command]
fn offline_status(state: State<'_, AppState>) -> OfflineInfo {
    OfflineInfo {
        cache_bytes: flux_core::offline::cache_size(&state.images.own),
        working: state.preparing.load(Ordering::SeqCst),
    }
}

/// Après une actualisation (ou un changement de réglage) : prépare les articles pour la lecture
/// hors ligne si l'option est active, puis ramène le cache sous sa limite de taille.
#[tauri::command]
async fn offline_maintain(state: State<'_, AppState>) -> Result<(), String> {
    let prefs = current_prefs(&state)?;
    if prefs.offline_enabled && !state.preparing.swap(true, Ordering::SeqCst) {
        let result = flux_core::offline::prepare(&state.db, &state.images).await;
        state.preparing.store(false, Ordering::SeqCst);
        result.map_err(|e| e.to_string())?;
    }
    let limit = u64::try_from(prefs.image_cache_limit_mb).unwrap_or(0) * 1024 * 1024;
    flux_core::offline::trim(&state.images.own, limit);
    Ok(())
}

/// Articles dont l'interface doit récupérer le texte complet pour la lecture hors ligne
/// et pour l'archive des favoris (liste vide si ni l'un ni l'autre n'est activé).
#[tauri::command]
fn offline_pending_texts(state: State<'_, AppState>) -> Result<Vec<flux_core::offline::PendingText>, String> {
    let prefs = current_prefs(&state)?;
    if !prefs.offline_enabled && !prefs.archive_favorites {
        return Ok(Vec::new());
    }
    let conn = lock(&state)?;
    flux_core::offline::pending_texts(&conn, !prefs.offline_enabled).map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_image_cache(state: State<'_, AppState>) {
    flux_core::offline::clear(&state.images.own);
}

/// Prefs lues dans la base, sans garder le verrou.
fn current_prefs(state: &State<'_, AppState>) -> Result<Prefs, String> {
    let conn = lock(state)?;
    flux_core::db::load_prefs(&conn).map_err(|e| e.to_string())
}

/// Ajoute un flux (adresse du flux ou du site) et importe ses articles.
#[tauri::command]
async fn add_feed(state: State<'_, AppState>, input: String, folder_uid: Option<String>) -> Result<AddedFeed, String> {
    let prefs = current_prefs(&state)?;
    flux_core::refresh::add_feed(&state.db, &input, folder_uid.as_deref(), &prefs, &state.images)
        .await
        .map_err(|e| e.to_string())
}

/// Change l'adresse d'un flux en gardant ses articles et leur état.
#[tauri::command]
async fn change_feed_address(state: State<'_, AppState>, url: String, input: String) -> Result<AddedFeed, String> {
    let prefs = current_prefs(&state)?;
    flux_core::refresh::change_address(&state.db, &url, &input, &prefs, &state.images)
        .await
        .map_err(|e| e.to_string())
}

/// Actualise tous les flux (sans effet si une actualisation est déjà en cours).
#[tauri::command]
async fn refresh_all(state: State<'_, AppState>) -> Result<RefreshReport, String> {
    if state.refreshing.swap(true, Ordering::SeqCst) {
        return Ok(RefreshReport::default());
    }
    let result = match current_prefs(&state) {
        Ok(prefs) => flux_core::refresh::refresh_all(&state.db, &prefs, &state.images).await.map_err(|e| e.to_string()),
        Err(e) => Err(e),
    };
    state.refreshing.store(false, Ordering::SeqCst);
    result
}

#[tauri::command]
fn rename_feed(state: State<'_, AppState>, url: String, name: String) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::manage::rename_feed(&conn, &url, &name).map_err(|e| e.to_string())
}

#[tauri::command]
fn move_feed(state: State<'_, AppState>, url: String, folder_uid: Option<String>) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::manage::move_feed(&conn, &url, folder_uid.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_feed(state: State<'_, AppState>, url: String) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::manage::delete_feed(&conn, &url).map_err(|e| e.to_string())
}

#[tauri::command]
fn rename_folder(state: State<'_, AppState>, uid: String, name: String) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::manage::rename_folder(&conn, &uid, &name).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_folder_style(state: State<'_, AppState>, uid: String, symbol: Option<String>, color: Option<String>) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::manage::set_folder_style(&conn, &uid, symbol.as_deref(), color.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_folder(state: State<'_, AppState>, uid: String) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::manage::delete_folder(&conn, &uid).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_folder_expanded(state: State<'_, AppState>, uid: String, expanded: bool) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::manage::set_folder_expanded(&conn, &uid, expanded).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_filters(state: State<'_, AppState>) -> Result<Vec<flux_core::filters::Rule>, String> {
    let conn = lock(&state)?;
    flux_core::filters::list(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_filter(state: State<'_, AppState>, keyword: String, whole_word: bool, action: String, feed_urls: Vec<String>) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::filters::add(&conn, &keyword, whole_word, &action, &feed_urls).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_filter(
    state: State<'_, AppState>,
    uid: String,
    keyword: String,
    whole_word: bool,
    action: String,
    feed_urls: Vec<String>,
) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::filters::update(&conn, &uid, &keyword, whole_word, &action, &feed_urls).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_filter(state: State<'_, AppState>, uid: String) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::filters::delete(&conn, &uid).map_err(|e| e.to_string())
}

/// Nombre d'articles (hors favoris) que la règle toucherait.
#[tauri::command]
fn preview_filter(state: State<'_, AppState>, keyword: String, whole_word: bool, feed_urls: Vec<String>) -> Result<i64, String> {
    let conn = lock(&state)?;
    flux_core::filters::preview_count(&conn, &keyword, whole_word, &feed_urls).map_err(|e| e.to_string())
}

/// Recalcule les groupes de doublons (après un changement de réglage).
#[tauri::command]
fn regroup_duplicates(state: State<'_, AppState>) -> Result<(), String> {
    let conn = lock(&state)?;
    let prefs = flux_core::db::load_prefs(&conn).map_err(|e| e.to_string())?;
    flux_core::cluster::regroup_recent(&conn, &prefs).map_err(|e| e.to_string())
}

#[tauri::command]
fn detach_from_cluster(state: State<'_, AppState>, key: String) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::cluster::detach(&conn, &key).map_err(|e| e.to_string())
}

/// Ajoute les flux d'un fichier OPML (sans les télécharger : l'interface actualise ensuite).
#[tauri::command]
fn import_opml(state: State<'_, AppState>, path: String) -> Result<flux_core::opml::ImportResult, String> {
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    let conn = lock(&state)?;
    flux_core::opml::import(&conn, &data).map_err(|e| e.to_string())
}

/// Écrit tous les abonnements dans un fichier OPML.
#[tauri::command]
fn export_opml(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let xml = {
        let conn = lock(&state)?;
        flux_core::opml::export(&conn).map_err(|e| e.to_string())?
    };
    std::fs::write(&path, xml).map_err(|e| e.to_string())
}

#[tauri::command]
fn rename_tag(state: State<'_, AppState>, uid: String, name: String) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::manage::rename_tag(&conn, &uid, &name).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_tag(state: State<'_, AppState>, uid: String) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::manage::delete_tag(&conn, &uid).map_err(|e| e.to_string())
}

/// Crée un dossier et renvoie son uid (None si le nom est vide).
#[tauri::command]
fn create_folder(state: State<'_, AppState>, name: String) -> Result<Option<String>, String> {
    let conn = lock(&state)?;
    flux_core::manage::create_folder(&conn, &name).map_err(|e| e.to_string())
}

// --- Synchronisation iCloud ---

/// Une erreur « Operation not permitted » dans iCloud Drive vient presque toujours de
/// l'autorisation macOS (Réglages Système > Confidentialité et sécurité) : code dédié.
fn icloud_error(e: String) -> String {
    if e.contains("Operation not permitted") || e.contains("Permission denied") {
        "icloud_permission".to_string()
    } else {
        e
    }
}

/// Une synchro complète ; prévient l'interface si la base a changé.
fn run_sync(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        let mut st = state.sync.lock().map_err(|_| "état verrouillé".to_string())?;
        if st.syncing {
            return Ok(());
        }
        st.syncing = true;
    }
    let result = (|| -> Result<flux_core::sync::SyncReport, String> {
        if !flux_core::sync::is_icloud_available() {
            return Err("icloud_unavailable".to_string());
        }
        let conn = state.db.lock().map_err(|_| "base de données verrouillée".to_string())?;
        let prefs = flux_core::db::load_prefs(&conn).map_err(|e| e.to_string())?;
        let dev = flux_core::sync::device(&conn).map_err(|e| e.to_string())?;
        flux_core::sync::sync(&conn, &flux_core::sync::devices_dir(), &dev, prefs.retention_days, flux_core::feeds::now())
            .map_err(|e| icloud_error(e.to_string()))
    })();
    let mut st = state.sync.lock().map_err(|_| "état verrouillé".to_string())?;
    st.syncing = false;
    st.icloud_available = flux_core::sync::is_icloud_available();
    match result {
        Ok(report) => {
            st.last_sync = Some(flux_core::feeds::now());
            st.error = None;
            st.others = report.others.clone();
            drop(st);
            if report.changed || !report.new_feeds.is_empty() {
                let _ = app.emit("sync-changed", report.new_feeds.len());
            }
            Ok(())
        }
        Err(e) => {
            st.error = Some(e.clone());
            Err(e)
        }
    }
}

#[tauri::command]
fn sync_status(state: State<'_, AppState>) -> Result<SyncStatus, String> {
    let mut st = state.sync.lock().map_err(|_| "état verrouillé".to_string())?.clone();
    st.icloud_available = flux_core::sync::is_icloud_available();
    Ok(st)
}

#[tauri::command]
fn sync_now(app: tauri::AppHandle) -> Result<(), String> {
    run_sync(&app)
}

/// Appelé après l'activation (synchro tout de suite) ou la désactivation (on retire ce Mac).
#[tauri::command]
fn sync_apply(app: tauri::AppHandle, state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    if enabled {
        return run_sync(&app);
    }
    let conn = lock(&state)?;
    let dev = flux_core::sync::device(&conn).map_err(|e| e.to_string())?;
    flux_core::sync::disable(&conn, &flux_core::sync::devices_dir(), &dev).map_err(|e| e.to_string())?;
    drop(conn);
    *state.sync.lock().map_err(|_| "état verrouillé".to_string())? = SyncStatus::default();
    Ok(())
}

/// Surveille la base (après 5 s sans modification) et les fichiers des autres Mac (toutes les 30 s).
fn sync_loop(app: tauri::AppHandle) {
    use std::time::{Duration, Instant};
    let mut last_changes = None;
    let mut dirty_since: Option<Instant> = None;
    let mut last_remote_check = Instant::now();
    let mut seen: HashMap<PathBuf, f64> = HashMap::new();
    let mut first = true;
    loop {
        std::thread::sleep(Duration::from_secs(2));
        let state = app.state::<AppState>();
        let (enabled, changes, dev_id) = {
            let Ok(conn) = state.db.lock() else { continue };
            let enabled = flux_core::db::load_prefs(&conn).map(|p| p.sync_enabled).unwrap_or(false);
            let id = flux_core::sync::device(&conn).map(|d| d.id).unwrap_or_default();
            (enabled, conn.total_changes(), id)
        };
        if !enabled {
            last_changes = None;
            dirty_since = None;
            first = true;
            continue;
        }
        let mut due = std::mem::take(&mut first);
        if last_changes.is_some_and(|prev| prev != changes) {
            dirty_since = Some(Instant::now());
        }
        last_changes = Some(changes);
        if dirty_since.is_some_and(|t| t.elapsed() >= Duration::from_secs(5)) {
            due = true;
        }
        let dir = flux_core::sync::devices_dir();
        if last_remote_check.elapsed() >= Duration::from_secs(30) {
            last_remote_check = Instant::now();
            let now: HashMap<PathBuf, f64> = flux_core::sync::remote_files(&dir, &dev_id).into_iter().collect();
            let has_error = state.sync.lock().map(|s| s.error.is_some()).unwrap_or(false);
            if now != seen || has_error {
                due = true;
            }
        }
        if due {
            dirty_since = None;
            let _ = run_sync(&app);
            seen = flux_core::sync::remote_files(&dir, &dev_id).into_iter().collect();
            last_changes = state.db.lock().ok().map(|c| c.total_changes());
        }
    }
}

// --- Sauvegarde iCloud ---

fn run_backup(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        let mut st = state.backup.lock().map_err(|_| "état verrouillé".to_string())?;
        if st.working {
            return Ok(());
        }
        st.working = true;
        st.error = None;
    }
    let result = (|| -> Result<(), String> {
        if !flux_core::sync::is_icloud_available() {
            return Err("icloud_unavailable".to_string());
        }
        // La base n'est bloquée que le temps de la copie ; la compression se fait ensuite.
        let (staging, dest, keep) = {
            let conn = state.db.lock().map_err(|_| "base de données verrouillée".to_string())?;
            let prefs = flux_core::db::load_prefs(&conn).map_err(|e| e.to_string())?;
            let dev = flux_core::sync::device(&conn).map_err(|e| e.to_string())?;
            let staging = flux_core::backup::stage(
                &conn,
                &state.images,
                prefs.backup_include_archive,
                &dev.name,
                &app.package_info().version.to_string(),
            )
            .map_err(|e| e.to_string())?;
            (staging, flux_core::backup::device_dir(&flux_core::backup::backups_root(), &dev.name), prefs.backup_keep)
        };
        flux_core::backup::pack(&staging, &dest, &flux_core::backup::stamp_now(), keep).map_err(|e| icloud_error(e.to_string()))?;
        let conn = state.db.lock().map_err(|_| "base de données verrouillée".to_string())?;
        flux_core::backup::record_backup(&conn, flux_core::feeds::now()).map_err(|e| e.to_string())
    })();
    let mut st = state.backup.lock().map_err(|_| "état verrouillé".to_string())?;
    st.working = false;
    if let Err(e) = &result {
        st.error = Some(e.clone());
    }
    result
}

#[tauri::command]
fn backup_status(state: State<'_, AppState>) -> Result<BackupInfo, String> {
    let st = state.backup.lock().map_err(|_| "état verrouillé".to_string())?.clone();
    let last_backup = {
        let conn = lock(&state)?;
        flux_core::backup::last_backup(&conn).map_err(|e| e.to_string())?
    };
    Ok(BackupInfo {
        working: st.working,
        error: st.error,
        last_backup,
        icloud_available: flux_core::sync::is_icloud_available(),
        backups: flux_core::backup::list_backups(&flux_core::backup::backups_root()),
    })
}

#[tauri::command]
fn backup_now(app: tauri::AppHandle) -> Result<(), String> {
    run_backup(&app)
}

/// Prépare la restauration, puis relance l'app pour l'appliquer. En développement, la relance
/// est laissée à l'utilisateur (renvoie false) : `tauri dev` ne survit pas à un redémarrage.
#[tauri::command]
fn restore_backup(app: tauri::AppHandle, state: State<'_, AppState>, path: String) -> Result<bool, String> {
    let zip = PathBuf::from(&path);
    // Uniquement une archive rangée dans le dossier des sauvegardes
    if !zip.starts_with(flux_core::backup::backups_root()) || zip.components().any(|c| c.as_os_str() == "..") {
        return Err("Chemin de sauvegarde invalide.".to_string());
    }
    let data_dir = state.db_path.parent().ok_or("dossier de données introuvable")?.to_path_buf();
    {
        let mut st = state.backup.lock().map_err(|_| "état verrouillé".to_string())?;
        st.working = true;
        st.error = None;
    }
    let result = flux_core::backup::prepare_restore(&data_dir, &zip).map_err(|e| e.to_string());
    let mut st = state.backup.lock().map_err(|_| "état verrouillé".to_string())?;
    st.working = false;
    if let Err(e) = &result {
        st.error = Some(e.clone());
    }
    drop(st);
    result?;
    if cfg!(debug_assertions) {
        Ok(false)
    } else {
        app.restart()
    }
}

/// Quitte l'app (après une restauration préparée en développement, où la relance est manuelle).
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// Sauvegarde automatique : vérifie une minute après le lancement, puis toutes les heures.
fn backup_loop(app: tauri::AppHandle) {
    std::thread::sleep(std::time::Duration::from_secs(60));
    loop {
        let due = {
            let state = app.state::<AppState>();
            let due = match state.db.lock() {
                Ok(conn) => {
                    let enabled = flux_core::db::load_prefs(&conn).map(|p| p.backup_enabled).unwrap_or(false);
                    let last = flux_core::backup::last_backup(&conn).ok().flatten().unwrap_or(0.0);
                    enabled && flux_core::feeds::now() - last >= 86_400.0
                }
                Err(_) => false,
            };
            due && flux_core::sync::is_icloud_available()
        };
        if due {
            let _ = run_backup(&app);
        }
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

/// Prototype d'impression / export PDF (étape 1) : ouvre la boîte d'impression
/// native de la webview.
#[tauri::command]
fn print_page(window: tauri::WebviewWindow) -> Result<(), String> {
    window.print().map_err(|e| e.to_string())
}

/// Exporte l'article affiché en PDF : demande où l'enregistrer, puis imprime dans le fichier.
/// Renvoie le chemin du PDF, ou `None` si l'utilisateur annule.
#[tauri::command]
async fn export_pdf(app: tauri::AppHandle, window: tauri::WebviewWindow, title: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let chosen = app
        .dialog()
        .file()
        .set_file_name(pdf::file_name(&title))
        .add_filter("PDF", &["pdf"])
        .blocking_save_file();
    let Some(chosen) = chosen else { return Ok(None) };
    let path = chosen.into_path().map_err(|e| e.to_string())?;
    let target = path.clone();
    tauri::async_runtime::spawn_blocking(move || pdf::export(&window, &target))
        .await
        .map_err(|e| e.to_string())??;
    Ok(Some(path.to_string_lossy().to_string()))
}

/// Chemins de l'app Swift (version précédente de Flux).
fn swift_paths(app: &tauri::AppHandle) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let home = app.path().home_dir().map_err(|e| e.to_string())?;
    let dir = home.join("Library/Application Support/Flux");
    Ok((
        dir.join("Flux.store"),
        home.join("Library/Preferences/io.github.aervorn.flux.plist"),
        dir,
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SwiftImportStatus {
    /// Un store Swift existe et la base actuelle est vide : on peut proposer l'import.
    available: bool,
    /// L'utilisateur a déjà refusé la proposition.
    dismissed: bool,
}

#[tauri::command]
fn swift_import_status(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<SwiftImportStatus, String> {
    let (store, _, _) = swift_paths(&app)?;
    let conn = lock(&state)?;
    let existing: i64 = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM feeds) + (SELECT COUNT(*) FROM articles)",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let dismissed = flux_core::db::load_setting::<bool>(&conn, "swiftImportDismissed")
        .map_err(|e| e.to_string())?
        .unwrap_or(false);
    Ok(SwiftImportStatus { available: store.is_file() && existing == 0, dismissed })
}

#[tauri::command]
fn swift_import_dismiss(state: State<'_, AppState>) -> Result<(), String> {
    let conn = lock(&state)?;
    flux_core::db::save_setting(&conn, "swiftImportDismissed", &true).map_err(|e| e.to_string())
}

/// Copie les fichiers d'un dossier d'images vers le nôtre (sans écraser). Meilleur effort.
fn copy_images(from: &std::path::Path, to: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(from) else { return };
    let _ = std::fs::create_dir_all(to);
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else { continue };
        if !flux_core::images::is_cache_name(name_str) {
            continue;
        }
        let dest = to.join(&name);
        if !dest.exists() {
            let _ = std::fs::copy(entry.path(), dest);
        }
    }
}

/// Reprend les données de l'app Swift (lecture seule de l'existant) dans la base actuelle,
/// qui doit être vide, puis copie les images en cache pour ne plus dépendre de l'ancienne app.
#[tauri::command]
fn import_swift_data(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<ImportReport, String> {
    let (store, prefs_plist, swift_dir) = swift_paths(&app)?;
    let report = {
        let mut conn = lock(&state)?;
        flux_core::import::import_all(&store, Some(&prefs_plist), &mut conn).map_err(|e| e.to_string())?
    };
    copy_images(&swift_dir.join("Archive"), &state.images.own);
    copy_images(&swift_dir.join("Images"), &state.images.own);
    Ok(report)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInfo {
    version: String,
}

/// Cherche une mise à jour (fichier `latest.json` de la dernière release GitHub).
#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    Ok(update.map(|u| UpdateInfo { version: u.version }))
}

/// Télécharge et installe la mise à jour (signature vérifiée avec la clé publique de
/// `tauri.conf.json`), puis relance l'app.
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "aucune mise à jour disponible".to_string())?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart()
}

/// Sert `flux-image://localhost/<nom de cache>` depuis le cache d'images de l'app Swift
/// (Archive/ puis Images/), en lecture seule.
fn serve_image(
    ctx: tauri::UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Cow<'static, [u8]>> {
    let name = request.uri().path().trim_start_matches('/').to_string();
    let state = ctx.app_handle().state::<AppState>();
    let found = flux_core::images::read_cached(&state.images.read_order(), &name);
    if found.is_some() && flux_core::images::is_cache_name(&name) {
        // Image utilisée à l'instant : la suppression du cache épargne les récentes
        flux_core::offline::touch(&state.images.own.join(&name));
    }
    match found {
        Some((data, mime)) => tauri::http::Response::builder()
            .status(200)
            .header("Content-Type", mime)
            .header("Cache-Control", "max-age=86400")
            .body(Cow::Owned(data))
            .unwrap(),
        None => tauri::http::Response::builder()
            .status(404)
            .body(Cow::Owned(Vec::new()))
            .unwrap(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .register_uri_scheme_protocol("flux-image", |ctx, request| serve_image(ctx, request))
        .setup(|app| {
            // Menu par défaut de macOS, mais « Tout sélectionner » (⌘A) est géré par l'interface :
            // il sélectionne les articles de la liste, ou le texte d'un champ en cours de saisie.
            {
                use tauri::menu::{MenuBuilder, MenuItem, SubmenuBuilder};
                use tauri::Emitter;
                // Menu construit de zéro (plutôt que modifié) : « Tout sélectionner » (⌘A) est géré
                // par l'interface, il sélectionne les articles de la liste ou le texte d'un champ.
                let select_all = MenuItem::with_id(app, "select_all", "Select All", true, Some("CmdOrCtrl+A"))?;
                let app_menu = SubmenuBuilder::new(app, "Flux")
                    .about(None)
                    .separator()
                    .services()
                    .separator()
                    .hide()
                    .hide_others()
                    .show_all()
                    .separator()
                    .quit()
                    .build()?;
                let edit_menu = SubmenuBuilder::new(app, "Edit")
                    .undo()
                    .redo()
                    .separator()
                    .cut()
                    .copy()
                    .paste()
                    .item(&select_all)
                    .build()?;
                let view_menu = SubmenuBuilder::new(app, "View").fullscreen().build()?;
                let window_menu = SubmenuBuilder::new(app, "Window")
                    .minimize()
                    .maximize()
                    .separator()
                    .close_window()
                    .build()?;
                let menu = MenuBuilder::new(app)
                    .items(&[&app_menu, &edit_menu, &view_menu, &window_menu])
                    .build()?;
                app.set_menu(menu)?;
                app.on_menu_event(|app, event| {
                    if event.id().as_ref() == "select_all" {
                        let _ = app.emit("select-all", ());
                    }
                });
            }
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            // Restauration préparée : appliquée avant d'ouvrir la base
            if let Err(e) = flux_core::backup::apply_pending_restore(&dir) {
                eprintln!("restauration : {e}");
            }
            let db_path = dir.join("flux.sqlite");
            let conn = flux_core::db::open(&db_path)?;
            let swift = app.path().home_dir()?.join("Library/Application Support/Flux");
            let images = ImageDirs {
                own: dir.join("images"),
                archive: dir.join("archive"),
                others: vec![swift.join("Archive"), swift.join("Images")],
            };
            app.manage(AppState {
                db: Mutex::new(conn),
                db_path,
                images,
                refreshing: AtomicBool::new(false),
                archiving: AtomicBool::new(false),
                preparing: AtomicBool::new(false),
                sync: Mutex::new(SyncStatus::default()),
                backup: Mutex::new(BackupStatus::default()),
            });
            let handle = app.handle().clone();
            std::thread::spawn(move || sync_loop(handle));
            let handle = app.handle().clone();
            std::thread::spawn(move || backup_loop(handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            get_settings,
            save_prefs,
            get_sidebar,
            list_articles,
            get_article,
            fetch_page,
            save_full_html,
            open_url,
            set_read,
            mark_all_read,
            set_starred,
            create_tag,
            set_tag,
            record_opened,
            add_reading_seconds,
            reading_stats,
            archive_status,
            archive_pending,
            is_online,
            offline_status,
            offline_maintain,
            offline_pending_texts,
            clear_image_cache,
            add_feed,
            refresh_all,
            rename_feed,
            move_feed,
            delete_feed,
            rename_folder,
            set_folder_style,
            delete_folder,
            set_folder_expanded,
            rename_tag,
            delete_tag,
            list_filters,
            add_filter,
            update_filter,
            delete_filter,
            preview_filter,
            regroup_duplicates,
            detach_from_cluster,
            sync_status,
            sync_now,
            sync_apply,
            backup_status,
            backup_now,
            restore_backup,
            quit_app,
            import_opml,
            export_opml,
            create_folder,
            print_page,
            export_pdf,
            change_feed_address,
            check_update,
            install_update,
            swift_import_status,
            swift_import_dismiss,
            import_swift_data
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de Flux");
}
