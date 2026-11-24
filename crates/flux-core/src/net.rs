//! Accès réseau : téléchargement de la page d'un article.
//! (Le téléchargement des flux et des images viendra aux étapes suivantes.)

use std::time::Duration;

use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE};
use serde::Serialize;

use crate::error::{Error, Result};

/// User-Agent de Safari : certains sites (Cloudflare…) bloquent les clients inconnus.
const USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";

/// Taille maximale d'une page téléchargée.
const MAX_PAGE_BYTES: usize = 10 << 20;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchedPage {
    pub html: String,
    /// Adresse finale, après redirections (sert de base aux liens relatifs).
    pub final_url: String,
}

/// Seules les adresses http(s) sont téléchargées.
pub fn is_http_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// UTF-8, sinon ISO Latin 1 (comme l'app Swift).
pub fn decode_html(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

pub async fn fetch_page(url: &str) -> Result<FetchedPage> {
    if !is_http_url(url) {
        return Err(Error::Other("adresse non prise en charge".into()));
    }
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(20))
        .build()?;
    let response = client
        .get(url.trim())
        .header(ACCEPT, "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8")
        .header(ACCEPT_LANGUAGE, "fr-FR,fr;q=0.9,en;q=0.8")
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        return Err(Error::Other(format!("le serveur a répondu avec l'erreur {}", status.as_u16())));
    }
    let final_url = response.url().to_string();
    if response.content_length().is_some_and(|n| n as usize > MAX_PAGE_BYTES) {
        return Err(Error::Other("page trop volumineuse".into()));
    }
    let bytes = response.bytes().await?;
    if bytes.len() > MAX_PAGE_BYTES {
        return Err(Error::Other("page trop volumineuse".into()));
    }
    Ok(FetchedPage { html: decode_html(&bytes), final_url })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_http_urls_are_allowed() {
        assert!(is_http_url("https://example.com/a"));
        assert!(is_http_url(" HTTP://example.com"));
        assert!(!is_http_url("file:///etc/passwd"));
        assert!(!is_http_url("javascript:alert(1)"));
        assert!(!is_http_url("/relatif"));
    }

    #[test]
    fn decodes_utf8_then_latin1() {
        assert_eq!(decode_html("é à".as_bytes()), "é à");
        assert_eq!(decode_html(&[0x63, 0x61, 0x66, 0xE9]), "café"); // ISO Latin 1
    }
}
