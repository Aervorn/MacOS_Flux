//! Flux et articles en base : ajout d'un flux, insertion des nouveaux articles, rétention.

use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension};
use url::Url;

use crate::error::{Error, FeedError, Result};
use crate::feed::{clean_title, ParsedFeed};
use crate::html::{first_image, plain_text, resolve};
use crate::prefs::Prefs;

/// Réglages utiles à l'insertion (calculés une fois par actualisation).
#[derive(Debug, Clone, Copy, Default)]
pub struct IngestOptions {
    /// Les articles publiés avant cette date (secondes Unix) ne sont pas importés (0 = tout garder).
    pub cutoff: f64,
    /// Les articles plus vieux que cette date arrivent déjà lus.
    pub auto_read_cutoff: Option<f64>,
    /// Mettre à jour le nom et le site du flux (faux pour les pages d'archive).
    pub update_info: bool,
}

impl IngestOptions {
    pub fn from_prefs(prefs: &Prefs, now: f64) -> Self {
        let cutoff = if prefs.retention_days > 0 { now - f64::from(prefs.retention_days.max(7)) * 86_400.0 } else { 0.0 };
        let auto_read_cutoff = prefs
            .auto_mark_read_enabled
            .then(|| now - f64::from(prefs.auto_mark_read_days.max(1)) * 86_400.0);
        IngestOptions { cutoff, auto_read_cutoff, update_info: true }
    }
}

/// Article nouvellement inséré (pour récupérer ses images ensuite).
#[derive(Debug, Clone, PartialEq)]
pub struct NewArticle {
    pub key: String,
    pub link: Option<String>,
    pub image_url: Option<String>,
}

pub fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn feed_urls(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT url FROM feeds")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Crée le flux. Les articles gardés (favoris) d'une précédente suppression de ce flux le retrouvent.
pub fn add_feed_row(conn: &Connection, url: &str, title: &str, site_url: Option<&str>, folder_uid: Option<&str>) -> Result<i64> {
    let exists: bool = conn.query_row("SELECT EXISTS (SELECT 1 FROM feeds WHERE url = ?1)", params![url], |r| r.get(0))?;
    if exists {
        return Err(Error::Feed(FeedError::AlreadyExists));
    }
    let folder_id: Option<i64> = match folder_uid {
        Some(uid) => conn.query_row("SELECT id FROM folders WHERE uid = ?1", params![uid], |r| r.get(0)).optional()?,
        None => None,
    };
    conn.execute(
        "INSERT INTO feeds (url, title, site_url, added_at, folder_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![url, title, site_url, now(), folder_id],
    )?;
    let id = conn.last_insert_rowid();
    conn.execute("UPDATE articles SET feed_id = ?1 WHERE feed_url = ?2 AND feed_id IS NULL", params![id, url])?;
    Ok(id)
}

/// Note le résultat d'une actualisation : date de succès, ou message d'erreur.
pub fn mark_refreshed(conn: &Connection, url: &str, error: Option<&str>) -> Result<()> {
    match error {
        None => conn.execute("UPDATE feeds SET last_refreshed = ?2, last_error = NULL WHERE url = ?1", params![url, now()])?,
        Some(message) => conn.execute("UPDATE feeds SET last_error = ?2 WHERE url = ?1", params![url, message])?,
    };
    Ok(())
}

/// Insère les nouveaux articles d'un flux, sans toucher à ceux déjà connus.
pub fn ingest(conn: &Connection, feed_url: &str, parsed: &ParsedFeed, opts: &IngestOptions) -> Result<Vec<NewArticle>> {
    let (feed_id, feed_title, site_url): (i64, String, Option<String>) = conn.query_row(
        "SELECT id, title, site_url FROM feeds WHERE url = ?1",
        params![feed_url],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;

    if opts.update_info {
        if site_url.is_none() {
            if let Some(site) = &parsed.site_url {
                conn.execute("UPDATE feeds SET site_url = ?2 WHERE id = ?1", params![feed_id, site])?;
            }
        }
        // Le nom d'origine suit le flux ; un nom personnalisé (custom_title) reste prioritaire à l'affichage.
        if let Some(fresh) = parsed.title.as_deref().map(clean_title).filter(|t| !t.is_empty()) {
            if fresh != feed_title {
                conn.execute("UPDATE feeds SET title = ?2 WHERE id = ?1", params![feed_id, fresh])?;
            }
        }
    }

    let mut known: HashSet<String> = {
        let mut stmt = conn.prepare("SELECT key FROM articles WHERE feed_url = ?1")?;
        let rows = stmt.query_map(params![feed_url], |r| r.get::<_, String>(0))?;
        rows.collect::<std::result::Result<_, _>>()?
    };
    let base = site_url
        .as_deref()
        .or(parsed.site_url.as_deref())
        .and_then(|s| Url::parse(s).ok())
        .or_else(|| Url::parse(feed_url).ok());
    let now = now();

    let mut added = Vec::new();
    let tx = conn.unchecked_transaction()?;
    for item in parsed.items.iter().take(200) {
        if item.published.is_some_and(|d| d < opts.cutoff) {
            continue;
        }
        let link = item.link.as_deref().and_then(|l| resolve(l, base.as_ref()));
        let Some(ident) = item.guid.clone().or_else(|| link.clone()).or_else(|| item.title.clone()) else { continue };
        let key = format!("{feed_url}|{ident}");
        if !known.insert(key.clone()) {
            continue;
        }

        let html = item.content_html.as_deref().or(item.summary_html.as_deref());
        let mut title = plain_text(item.title.as_deref().unwrap_or(""));
        if title.is_empty() {
            title = "(Sans titre)".to_string();
        }
        let summary: String = plain_text(item.summary_html.as_deref().or(item.content_html.as_deref()).unwrap_or(""))
            .chars()
            .take(280)
            .collect();
        let image_base = link.as_deref().and_then(|l| Url::parse(l).ok()).or_else(|| base.clone());
        let image = item
            .image_url
            .clone()
            .or_else(|| html.and_then(first_image))
            .and_then(|i| resolve(&i, image_base.as_ref()));
        let body = plain_text(html.unwrap_or(""));
        let word_count = body.split_whitespace().count() as i64;
        let search_text: String = format!("{title}\n{summary}\n{body}").chars().take(30_000).collect();
        let published = item.published.unwrap_or(now);
        let is_read = opts.auto_read_cutoff.is_some_and(|c| published < c);
        let author = item.author.as_deref().map(plain_text);

        tx.execute(
            "INSERT OR IGNORE INTO articles
                (key, feed_id, feed_url, title, link, summary, content_html, search_text, word_count,
                 image_url, author, published, is_read)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![key, feed_id, feed_url, title, link, summary, html, search_text, word_count, image, author, published, is_read],
        )?;
        added.push(NewArticle { key, link, image_url: image });
    }
    tx.commit()?;
    Ok(added)
}

/// Supprime les articles lus, non favoris, plus vieux que la durée de conservation.
pub fn prune(conn: &Connection, cutoff: f64) -> Result<usize> {
    if cutoff <= 0.0 {
        return Ok(0);
    }
    Ok(conn.execute("DELETE FROM articles WHERE is_read = 1 AND is_starred = 0 AND published < ?1", params![cutoff])?)
}

/// Réglage « marquer comme lus les articles de plus de X jours » (les favoris ne sont pas concernés).
pub fn apply_auto_mark_read(conn: &Connection, cutoff: f64) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE articles SET is_read = 1 WHERE is_read = 0 AND is_starred = 0 AND published < ?1",
        params![cutoff],
    )?)
}

/// Renseigne l'image d'un article trouvée après coup (og:image de sa page).
pub fn set_image_url(conn: &Connection, key: &str, url: &str) -> Result<()> {
    conn.execute("UPDATE articles SET image_url = ?2 WHERE key = ?1 AND image_url IS NULL", params![key, url])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::feed::{parse, ParsedItem};

    const FEED: &str = r#"<rss><channel><title> Mon &amp; blog </title><link>https://b.example/</link>
        <item><title>Un</title><link>/un</link><guid>g1</guid><pubDate>Wed, 02 Oct 2024 13:00:00 GMT</pubDate>
          <description>Court</description><content:encoded><![CDATA[<p>Corps <img src="/i.jpg"></p>]]></content:encoded></item>
        <item><title>Vieux</title><guid>g2</guid><pubDate>Mon, 01 Jan 2024 10:00:00 GMT</pubDate></item>
        <item><link>https://b.example/sans-guid</link></item></channel></rss>"#;

    fn setup() -> (Connection, ParsedFeed) {
        let c = db::open_in_memory().unwrap();
        add_feed_row(&c, "https://b.example/feed", "b.example", None, None).unwrap();
        (c, parse(FEED.as_bytes()).unwrap())
    }

    const OCT_2024: f64 = 1_727_874_000.0;

    #[test]
    fn inserts_new_articles_once() {
        let (c, parsed) = setup();
        let opts = IngestOptions { cutoff: 0.0, auto_read_cutoff: None, update_info: true };
        let added = ingest(&c, "https://b.example/feed", &parsed, &opts).unwrap();
        assert_eq!(added.len(), 3);
        assert_eq!(added[0].key, "https://b.example/feed|g1");
        assert_eq!(added[0].link.as_deref(), Some("https://b.example/un"));
        assert_eq!(added[0].image_url.as_deref(), Some("https://b.example/i.jpg"));
        // même flux une 2e fois : rien de nouveau
        assert!(ingest(&c, "https://b.example/feed", &parsed, &opts).unwrap().is_empty());

        let (title, site): (String, Option<String>) = c
            .query_row("SELECT title, site_url FROM feeds", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(title, "Mon & blog");
        assert_eq!(site.as_deref(), Some("https://b.example/"));

        let (summary, words, search): (String, i64, String) = c
            .query_row("SELECT summary, word_count, search_text FROM articles WHERE key LIKE '%g1'", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(summary, "Court");
        assert_eq!(words, 1);
        assert_eq!(search, "Un\nCourt\nCorps");
    }

    #[test]
    fn applies_retention_auto_read_and_untitled() {
        let (c, parsed) = setup();
        let opts = IngestOptions { cutoff: OCT_2024 - 86_400.0, auto_read_cutoff: Some(OCT_2024 + 1.0), update_info: false };
        let added = ingest(&c, "https://b.example/feed", &parsed, &opts).unwrap();
        // « Vieux » est avant la date limite : ignoré. L'article sans date est daté de maintenant.
        assert_eq!(added.len(), 2);
        let read: i64 = c.query_row("SELECT is_read FROM articles WHERE key LIKE '%g1'", [], |r| r.get(0)).unwrap();
        assert_eq!(read, 1); // plus vieux que la limite de lecture automatique
        let title: String =
            c.query_row("SELECT title FROM articles WHERE key LIKE '%sans-guid'", [], |r| r.get(0)).unwrap();
        assert_eq!(title, "(Sans titre)");
        // update_info faux : le titre du flux reste celui d'origine
        let feed_title: String = c.query_row("SELECT title FROM feeds", [], |r| r.get(0)).unwrap();
        assert_eq!(feed_title, "b.example");
    }

    #[test]
    fn item_needs_some_identifier() {
        let (c, _) = setup();
        let parsed = ParsedFeed { items: vec![ParsedItem::default()], ..Default::default() };
        let opts = IngestOptions { update_info: false, ..Default::default() };
        assert!(ingest(&c, "https://b.example/feed", &parsed, &opts).unwrap().is_empty());
    }

    #[test]
    fn add_feed_rejects_duplicates_and_reattaches_orphans() {
        let c = db::open_in_memory().unwrap();
        c.execute("INSERT INTO folders (uid, name, created_at) VALUES ('F1', 'Dossier', 0)", []).unwrap();
        c.execute(
            "INSERT INTO articles (key, feed_id, feed_url, title, published, is_starred) VALUES ('u|1', NULL, 'u', 'Gardé', 1, 1)",
            [],
        )
        .unwrap();
        let id = add_feed_row(&c, "u", "Titre", None, Some("F1")).unwrap();
        let attached: i64 = c.query_row("SELECT feed_id FROM articles", [], |r| r.get(0)).unwrap();
        assert_eq!(attached, id);
        let folder: Option<i64> = c.query_row("SELECT folder_id FROM feeds", [], |r| r.get(0)).unwrap();
        assert!(folder.is_some());
        let err = add_feed_row(&c, "u", "Titre", None, None).unwrap_err();
        assert_eq!(err.to_string(), "feed:already_exists");
        assert_eq!(feed_urls(&c).unwrap(), ["u"]);
    }

    #[test]
    fn prune_keeps_unread_and_favorites() {
        let (c, parsed) = setup();
        let opts = IngestOptions { update_info: false, ..Default::default() };
        ingest(&c, "https://b.example/feed", &parsed, &opts).unwrap();
        c.execute("UPDATE articles SET is_read = 1", []).unwrap();
        c.execute("UPDATE articles SET is_starred = 1 WHERE key LIKE '%g1'", []).unwrap();
        // limite après tous les articles : seuls les lus non favoris et assez vieux partent
        let removed = prune(&c, OCT_2024 + 10.0).unwrap();
        assert_eq!(removed, 1); // « Vieux »
        assert_eq!(prune(&c, 0.0).unwrap(), 0);
        let left: i64 = c.query_row("SELECT COUNT(*) FROM articles", [], |r| r.get(0)).unwrap();
        assert_eq!(left, 2);
    }

    #[test]
    fn marks_refresh_result_and_options_from_prefs() {
        let (c, _) = setup();
        mark_refreshed(&c, "https://b.example/feed", Some("feed:http:404")).unwrap();
        let e: Option<String> = c.query_row("SELECT last_error FROM feeds", [], |r| r.get(0)).unwrap();
        assert_eq!(e.as_deref(), Some("feed:http:404"));
        mark_refreshed(&c, "https://b.example/feed", None).unwrap();
        let (e, ok): (Option<String>, Option<f64>) =
            c.query_row("SELECT last_error, last_refreshed FROM feeds", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert!(e.is_none() && ok.is_some());

        let mut prefs = Prefs::default();
        prefs.retention_days = 3; // minimum de 7 jours
        prefs.auto_mark_read_enabled = true;
        prefs.auto_mark_read_days = 0; // minimum de 1 jour
        let o = IngestOptions::from_prefs(&prefs, 1_000_000.0);
        assert_eq!(o.cutoff, 1_000_000.0 - 7.0 * 86_400.0);
        assert_eq!(o.auto_read_cutoff, Some(1_000_000.0 - 86_400.0));
        prefs.retention_days = 0;
        assert_eq!(IngestOptions::from_prefs(&prefs, 1_000_000.0).cutoff, 0.0);
    }
}
