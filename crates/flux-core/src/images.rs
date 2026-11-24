//! Cache d'images : mêmes noms de fichiers que l'app Swift.
//!
//! Une image = un fichier nommé d'après le SHA-256 (en hexadécimal) de son adresse,
//! sans extension. Les favoris archivés sont dans `Archive/`, le reste dans `Images/`.

use std::fs;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

/// Nom du fichier de cache d'une image (identique à `ImageCache.fileName(for:)` côté Swift).
pub fn cache_name(url: &str) -> String {
    let hash = Sha256::digest(url.as_bytes());
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

/// Un nom de cache valide : 64 caractères hexadécimaux minuscules (évite toute sortie du dossier).
pub fn is_cache_name(name: &str) -> bool {
    name.len() == 64 && name.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Type de l'image d'après ses premiers octets (comme `ImageCache.mimeType(for:)`).
pub fn sniff_mime(data: &[u8]) -> Option<&'static str> {
    let b = data;
    if b.len() < 4 {
        return None;
    }
    if b[0] == 0xFF && b[1] == 0xD8 && b[2] == 0xFF {
        return Some("image/jpeg");
    }
    if b[..4] == [0x89, 0x50, 0x4E, 0x47] {
        return Some("image/png");
    }
    if b[..3] == *b"GIF" {
        return Some("image/gif");
    }
    if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if b.len() >= 12 && &b[4..8] == b"ftyp" {
        let brand = &b[8..12];
        if brand.starts_with(b"avi") {
            return Some("image/avif");
        }
        if brand.starts_with(b"hei") || brand.starts_with(b"mif") {
            return Some("image/heic");
        }
    }
    let head = String::from_utf8_lossy(&b[..b.len().min(512)]).to_lowercase();
    if head.contains("<svg") {
        return Some("image/svg+xml");
    }
    None
}

/// Dossiers d'images : le nôtre (lecture et écriture), puis ceux de l'app Swift (lecture seule).
#[derive(Debug, Clone, Default)]
pub struct ImageDirs {
    pub own: PathBuf,
    pub others: Vec<PathBuf>,
}

impl ImageDirs {
    /// Ordre de lecture : le nôtre d'abord.
    pub fn read_order(&self) -> Vec<PathBuf> {
        std::iter::once(self.own.clone()).chain(self.others.iter().cloned()).collect()
    }

    pub fn has(&self, name: &str) -> bool {
        self.read_order().iter().any(|d| d.join(name).is_file())
    }
}

/// Lit une image dans les dossiers donnés (dans l'ordre : archive d'abord), en lecture seule.
pub fn read_cached(dirs: &[PathBuf], name: &str) -> Option<(Vec<u8>, &'static str)> {
    if !is_cache_name(name) {
        return None;
    }
    for dir in dirs {
        if let Ok(data) = fs::read(dir.join(name)) {
            if let Some(mime) = sniff_mime(&data) {
                return Some((data, mime));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_name_is_sha256_hex() {
        // SHA-256 de "abc"
        assert_eq!(
            cache_name("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn rejects_unsafe_names() {
        assert!(is_cache_name(&cache_name("x")));
        assert!(!is_cache_name("../etc/passwd"));
        assert!(!is_cache_name(&"A".repeat(64)));
        assert!(!is_cache_name("abc"));
    }

    #[test]
    fn sniffs_common_formats() {
        assert_eq!(sniff_mime(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(sniff_mime(&[0x89, 0x50, 0x4E, 0x47, 0, 0]), Some("image/png"));
        assert_eq!(sniff_mime(b"GIF89a"), Some("image/gif"));
        assert_eq!(sniff_mime(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff_mime(b"<svg xmlns='x'></svg>"), Some("image/svg+xml"));
        assert_eq!(sniff_mime(b"pas une image"), None);
    }

    #[test]
    fn reads_from_first_directory_that_has_it() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let name = cache_name("https://x.example/i.png");
        fs::write(b.path().join(&name), [0x89, 0x50, 0x4E, 0x47, 1, 2, 3]).unwrap();
        let dirs = vec![a.path().to_path_buf(), b.path().to_path_buf()];
        let (data, mime) = read_cached(&dirs, &name).unwrap();
        assert_eq!(mime, "image/png");
        assert_eq!(data.len(), 7);
        assert!(read_cached(&dirs, &cache_name("autre")).is_none());
    }
}
