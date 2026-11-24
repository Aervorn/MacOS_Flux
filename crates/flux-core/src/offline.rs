//! Lecture hors ligne : préparation des images des articles à lire, et gestion de la taille
//! du cache d'images (suppression des moins récemment utilisées, bouton « Vider »).
//!
//! Limite : le texte complet d'un article n'est téléchargé que par le lecteur (Readability.js
//! tourne dans l'interface). Hors ligne, un article jamais ouvert s'affiche donc avec le contenu
//! que son flux fournit, images comprises.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::SystemTime;

use futures::stream::{self, StreamExt};
use rusqlite::Connection;

use crate::error::{Error, Result};
use crate::html;
use crate::images::{cache_name, is_cache_name, ImageDirs};
use crate::refresh::fetch_to_cache;

/// Articles préparés au maximum, et images retenues par article (comme côté Swift).
const MAX_ARTICLES: usize = 400;
const MAX_IMAGES_PER_ARTICLE: usize = 25;
const CONCURRENCY: usize = 6;

fn lock(db: &Mutex<Connection>) -> Result<MutexGuard<'_, Connection>> {
    db.lock().map_err(|_| Error::Other("base de données verrouillée".into()))
}

/// Télécharge les images des articles non lus et des favoris qui ne sont pas encore en cache.
/// Renvoie le nombre d'images téléchargées.
pub async fn prepare(db: &Mutex<Connection>, images: &ImageDirs) -> Result<usize> {
    let rows: Vec<(Option<String>, Option<String>)> = {
        let conn = lock(db)?;
        let mut stmt = conn.prepare(
            "SELECT image_url, COALESCE(full_html, content_html) FROM articles
             WHERE (is_read = 0 AND is_hidden = 0 AND is_duplicate = 0) OR is_starred = 1
             ORDER BY published DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([MAX_ARTICLES as i64], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<std::result::Result<_, _>>()?
    };

    let mut seen: HashSet<String> = HashSet::new();
    let mut missing: Vec<String> = Vec::new();
    for (thumb, body) in rows {
        let urls = thumb
            .into_iter()
            .filter(|u| !u.is_empty())
            .chain(html::img_sources(body.as_deref().unwrap_or("")).into_iter().take(MAX_IMAGES_PER_ARTICLE));
        for url in urls {
            if !images.has(&cache_name(&url)) && seen.insert(url.clone()) {
                missing.push(url);
            }
        }
    }

    let results: Vec<bool> = stream::iter(missing.into_iter().map(|url| {
        let images = images.clone();
        async move { fetch_to_cache(&images, &url).await }
    }))
    .buffer_unordered(CONCURRENCY)
    .collect()
    .await;
    Ok(results.into_iter().filter(|ok| *ok).count())
}

/// Taille des images du dossier, en octets.
pub fn cache_size(dir: &Path) -> u64 {
    entries(dir).iter().map(|e| e.1).sum()
}

/// (chemin, taille, dernière utilisation) des images du dossier.
fn entries(dir: &Path) -> Vec<(std::path::PathBuf, u64, SystemTime)> {
    fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| is_cache_name(&e.file_name().to_string_lossy()))
                .filter_map(|e| {
                    let m = e.metadata().ok()?;
                    m.is_file().then(|| (e.path(), m.len(), m.modified().unwrap_or(SystemTime::UNIX_EPOCH)))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Supprime les images les moins récemment utilisées jusqu'à passer sous la limite.
/// Renvoie le nombre d'octets libérés.
pub fn trim(dir: &Path, limit_bytes: u64) -> u64 {
    let mut files = entries(dir);
    let mut total: u64 = files.iter().map(|e| e.1).sum();
    if total <= limit_bytes {
        return 0;
    }
    files.sort_by_key(|e| e.2);
    let mut freed = 0;
    for (path, size, _) in files {
        if total <= limit_bytes {
            break;
        }
        if fs::remove_file(path).is_ok() {
            total -= size;
            freed += size;
        }
    }
    freed
}

/// Vide le cache (l'archive des favoris, dans un autre dossier, n'est pas touchée).
pub fn clear(dir: &Path) {
    for (path, _, _) in entries(dir) {
        let _ = fs::remove_file(path);
    }
}

/// Marque une image comme utilisée à l'instant (pour que la suppression épargne les récentes).
pub fn touch(path: &Path) {
    if let Ok(file) = fs::OpenOptions::new().write(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn put(dir: &Path, url: &str, bytes: usize, age_secs: u64) {
        let path = dir.join(cache_name(url));
        fs::write(&path, vec![1u8; bytes]).unwrap();
        let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(age_secs)).unwrap();
    }

    #[test]
    fn trim_removes_least_recently_used_first() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "old", 100, 3000);
        put(dir.path(), "mid", 100, 2000);
        put(dir.path(), "new", 100, 1000);
        fs::write(dir.path().join("pas-une-image.txt"), "x").unwrap(); // ignoré
        assert_eq!(cache_size(dir.path()), 300);
        assert_eq!(trim(dir.path(), 150), 200);
        assert!(!dir.path().join(cache_name("old")).exists());
        assert!(!dir.path().join(cache_name("mid")).exists());
        assert!(dir.path().join(cache_name("new")).exists());
        // Une image « touchée » devient la plus récente
        put(dir.path(), "x", 100, 5000);
        touch(&dir.path().join(cache_name("x")));
        put(dir.path(), "y", 100, 10);
        trim(dir.path(), 200);
        assert!(!dir.path().join(cache_name("new")).exists());
        assert!(dir.path().join(cache_name("x")).exists(), "l'image touchée est épargnée");
        clear(dir.path());
        assert_eq!(cache_size(dir.path()), 0);
        assert!(dir.path().join("pas-une-image.txt").exists());
    }
}
