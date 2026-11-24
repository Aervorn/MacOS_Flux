use flux_core::import::ImportReport;
use flux_core::prefs::Prefs;
use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    core_version: String,
}

#[tauri::command]
fn app_info(app: tauri::AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        core_version: flux_core::VERSION.to_string(),
    }
}

#[tauri::command]
fn default_prefs() -> Prefs {
    Prefs::default()
}

/// Prototype d'impression / export PDF (étape 1) : ouvre la boîte d'impression
/// native de la webview. À valider sur macOS 14 : c'est un des inconnus du plan.
#[tauri::command]
fn print_page(window: tauri::WebviewWindow) -> Result<(), String> {
    window.print().map_err(|e| e.to_string())
}

/// Import de développement : repart d'une base vide (`flux.sqlite` dans le dossier de données
/// de l'app) et y recopie le store Swift (lu en lecture seule) et les réglages.
/// À remplacer par un vrai écran d'import ; supprime la base à chaque appel.
#[tauri::command]
async fn dev_import_swift_data(app: tauri::AppHandle) -> Result<ImportReport, String> {
    let home = app.path().home_dir().map_err(|e| e.to_string())?;
    let store = home.join("Library/Application Support/Flux/Flux.store");
    let prefs_plist = home.join("Library/Preferences/io.github.aervorn.flux.plist");

    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let db_path = dir.join("flux.sqlite");
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", db_path.display(), suffix));
    }

    let mut conn = flux_core::db::open(&db_path).map_err(|e| e.to_string())?;
    flux_core::import::import_all(&store, Some(&prefs_plist), &mut conn).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_info, default_prefs, print_page, dev_import_swift_data])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de Flux");
}
