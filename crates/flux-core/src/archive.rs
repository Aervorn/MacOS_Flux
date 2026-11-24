//! Archive des favoris : les images d'un favori (miniature et images du texte) sont rangées
//! dans un dossier permanent, que ni la limite de taille du cache ni « Vider le cache »
//! ne touchent. Le texte de l'article est déjà dans la base ; l'article complet n'y est que
//! si le lecteur l'a déjà chargé (voir les limites dans le README).

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use futures::stream::{self, StreamExt};
use rusqlite::{params, params_from_iter, Connection};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::html;
use crate::images::{cache_name, sniff_mime, ImageDirs};
use crate::net;

/// Images conservées par article (comme côté Swift).
const MAX_IMAGES_PER_ARTICLE: usize = 50;
const CONCURRENCY: usize = 6;

fn lock(db: &Mutex<Connection>) -> Result<MutexGuard<'_, Connection>> {
    db.lock().map_err(|_| Error::Other("base de données verrouillée".into()))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveStatus {
    /// Favoris pas encore archivés
    pub pending: u32,
    pub archived: u32,
    /// Taille du dossier d'archive, en octets
    pub bytes: u64,
}

pub fn status(conn: &Connection, images: &ImageDirs) -> Result<ArchiveStatus> {
    let pending = conn.query_row("SELECT COUNT(*) FROM articles WHERE is_starred = 1 AND is_archived = 0", [], |r| r.get(0))?;
    let archived = conn.query_row("SELECT COUNT(*) FROM articles WHERE is_starred = 1 AND is_archived = 1", [], |r| r.get(0))?;
    Ok(ArchiveStatus { pending, archived, bytes: dir_size(&images.archive) })
}

fn dir_size(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .map(|rd| rd.flatten().filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum())
        .unwrap_or(0)
}

/// Adresses des images à garder pour un article : miniature, puis images du texte.
fn article_images(image_url: Option<&str>, body: Option<&str>) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    if let Some(u) = image_url.filter(|u| !u.is_empty()) {
        urls.push(u.to_string());
    }
    for u in html::img_sources(body.unwrap_or("")).into_iter().take(MAX_IMAGES_PER_ARTICLE) {
        if !urls.contains(&u) {
            urls.push(u);
        }
    }
    urls
}

/// Met l'image dans l'archive : déplacée depuis notre cache, copiée depuis un autre dossier
/// (celui de l'app Swift), sinon téléchargée.
async fn archive_image(images: &ImageDirs, url: &str) -> bool {
    let name = cache_name(url);
    let target = images.archive.join(&name);
    if target.is_file() {
        return true;
    }
    if fs::create_dir_all(&images.archive).is_err() {
        return false;
    }
    let own = images.own.join(&name);
    if own.is_file() {
        return fs::rename(&own, &target).is_ok() || fs::copy(&own, &target).is_ok();
    }
    for dir in &images.others {
        let src = dir.join(&name);
        if src.is_file() {
            return fs::copy(src, &target).is_ok();
        }
    }
    let Ok(response) = net::get_image(url).await else { return false };
    sniff_mime(&response.data).is_some() && fs::write(&target, &response.data).is_ok()
}

/// Archive les favoris qui ne le sont pas encore. Renvoie le nombre d'articles archivés.
pub async fn run(db: &Mutex<Connection>, images: &ImageDirs) -> Result<usize> {
    struct Todo {
        key: String,
        image_url: Option<String>,
        body: Option<String>,
    }
    let todo: Vec<Todo> = {
        let conn = lock(db)?;
        let mut stmt = conn.prepare(
            "SELECT key, image_url, COALESCE(full_html, content_html) FROM articles WHERE is_starred = 1 AND is_archived = 0",
        )?;
        let rows = stmt.query_map([], |r| Ok(Todo { key: r.get(0)?, image_url: r.get(1)?, body: r.get(2)? }))?;
        rows.collect::<std::result::Result<_, _>>()?
    };
    let mut done = 0;
    for article in todo.iter() {
        let urls = article_images(article.image_url.as_deref(), article.body.as_deref());
        // Chaque téléchargement possède ses données (évite un souci de durées de vie avec Tauri)
        let results: Vec<bool> = stream::iter(urls.into_iter().map(|u| {
            let images = images.clone();
            async move { archive_image(&images, &u).await }
        }))
        .buffer_unordered(CONCURRENCY)
        .collect()
        .await;
        // Un article n'est marqué archivé que si sa miniature l'est ; les images du texte
        // introuvables (supprimées du site) ne bloquent pas.
        let thumb_ok = match article.image_url.as_deref().filter(|u| !u.is_empty()) {
            Some(u) => images.archive.join(cache_name(u)).is_file(),
            None => true,
        };
        let _ = results;
        if thumb_ok {
            let conn = lock(db)?;
            let changed = conn.execute("UPDATE articles SET is_archived = 1 WHERE key = ?1 AND is_starred = 1", params![article.key])?;
            done += changed;
        }
    }
    Ok(done)
}

/// Favoris retirés : leurs images quittent l'archive (elles retournent dans le cache normal),
/// sauf si un autre favori les utilise.
pub fn release(conn: &Connection, images: &ImageDirs, keys: &[String]) -> Result<()> {
    if !keys.is_empty() {
        let marks = vec!["?"; keys.len()].join(",");
        conn.execute(
            &format!("UPDATE articles SET is_archived = 0 WHERE key IN ({marks})"),
            params_from_iter(keys.iter()),
        )?;
    }
    let mut keep: HashSet<String> = HashSet::new();
    let mut stmt =
        conn.prepare("SELECT image_url, COALESCE(full_html, content_html) FROM articles WHERE is_starred = 1")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?)))?;
    for row in rows {
        let (image, body) = row?;
        keep.extend(article_images(image.as_deref(), body.as_deref()).iter().map(|u| cache_name(u)));
    }
    let Ok(entries) = fs::read_dir(&images.archive) else { return Ok(()) };
    let _ = fs::create_dir_all(&images.own);
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !keep.contains(&name) {
            let dest = images.own.join(&name);
            let _ = fs::remove_file(&dest);
            let _ = fs::rename(entry.path(), dest);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn release_moves_unused_images_back_to_cache() {
        let c = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let images = ImageDirs { own: dir.path().join("images"), archive: dir.path().join("archive"), others: vec![] };
        fs::create_dir_all(&images.archive).unwrap();
        let (a, b) = ("https://x.example/a.jpg", "https://x.example/b.jpg");
        for u in [a, b] {
            fs::write(images.archive.join(cache_name(u)), [0x89, 0x50, 0x4E, 0x47, 1, 2, 3]).unwrap();
        }
        c.execute_batch(
            "INSERT INTO feeds (id, url, title, added_at) VALUES (1, 'u', 'F', 0);
             INSERT INTO articles (id, key, feed_id, feed_url, title, published, is_starred, is_archived, image_url) VALUES
               (1, 'k1', 1, 'u', 'A', 1, 1, 1, 'https://x.example/a.jpg'),
               (2, 'k2', 1, 'u', 'B', 2, 0, 1, 'https://x.example/b.jpg');",
        )
        .unwrap();
        release(&c, &images, &["k2".into()]).unwrap();
        assert!(images.archive.join(cache_name(a)).is_file(), "l'image du favori reste archivée");
        assert!(!images.archive.join(cache_name(b)).exists());
        assert!(images.own.join(cache_name(b)).is_file(), "l'autre retourne dans le cache");
        let st = status(&c, &images).unwrap();
        assert_eq!((st.pending, st.archived), (0, 1));
    }
}
