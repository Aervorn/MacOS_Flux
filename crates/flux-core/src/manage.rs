//! Gestion des flux, des dossiers et des tags (renommer, déplacer, supprimer).

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{Error, Result};
use crate::feeds::now;

/// Renomme un flux ; un nom vide (ou identique à l'original) rétablit le nom d'origine.
pub fn rename_feed(conn: &Connection, url: &str, name: &str) -> Result<()> {
    let name = name.trim();
    let original: String = conn
        .query_row("SELECT title FROM feeds WHERE url = ?1", params![url], |r| r.get(0))
        .optional()?
        .ok_or_else(|| Error::Other("flux introuvable".into()))?;
    let custom = if name.is_empty() || name == original { None } else { Some(name) };
    conn.execute("UPDATE feeds SET custom_title = ?2 WHERE url = ?1", params![url, custom])?;
    Ok(())
}

/// Déplace un flux dans un dossier (None : à la racine).
pub fn move_feed(conn: &Connection, url: &str, folder_uid: Option<&str>) -> Result<()> {
    let folder_id: Option<i64> = match folder_uid {
        Some(uid) => Some(
            conn.query_row("SELECT id FROM folders WHERE uid = ?1", params![uid], |r| r.get(0))
                .optional()?
                .ok_or_else(|| Error::Other("dossier introuvable".into()))?,
        ),
        None => None,
    };
    conn.execute("UPDATE feeds SET folder_id = ?2 WHERE url = ?1", params![url, folder_id])?;
    Ok(())
}

/// Supprime un flux et ses articles, sauf les favoris : ils restent dans Favoris
/// (et reviennent dans le flux si on l'ajoute à nouveau).
pub fn delete_feed(conn: &Connection, url: &str) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE articles SET source_title = (SELECT COALESCE(custom_title, title) FROM feeds WHERE url = ?1), feed_id = NULL
         WHERE feed_url = ?1 AND is_starred = 1",
        params![url],
    )?;
    tx.execute("DELETE FROM articles WHERE feed_url = ?1 AND is_starred = 0", params![url])?;
    tx.execute("DELETE FROM feeds WHERE url = ?1", params![url])?;
    tx.commit()?;
    Ok(())
}

/// Réécrit les données d'un flux sous une nouvelle adresse, sans réseau : ses articles gardent
/// leur état lu / favori / tags, le flux garde son nom, son dossier et ses règles de filtrage.
/// Un article déjà présent sous la nouvelle adresse (favori resté d'une ancienne suppression)
/// absorbe son jumeau : un seul exemplaire reste.
pub fn migrate_feed(conn: &Connection, old_url: &str, new_url: &str) -> Result<()> {
    if old_url == new_url {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    let feed_id: i64 = tx
        .query_row("SELECT id FROM feeds WHERE url = ?1", params![old_url], |r| r.get(0))
        .optional()?
        .ok_or_else(|| Error::Other("flux introuvable".into()))?;
    let taken: i64 = tx.query_row("SELECT COUNT(*) FROM feeds WHERE url = ?1", params![new_url], |r| r.get(0))?;
    if taken > 0 {
        return Err(Error::Feed(crate::error::FeedError::AlreadyExists));
    }

    let old_prefix = format!("{old_url}|");
    let articles: Vec<(i64, String, bool, bool)> = {
        let mut stmt = tx.prepare("SELECT id, key, is_read, is_starred FROM articles WHERE feed_url = ?1")?;
        let rows = stmt.query_map(params![old_url], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0, r.get::<_, i64>(3)? != 0)))?;
        rows.collect::<std::result::Result<_, _>>()?
    };
    tx.execute("UPDATE feeds SET url = ?2, previous_url = ?1, last_error = NULL WHERE id = ?3", params![old_url, new_url, feed_id])?;
    for (id, key, read, starred) in articles {
        let ident = key.strip_prefix(&old_prefix).unwrap_or(&key);
        let new_key = format!("{new_url}|{ident}");
        let twin: Option<i64> = tx
            .query_row("SELECT id FROM articles WHERE key = ?1", params![new_key], |r| r.get(0))
            .optional()?;
        match twin {
            Some(twin_id) if twin_id != id => {
                if read {
                    tx.execute("UPDATE articles SET is_read = 1 WHERE id = ?1", params![twin_id])?;
                }
                if starred {
                    tx.execute("UPDATE articles SET is_starred = 1 WHERE id = ?1", params![twin_id])?;
                }
                tx.execute(
                    "INSERT OR IGNORE INTO article_tags (article_id, tag_id) SELECT ?2, tag_id FROM article_tags WHERE article_id = ?1",
                    params![id, twin_id],
                )?;
                tx.execute("DELETE FROM articles WHERE id = ?1", params![id])?;
            }
            _ => {
                tx.execute("UPDATE articles SET key = ?2, feed_url = ?3 WHERE id = ?1", params![id, new_key, new_url])?;
            }
        }
    }
    // Favoris restés sous la nouvelle adresse après une ancienne suppression : de nouveau rattachés
    tx.execute("UPDATE articles SET feed_id = ?2 WHERE feed_url = ?1 AND feed_id IS NULL", params![new_url, feed_id])?;
    // Règles de filtrage visant ce flux
    tx.execute("UPDATE OR IGNORE filter_rule_feeds SET feed_url = ?2 WHERE feed_url = ?1", params![old_url, new_url])?;
    tx.execute("DELETE FROM filter_rule_feeds WHERE feed_url = ?1", params![old_url])?;
    tx.commit()?;
    Ok(())
}

/// Crée un dossier et renvoie son uid (None si le nom est vide).
pub fn create_folder(conn: &Connection, name: &str) -> Result<Option<String>> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }
    let uid = uuid::Uuid::new_v4().to_string().to_uppercase();
    conn.execute("INSERT INTO folders (uid, name, created_at) VALUES (?1, ?2, ?3)", params![uid, name, now()])?;
    Ok(Some(uid))
}

pub fn rename_folder(conn: &Connection, uid: &str, name: &str) -> Result<()> {
    let name = name.trim();
    if !name.is_empty() {
        conn.execute("UPDATE folders SET name = ?2 WHERE uid = ?1", params![uid, name])?;
    }
    Ok(())
}

/// Les flux du dossier reviennent à la racine.
pub fn delete_folder(conn: &Connection, uid: &str) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("UPDATE feeds SET folder_id = NULL WHERE folder_id = (SELECT id FROM folders WHERE uid = ?1)", params![uid])?;
    tx.execute("DELETE FROM folders WHERE uid = ?1", params![uid])?;
    tx.commit()?;
    Ok(())
}

pub fn set_folder_expanded(conn: &Connection, uid: &str, expanded: bool) -> Result<()> {
    conn.execute("UPDATE folders SET is_expanded = ?2 WHERE uid = ?1", params![uid, expanded])?;
    Ok(())
}

pub fn rename_tag(conn: &Connection, uid: &str, name: &str) -> Result<()> {
    let name = name.trim();
    if !name.is_empty() {
        conn.execute("UPDATE favorite_tags SET name = ?2 WHERE uid = ?1", params![uid, name])?;
    }
    Ok(())
}

/// Supprime le tag ; les favoris restent, sans ce tag.
pub fn delete_tag(conn: &Connection, uid: &str) -> Result<()> {
    conn.execute("DELETE FROM favorite_tags WHERE uid = ?1", params![uid])?;
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
             INSERT INTO feeds (id, url, title, added_at, folder_id) VALUES (1, 'u1', 'Original', 0, 1), (2, 'u2', 'Autre', 0, NULL);
             INSERT INTO articles (id, key, feed_id, feed_url, title, published, is_starred) VALUES
                 (1, 'a', 1, 'u1', 'A', 1, 1), (2, 'b', 1, 'u1', 'B', 2, 0), (3, 'c', 2, 'u2', 'C', 3, 0);
             INSERT INTO favorite_tags (id, uid, name, created_at) VALUES (1, 'T1', 'Tag', 0);
             INSERT INTO article_tags (article_id, tag_id) VALUES (1, 1);",
        )
        .unwrap();
        c
    }

    fn one<T: rusqlite::types::FromSql>(c: &Connection, sql: &str) -> T {
        c.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn renames_feed_and_restores_original_name() {
        let c = fixture();
        rename_feed(&c, "u1", "  Mon nom ").unwrap();
        assert_eq!(one::<String>(&c, "SELECT custom_title FROM feeds WHERE id = 1"), "Mon nom");
        rename_feed(&c, "u1", "Original").unwrap();
        assert_eq!(one::<Option<String>>(&c, "SELECT custom_title FROM feeds WHERE id = 1"), None);
        rename_feed(&c, "u1", "X").unwrap();
        rename_feed(&c, "u1", "").unwrap();
        assert_eq!(one::<Option<String>>(&c, "SELECT custom_title FROM feeds WHERE id = 1"), None);
        assert!(rename_feed(&c, "inconnu", "X").is_err());
    }

    #[test]
    fn moves_feed_between_folders() {
        let c = fixture();
        move_feed(&c, "u2", Some("F1")).unwrap();
        assert_eq!(one::<i64>(&c, "SELECT folder_id FROM feeds WHERE id = 2"), 1);
        move_feed(&c, "u2", None).unwrap();
        assert_eq!(one::<Option<i64>>(&c, "SELECT folder_id FROM feeds WHERE id = 2"), None);
        assert!(move_feed(&c, "u2", Some("inconnu")).is_err());
    }

    #[test]
    fn deleting_a_feed_keeps_its_favorites() {
        let c = fixture();
        rename_feed(&c, "u1", "Mon nom").unwrap();
        delete_feed(&c, "u1").unwrap();
        assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM feeds"), 1);
        assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM articles WHERE key = 'b'"), 0);
        assert_eq!(one::<Option<i64>>(&c, "SELECT feed_id FROM articles WHERE key = 'a'"), None);
        assert_eq!(one::<String>(&c, "SELECT source_title FROM articles WHERE key = 'a'"), "Mon nom");
        assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM articles WHERE key = 'c'"), 1);
    }

    #[test]
    fn folder_lifecycle() {
        let c = fixture();
        let uid = create_folder(&c, " Neuf ").unwrap().unwrap();
        assert!(create_folder(&c, "  ").unwrap().is_none());
        rename_folder(&c, &uid, "Renommé").unwrap();
        rename_folder(&c, &uid, "  ").unwrap(); // nom vide ignoré
        assert_eq!(one::<String>(&c, &format!("SELECT name FROM folders WHERE uid = '{uid}'")), "Renommé");
        set_folder_expanded(&c, "F1", false).unwrap();
        assert_eq!(one::<i64>(&c, "SELECT is_expanded FROM folders WHERE uid = 'F1'"), 0);
        delete_folder(&c, "F1").unwrap();
        assert_eq!(one::<Option<i64>>(&c, "SELECT folder_id FROM feeds WHERE id = 1"), None);
        assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM feeds"), 2);
    }

    #[test]
    fn tag_rename_and_delete() {
        let c = fixture();
        rename_tag(&c, "T1", "Neuf").unwrap();
        assert_eq!(one::<String>(&c, "SELECT name FROM favorite_tags"), "Neuf");
        delete_tag(&c, "T1").unwrap();
        assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM favorite_tags"), 0);
        assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM article_tags"), 0);
        assert_eq!(one::<i64>(&c, "SELECT is_starred FROM articles WHERE key = 'a'"), 1);
    }

    #[test]
    fn migrate_feed_keeps_state() {
        let c = fixture();
        c.execute("UPDATE articles SET is_read = 1 WHERE key = 'b'", []).unwrap();
        migrate_feed(&c, "u1", "https://new.example/feed").unwrap();
        assert_eq!(one::<String>(&c, "SELECT url FROM feeds WHERE id = 1"), "https://new.example/feed");
        assert_eq!(one::<String>(&c, "SELECT previous_url FROM feeds WHERE id = 1"), "u1");
        // articles : clé réécrite, état et tags conservés
        assert_eq!(one::<i64>(&c, "SELECT is_read FROM articles WHERE key = 'https://new.example/feed|b'"), 1);
        assert_eq!(one::<i64>(&c, "SELECT is_starred FROM articles WHERE key = 'https://new.example/feed|a'"), 1);
        assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM article_tags"), 1);
        assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM articles WHERE feed_url = 'u1'"), 0);
        // adresse déjà prise
        assert!(migrate_feed(&c, "https://new.example/feed", "u2").is_err());
    }

    #[test]
    fn migrate_feed_merges_twin() {
        let c = fixture();
        c.execute_batch(
            "INSERT INTO articles (id, key, feed_id, feed_url, title, published, is_starred) VALUES (9, 'n|a', NULL, 'n', 'A', 1, 0);",
        )
        .unwrap();
        migrate_feed(&c, "u1", "n").unwrap();
        // l'article « a » (favori, avec tag) absorbe son jumeau déjà présent sous « n »
        assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM articles WHERE key = 'n|a'"), 1);
        assert_eq!(one::<i64>(&c, "SELECT is_starred FROM articles WHERE key = 'n|a'"), 1);
        assert_eq!(one::<i64>(&c, "SELECT COUNT(*) FROM article_tags"), 1);
    }
}
