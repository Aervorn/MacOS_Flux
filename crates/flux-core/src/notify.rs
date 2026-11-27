//! Choix des nouveaux articles qui méritent une notification.
//!
//! Seuls comptent les articles ni masqués ni lus (un filtre a pu les marquer), qui ne sont pas
//! le doublon d'un sujet déjà présent, et publiés depuis moins de 48 h : un flux qui republie
//! d'anciens articles ne déclenche pas d'avalanche.

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::Result;
use crate::filters::contains_whole_word;
use crate::prefs::Prefs;
use crate::query::{fold, source_name};

/// Âge maximal d'un article pour mériter une notification.
const MAX_AGE: f64 = 48.0 * 3600.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub key: String,
    pub title: String,
    /// Nom du flux (ou de la source) affiché en titre de la notification
    pub source: String,
}

/// Parmi les articles qui viennent d'arriver (`keys`), ceux à signaler, du plus récent au plus ancien.
pub fn candidates(conn: &Connection, keys: &[String], prefs: &Prefs, now: f64) -> Result<Vec<Candidate>> {
    if !prefs.notify_enabled || keys.is_empty() {
        return Ok(Vec::new());
    }
    let keywords: Vec<String> = prefs
        .notify_keywords
        .iter()
        .map(|k| fold(k.trim()))
        .filter(|k| !k.is_empty())
        .collect();
    let mut article = conn.prepare(
        "SELECT a.id, a.title, a.feed_url, COALESCE(f.custom_title, f.title), a.source_title, fo.uid, a.published
         FROM articles a
         LEFT JOIN feeds f ON f.id = a.feed_id
         LEFT JOIN folders fo ON fo.id = f.folder_id
         WHERE a.key = ?1 AND a.is_hidden = 0 AND a.is_read = 0 AND a.is_duplicate = 0 AND a.published >= ?2",
    )?;
    let mut smart = conn.prepare(
        "SELECT s.uid FROM smart_folder_articles sa JOIN smart_folders s ON s.id = sa.folder_id WHERE sa.article_id = ?1",
    )?;

    let mut found: Vec<(f64, Candidate)> = Vec::new();
    for key in keys {
        let row = article
            .query_row(params![key, now - MAX_AGE], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, f64>(6)?,
                ))
            })
            .optional()?;
        let Some((id, title, feed_url, feed_title, source_title, folder, published)) = row else { continue };

        let wanted = prefs.notify_all
            || prefs.notify_feeds.contains(&feed_url)
            || folder.as_ref().is_some_and(|uid| prefs.notify_folders.contains(uid))
            || (!keywords.is_empty() && {
                let folded = fold(&title);
                keywords.iter().any(|k| contains_whole_word(&folded, k))
            })
            || (!prefs.notify_smart_folders.is_empty()
                && smart
                    .query_map(params![id], |r| r.get::<_, String>(0))?
                    .flatten()
                    .any(|uid| prefs.notify_smart_folders.contains(&uid)));
        if wanted {
            let source = source_name(feed_title, source_title, &feed_url);
            found.push((published, Candidate { key: key.clone(), title, source }));
        }
    }
    found.sort_by(|a, b| b.0.total_cmp(&a.0));
    Ok(found.into_iter().map(|(_, c)| c).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: f64 = 1_000_000.0;

    fn setup() -> Connection {
        let c = crate::db::open_in_memory().unwrap();
        c.execute_batch(
            "INSERT INTO folders (id, uid, name, created_at) VALUES (1, 'F1', 'Tech', 0);
             INSERT INTO feeds (id, url, title, added_at, folder_id) VALUES (1, 'https://a/f', 'A', 0, 1), (2, 'https://b/f', 'B', 0, NULL);
             INSERT INTO smart_folders (id, uid, name, created_at) VALUES (1, 'S1', 'Domotique', 0);
             INSERT INTO articles (id, key, feed_id, feed_url, title, published, is_read, is_hidden, is_duplicate) VALUES
               (1, 'k1', 1, 'https://a/f', 'Un nouvel iPhone arrive', 999000, 0, 0, 0),
               (2, 'k2', 2, 'https://b/f', 'Home Assistant 2026.12', 999500, 0, 0, 0),
               (3, 'k3', 2, 'https://b/f', 'Déjà lu', 999000, 1, 0, 0),
               (4, 'k4', 2, 'https://b/f', 'Masqué', 999000, 0, 1, 0),
               (5, 'k5', 2, 'https://b/f', 'Doublon', 999000, 0, 0, 1),
               (6, 'k6', 2, 'https://b/f', 'Vieux de trois jours', 740000, 0, 0, 0),
               (7, 'k7', 2, 'https://b/f', 'Météo : l’IA prévoit la pluie', 999100, 0, 0, 0);
             INSERT INTO smart_folder_articles (folder_id, article_id) VALUES (1, 2);",
        )
        .unwrap();
        c
    }

    fn keys() -> Vec<String> {
        (1..=7).map(|i| format!("k{i}")).collect()
    }

    fn found(c: &Connection, prefs: &Prefs) -> Vec<String> {
        candidates(c, &keys(), prefs, NOW).unwrap().into_iter().map(|c| c.key).collect()
    }

    #[test]
    fn nothing_when_disabled() {
        let c = setup();
        assert!(found(&c, &Prefs::default()).is_empty());
    }

    #[test]
    fn all_feeds_skips_read_hidden_duplicate_and_old_articles() {
        let c = setup();
        let prefs = Prefs { notify_enabled: true, ..Prefs::default() };
        // du plus récent au plus ancien
        assert_eq!(found(&c, &prefs), ["k2", "k7", "k1"]);
        assert_eq!(candidates(&c, &keys(), &prefs, NOW).unwrap()[2].source, "A");
    }

    #[test]
    fn a_selection_matches_feeds_folders_smart_folders_and_keywords() {
        let c = setup();
        let base = Prefs { notify_enabled: true, notify_all: false, ..Prefs::default() };
        assert!(found(&c, &base).is_empty());
        assert_eq!(found(&c, &Prefs { notify_folders: vec!["F1".into()], ..base.clone() }), ["k1"]);
        assert_eq!(found(&c, &Prefs { notify_feeds: vec!["https://b/f".into()], ..base.clone() }), ["k2", "k7"]);
        assert_eq!(found(&c, &Prefs { notify_smart_folders: vec!["S1".into()], ..base.clone() }), ["k2"]);
        // mot entier, sans majuscules ni accents : « ia » ne trouve pas « iPhone »
        assert_eq!(found(&c, &Prefs { notify_keywords: vec!["ia".into()], ..base.clone() }), ["k7"]);
        assert_eq!(found(&c, &Prefs { notify_keywords: vec!["  METEO ".into()], ..base }), ["k7"]);
    }
}
