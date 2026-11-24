//! Lectures pour l'interface : barre latérale (dossiers, flux, tags, compteurs)
//! et listes d'articles. Reprend les règles de `SidebarView` et `ArticleListView` de l'app Swift.

use std::collections::HashMap;

use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};
use serde::{Deserialize, Serialize};
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use crate::error::Result;
use crate::images;

/// Ce que la barre latérale permet de sélectionner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Selection {
    All,
    Unread,
    Starred,
    History,
    Folder { uid: String },
    Tag { uid: String },
    Feed { url: String },
}

/// Minuscules et sans accents, pour comparer et chercher (équivalent approché de
/// `localizedStandardContains` / `localizedStandardCompare`).
pub fn fold(s: &str) -> String {
    s.nfd().filter(|c| !is_combining_mark(*c)).collect::<String>().to_lowercase()
}

// --- Barre latérale ----------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidebarFeed {
    pub url: String,
    pub title: String,
    pub site_url: Option<String>,
    pub unread: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidebarFolder {
    pub uid: String,
    pub name: String,
    pub symbol: Option<String>,
    pub color_name: Option<String>,
    pub is_expanded: bool,
    pub unread: i64,
    pub feeds: Vec<SidebarFeed>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidebarTag {
    pub uid: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sidebar {
    /// Non lus de « Non lus » : ni masqués, ni doublons.
    pub unread: i64,
    pub folders: Vec<SidebarFolder>,
    /// Flux qui ne sont dans aucun dossier.
    pub feeds: Vec<SidebarFeed>,
    pub tags: Vec<SidebarTag>,
}

pub fn sidebar(conn: &Connection) -> Result<Sidebar> {
    let unread: i64 = conn.query_row(
        "SELECT COUNT(*) FROM articles WHERE is_read = 0 AND is_hidden = 0 AND is_duplicate = 0",
        [],
        |r| r.get(0),
    )?;

    // Non lus par flux : comme côté Swift, les doublons comptent ici (seul le total les exclut).
    let mut counts: HashMap<String, i64> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT feed_url, COUNT(*) FROM articles WHERE is_read = 0 AND is_hidden = 0 GROUP BY feed_url",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        for row in rows {
            let (url, n) = row?;
            counts.insert(url, n);
        }
    }

    // Flux avec leur dossier
    let mut feeds_by_folder: HashMap<Option<i64>, Vec<SidebarFeed>> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT url, COALESCE(custom_title, title), site_url, folder_id FROM feeds")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<i64>>(3)?,
            ))
        })?;
        for row in rows {
            let (url, title, site_url, folder) = row?;
            let unread = counts.get(&url).copied().unwrap_or(0);
            feeds_by_folder.entry(folder).or_default().push(SidebarFeed { url, title, site_url, unread });
        }
    }
    for feeds in feeds_by_folder.values_mut() {
        feeds.sort_by_cached_key(|f| fold(&f.title));
    }

    let mut folders = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT id, uid, name, symbol, color_name, is_expanded FROM folders")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })?;
        for row in rows {
            let (id, uid, name, symbol, color_name, expanded) = row?;
            let feeds = feeds_by_folder.remove(&Some(id)).unwrap_or_default();
            let unread: i64 = feeds.iter().map(|f| f.unread).sum();
            folders.push(SidebarFolder { uid, name, symbol, color_name, is_expanded: expanded != 0, unread, feeds });
        }
    }
    folders.sort_by_cached_key(|f| fold(&f.name));

    let root_feeds = feeds_by_folder.remove(&None).unwrap_or_default();

    let mut tags = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT uid, name FROM favorite_tags")?;
        let rows = stmt.query_map([], |r| Ok(SidebarTag { uid: r.get(0)?, name: r.get(1)? }))?;
        for row in rows {
            tags.push(row?);
        }
    }
    tags.sort_by_cached_key(|t| fold(&t.name));

    Ok(Sidebar { unread, folders, feeds: root_feeds, tags })
}

// --- Listes d'articles -------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ListQuery {
    pub selection: Selection,
    /// Recherche dans le titre et le texte (vide = pas de recherche).
    pub search: String,
    /// Masque les articles lus (Tous, dossiers, flux), comme le réglage de l'app Swift.
    pub hide_read: bool,
    /// Favoris : ne montre que les articles sans tag.
    pub hide_tagged_favorites: bool,
    /// Clés d'articles lus à garder visibles (l'article ouvert ne disparaît pas de la liste).
    pub sticky: Vec<String>,
}

impl Default for ListQuery {
    fn default() -> Self {
        Self {
            selection: Selection::All,
            search: String::new(),
            hide_read: false,
            hide_tagged_favorites: false,
            sticky: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleRow {
    pub key: String,
    pub title: String,
    pub summary: String,
    pub source: String,
    pub feed_url: String,
    /// Secondes Unix
    pub published: f64,
    pub opened_at: Option<f64>,
    pub is_read: bool,
    pub is_starred: bool,
    /// Nom du fichier de cache de la miniature (voir `images::cache_name`).
    pub thumb: Option<String>,
    pub author: Option<String>,
    pub link: Option<String>,
    /// Temps de lecture estimé (230 mots par minute), 0 si inconnu.
    pub reading_minutes: u32,
    /// Nombre d'autres sources qui parlent du même sujet (articles regroupés sous celui-ci).
    pub other_sources: u32,
}

fn host_of(url: &str) -> &str {
    let rest = url.split("://").nth(1).unwrap_or(url);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

/// Nom de la source affiché : nom du flux, sinon nom gardé à sa suppression, sinon son domaine.
pub(crate) fn source_name(feed_title: Option<String>, source_title: Option<String>, feed_url: &str) -> String {
    feed_title.or(source_title).unwrap_or_else(|| host_of(feed_url).to_string())
}

/// Condition « non lu, ou gardé visible » ; ajoute les clés à `params`.
fn unread_or_sticky(sticky: &[String], params: &mut Vec<Value>) -> String {
    if sticky.is_empty() {
        return "a.is_read = 0".to_string();
    }
    let marks = vec!["?"; sticky.len()].join(",");
    params.extend(sticky.iter().cloned().map(Value::Text));
    format!("(a.is_read = 0 OR a.key IN ({marks}))")
}

pub fn list_articles(conn: &Connection, q: &ListQuery) -> Result<Vec<ArticleRow>> {
    let mut params: Vec<Value> = Vec::new();
    let mut conds: Vec<String> = Vec::new();
    let mut order = "a.published DESC, a.id DESC";

    // « Tous » avec masquage des lus = « Non lus » (comme côté Swift)
    let unread_selection = Selection::Unread;
    let selection = if q.hide_read && q.selection == Selection::All { &unread_selection } else { &q.selection };

    match selection {
        Selection::All => conds.push("a.is_hidden = 0 AND a.is_duplicate = 0".into()),
        Selection::Unread => {
            conds.push("a.is_hidden = 0 AND a.is_duplicate = 0".into());
            conds.push(unread_or_sticky(&q.sticky, &mut params));
        }
        Selection::Starred => {
            conds.push("a.is_starred = 1".into());
            if q.hide_tagged_favorites {
                conds.push("NOT EXISTS (SELECT 1 FROM article_tags x WHERE x.article_id = a.id)".into());
            }
        }
        Selection::History => {
            conds.push("a.opened_at IS NOT NULL".into());
            order = "a.opened_at DESC, a.id DESC";
        }
        Selection::Tag { uid } => {
            conds.push(
                "a.is_starred = 1 AND EXISTS (SELECT 1 FROM article_tags x JOIN favorite_tags t ON t.id = x.tag_id
                                              WHERE x.article_id = a.id AND t.uid = ?)"
                    .into(),
            );
            params.push(Value::Text(uid.clone()));
        }
        Selection::Feed { url } => {
            conds.push("a.is_hidden = 0 AND a.feed_url = ?".into());
            params.push(Value::Text(url.clone()));
            if q.hide_read {
                conds.push(unread_or_sticky(&q.sticky, &mut params));
            }
        }
        Selection::Folder { uid } => {
            conds.push(
                "a.is_hidden = 0 AND a.is_duplicate = 0 AND a.feed_url IN
                    (SELECT f.url FROM feeds f JOIN folders d ON d.id = f.folder_id WHERE d.uid = ?)"
                    .into(),
            );
            params.push(Value::Text(uid.clone()));
            if q.hide_read {
                conds.push(unread_or_sticky(&q.sticky, &mut params));
            }
        }
    }

    let search = fold(q.search.trim());
    let searching = !search.is_empty();

    let sql = format!(
        "SELECT a.key, a.title, a.summary, a.feed_url, a.published, a.opened_at, a.is_read, a.is_starred,
                a.image_url, a.author, a.link, a.word_count, a.source_title,
                COALESCE(f.custom_title, f.title),
                {text},
                CASE WHEN a.is_duplicate = 0 AND a.cluster_id IS NOT NULL
                     THEN (SELECT COUNT(*) FROM articles d
                           WHERE d.cluster_id = a.cluster_id AND d.is_duplicate = 1 AND d.is_hidden = 0)
                     ELSE 0 END
         FROM articles a LEFT JOIN feeds f ON f.id = a.feed_id
         WHERE {where_}
         ORDER BY {order}",
        text = if searching { "a.search_text" } else { "NULL" },
        where_ = conds.join(" AND "),
    );

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(params.iter()))?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let title: String = row.get(1)?;
        if searching {
            let text: Option<String> = row.get(14)?;
            let haystack = fold(&format!("{title}\n{}", text.unwrap_or_default()));
            if !haystack.contains(&search) {
                continue;
            }
        }
        let feed_url: String = row.get(3)?;
        let words: i64 = row.get(11)?;
        let source_title: Option<String> = row.get(12)?;
        let feed_title: Option<String> = row.get(13)?;
        let image_url: Option<String> = row.get(8)?;
        let source = source_name(feed_title, source_title, &feed_url);
        out.push(ArticleRow {
            key: row.get(0)?,
            title,
            summary: row.get(2)?,
            source,
            feed_url,
            published: row.get(4)?,
            opened_at: row.get(5)?,
            is_read: row.get::<_, i64>(6)? != 0,
            is_starred: row.get::<_, i64>(7)? != 0,
            thumb: image_url.filter(|u| !u.is_empty()).map(|u| images::cache_name(&u)),
            author: row.get(9)?,
            link: row.get(10)?,
            reading_minutes: if words > 0 { ((words as f64) / 230.0).ceil() as u32 } else { 0 },
            other_sources: row.get::<_, i64>(15)?.max(0) as u32,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn fixture() -> Connection {
        let c = db::open_in_memory().unwrap();
        c.execute_batch(
            "INSERT INTO folders (id, uid, name, created_at) VALUES (1, 'F1', 'Tech', 0);
             INSERT INTO feeds (id, url, title, added_at, folder_id) VALUES
                 (1, 'u1', 'Korben', 0, 1), (2, 'u2', 'Éclair', 0, NULL), (3, 'u3', 'Alpha', 0, 1);
             UPDATE feeds SET custom_title = 'Zèbre' WHERE id = 3;
             INSERT INTO favorite_tags (id, uid, name, created_at) VALUES (1, 'T1', 'IA', 0);
             INSERT INTO articles (id, key, feed_id, feed_url, title, summary, search_text, published, is_read, is_starred, is_duplicate, is_hidden, image_url, word_count, opened_at) VALUES
                 (1, 'a', 1, 'u1', 'Premier',  's', 'contenu sur les ÉLÈVES', 100, 0, 1, 0, 0, 'https://x/i.png', 460, NULL),
                 (2, 'b', 1, 'u1', 'Deuxième', 's', '',                       200, 1, 0, 0, 0, NULL, 0, 500),
                 (3, 'c', 2, 'u2', 'Doublon',  's', '',                       300, 0, 0, 1, 0, NULL, 0, NULL),
                 (4, 'd', 2, 'u2', 'Masqué',   's', '',                       400, 0, 0, 0, 1, NULL, 0, NULL),
                 (5, 'e', 3, 'u3', 'Cinquième','s', '',                       500, 0, 1, 0, 0, NULL, 0, 600);
             INSERT INTO article_tags (article_id, tag_id) VALUES (1, 1);",
        )
        .unwrap();
        c
    }

    fn keys(rows: Vec<ArticleRow>) -> Vec<String> {
        rows.into_iter().map(|r| r.key).collect()
    }

    fn query(selection: Selection) -> ListQuery {
        ListQuery { selection, ..ListQuery::default() }
    }

    #[test]
    fn fold_removes_case_and_accents() {
        assert_eq!(fold("Éclair ÉLÈVES"), "eclair eleves");
    }

    #[test]
    fn all_excludes_hidden_and_duplicates_newest_first() {
        let c = fixture();
        assert_eq!(keys(list_articles(&c, &query(Selection::All)).unwrap()), ["e", "b", "a"]);
    }

    #[test]
    fn unread_and_hide_read() {
        let c = fixture();
        assert_eq!(keys(list_articles(&c, &query(Selection::Unread)).unwrap()), ["e", "a"]);
        let q = ListQuery { hide_read: true, ..query(Selection::All) };
        assert_eq!(keys(list_articles(&c, &q).unwrap()), ["e", "a"]);
        let q = ListQuery { sticky: vec!["b".into()], ..query(Selection::Unread) };
        assert_eq!(keys(list_articles(&c, &q).unwrap()), ["e", "b", "a"]);
    }

    #[test]
    fn feed_view_keeps_duplicates_but_not_hidden() {
        let c = fixture();
        assert_eq!(keys(list_articles(&c, &query(Selection::Feed { url: "u2".into() })).unwrap()), ["c"]);
    }

    #[test]
    fn folder_view_and_hide_read() {
        let c = fixture();
        let q = query(Selection::Folder { uid: "F1".into() });
        assert_eq!(keys(list_articles(&c, &q).unwrap()), ["e", "b", "a"]);
        let q = ListQuery { hide_read: true, ..q };
        assert_eq!(keys(list_articles(&c, &q).unwrap()), ["e", "a"]);
    }

    #[test]
    fn starred_tags_and_history() {
        let c = fixture();
        assert_eq!(keys(list_articles(&c, &query(Selection::Starred)).unwrap()), ["e", "a"]);
        let q = ListQuery { hide_tagged_favorites: true, ..query(Selection::Starred) };
        assert_eq!(keys(list_articles(&c, &q).unwrap()), ["e"]);
        assert_eq!(keys(list_articles(&c, &query(Selection::Tag { uid: "T1".into() })).unwrap()), ["a"]);
        assert_eq!(keys(list_articles(&c, &query(Selection::History)).unwrap()), ["e", "b"]);
    }

    #[test]
    fn other_sources_counts_grouped_duplicates() {
        let c = fixture();
        c.execute_batch("UPDATE articles SET cluster_id = 'C1' WHERE id IN (1, 3)").unwrap();
        let rows = list_articles(&c, &query(Selection::All)).unwrap();
        assert_eq!(rows.iter().find(|r| r.key == "a").unwrap().other_sources, 1);
        assert_eq!(rows.iter().find(|r| r.key == "b").unwrap().other_sources, 0);
        // Le doublon lui-même (vue du flux) n'affiche pas de badge
        let feed = list_articles(&c, &query(Selection::Feed { url: "u2".into() })).unwrap();
        assert_eq!(feed[0].other_sources, 0);
    }

    #[test]
    fn search_ignores_case_and_accents() {
        let c = fixture();
        let q = ListQuery { search: "eleves".into(), ..query(Selection::All) };
        assert_eq!(keys(list_articles(&c, &q).unwrap()), ["a"]);
        let q = ListQuery { search: "DEUXIEME".into(), ..query(Selection::All) };
        assert_eq!(keys(list_articles(&c, &q).unwrap()), ["b"]);
    }

    #[test]
    fn row_has_source_thumbnail_and_reading_time() {
        let c = fixture();
        let rows = list_articles(&c, &query(Selection::All)).unwrap();
        let a = rows.iter().find(|r| r.key == "a").unwrap();
        assert_eq!(a.source, "Korben");
        assert_eq!(a.thumb.as_deref(), Some(images::cache_name("https://x/i.png").as_str()));
        assert_eq!(a.reading_minutes, 2);
        // flux renommé : le nom personnalisé est affiché
        assert_eq!(rows.iter().find(|r| r.key == "e").unwrap().source, "Zèbre");
    }

    #[test]
    fn sidebar_counts_sorting_and_grouping() {
        let c = fixture();
        let s = sidebar(&c).unwrap();
        assert_eq!(s.unread, 2); // a, e (c est un doublon, d est masqué)
        assert_eq!(s.folders.len(), 1);
        let tech = &s.folders[0];
        assert_eq!(tech.unread, 2);
        assert_eq!(tech.feeds.iter().map(|f| f.title.as_str()).collect::<Vec<_>>(), ["Korben", "Zèbre"]);
        assert_eq!(s.feeds.len(), 1);
        assert_eq!(s.feeds[0].unread, 1); // le doublon compte dans le flux, pas le masqué
        assert_eq!(s.tags.len(), 1);
    }
}
