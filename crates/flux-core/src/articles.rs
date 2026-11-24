//! Un article en détail (lecteur) et les écritures liées à son contenu.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::error::Result;
use crate::query::source_name;

/// Autre source de la même actualité.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimilarArticle {
    pub key: String,
    pub source: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleDetail {
    pub key: String,
    pub title: String,
    pub summary: String,
    pub source: String,
    pub feed_url: String,
    pub link: Option<String>,
    pub author: Option<String>,
    /// Secondes Unix
    pub published: f64,
    pub image_url: Option<String>,
    /// HTML fourni par le flux.
    pub content_html: Option<String>,
    /// HTML extrait de la page du site (Readability), s'il a déjà été récupéré.
    pub full_html: Option<String>,
    pub is_read: bool,
    pub is_starred: bool,
    /// Identifiants (uid) des tags de l'article.
    pub tags: Vec<String>,
    pub similar: Vec<SimilarArticle>,
}

pub fn get_article(conn: &Connection, key: &str) -> Result<Option<ArticleDetail>> {
    let row = conn
        .query_row(
            "SELECT a.title, a.summary, a.feed_url, a.link, a.author, a.published, a.image_url,
                    a.content_html, a.full_html, a.is_read, a.is_starred, a.cluster_id,
                    a.source_title, COALESCE(f.custom_title, f.title)
             FROM articles a LEFT JOIN feeds f ON f.id = a.feed_id
             WHERE a.key = ?1",
            params![key],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, f64>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, i64>(9)?,
                    r.get::<_, i64>(10)?,
                    r.get::<_, Option<String>>(11)?,
                    r.get::<_, Option<String>>(12)?,
                    r.get::<_, Option<String>>(13)?,
                ))
            },
        )
        .optional()?;

    let Some((title, summary, feed_url, link, author, published, image_url, content_html, full_html, read, starred, cluster, source_title, feed_title)) =
        row
    else {
        return Ok(None);
    };

    let article_id: i64 = conn.query_row("SELECT id FROM articles WHERE key = ?1", params![key], |r| r.get(0))?;
    let tags = {
        let mut stmt = conn.prepare(
            "SELECT t.uid FROM article_tags x JOIN favorite_tags t ON t.id = x.tag_id
             WHERE x.article_id = ?1 ORDER BY t.name",
        )?;
        let rows = stmt.query_map(params![article_id], |r| r.get::<_, String>(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };

    let mut similar = Vec::new();
    if let Some(cluster) = cluster {
        let mut stmt = conn.prepare(
            "SELECT a.key, a.title, a.feed_url, a.source_title, COALESCE(f.custom_title, f.title)
             FROM articles a LEFT JOIN feeds f ON f.id = a.feed_id
             WHERE a.cluster_id = ?1 AND a.key != ?2 AND a.is_hidden = 0
             ORDER BY a.published ASC, a.id ASC",
        )?;
        let rows = stmt.query_map(params![cluster, key], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })?;
        for row in rows {
            let (key, title, feed_url, source_title, feed_title) = row?;
            similar.push(SimilarArticle { key, title, source: source_name(feed_title, source_title, &feed_url) });
        }
    }

    Ok(Some(ArticleDetail {
        key: key.to_string(),
        source: source_name(feed_title, source_title, &feed_url),
        title,
        summary,
        feed_url,
        link,
        author,
        published,
        image_url,
        content_html,
        full_html,
        is_read: read != 0,
        is_starred: starred != 0,
        tags,
        similar,
    }))
}

/// Enregistre l'article complet récupéré sur le site, avec le texte de recherche et le nombre
/// de mots recalculés. L'image de l'article n'est renseignée que s'il n'en avait pas.
/// Renvoie false si l'article n'existe pas.
pub fn save_full_html(
    conn: &Connection,
    key: &str,
    html: &str,
    search_text: &str,
    word_count: i64,
    image_url: Option<&str>,
) -> Result<bool> {
    let changed = conn.execute(
        "UPDATE articles
         SET full_html = ?2, search_text = ?3, word_count = ?4, image_url = COALESCE(image_url, ?5)
         WHERE key = ?1",
        params![key, html, search_text, word_count, image_url],
    )?;
    Ok(changed > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn fixture() -> Connection {
        let c = db::open_in_memory().unwrap();
        c.execute_batch(
            "INSERT INTO feeds (id, url, title, added_at) VALUES (1, 'https://a.example/feed', 'Source A', 0), (2, 'https://b.example/feed', 'Source B', 0);
             INSERT INTO articles (id, key, feed_id, feed_url, title, summary, published, content_html, cluster_id, link) VALUES
                 (1, 'k1', 1, 'https://a.example/feed', 'Titre 1', 'Résumé', 100, '<p>Corps</p>', 'C1', 'https://a.example/1'),
                 (2, 'k2', 2, 'https://b.example/feed', 'Titre 2', 'Résumé', 200, NULL, 'C1', NULL),
                 (3, 'k3', 2, 'https://b.example/feed', 'Masqué',  'Résumé', 300, NULL, 'C1', NULL),
                 (4, 'k4', 1, 'https://a.example/feed', 'Seul',    'Résumé', 400, NULL, NULL, NULL);
             UPDATE articles SET is_hidden = 1 WHERE key = 'k3';",
        )
        .unwrap();
        c
    }

    #[test]
    fn returns_detail_and_other_sources_of_the_same_story() {
        let c = fixture();
        let d = get_article(&c, "k1").unwrap().unwrap();
        assert_eq!(d.title, "Titre 1");
        assert_eq!(d.source, "Source A");
        assert_eq!(d.content_html.as_deref(), Some("<p>Corps</p>"));
        assert_eq!(d.link.as_deref(), Some("https://a.example/1"));
        // k3 est masqué : seul k2 est proposé
        assert_eq!(d.similar.len(), 1);
        assert_eq!((d.similar[0].key.as_str(), d.similar[0].source.as_str()), ("k2", "Source B"));
        assert!(get_article(&c, "k4").unwrap().unwrap().similar.is_empty());
        assert!(get_article(&c, "absent").unwrap().is_none());
    }

    #[test]
    fn saves_full_html_and_keeps_an_existing_image() {
        let c = fixture();
        assert!(save_full_html(&c, "k1", "<p>Complet</p>", "texte", 42, Some("https://img/1.jpg")).unwrap());
        let d = get_article(&c, "k1").unwrap().unwrap();
        assert_eq!(d.full_html.as_deref(), Some("<p>Complet</p>"));
        assert_eq!(d.image_url.as_deref(), Some("https://img/1.jpg"));
        let words: i64 = c.query_row("SELECT word_count FROM articles WHERE key = 'k1'", [], |r| r.get(0)).unwrap();
        assert_eq!(words, 42);

        // une image déjà connue n'est pas remplacée
        save_full_html(&c, "k1", "<p>Autre</p>", "texte", 1, Some("https://img/2.jpg")).unwrap();
        assert_eq!(get_article(&c, "k1").unwrap().unwrap().image_url.as_deref(), Some("https://img/1.jpg"));

        assert!(!save_full_html(&c, "absent", "x", "x", 0, None).unwrap());
    }
}
