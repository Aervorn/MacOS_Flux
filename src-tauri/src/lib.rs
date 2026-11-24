use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Mutex;

use flux_core::import::ImportReport;
use flux_core::prefs::Prefs;
use flux_core::query::{ArticleRow, ListQuery, Sidebar};
use rusqlite::Connection;
use serde::Serialize;
use tauri::{Manager, State};

struct AppState {
    db: Mutex<Connection>,
    db_path: PathBuf,
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
    let found = ctx.app_handle().path().home_dir().ok().and_then(|home| {
        let base = home.join("Library/Application Support/Flux");
        flux_core::images::read_cached(&[base.join("Archive"), base.join("Images")], &name)
    });
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
            app.manage(AppState { db: Mutex::new(conn), db_path });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            get_settings,
            save_prefs,
            get_sidebar,
            list_articles,
            print_page,
            dev_import_swift_data
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de Flux");
}
