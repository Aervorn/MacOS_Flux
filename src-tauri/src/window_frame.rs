//! Taille et position de la fenêtre, retenues par macOS lui-même.
//!
//! La restauration en plusieurs étapes de `tauri-plugin-window-state` (position, taille de la
//! vue, puis « agrandie ») rouvrait une fenêtre agrandie trop courte : il manquait en haut la
//! hauteur d'une barre de titre. AppKit sait enregistrer puis reposer le cadre complet de la
//! fenêtre, en une fois et sans conversion de taille.

#[cfg(target_os = "macos")]
pub fn remember(window: &tauri::WebviewWindow) {
    use objc2::msg_send;
    use objc2::runtime::{AnyObject, Bool};
    use objc2_foundation::NSString;

    /// Clé sous laquelle macOS range le cadre dans les préférences de l'app.
    const FRAME_NAME: &str = "FluxMainWindow";

    let Ok(pointer) = window.ns_window() else { return };
    // SAFETY : `ns_window` renvoie la NSWindow de cette fenêtre, et on est sur le fil
    // principal (appel depuis `setup`).
    let Some(ns_window) = (unsafe { pointer.cast::<AnyObject>().as_ref() }) else { return };
    let name = NSString::from_str(FRAME_NAME);
    unsafe {
        // Repose le dernier cadre connu (sans effet au tout premier lancement)…
        let _: Bool = msg_send![ns_window, setFrameUsingName: &*name];
        // … puis demande à macOS de l'enregistrer à chaque déplacement ou redimensionnement.
        let _: Bool = msg_send![ns_window, setFrameAutosaveName: &*name];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn remember(_window: &tauri::WebviewWindow) {}
