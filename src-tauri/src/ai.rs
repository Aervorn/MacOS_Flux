//! Résumés d'articles par Apple Intelligence (modèle local d'Apple, voir `swift/Summarizer.swift`).
//!
//! Disponible seulement sur macOS 26 ou plus, sur un Mac Apple Silicon où Apple Intelligence
//! est activée. Ailleurs, `status` le dit et l'interface masque le bouton.

/// Au-delà, le texte est coupé : le modèle d'Apple a une fenêtre de contexte réduite.
const MAX_CHARS: usize = 7000;

#[cfg(target_os = "macos")]
mod ffi {
    use std::ffi::c_char;
    unsafe extern "C" {
        pub fn flux_ai_status() -> i32;
        pub fn flux_ai_summarize(text: *const c_char, lang: *const c_char) -> *mut c_char;
        pub fn flux_ai_free(pointer: *mut c_char);
    }
}

/// "available", "unsupported" (macOS trop ancien ou Mac Intel), "deviceNotEligible",
/// "notEnabled" (Apple Intelligence désactivée), "notReady" (modèle en cours de téléchargement)
/// ou "unavailable".
pub fn status() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        // SAFETY : fonction sans argument, sans état partagé
        match unsafe { ffi::flux_ai_status() } {
            0 => "available",
            2 => "deviceNotEligible",
            3 => "notEnabled",
            4 => "notReady",
            5 => "unavailable",
            _ => "unsupported",
        }
    }
    #[cfg(not(target_os = "macos"))]
    "unsupported"
}

/// Résume le texte d'un article (bloquant : à lancer hors du fil principal).
pub fn summarize(text: &str, lang: &str) -> Result<String, String> {
    let text: String = text.replace('\0', " ").chars().take(MAX_CHARS).collect();
    #[cfg(target_os = "macos")]
    {
        use std::ffi::{CStr, CString};
        let text = CString::new(text).map_err(|e| e.to_string())?;
        let lang = CString::new(if lang.starts_with("fr") { "fr" } else { "en" }).map_err(|e| e.to_string())?;
        // SAFETY : chaînes C valides pendant l'appel ; le résultat est rendu à Swift qui l'a alloué
        let raw = unsafe {
            let pointer = ffi::flux_ai_summarize(text.as_ptr(), lang.as_ptr());
            if pointer.is_null() {
                return Err("unknown".into());
            }
            let value = CStr::from_ptr(pointer).to_string_lossy().into_owned();
            ffi::flux_ai_free(pointer);
            value
        };
        match raw.strip_prefix("ok:") {
            Some(summary) if !summary.trim().is_empty() => Ok(summary.trim().to_string()),
            Some(_) => Err("empty".into()),
            None => Err(raw.strip_prefix("err:").unwrap_or(&raw).to_string()),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (text, lang);
        Err("unsupported".into())
    }
}
