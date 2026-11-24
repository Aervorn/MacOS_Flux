//! Export PDF direct : WebKit imprime l'article (avec la mise en page d'impression de l'app)
//! dans un fichier, sans passer par la boîte d'impression de macOS.

use std::path::Path;
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
mod apple {
    use std::ffi::c_void;

    use objc2::encode::{Encode, Encoding};
    use objc2::msg_send;
    use objc2::rc::{autoreleasepool, Retained};
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2_foundation::NSString;

    // Constantes d'AppKit : travail d'impression « enregistrer » et adresse du fichier.
    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {
        static NSPrintSaveJob: *const AnyObject;
        static NSPrintJobSavingURL: *const AnyObject;
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Point {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Size {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Rect {
        origin: Point,
        size: Size,
    }
    unsafe impl Encode for Point {
        const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
    }
    unsafe impl Encode for Size {
        const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
    }
    unsafe impl Encode for Rect {
        const ENCODING: Encoding = Encoding::Struct("CGRect", &[Point::ENCODING, Size::ENCODING]);
    }

    /// Lance l'impression de la WKWebView vers `path`. À appeler sur le fil principal ;
    /// le fichier apparaît un peu plus tard (l'opération se termine en arrière-plan).
    pub fn start(webview: *mut c_void, ns_window: *mut c_void, path: &str) -> Result<(), String> {
        let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(webview, ns_window, path)));
        match attempt {
            Ok(result) => result,
            Err(_) => Err("l'appel à macOS a échoué".to_string()),
        }
    }

    fn run(webview: *mut c_void, ns_window: *mut c_void, path: &str) -> Result<(), String> {
        autoreleasepool(|_| {
            let web = webview as *mut AnyObject;
            // Les arguments « id » se passent par référence (encodage `@`), pas par pointeur brut.
            let window: &AnyObject = unsafe { &*(ns_window as *mut AnyObject) };
            let save_job: &AnyObject = unsafe { &*NSPrintSaveJob };
            let saving_url_key: &AnyObject = unsafe { &*NSPrintJobSavingURL };

            let print_info = AnyClass::get(c"NSPrintInfo").ok_or("NSPrintInfo introuvable")?;
            let shared: *mut AnyObject = unsafe { msg_send![print_info, sharedPrintInfo] };
            let info: Retained<AnyObject> = unsafe { msg_send![shared, copy] };

            let nsurl = AnyClass::get(c"NSURL").ok_or("NSURL introuvable")?;
            let path_ns = NSString::from_str(path);
            let url: Retained<AnyObject> = unsafe { msg_send![nsurl, fileURLWithPath: &*path_ns] };
            let dictionary: Retained<AnyObject> = unsafe { msg_send![&*info, dictionary] };
            unsafe {
                let _: () = msg_send![&*info, setJobDisposition: save_job];
                let _: () = msg_send![&*dictionary, setObject: &*url, forKey: saving_url_key];
                let _: () = msg_send![&*info, setTopMargin: 40.0f64];
                let _: () = msg_send![&*info, setBottomMargin: 40.0f64];
                let _: () = msg_send![&*info, setLeftMargin: 40.0f64];
                let _: () = msg_send![&*info, setRightMargin: 40.0f64];
            }
            let paper: Size = unsafe { msg_send![&*info, paperSize] };

            let operation: Option<Retained<AnyObject>> = unsafe { msg_send![web, printOperationWithPrintInfo: &*info] };
            let operation = operation.ok_or("WebKit n'a pas créé l'opération d'impression")?;

            // WebKit imprime une vue vide si son cadre n'a pas la taille de la page.
            let view: Option<Retained<AnyObject>> = unsafe { msg_send![&*operation, view] };
            if let Some(view) = view {
                let frame = Rect { origin: Point { x: 0.0, y: 0.0 }, size: paper };
                unsafe {
                    let _: () = msg_send![&*view, setFrame: frame];
                }
            }

            let no = Bool::NO;
            let none: Option<&AnyObject> = None;
            // Sans délégué, ce sélecteur n'est jamais appelé : il sert seulement à remplir l'argument.
            let did_run = objc2::sel!(printOperationDidRun:success:contextInfo:);
            unsafe {
                let _: () = msg_send![&*operation, setShowsPrintPanel: no];
                let _: () = msg_send![&*operation, setShowsProgressPanel: no];
                let _: () = msg_send![
                    &*operation,
                    runOperationModalForWindow: window,
                    delegate: none,
                    didRunSelector: did_run,
                    contextInfo: std::ptr::null_mut::<c_void>()
                ];
            }
            // L'opération continue après le retour de cette fonction : on ne la libère pas
            // (quelques octets perdus par export).
            std::mem::forget(operation);
            std::mem::forget(info);
            Ok(())
        })
    }
}

/// Imprime la fenêtre dans `path` (PDF). Bloque jusqu'à ce que le fichier soit écrit.
#[cfg(target_os = "macos")]
pub fn export(window: &tauri::WebviewWindow, path: &Path) -> Result<(), String> {
    use std::sync::mpsc;

    // Un ancien fichier (remplacement confirmé dans la fenêtre d'enregistrement) fausserait l'attente.
    let _ = std::fs::remove_file(path);

    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    let target = path.to_string_lossy().to_string();
    window
        .with_webview(move |webview| {
            let _ = tx.send(apple::start(webview.inner(), webview.ns_window(), &target));
        })
        .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(10)).map_err(|_| "pas de réponse de la fenêtre".to_string())??;

    // Le fichier apparaît puis grossit : on attend qu'il ne change plus.
    let started = Instant::now();
    let mut last_size = 0u64;
    let mut stable = 0;
    while started.elapsed() < Duration::from_secs(40) {
        std::thread::sleep(Duration::from_millis(400));
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if size > 0 && size == last_size {
            stable += 1;
            if stable >= 2 {
                return Ok(());
            }
        } else {
            stable = 0;
        }
        last_size = size;
    }
    Err("le PDF n'a pas été créé".to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn export(_window: &tauri::WebviewWindow, _path: &Path) -> Result<(), String> {
    Err("l'export PDF n'est disponible que sur macOS".to_string())
}

/// Nom de fichier proposé : sans caractères interdits, de longueur raisonnable.
pub fn file_name(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { ' ' } else { c })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned: String = cleaned.chars().take(100).collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() { "Article.pdf".to_string() } else { format!("{cleaned}.pdf") }
}

#[cfg(test)]
mod tests {
    use super::file_name;

    #[test]
    fn cleans_the_suggested_name() {
        assert_eq!(file_name("Qui a gagné ? : « le match »"), "Qui a gagné « le match ».pdf");
        assert_eq!(file_name("  ///  "), "Article.pdf");
        assert!(file_name(&"a".repeat(300)).chars().count() <= 104);
    }
}
