//! Base SQLite de Flux : ouverture, migrations, réglages.

use std::path::Path;

use rusqlite::{params, Connection};
use serde::{de::DeserializeOwned, Serialize};

use crate::error::{Error, Result};
use crate::prefs::Prefs;

/// Migrations dans l'ordre : l'indice + 1 est le numéro de version (`PRAGMA user_version`).
const MIGRATIONS: &[&str] = &[include_str!("schema_v1.sql")];

/// Ouvre (et crée au besoin) la base d'un fichier, en mode WAL.
pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    // PRAGMA journal_mode renvoie une ligne : on la lit plutôt que de l'ignorer.
    let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
    init(conn)
}

/// Base en mémoire (tests).
pub fn open_in_memory() -> Result<Connection> {
    init(Connection::open_in_memory()?)
}

fn init(mut conn: Connection) -> Result<Connection> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if current < 0 || current as usize > MIGRATIONS.len() {
        return Err(Error::Other(format!(
            "la base est en version {current}, plus récente que cette app ({})",
            MIGRATIONS.len()
        )));
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute_batch(&format!("PRAGMA user_version = {}", i + 1))?;
        tx.commit()?;
    }
    Ok(())
}

/// Écrit une valeur JSON dans la table `settings`.
pub fn save_setting<T: Serialize>(conn: &Connection, key: &str, value: &T) -> Result<()> {
    let json = serde_json::to_string(value)?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, json],
    )?;
    Ok(())
}

/// Lit une valeur JSON de la table `settings` (None si absente).
pub fn load_setting<T: DeserializeOwned>(conn: &Connection, key: &str) -> Result<Option<T>> {
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let mut rows = stmt.query(params![key])?;
    let json: Option<String> = match rows.next()? {
        Some(row) => Some(row.get(0)?),
        None => None,
    };
    match json {
        Some(json) => Ok(Some(serde_json::from_str(&json)?)),
        None => Ok(None),
    }
}

pub const PREFS_KEY: &str = "prefs";

pub fn save_prefs(conn: &Connection, prefs: &Prefs) -> Result<()> {
    save_setting(conn, PREFS_KEY, prefs)
}

/// Réglages enregistrés, ou valeurs par défaut s'il n'y en a pas encore.
pub fn load_prefs(conn: &Connection) -> Result<Prefs> {
    Ok(load_setting(conn, PREFS_KEY)?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_schema_and_sets_version() {
        let conn = open_in_memory().unwrap();
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, MIGRATIONS.len() as i64);
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name = 'articles'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn prefs_round_trip_and_default() {
        let conn = open_in_memory().unwrap();
        assert_eq!(load_prefs(&conn).unwrap(), Prefs::default());
        let mut p = Prefs::default();
        p.retention_days = 14;
        save_prefs(&conn, &p).unwrap();
        assert_eq!(load_prefs(&conn).unwrap().retention_days, 14);
    }

    #[test]
    fn deleting_a_feed_keeps_its_articles() {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO feeds (id, url, title, added_at) VALUES (1, 'u', 't', 0);
             INSERT INTO articles (id, key, feed_id, feed_url, title, published) VALUES (1, 'u|1', 1, 'u', 'a', 0);
             DELETE FROM feeds WHERE id = 1;",
        )
        .unwrap();
        let feed_id: Option<i64> = conn.query_row("SELECT feed_id FROM articles WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(feed_id, None);
    }
}
