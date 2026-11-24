//! Ajout et actualisation des flux : téléchargements puis écriture en base.
//!
//! La base est derrière un `Mutex` : on ne le garde jamais pendant un téléchargement.

use std::fs;
use std::sync::{Mutex, MutexGuard};

use futures::stream::{self, StreamExt};
use rusqlite::Connection;
use serde::Serialize;
use url::Url;

use crate::error::{Error, FeedError, Result};
use crate::feed::{self, clean_title, ParsedFeed};
use crate::feeds::{self, IngestOptions, NewArticle};
use crate::html;
use crate::images::{cache_name, sniff_mime, ImageDirs};
use crate::net;
use crate::prefs::Prefs;

/// Nombre de téléchargements de flux en parallèle.
const FEED_CONCURRENCY: usize = 8;
/// Nombre de téléchargements d'images ou de pages en parallèle.
const IMAGE_CONCURRENCY: usize = 6;
/// Nombre maximal de pages d'articles consultées pour trouver une miniature (og:image).
const MAX_OG_LOOKUPS: usize = 40;

fn lock(db: &Mutex<Connection>) -> Result<MutexGuard<'_, Connection>> {
    db.lock().map_err(|_| Error::Other("base de données verrouillée".into()))
}

pub async fn fetch_feed(url: &str) -> Result<ParsedFeed> {
    let response = net::get_feed(url).await?;
    feed::parse(&response.data).ok_or(Error::Feed(FeedError::NotAFeed))
}

/// Accepte l'adresse d'un flux, ou celle d'un site (détection automatique).
/// Renvoie l'adresse finale du flux et son contenu.
pub async fn discover(url: &Url) -> Result<(String, ParsedFeed)> {
    let response = net::get_feed(url.as_str()).await?;
    if let Some(parsed) = feed::parse(&response.data) {
        return Ok((response.final_url, parsed));
    }

    let page = net::decode_html(&response.data);
    let base = Url::parse(&response.final_url).unwrap_or_else(|_| url.clone());
    for candidate in html::feed_links(&page, &base) {
        if let Ok(r) = net::get_feed(candidate.as_str()).await {
            if let Some(parsed) = feed::parse(&r.data) {
                return Ok((r.final_url, parsed));
            }
        }
    }
    for path in ["/feed", "/rss", "/feed.xml", "/rss.xml", "/atom.xml", "/index.xml", "/feed.json"] {
        let Ok(candidate) = base.join(path) else { continue };
        if let Ok(r) = net::get_feed(candidate.as_str()).await {
            if let Some(parsed) = feed::parse(&r.data) {
                return Ok((r.final_url, parsed));
            }
        }
    }
    Err(Error::Feed(FeedError::NotAFeed))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedFeed {
    pub url: String,
    pub title: String,
    pub articles: usize,
}

/// Ajoute un flux à partir de l'adresse saisie (flux ou site) et importe ses articles.
pub async fn add_feed(
    db: &Mutex<Connection>,
    input: &str,
    folder_uid: Option<&str>,
    prefs: &Prefs,
    images: &ImageDirs,
) -> Result<AddedFeed> {
    let mut address = input.trim().to_string();
    if !address.contains("://") {
        address = format!("https://{address}");
    }
    let url = Url::parse(&address).map_err(|_| Error::Feed(FeedError::InvalidUrl))?;
    if url.host_str().is_none() {
        return Err(Error::Feed(FeedError::InvalidUrl));
    }

    let (feed_url, parsed) = discover(&url).await?;
    let opts = IngestOptions::from_prefs(prefs, feeds::now());
    let title = parsed
        .title
        .as_deref()
        .map(clean_title)
        .filter(|t| !t.is_empty())
        .or_else(|| url.host_str().map(str::to_string))
        .unwrap_or_else(|| feed_url.clone());

    let mut added = {
        let conn = lock(db)?;
        feeds::add_feed_row(&conn, &feed_url, &title, parsed.site_url.as_deref(), folder_uid)?;
        let added = feeds::ingest(&conn, &feed_url, &parsed, &opts)?;
        feeds::mark_refreshed(&conn, &feed_url, None)?;
        added
    };

    // Flux court (WordPress n'en publie souvent que 10, parfois 5) : on remonte les pages précédentes.
    if parsed.items.len() < 20 {
        added.extend(fetch_older(db, &feed_url, &opts).await);
    }
    fill_images(db, &added, images).await;
    Ok(AddedFeed { url: feed_url, title, articles: added.len() })
}

/// Articles plus anciens d'un flux paginé (`?paged=2`, `?paged=3`… : WordPress et la plupart des CMS).
/// Un flux non paginé renvoie la même page : rien n'est ajouté et on s'arrête.
pub async fn fetch_older(db: &Mutex<Connection>, feed_url: &str, opts: &IngestOptions) -> Vec<NewArticle> {
    let mut added = Vec::new();
    let Ok(base) = Url::parse(feed_url) else { return added };
    let kept: Vec<(String, String)> =
        base.query_pairs().filter(|(k, _)| k != "paged").map(|(k, v)| (k.into_owned(), v.into_owned())).collect();

    for page in 2..=8 {
        let mut url = base.clone();
        url.set_query(None);
        {
            let mut pairs = url.query_pairs_mut();
            pairs.extend_pairs(kept.iter());
            pairs.append_pair("paged", &page.to_string());
        }
        let Ok(parsed) = fetch_feed(url.as_str()).await else { break };
        if parsed.items.is_empty() {
            break;
        }
        // Pages d'archive : leur titre (« Page 8 - … ») ne doit pas remplacer celui du flux.
        let page_opts = IngestOptions { update_info: false, ..*opts };
        let new = match lock(db).and_then(|conn| feeds::ingest(&conn, feed_url, &parsed, &page_opts)) {
            Ok(new) => new,
            Err(_) => break,
        };
        let nothing_new = new.is_empty();
        added.extend(new);
        let oldest = parsed.items.iter().filter_map(|i| i.published).fold(f64::INFINITY, f64::min);
        if nothing_new || oldest < opts.cutoff {
            break;
        }
    }
    added
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshReport {
    pub feeds: usize,
    pub added: usize,
    pub failed: usize,
    /// Tous les téléchargements ont échoué faute de connexion.
    pub offline: bool,
}

/// Actualise tous les flux.
pub async fn refresh_all(db: &Mutex<Connection>, prefs: &Prefs, images: &ImageDirs) -> Result<RefreshReport> {
    let urls = {
        let conn = lock(db)?;
        feeds::feed_urls(&conn)?
    };

    let results: Vec<(String, Result<ParsedFeed>)> = stream::iter(urls.into_iter().map(|url| async move {
        let result = fetch_feed(&url).await;
        (url, result)
    }))
    .buffer_unordered(FEED_CONCURRENCY)
    .collect()
    .await;

    // Hors ligne si toutes les requêtes ont échoué faute de connexion.
    let offline = !results.is_empty() && results.iter().all(|(_, r)| matches!(r, Err(e) if e.is_offline()));
    let opts = IngestOptions::from_prefs(prefs, feeds::now());
    let mut report = RefreshReport { feeds: results.len(), offline, ..Default::default() };
    let mut added: Vec<NewArticle> = Vec::new();

    {
        let conn = lock(db)?;
        for (url, result) in &results {
            match result {
                Ok(parsed) => {
                    feeds::mark_refreshed(&conn, url, None)?;
                    if let Ok(new) = feeds::ingest(&conn, url, parsed, &opts) {
                        added.extend(new);
                    }
                }
                // Pas de connexion : ce n'est pas la faute du flux, on ne le signale pas en erreur.
                Err(e) if e.is_offline() => {}
                Err(e) => {
                    report.failed += 1;
                    feeds::mark_refreshed(&conn, url, Some(&e.to_string()))?;
                }
            }
        }
        if let Some(cutoff) = opts.auto_read_cutoff {
            feeds::apply_auto_mark_read(&conn, cutoff)?;
        }
        feeds::prune(&conn, opts.cutoff)?;
    }
    report.added = added.len();

    if !offline {
        fill_images(db, &added, images).await;
    }
    Ok(report)
}

/// Télécharge une image dans notre cache (sauf si elle y est déjà, ou dans celui de l'app Swift).
async fn fetch_to_cache(images: &ImageDirs, url: &str) -> bool {
    let name = cache_name(url);
    if images.has(&name) {
        return true;
    }
    let Ok(response) = net::get_image(url).await else { return false };
    if sniff_mime(&response.data).is_none() {
        return false;
    }
    fs::create_dir_all(&images.own).is_ok() && fs::write(images.own.join(&name), &response.data).is_ok()
}

/// og:image de la page d'un article.
async fn og_image_for(link: &str) -> Option<String> {
    let page = net::fetch_page(link).await.ok()?;
    let base = Url::parse(&page.final_url).ok();
    html::og_image(&page.html).and_then(|image| html::resolve(&image, base.as_ref()))
}

/// Miniatures des nouveaux articles : cherche l'og:image de ceux qui n'en ont pas, puis télécharge.
async fn fill_images(db: &Mutex<Connection>, articles: &[NewArticle], images: &ImageDirs) {
    let missing: Vec<(String, String)> = articles
        .iter()
        .filter(|a| a.image_url.is_none())
        .filter_map(|a| a.link.clone().map(|l| (a.key.clone(), l)))
        .take(MAX_OG_LOOKUPS)
        .collect();

    let found: Vec<(String, Option<String>)> = stream::iter(missing.into_iter().map(|(key, link)| async move {
        let image = og_image_for(&link).await;
        (key, image)
    }))
    .buffer_unordered(IMAGE_CONCURRENCY)
    .collect()
    .await;

    let mut urls: Vec<String> = articles.iter().filter_map(|a| a.image_url.clone()).collect();
    if let Ok(conn) = lock(db) {
        for (key, image) in &found {
            if let Some(image) = image {
                if feeds::set_image_url(&conn, key, image).is_ok() {
                    urls.push(image.clone());
                }
            }
        }
    }

    stream::iter(urls.into_iter().map(|url| async move { fetch_to_cache(images, &url).await }))
        .buffer_unordered(IMAGE_CONCURRENCY)
        .collect::<Vec<bool>>()
        .await;
}
