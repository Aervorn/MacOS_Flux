//! Notifications des nouveaux articles et icône dans la barre des menus.
//!
//! Tout se passe côté Rust : l'actualisation régulière tourne ici (voir `refresh_loop`), même
//! quand la fenêtre est fermée et que Flux ne vit plus que dans la barre des menus.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

use tauri::image::Image;
use tauri::menu::{Menu, MenuBuilder, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::AppState;

/// Au-delà, une seule notification regroupe les nouveaux articles.
const MAX_SEPARATE: usize = 3;
const TRAY_ID: &str = "flux";

/// Ce qu'ouvre un clic sur chaque notification : un article, ou la vue Non lus (`None`).
#[derive(Default)]
pub struct Notifier {
    next_id: AtomicI32,
    targets: Mutex<HashMap<i32, Option<String>>>,
}

fn is_french(app: &AppHandle) -> bool {
    app.state::<AppState>().language.lock().map(|l| l.as_str() == "fr").unwrap_or(true)
}

fn show(app: &AppHandle, title: &str, body: &str, target: Option<String>) {
    let notifier = &app.state::<AppState>().notifier;
    let id = notifier.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    if let Ok(mut targets) = notifier.targets.lock() {
        // Les anciennes notifications n'ont plus d'intérêt : on ne garde pas tout indéfiniment
        if targets.len() > 200 {
            targets.clear();
        }
        targets.insert(id, target);
    }
    if let Err(e) = app.notification().builder().id(id).title(title).body(body).show() {
        eprintln!("notification : {e}");
    }
}

/// La fenêtre est devant l'utilisateur : inutile de le prévenir.
fn window_in_front(app: &AppHandle) -> bool {
    app.get_webview_window("main").is_some_and(|w| {
        w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false) && !w.is_minimized().unwrap_or(false)
    })
}

/// Après une actualisation : prévient des articles arrivés, selon les réglages.
pub fn after_refresh(app: &AppHandle, keys: &[String]) {
    let in_front = window_in_front(app);
    // Diagnostic (visible en lançant Flux depuis le Terminal)
    eprintln!("notifications : {} nouvel(s) article(s), fenêtre au premier plan : {in_front}", keys.len());
    if keys.is_empty() {
        return;
    }
    let state = app.state::<AppState>();
    let items = {
        let Ok(conn) = state.db.lock() else { return };
        let Ok(prefs) = flux_core::db::load_prefs(&conn) else { return };
        if in_front && !prefs.notify_in_front {
            return;
        }
        match flux_core::notify::candidates(&conn, keys, &prefs, flux_core::feeds::now()) {
            Ok(items) => items,
            Err(e) => {
                eprintln!("notifications : {e}");
                return;
            }
        }
    };
    eprintln!("notifications : {} article(s) retenu(s) par les réglages", items.len());
    if items.is_empty() {
        return;
    }
    if items.len() <= MAX_SEPARATE {
        for item in items {
            show(app, &item.source, &item.title, Some(item.key));
        }
        return;
    }
    let mut sources: Vec<&str> = Vec::new();
    for item in &items {
        if !sources.contains(&item.source.as_str()) {
            sources.push(&item.source);
        }
    }
    let mut body = sources.iter().take(3).copied().collect::<Vec<_>>().join(", ");
    if sources.len() > 3 {
        body.push('…');
    }
    let title = if is_french(app) {
        format!("{} nouveaux articles", items.len())
    } else {
        format!("{} new articles", items.len())
    };
    show(app, &title, &body, None);
}

/// Notification d'essai (réglages), même quand la fenêtre est devant.
pub fn test(app: &AppHandle) {
    let body = if is_french(app) {
        "Les notifications de Flux fonctionnent."
    } else {
        "Flux notifications are working."
    };
    show(app, "Flux", body, None);
}

/// Un clic sur une notification ouvre Flux sur l'article (ou sur les non-lus).
pub fn listen(app: &AppHandle) {
    let handle = app.clone();
    let registered = app.notification().on_action(move |performed| {
        if performed.action_id() != "tap" {
            return;
        }
        let target = performed
            .notification()
            .map(|n| n.id())
            .and_then(|id| handle.state::<AppState>().notifier.targets.lock().ok().and_then(|mut t| t.remove(&id)))
            .flatten();
        show_main(&handle);
        let _ = handle.emit("notification-open", target);
    });
    if let Err(e) = registered {
        eprintln!("notifications : {e}");
    }
}

/// Fait réapparaître la fenêtre principale (masquée, réduite ou derrière d'autres).
pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let fr = is_french(app);
    let label = |f: &'static str, e: &'static str| if fr { f } else { e };
    MenuBuilder::new(app)
        .item(&MenuItem::with_id(app, "tray_open", label("Ouvrir Flux", "Open Flux"), true, None::<&str>)?)
        .item(&MenuItem::with_id(app, "tray_refresh", label("Actualiser les flux", "Refresh feeds"), true, None::<&str>)?)
        .separator()
        .item(&MenuItem::with_id(app, "tray_quit", label("Quitter Flux", "Quit Flux"), true, None::<&str>)?)
        .build()
}

/// Crée l'icône de la barre des menus (visible seulement si l'option est activée).
pub fn setup_tray(app: &AppHandle, visible: bool) -> tauri::Result<()> {
    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        // Image « modèle » : macOS la colore lui-même selon la barre des menus (claire ou sombre)
        .icon_as_template(true)
        .tooltip("Flux")
        .menu(&tray_menu(app)?)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray_open" => show_main(app),
            "tray_refresh" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Ok(report) = crate::run_refresh(&app).await {
                        let _ = app.emit("feeds-refreshed", &report);
                    }
                });
            }
            "tray_quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    tray.set_visible(visible)
}

pub fn set_tray_visible(app: &AppHandle, visible: bool) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_visible(visible);
    }
}

/// Menu de l'icône dans la langue de l'interface.
pub fn relabel_tray(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), tray_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

#[cfg(target_os = "macos")]
mod ffi {
    unsafe extern "C" {
        pub fn flux_notify_status() -> i32;
        pub fn flux_notify_request() -> i32;
    }
}

fn permission_name(code: i32) -> &'static str {
    match code {
        0 => "notDetermined",
        1 => "denied",
        2 => "authorized",
        _ => "unknown",
    }
}

/// Autorisation donnée à Flux dans Réglages Système › Notifications (bloquant).
pub fn permission() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        // SAFETY : fonction sans argument (voir swift/Notifications.swift)
        permission_name(unsafe { ffi::flux_notify_status() })
    }
    #[cfg(not(target_os = "macos"))]
    permission_name(-1)
}

/// Demande l'autorisation à macOS (bloquant), puis renvoie l'état obtenu.
pub fn request_permission() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        // SAFETY : fonction sans argument (voir swift/Notifications.swift)
        permission_name(unsafe { ffi::flux_notify_request() })
    }
    #[cfg(not(target_os = "macos"))]
    permission_name(-1)
}
