use flux_core::prefs::Prefs;
use serde::Serialize;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_info, default_prefs, print_page])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de Flux");
}
