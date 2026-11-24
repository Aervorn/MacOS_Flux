//! Sauvegardes complètes dans iCloud Drive.
//!
//! Contrairement à la synchronisation (qui propage aussi les suppressions et les erreurs), une
//! sauvegarde est une photo figée : une archive `.zip` par jour, rangée dans
//!
//! ```text
//! iCloud Drive/Flux/Sauvegardes Tauri/<nom du Mac>/Flux AAAA-MM-JJ HH.mm.zip
//! ```
//!
//! (dossier distinct de celui de l'app Swift : les deux formats ne sont pas compatibles).
//! Elle contient la base (flux, dossiers, articles, favoris, règles, historique, réglages) et,
//! en option, les images des favoris. Le reste du cache d'images n'y est pas : il se retélécharge.
//!
//! Restaurer ne remplace pas la base pendant que l'app tourne : la sauvegarde est décompressée
//! à côté (`prepare_restore`), puis appliquée au lancement suivant (`apply_pending_restore`),
//! avant l'ouverture de la base.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::images::{cache_name, ImageDirs};
use crate::{db, sync};

const DB_NAME: &str = "flux.sqlite";
const PENDING: &str = "pending-restore";
const BEFORE: &str = "before-restore";
const LAST_BACKUP_KEY: &str = "lastBackup";

fn io(e: std::io::Error) -> Error {
    Error::Other(e.to_string())
}

pub fn backups_root() -> PathBuf {
    sync::icloud_drive().join("Flux").join("Sauvegardes Tauri")
}

/// Un sous-dossier par Mac : la rotation d'un Mac n'efface pas les sauvegardes de l'autre.
pub fn device_dir(root: &Path, device_name: &str) -> PathBuf {
    let name: String = device_name.chars().map(|c| if matches!(c, '/' | ':' | '\\') { '-' } else { c }).collect();
    root.join(name)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    /// Nom du fichier, par exemple « Flux 2026-11-24 17.30.zip »
    pub name: String,
    /// Nom du Mac qui l'a faite (nom du dossier)
    pub device: String,
    /// Chemin de l'archive (même si iCloud ne l'a pas encore téléchargée)
    pub path: String,
    pub size: u64,
    /// Secondes Unix
    pub modified: f64,
    /// Faux si le fichier n'est pas encore sur ce Mac (il sera téléchargé à la restauration)
    pub downloaded: bool,
}

/// Toutes les sauvegardes de tous les Mac, de la plus récente à la plus ancienne.
pub fn list_backups(root: &Path) -> Vec<BackupFile> {
    let mut out = Vec::new();
    let Ok(devices) = fs::read_dir(root) else {
        return out;
    };
    for device in devices.flatten() {
        let device_name = device.file_name().to_string_lossy().to_string();
        let Ok(files) = fs::read_dir(device.path()) else { continue };
        for file in files.flatten() {
            let mut name = file.file_name().to_string_lossy().to_string();
            // Pas téléchargée sur ce Mac : « .Flux ….zip.icloud »
            let downloaded = !(name.starts_with('.') && name.ends_with(".icloud"));
            if !downloaded {
                name = name.trim_start_matches('.').trim_end_matches(".icloud").to_string();
            }
            if !name.starts_with("Flux ") || !name.ends_with(".zip") {
                continue;
            }
            let meta = file.metadata().ok();
            out.push(BackupFile {
                path: device.path().join(&name).to_string_lossy().to_string(),
                name,
                device: device_name.clone(),
                size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                modified: meta
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0),
                downloaded,
            });
        }
    }
    out.sort_by(|a, b| b.name.cmp(&a.name).then(a.device.cmp(&b.device)));
    out
}

/// Horodatage du nom de fichier (heure locale).
pub fn stamp_now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H.%M").to_string()
}

pub fn last_backup(conn: &Connection) -> Result<Option<f64>> {
    db::load_setting(conn, LAST_BACKUP_KEY)
}

fn ditto(args: &[&Path]) -> Result<()> {
    let status = Command::new("/usr/bin/ditto").args(args).status().map_err(io)?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Other("Impossible de créer ou d'ouvrir l'archive de la sauvegarde.".into()))
    }
}

/// Étape 1 (avec la base) : copie cohérente de la base, informations et images des favoris,
/// dans un dossier temporaire. Rapide : la compression se fait ensuite, sans la base.
pub fn stage(conn: &Connection, images: &ImageDirs, include_archive: bool, device_name: &str, app_version: &str) -> Result<PathBuf> {
    let staging = std::env::temp_dir().join(format!("flux-backup-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&staging).map_err(io)?;
    let result = (|| -> Result<()> {
        // VACUUM INTO : copie cohérente même pendant une écriture
        conn.execute("VACUUM INTO ?1", params![staging.join(DB_NAME).to_string_lossy().to_string()])?;

        if include_archive {
            let archive = staging.join("Archive");
            let mut stmt = conn.prepare("SELECT image_url FROM articles WHERE is_starred = 1 AND image_url IS NOT NULL AND image_url != ''")?;
            let urls: Vec<String> = stmt.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
            for url in urls {
                let name = cache_name(&url);
                if let Some(src) = images.read_order().into_iter().map(|d| d.join(&name)).find(|p| p.is_file()) {
                    fs::create_dir_all(&archive).map_err(io)?;
                    fs::copy(src, archive.join(&name)).map_err(io)?;
                }
            }
        }

        let info = serde_json::json!({
            "format": 1,
            "app": "tauri",
            "date": chrono::Local::now().to_rfc3339(),
            "device": device_name,
            "appVersion": app_version,
        });
        fs::write(staging.join("Infos.json"), serde_json::to_vec_pretty(&info)?).map_err(io)?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }
    Ok(staging)
}

/// Étape 2 : compression, dépôt dans iCloud Drive (`dest` = dossier de ce Mac), rotation.
/// Le dossier temporaire est supprimé dans tous les cas.
pub fn pack(staging: &Path, dest: &Path, stamp: &str, keep: u32) -> Result<PathBuf> {
    let result = (|| -> Result<PathBuf> {
        let name = format!("Flux {stamp}.zip");
        let zip = std::env::temp_dir().join(format!("{}.zip", uuid::Uuid::new_v4()));
        let packed = ditto(&[Path::new("-c"), Path::new("-k"), Path::new("--sequesterRsrc"), staging, &zip]);
        if let Err(e) = packed {
            let _ = fs::remove_file(&zip);
            return Err(e);
        }
        fs::create_dir_all(dest).map_err(io)?;
        let target = dest.join(&name);
        let _ = fs::remove_file(&target);
        // Entre deux volumes, `rename` échoue : on copie puis on supprime
        if fs::rename(&zip, &target).is_err() {
            fs::copy(&zip, &target).map_err(io)?;
            let _ = fs::remove_file(&zip);
        }
        rotate(dest, keep);
        Ok(target)
    })();
    let _ = fs::remove_dir_all(staging);
    result
}

/// Sauvegarde complète en une fois (utilisée par les tests).
pub fn make_backup(
    conn: &Connection,
    images: &ImageDirs,
    include_archive: bool,
    device_name: &str,
    app_version: &str,
    dest: &Path,
    stamp: &str,
    keep: u32,
) -> Result<PathBuf> {
    let staging = stage(conn, images, include_archive, device_name, app_version)?;
    pack(&staging, dest, stamp, keep)
}

/// Supprime les plus anciennes (les noms datés se trient chronologiquement).
fn rotate(dir: &Path, keep: u32) {
    let Ok(files) = fs::read_dir(dir) else { return };
    let mut backups: Vec<(String, PathBuf)> = files
        .flatten()
        .filter_map(|f| {
            let mut name = f.file_name().to_string_lossy().to_string();
            if name.starts_with('.') && name.ends_with(".icloud") {
                name = name.trim_start_matches('.').trim_end_matches(".icloud").to_string();
            }
            (name.starts_with("Flux ") && name.ends_with(".zip")).then(|| (name, f.path()))
        })
        .collect();
    backups.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in backups.into_iter().skip(keep.max(1) as usize) {
        let _ = fs::remove_file(path);
    }
}

pub fn record_backup(conn: &Connection, now: f64) -> Result<()> {
    db::save_setting(conn, LAST_BACKUP_KEY, &now)
}

// ---------------------------------------------------------------------------
// Restauration
// ---------------------------------------------------------------------------

/// Si l'archive est dans iCloud Drive mais pas encore sur ce Mac, on la demande et on attend.
fn ensure_local(zip: &Path) -> Result<()> {
    if zip.is_file() {
        return Ok(());
    }
    let _ = Command::new("brctl").arg("download").arg(zip).status();
    for _ in 0..120 {
        if zip.is_file() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    Err(Error::Other("L'archive n'a pas pu être téléchargée depuis iCloud Drive.".into()))
}

/// Décompresse la sauvegarde à côté de la base ; elle sera appliquée au prochain lancement.
pub fn prepare_restore(data_dir: &Path, zip: &Path) -> Result<()> {
    ensure_local(zip)?;
    let pending = data_dir.join(PENDING);
    let _ = fs::remove_dir_all(&pending);
    fs::create_dir_all(&pending).map_err(io)?;
    if let Err(e) = ditto(&[Path::new("-x"), Path::new("-k"), zip, &pending]) {
        let _ = fs::remove_dir_all(&pending);
        return Err(e);
    }
    if !pending.join(DB_NAME).is_file() {
        let _ = fs::remove_dir_all(&pending);
        return Err(Error::Other("Ce fichier n'est pas une sauvegarde de Flux.".into()));
    }
    Ok(())
}

/// Au lancement, avant d'ouvrir la base : applique une restauration préparée.
/// Renvoie vrai si une restauration a été appliquée.
///
/// La base actuelle est mise de côté (`before-restore`, au cas où). L'identifiant de ce Mac pour
/// la synchro est conservé, et la mémoire de synchro est effacée : tout le contenu restauré est
/// alors daté de maintenant, il l'emporte sur l'état des autres Mac (suppressions comprises) et
/// leur est envoyé ; ce qu'ils ont ajouté depuis la sauvegarde est récupéré en plus.
pub fn apply_pending_restore(data_dir: &Path) -> Result<bool> {
    let pending = data_dir.join(PENDING);
    let restored = pending.join(DB_NAME);
    if !restored.is_file() {
        return Ok(false);
    }
    let db_path = data_dir.join(DB_NAME);

    // Identifiant de ce Mac, lu avant de mettre la base de côté
    let own_id = Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .ok()
        .and_then(|c| db::load_setting::<String>(&c, "syncDeviceId").ok().flatten());

    let aside = data_dir.join(BEFORE);
    let _ = fs::remove_dir_all(&aside);
    fs::create_dir_all(&aside).map_err(io)?;
    let suffixes = ["", "-wal", "-shm"];
    for s in suffixes {
        let file = data_dir.join(format!("{DB_NAME}{s}"));
        if file.exists() {
            let _ = fs::rename(&file, aside.join(format!("{DB_NAME}{s}")));
        }
    }
    if fs::rename(&restored, &db_path).is_err() {
        // Échec : on remet la base d'origine
        for s in suffixes {
            let _ = fs::rename(aside.join(format!("{DB_NAME}{s}")), data_dir.join(format!("{DB_NAME}{s}")));
        }
        return Err(Error::Other("Impossible d'appliquer la sauvegarde.".into()));
    }

    {
        let conn = Connection::open(&db_path)?;
        match own_id {
            Some(id) => db::save_setting(&conn, "syncDeviceId", &id)?,
            None => {
                conn.execute("DELETE FROM settings WHERE key = 'syncDeviceId'", [])?;
            }
        }
        conn.execute("DELETE FROM settings WHERE key = 'syncState'", [])?;
    }

    // Images des favoris
    let archive = pending.join("Archive");
    if let Ok(files) = fs::read_dir(&archive) {
        let dest = data_dir.join("archive");
        let _ = fs::create_dir_all(&dest);
        for f in files.flatten() {
            let _ = fs::rename(f.path(), dest.join(f.file_name()));
        }
    }
    let _ = fs::remove_dir_all(&pending);
    Ok(true)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn backup_rotates_and_restore_keeps_this_macs_identity() {
        let data = tempfile::tempdir().unwrap();
        let icloud = tempfile::tempdir().unwrap();
        let db_path = data.path().join(DB_NAME);
        let conn = db::open(&db_path).unwrap();
        conn.execute_batch("INSERT INTO feeds (id, url, title, added_at) VALUES (1, 'u1', 'Korben', 0);").unwrap();
        db::save_setting(&conn, "syncDeviceId", &"MAC-A".to_string()).unwrap();
        let images = ImageDirs { own: data.path().join("images"), archive: data.path().join("archive"), others: vec![] };
        let dest = device_dir(icloud.path(), "Mac/A");
        assert!(dest.ends_with("Mac-A"));

        for stamp in ["2026-11-20 10.00", "2026-11-21 10.00", "2026-11-22 10.00"] {
            make_backup(&conn, &images, false, "Mac A", "0.7.0", &dest, stamp, 2).unwrap();
        }
        let list = list_backups(icloud.path());
        assert_eq!(list.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["Flux 2026-11-22 10.00.zip", "Flux 2026-11-21 10.00.zip"]);

        // On modifie la base, puis on restaure la plus récente
        conn.execute("DELETE FROM feeds", []).unwrap();
        db::save_setting(&conn, "syncState", &"état".to_string()).unwrap();
        drop(conn);
        prepare_restore(data.path(), Path::new(&list[0].path)).unwrap();
        assert!(apply_pending_restore(data.path()).unwrap());

        let conn = db::open(&db_path).unwrap();
        let feeds: i64 = conn.query_row("SELECT COUNT(*) FROM feeds", [], |r| r.get(0)).unwrap();
        assert_eq!(feeds, 1);
        assert_eq!(db::load_setting::<String>(&conn, "syncDeviceId").unwrap().as_deref(), Some("MAC-A"));
        assert!(db::load_setting::<String>(&conn, "syncState").unwrap().is_none());
        assert!(data.path().join(BEFORE).join(DB_NAME).is_file());
        assert!(!data.path().join(PENDING).exists());
    }

    #[test]
    fn rejects_an_archive_without_a_database() {
        let data = tempfile::tempdir().unwrap();
        let staging = tempfile::tempdir().unwrap();
        fs::write(staging.path().join("autre.txt"), "x").unwrap();
        let zip = data.path().join("faux.zip");
        ditto(&[Path::new("-c"), Path::new("-k"), staging.path(), &zip]).unwrap();
        assert!(prepare_restore(data.path(), &zip).is_err());
        assert!(!data.path().join(PENDING).exists());
    }
}
