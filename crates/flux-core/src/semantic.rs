//! Comparaison des titres « par le sens », avec les plongements de phrases d'Apple
//! (NaturalLanguage : `NLEmbedding`), calculés sur le Mac.
//!
//! Hors macOS, rien n'est disponible : `similarity` renvoie toujours `None` et seule la
//! comparaison des mots joue.

#[cfg(target_os = "macos")]
mod apple {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use objc2::rc::{autoreleasepool, Retained};
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2::msg_send;
    use objc2_foundation::NSString;

    // NaturalLanguage n'est pas lié par défaut : on le demande à l'éditeur de liens.
    #[link(name = "NaturalLanguage", kind = "framework")]
    unsafe extern "C" {}

    /// Calcule (et garde en mémoire) les vecteurs de sens des titres.
    #[derive(Default)]
    pub struct Embedder {
        languages: RefCell<HashMap<String, Option<String>>>,
        embeddings: RefCell<HashMap<String, Option<Retained<AnyObject>>>>,
        vectors: RefCell<HashMap<String, Option<Vec<f64>>>>,
    }

    impl Embedder {
        /// Langue dominante du titre (« fr », « en »…), si elle est reconnue.
        pub fn language(&self, title: &str) -> Option<String> {
            if let Some(cached) = self.languages.borrow().get(title) {
                return cached.clone();
            }
            let found = autoreleasepool(|_| {
                let class = AnyClass::get(c"NLLanguageRecognizer")?;
                let text = NSString::from_str(title);
                let language: Option<Retained<NSString>> =
                    unsafe { msg_send![class, dominantLanguageForString: &*text] };
                language.map(|l| l.to_string()).filter(|l| l != "und")
            });
            self.languages.borrow_mut().insert(title.to_string(), found.clone());
            found
        }

        fn embedding(&self, language: &str) -> Option<Retained<AnyObject>> {
            if let Some(cached) = self.embeddings.borrow().get(language) {
                return cached.clone();
            }
            let found = autoreleasepool(|_| {
                let class = AnyClass::get(c"NLEmbedding")?;
                let name = NSString::from_str(language);
                let embedding: Option<Retained<AnyObject>> =
                    unsafe { msg_send![class, sentenceEmbeddingForLanguage: &*name] };
                embedding
            });
            self.embeddings.borrow_mut().insert(language.to_string(), found.clone());
            found
        }

        fn vector(&self, language: &str, title: &str) -> Option<Vec<f64>> {
            let key = format!("{language}\u{1}{title}");
            if let Some(cached) = self.vectors.borrow().get(&key) {
                return cached.clone();
            }
            let embedding = self.embedding(language);
            let found = embedding.and_then(|embedding| {
                autoreleasepool(|_| {
                    let text = NSString::from_str(&title.to_lowercase());
                    let array: Option<Retained<AnyObject>> =
                        unsafe { msg_send![&*embedding, vectorForString: &*text] };
                    let array = array?;
                    let count: usize = unsafe { msg_send![&*array, count] };
                    let mut out = Vec::with_capacity(count);
                    for i in 0..count {
                        let number: *mut AnyObject = unsafe { msg_send![&*array, objectAtIndex: i] };
                        if number.is_null() {
                            return None;
                        }
                        let value: f64 = unsafe { msg_send![number, doubleValue] };
                        out.push(value);
                    }
                    Some(out)
                })
            });
            self.vectors.borrow_mut().insert(key, found.clone());
            found
        }

        /// Ressemblance de sens (cosinus, entre -1 et 1) de deux titres de même langue.
        pub fn similarity(&self, a: &str, b: &str) -> Option<f64> {
            let language = self.language(a)?;
            if self.language(b)? != language {
                return None;
            }
            let (va, vb) = (self.vector(&language, a)?, self.vector(&language, b)?);
            Some(cosine(&va, &vb))
        }
    }

    pub(super) fn cosine(a: &[f64], b: &[f64]) -> f64 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let (mut dot, mut na, mut nb) = (0.0, 0.0, 0.0);
        for i in 0..a.len() {
            dot += a[i] * b[i];
            na += a[i] * a[i];
            nb += b[i] * b[i];
        }
        if na <= 0.0 || nb <= 0.0 {
            return 0.0;
        }
        dot / (na.sqrt() * nb.sqrt())
    }
}

#[cfg(target_os = "macos")]
pub use apple::Embedder;

/// Hors macOS : aucune comparaison par le sens.
#[cfg(not(target_os = "macos"))]
#[derive(Default)]
pub struct Embedder;

#[cfg(not(target_os = "macos"))]
impl Embedder {
    pub fn similarity(&self, _a: &str, _b: &str) -> Option<f64> {
        None
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn same_meaning_scores_higher_than_unrelated_titles() {
        let e = Embedder::default();
        let a = "Apple dévoile son nouvel iPhone lors de sa keynote";
        let b = "Keynote Apple : le nouvel iPhone est présenté";
        let c = "Recette de la tarte aux pommes de grand-mère";
        // Le modèle peut manquer sur un Mac sans la langue : on ne fait alors rien
        let (Some(close), Some(far)) = (e.similarity(a, b), e.similarity(a, c)) else {
            eprintln!("NLEmbedding indisponible : test ignoré");
            return;
        };
        println!("proche = {close:.3}, éloigné = {far:.3}");
        assert!(close > far);
    }

    #[test]
    fn cosine_of_identical_vectors_is_one() {
        assert!((apple::cosine(&[1.0, 2.0], &[1.0, 2.0]) - 1.0).abs() < 1e-9);
        assert_eq!(apple::cosine(&[1.0], &[1.0, 2.0]), 0.0);
    }
}
