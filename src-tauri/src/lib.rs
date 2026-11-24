use std::borrow::Cow;
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
use tauri::{Manager, State};

struct AppState {
    db: Mutex<Connection>,
    db_path: PathBuf,
    /// Images des articles : notre dossier, puis ceux de l'app Swift (lecture seule).
    images: ImageDirs,
    /// Une actualisation à la fois.
    refreshing: AtomicBool,
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
    flux_core::actions::set_starred(&conn, &keys, starred).map_err(|e| e.to_string())
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

/// Prototype d'impression / export PDF (étape 1) : ouvre la boîte d'impression
/// native de la webview.
#[tauri::command]
fn print_page(window: tauri::WebviewWindow) -> Result<(), String> {
    window.print().map_err(|e| e.to_string())
}

/// Import de développement : repart d'une base vide (`flux.sqlite` dans le dossier de données
/// de l'app) et y recopie le store Swift (lu en lecture seule) et les réglages.
/// À remplacer par un vrai écran d'import ; supprime la base à chaque appel.
#[tauri::command]
fn dev_import_swift_data(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<ImportReport, String> {
    let home = app.path().home_dir().map_err(|e| e.to_string())?;
    let store = home.join("Library/Application Support/Flux/Flux.store");
    let prefs_plist = home.join("Library/Preferences/io.github.aervorn.flux.plist");

    let mut guard = lock(&state)?;
    // Libère le fichier avant de le supprimer
    *guard = flux_core::db::open_in_memory().map_err(|e| e.to_string())?;
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", state.db_path.display(), suffix));
    }

    let mut conn = flux_core::db::open(&state.db_path).map_err(|e| e.to_string())?;
    let report = flux_core::import::import_all(&store, Some(&prefs_plist), &mut conn).map_err(|e| e.to_string())?;
    *guard = conn;
    Ok(report)
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
        .register_uri_scheme_protocol("flux-image", |ctx, request| serve_image(ctx, request))
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("flux.sqlite");
            let conn = flux_core::db::open(&db_path)?;
            let swift = app.path().home_dir()?.join("Library/Application Support/Flux");
            let images = ImageDirs {
                own: dir.join("images"),
                others: vec![swift.join("Archive"), swift.join("Images")],
            };
            app.manage(AppState { db: Mutex::new(conn), db_path, images, refreshing: AtomicBool::new(false) });
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
            add_feed,
            refresh_all,
            rename_feed,
            move_feed,
            delete_feed,
            rename_folder,
            delete_folder,
            set_folder_expanded,
            rename_tag,
            delete_tag,
            create_folder,
            print_page,
            dev_import_swift_data
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de Flux");
}
