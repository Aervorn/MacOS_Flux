//! Règles de filtres : un mot-clé cherché dans le titre des articles.
//!
//! Reprend `Filters.swift` : comparaison sans majuscules ni accents, option « mot entier »,
//! règles limitées à certains flux, deux actions (masquer, marquer comme lu).
//! Les favoris ne sont jamais filtrés. La première règle qui correspond l'emporte.

use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::feeds::now;
use crate::query::fold;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Hide,
    MarkRead,
}

impl Action {
    pub fn parse(s: &str) -> Option<Action> {
        match s {
            "hide" => Some(Action::Hide),
            "markRead" => Some(Action::MarkRead),
            _ => None,
        }
    }
}

/// Règle prête à être testée (mot-clé normalisé).
#[derive(Debug, Clone)]
pub struct CompiledFilter {
    pub uid: String,
    /// Flux concernés (vide = tous les flux).
    feed_urls: HashSet<String>,
    pub action: Action,
    needle: String,
    whole_word: bool,
}

impl CompiledFilter {
    pub fn new(uid: &str, keyword: &str, whole_word: bool, action: Action, feed_urls: &[String]) -> Self {
        CompiledFilter {
            uid: uid.to_string(),
            feed_urls: feed_urls.iter().cloned().collect(),
            action,
            needle: fold(keyword.trim()),
            whole_word,
        }
    }

    pub fn matches(&self, title: &str, feed_url: &str) -> bool {
        if self.needle.is_empty() {
            return false;
        }
        if !self.feed_urls.is_empty() && !self.feed_urls.contains(feed_url) {
            return false;
        }
        let folded = fold(title);
        if self.whole_word {
            contains_whole_word(&folded, &self.needle)
        } else {
            folded.contains(&self.needle)
        }
    }
}

/// Le mot-clé apparaît sans lettre ni chiffre juste avant ou juste après.
fn contains_whole_word(haystack: &str, needle: &str) -> bool {
    let mut from = 0;
    while let Some(pos) = haystack[from..].find(needle) {
        let start = from + pos;
        let end = start + needle.len();
        let before_ok = haystack[..start].chars().next_back().map_or(true, |c| !c.is_alphanumeric());
        let after_ok = haystack[end..].chars().next().map_or(true, |c| !c.is_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        // Avance d'un caractère (et non d'un octet) pour rester sur une frontière valide.
        from = start + haystack[start..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// Règle telle que l'interface l'affiche.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub uid: String,
    pub keyword: String,
    pub whole_word: bool,
    /// "hide" ou "markRead"
    pub action: String,
    pub feed_urls: Vec<String>,
    /// Nombre d'articles actuellement touchés par la règle.
    pub match_count: i64,
}

fn compiled_rules(conn: &Connection) -> Result<Vec<CompiledFilter>> {
    Ok(list(conn)?
        .into_iter()
        .filter_map(|r| {
            Action::parse(&r.action).map(|a| CompiledFilter::new(&r.uid, &r.keyword, r.whole_word, a, &r.feed_urls))
        })
        .collect())
}

/// Toutes les règles, de la plus ancienne à la plus récente.
pub fn list(conn: &Connection) -> Result<Vec<Rule>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.uid, r.keyword, r.whole_word, r.action,
                (SELECT COUNT(*) FROM articles a WHERE a.filtered_by = r.uid)
         FROM filter_rules r ORDER BY r.created_at, r.id",
    )?;
    let rows: Vec<(i64, Rule)> = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                Rule {
                    uid: r.get(1)?,
                    keyword: r.get(2)?,
                    whole_word: r.get::<_, i64>(3)? != 0,
                    action: r.get(4)?,
                    feed_urls: Vec::new(),
                    match_count: r.get(5)?,
                },
            ))
        })?
        .collect::<std::result::Result<_, _>>()?;
    let mut feeds_stmt = conn.prepare("SELECT feed_url FROM filter_rule_feeds WHERE rule_id = ?1 ORDER BY feed_url")?;
    let mut out = Vec::with_capacity(rows.len());
    for (id, mut rule) in rows {
        rule.feed_urls = feeds_stmt.query_map(params![id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        out.push(rule);
    }
    Ok(out)
}

/// Passe les règles sur les articles non encore filtrés (hors favoris).
/// `only` limite aux articles dont la clé est donnée (nouveaux articles).
pub fn apply(conn: &Connection, only: Option<&[String]>) -> Result<usize> {
    let filters = compiled_rules(conn)?;
    if filters.is_empty() {
        return Ok(0);
    }
    let wanted: Option<HashSet<&str>> = only.map(|keys| keys.iter().map(String::as_str).collect());
    let candidates: Vec<(String, String, String)> = {
        let mut stmt =
            conn.prepare("SELECT key, title, feed_url FROM articles WHERE filtered_by IS NULL AND is_starred = 0")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<std::result::Result<_, _>>()?
    };

    let tx = conn.unchecked_transaction()?;
    let mut changed = 0;
    for (key, title, feed_url) in candidates {
        if wanted.as_ref().is_some_and(|w| !w.contains(key.as_str())) {
            continue;
        }
        let Some(rule) = filters.iter().find(|f| f.matches(&title, &feed_url)) else { continue };
        let sql = match rule.action {
            Action::Hide => "UPDATE articles SET filtered_by = ?2, is_hidden = 1 WHERE key = ?1",
            Action::MarkRead => "UPDATE articles SET filtered_by = ?2, is_read = 1 WHERE key = ?1",
        };
        tx.execute(sql, params![key, rule.uid])?;
        changed += 1;
    }
    tx.commit()?;
    Ok(changed)
}

/// Libère les articles filtrés par une règle (ceux déjà marqués comme lus le restent).
fn release(conn: &Connection, uid: &str) -> Result<()> {
    conn.execute("UPDATE articles SET filtered_by = NULL, is_hidden = 0 WHERE filtered_by = ?1", params![uid])?;
    Ok(())
}

fn validate(keyword: &str, action: &str) -> Result<String> {
    let keyword = keyword.trim().to_string();
    if keyword.is_empty() {
        return Err(Error::Other("mot-clé vide".into()));
    }
    if Action::parse(action).is_none() {
        return Err(Error::Other("action inconnue".into()));
    }
    Ok(keyword)
}

fn set_feeds(conn: &Connection, rule_id: i64, feed_urls: &[String]) -> Result<()> {
    conn.execute("DELETE FROM filter_rule_feeds WHERE rule_id = ?1", params![rule_id])?;
    for url in feed_urls {
        conn.execute("INSERT OR IGNORE INTO filter_rule_feeds (rule_id, feed_url) VALUES (?1, ?2)", params![rule_id, url])?;
    }
    Ok(())
}

pub fn add(conn: &Connection, keyword: &str, whole_word: bool, action: &str, feed_urls: &[String]) -> Result<()> {
    let keyword = validate(keyword, action)?;
    let uid = uuid::Uuid::new_v4().to_string().to_uppercase();
    conn.execute(
        "INSERT INTO filter_rules (uid, keyword, whole_word, action, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![uid, keyword, whole_word, action, now()],
    )?;
    set_feeds(conn, conn.last_insert_rowid(), feed_urls)?;
    apply(conn, None)?;
    Ok(())
}

pub fn update(conn: &Connection, uid: &str, keyword: &str, whole_word: bool, action: &str, feed_urls: &[String]) -> Result<()> {
    let keyword = validate(keyword, action)?;
    let id: i64 = conn
        .query_row("SELECT id FROM filter_rules WHERE uid = ?1", params![uid], |r| r.get(0))
        .optional()?
        .ok_or_else(|| Error::Other("règle introuvable".into()))?;
    release(conn, uid)?;
    conn.execute(
        "UPDATE filter_rules SET keyword = ?2, whole_word = ?3, action = ?4 WHERE id = ?1",
        params![id, keyword, whole_word, action],
    )?;
    set_feeds(conn, id, feed_urls)?;
    apply(conn, None)?;
    Ok(())
}

pub fn delete(conn: &Connection, uid: &str) -> Result<()> {
    release(conn, uid)?;
    conn.execute("DELETE FROM filter_rules WHERE uid = ?1", params![uid])?;
    apply(conn, None)?; // une autre règle peut encore s'appliquer
    Ok(())
}

/// Aperçu d'une règle pas encore enregistrée : nombre d'articles (hors favoris) dont le titre correspond.
pub fn preview_count(conn: &Connection, keyword: &str, whole_word: bool, feed_urls: &[String]) -> Result<i64> {
    let filter = CompiledFilter::new("", keyword, whole_word, Action::Hide, feed_urls);
    let mut stmt = conn.prepare("SELECT title, feed_url FROM articles WHERE is_starred = 0")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut count = 0;
    for row in rows {
        let (title, feed_url) = row?;
        if filter.matches(&title, &feed_url) {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let c = crate::db::open_in_memory().unwrap();
        c.execute_batch(
            "INSERT INTO feeds (id, url, title, added_at) VALUES (1, 'https://a/f', 'A', 0), (2, 'https://b/f', 'B', 0);
             INSERT INTO articles (key, feed_id, feed_url, title, published, is_starred) VALUES
               ('k1', 1, 'https://a/f', 'Élection : le grand soir', 1, 0),
               ('k2', 1, 'https://a/f', 'Sélections sportives', 2, 0),
               ('k3', 2, 'https://b/f', 'Résultats de l''ELECTION', 3, 0),
               ('k4', 1, 'https://a/f', 'Election en direct', 4, 1);",
        )
        .unwrap();
        c
    }

    fn keys(c: &Connection, cond: &str) -> Vec<String> {
        let mut s = c.prepare(&format!("SELECT key FROM articles WHERE {cond} ORDER BY key")).unwrap();
        s.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
    }

    #[test]
    fn whole_word_ignores_inner_matches() {
        assert!(contains_whole_word("election en direct", "election"));
        assert!(!contains_whole_word("selections sportives", "election"));
        assert!(contains_whole_word("a l'election.", "election"));
        assert!(contains_whole_word("selections election", "election"));
    }

    #[test]
    fn rule_hides_matching_titles_except_favorites() {
        let c = setup();
        add(&c, "élection", true, "hide", &[]).unwrap();
        assert_eq!(keys(&c, "is_hidden = 1"), vec!["k1", "k3"]);
        let rules = list(&c).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].match_count, 2);
    }

    #[test]
    fn substring_rule_scoped_to_a_feed() {
        let c = setup();
        add(&c, "election", false, "markRead", &["https://a/f".to_string()]).unwrap();
        // k1 et k2 (« Sélections » contient « election »), pas k3 (autre flux) ni k4 (favori)
        assert_eq!(keys(&c, "is_read = 1"), vec!["k1", "k2"]);
        assert_eq!(preview_count(&c, "election", false, &[]).unwrap(), 3);
    }

    #[test]
    fn deleting_a_rule_releases_hidden_articles() {
        let c = setup();
        add(&c, "election", true, "hide", &[]).unwrap();
        let uid = list(&c).unwrap()[0].uid.clone();
        delete(&c, &uid).unwrap();
        assert!(keys(&c, "is_hidden = 1").is_empty());
        assert!(list(&c).unwrap().is_empty());
    }

    #[test]
    fn apply_only_touches_given_keys() {
        let c = setup();
        c.execute(
            "INSERT INTO filter_rules (uid, keyword, whole_word, action, created_at) VALUES ('U', 'election', 1, 'hide', 0)",
            [],
        )
        .unwrap();
        apply(&c, Some(&["k1".to_string()])).unwrap();
        assert_eq!(keys(&c, "is_hidden = 1"), vec!["k1"]);
    }
}
