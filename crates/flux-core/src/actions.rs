//! Actions de l'utilisateur sur les articles : lu, favori, tags, historique.

use std::collections::HashSet;

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::query::Selection;

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn placeholders(n: usize) -> String {
    vec!["?"; n].join(",")
}

/// Marque comme lus / non lus les articles donnés, avec leurs articles similaires des autres
/// sources (même groupe de doublons), comme l'app Swift.
pub fn set_read(conn: &Connection, keys: &[String], read: bool) -> Result<()> {
    if keys.is_empty() {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    let mut clusters: HashSet<String> = HashSet::new();
    {
        let sql = format!(
            "SELECT DISTINCT cluster_id FROM articles WHERE key IN ({}) AND cluster_id IS NOT NULL",
            placeholders(keys.len())
        );
        let mut stmt = tx.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(keys.iter()), |r| r.get::<_, String>(0))?;
        for row in rows {
            clusters.insert(row?);
        }
    }
    let flag = i64::from(read);
    tx.execute(
        &format!("UPDATE articles SET is_read = ? WHERE is_read != ?1 AND key IN ({})", placeholders(keys.len())),
        params_from_iter(std::iter::once(Value::Integer(flag)).chain(keys.iter().map(|k| Value::Text(k.clone())))),
    )?;
    if !clusters.is_empty() {
        tx.execute(
            &format!(
                "UPDATE articles SET is_read = ? WHERE is_read != ?1 AND cluster_id IN ({})",
                placeholders(clusters.len())
            ),
            params_from_iter(std::iter::once(Value::Integer(flag)).chain(clusters.iter().map(|c| Value::Text(c.clone())))),
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// « Tout marquer comme lu » pour la sélection affichée. Renvoie le nombre d'articles changés.
pub fn mark_all_read(conn: &Connection, selection: &Selection, hide_tagged_favorites: bool) -> Result<usize> {
    let mut params: Vec<Value> = Vec::new();
    let cond = match selection {
        Selection::All | Selection::Unread => "1 = 1".to_string(),
        Selection::Feed { url } => {
            params.push(Value::Text(url.clone()));
            "feed_url = ?".into()
        }
        Selection::Folder { uid } => {
            params.push(Value::Text(uid.clone()));
            "feed_url IN (SELECT f.url FROM feeds f JOIN folders d ON d.id = f.folder_id WHERE d.uid = ?)".into()
        }
        Selection::Starred if hide_tagged_favorites => {
            "is_starred = 1 AND NOT EXISTS (SELECT 1 FROM article_tags x WHERE x.article_id = articles.id)".into()
        }
        Selection::Starred => "is_starred = 1".into(),
        Selection::History => "opened_at IS NOT NULL".into(),
        Selection::Tag { uid } => {
            params.push(Value::Text(uid.clone()));
            "is_starred = 1 AND EXISTS (SELECT 1 FROM article_tags x JOIN favorite_tags t ON t.id = x.tag_id
                                        WHERE x.article_id = articles.id AND t.uid = ?)"
                .into()
        }
    };
    let changed = conn.execute(
        &format!("UPDATE articles SET is_read = 1 WHERE is_read = 0 AND {cond}"),
        params_from_iter(params.iter()),
    )?;
    Ok(changed)
}

pub fn set_starred(conn: &Connection, keys: &[String], starred: bool) -> Result<()> {
    if keys.is_empty() {
        return Ok(());
    }
    conn.execute(
        &format!("UPDATE articles SET is_starred = ? WHERE is_starred != ?1 AND key IN ({})", placeholders(keys.len())),
        params_from_iter(
            std::iter::once(Value::Integer(i64::from(starred))).chain(keys.iter().map(|k| Value::Text(k.clone()))),
        ),
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    pub uid: String,
    pub name: String,
}

/// Crée un tag (ou renvoie celui qui porte déjà ce nom, sans tenir compte de la casse).
/// Renvoie None si le nom est vide.
pub fn create_tag(conn: &Connection, name: &str) -> Result<Option<TagInfo>> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }
    let wanted = name.to_lowercase();
    {
        let mut stmt = conn.prepare("SELECT uid, name FROM favorite_tags")?;
        let rows = stmt.query_map([], |r| Ok(TagInfo { uid: r.get(0)?, name: r.get(1)? }))?;
        for row in rows {
            let tag = row?;
            if tag.name.to_lowercase() == wanted {
                return Ok(Some(tag));
            }
        }
    }
    let uid = uuid::Uuid::new_v4().to_string().to_uppercase();
    conn.execute(
        "INSERT INTO favorite_tags (uid, name, created_at) VALUES (?1, ?2, ?3)",
        params![uid, name, now()],
    )?;
    Ok(Some(TagInfo { uid, name: name.to_string() }))
}

/// Ajoute ou retire un tag. Un tag range un favori : l'ajouter à un article qui n'en est pas
/// un l'ajoute aux favoris.
pub fn set_tag(conn: &Connection, uid: &str, on: bool, keys: &[String]) -> Result<()> {
    let tag_id: i64 = conn
        .query_row("SELECT id FROM favorite_tags WHERE uid = ?1", params![uid], |r| r.get(0))
        .optional()?
        .ok_or_else(|| Error::Other("tag introuvable".into()))?;
    for key in keys {
        let Some(article_id) = conn
            .query_row("SELECT id FROM articles WHERE key = ?1", params![key], |r| r.get::<_, i64>(0))
            .optional()?
        else {
            continue;
        };
        if on {
            conn.execute(
                "INSERT OR IGNORE INTO article_tags (article_id, tag_id) VALUES (?1, ?2)",
                params![article_id, tag_id],
            )?;
        } else {
            conn.execute(
                "DELETE FROM article_tags WHERE article_id = ?1 AND tag_id = ?2",
                params![article_id, tag_id],
            )?;
        }
    }
    if on {
        set_starred(conn, keys, true)?;
    }
    Ok(())
}

/// Historique : date de la première ouverture (conservée si elle existe déjà).
pub fn record_opened(conn: &Connection, key: &str) -> Result<()> {
    conn.execute("UPDATE articles SET opened_at = ?2 WHERE key = ?1 AND opened_at IS NULL", params![key, now()])?;
    Ok(())
}

/// Temps passé sur l'article (plafonné à 30 minutes par visite, comme côté Swift).
pub fn add_reading_seconds(conn: &Connection, key: &str, seconds: f64) -> Result<()> {
    let seconds = seconds.clamp(0.0, 30.0 * 60.0);
    if seconds < 1.0 {
        return Ok(());
    }
    conn.execute("UPDATE articles SET reading_seconds = reading_seconds + ?2 WHERE key = ?1", params![key, seconds])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn fixture() -> Connection {
        let c = db::open_in_memory().unwrap();
        c.execute_batch(
            "INSERT INTO folders (id, uid, name, created_at) VALUES (1, 'F1', 'Dossier', 0);
             INSERT INTO feeds (id, url, title, added_at, folder_id) VALUES (1, 'u1', 'A', 0, 1), (2, 'u2', 'B', 0, NULL);
             INSERT INTO articles (id, key, feed_id, feed_url, title, published, cluster_id) VALUES
                 (1, 'a', 1, 'u1', 'A', 1, 'C1'),
                 (2, 'b', 2, 'u2', 'B', 2, 'C1'),
                 (3, 'c', 2, 'u2', 'C', 3, NULL),
                 (4, 'd', 1, 'u1', 'D', 4, NULL);",
        )
        .unwrap();
        c
    }

    fn read_keys(c: &Connection) -> Vec<String> {
        let mut stmt = c.prepare("SELECT key FROM articles WHERE is_read = 1 ORDER BY key").unwrap();
        let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
        rows.map(|r| r.unwrap()).collect()
    }

    #[test]
    fn read_state_follows_the_cluster() {
        let c = fixture();
        set_read(&c, &["a".into()], true).unwrap();
        assert_eq!(read_keys(&c), ["a", "b"]); // b est le même sujet que a
        set_read(&c, &["b".into()], false).unwrap();
        assert!(read_keys(&c).is_empty());
        set_read(&c, &["c".into()], true).unwrap();
        assert_eq!(read_keys(&c), ["c"]);
        set_read(&c, &[], true).unwrap();
    }

    #[test]
    fn marks_all_read_per_selection() {
        let c = fixture();
        assert_eq!(mark_all_read(&c, &Selection::Feed { url: "u2".into() }, false).unwrap(), 2);
        assert_eq!(read_keys(&c), ["b", "c"]);
        assert_eq!(mark_all_read(&c, &Selection::Folder { uid: "F1".into() }, false).unwrap(), 2);
        assert_eq!(read_keys(&c).len(), 4);
        assert_eq!(mark_all_read(&c, &Selection::All, false).unwrap(), 0);
    }

    #[test]
    fn tags_make_favorites_and_are_reused() {
        let c = fixture();
        assert!(create_tag(&c, "  ").unwrap().is_none());
        let tag = create_tag(&c, "Idées").unwrap().unwrap();
        assert_eq!(create_tag(&c, "idées").unwrap().unwrap().uid, tag.uid); // casse ignorée
        assert_eq!(tag.uid.len(), 36);

        set_tag(&c, &tag.uid, true, &["a".into(), "c".into()]).unwrap();
        let starred: i64 = c.query_row("SELECT COUNT(*) FROM articles WHERE is_starred = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(starred, 2);
        assert_eq!(mark_all_read(&c, &Selection::Tag { uid: tag.uid.clone() }, false).unwrap(), 2);

        set_tag(&c, &tag.uid, false, &["a".into()]).unwrap();
        let tagged: i64 = c.query_row("SELECT COUNT(*) FROM article_tags", [], |r| r.get(0)).unwrap();
        assert_eq!(tagged, 1);
        // retirer le tag laisse l'article en favori
        let still: i64 = c.query_row("SELECT is_starred FROM articles WHERE key = 'a'", [], |r| r.get(0)).unwrap();
        assert_eq!(still, 1);
        assert!(set_tag(&c, "inconnu", true, &["a".into()]).is_err());

        set_starred(&c, &["a".into(), "c".into()], false).unwrap();
        let starred: i64 = c.query_row("SELECT COUNT(*) FROM articles WHERE is_starred = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(starred, 0);
    }

    #[test]
    fn history_and_reading_time() {
        let c = fixture();
        record_opened(&c, "a").unwrap();
        let first: f64 = c.query_row("SELECT opened_at FROM articles WHERE key = 'a'", [], |r| r.get(0)).unwrap();
        assert!(first > 1.0e9);
        record_opened(&c, "a").unwrap(); // la première date est conservée
        let again: f64 = c.query_row("SELECT opened_at FROM articles WHERE key = 'a'", [], |r| r.get(0)).unwrap();
        assert_eq!(first, again);

        add_reading_seconds(&c, "a", 0.5).unwrap(); // trop court : ignoré
        add_reading_seconds(&c, "a", 10.0).unwrap();
        add_reading_seconds(&c, "a", 99_999.0).unwrap(); // plafonné à 30 minutes
        let secs: f64 = c.query_row("SELECT reading_seconds FROM articles WHERE key = 'a'", [], |r| r.get(0)).unwrap();
        assert_eq!(secs, 10.0 + 1800.0);
    }
}
