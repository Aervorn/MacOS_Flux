//! Accès réseau : téléchargement de la page d'un article.
//! (Le téléchargement des flux et des images viendra aux étapes suivantes.)

use std::time::Duration;

use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE};
use serde::Serialize;

use crate::error::{Error, FeedError, Result};

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

const PAGE_ACCEPT: &str = "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8";
const FEED_ACCEPT: &str = "application/rss+xml, application/atom+xml, application/feed+json, application/xml, text/xml, application/json;q=0.9, text/html;q=0.9, */*;q=0.8";
const IMAGE_ACCEPT: &str = "image/avif,image/webp,image/png,image/jpeg,image/*;q=0.8,*/*;q=0.5";

/// Client partagé (connexions réutilisées).
fn client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(20))
            .build()
            .expect("client HTTP")
    })
}

/// Réponse brute : contenu et adresse finale (après redirections).
pub struct Response {
    pub data: Vec<u8>,
    pub final_url: String,
}

/// Télécharge une adresse http(s). Une réponse hors 2xx est une erreur.
pub async fn get(url: &str, accept: &str, max_bytes: usize) -> Result<Response> {
    if !is_http_url(url) {
        return Err(Error::Feed(FeedError::InvalidUrl));
    }
    let response = client()
        .get(url.trim())
        .header(ACCEPT, accept)
        .header(ACCEPT_LANGUAGE, "fr-FR,fr;q=0.9,en;q=0.8")
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        return Err(Error::Feed(FeedError::Http(status.as_u16())));
    }
    let final_url = response.url().to_string();
    if response.content_length().is_some_and(|n| n as usize > max_bytes) {
        return Err(Error::Other("réponse trop volumineuse".into()));
    }
    let bytes = response.bytes().await?;
    if bytes.len() > max_bytes {
        return Err(Error::Other("réponse trop volumineuse".into()));
    }
    Ok(Response { data: bytes.to_vec(), final_url })
}

pub async fn get_feed(url: &str) -> Result<Response> {
    get(url, FEED_ACCEPT, 30 << 20).await
}

pub async fn get_image(url: &str) -> Result<Response> {
    get(url, IMAGE_ACCEPT, 8 << 20).await
}

pub async fn fetch_page(url: &str) -> Result<FetchedPage> {
    let r = get(url, PAGE_ACCEPT, MAX_PAGE_BYTES).await?;
    Ok(FetchedPage { html: decode_html(&r.data), final_url: r.final_url })
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
