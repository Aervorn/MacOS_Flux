//! Dossiers intelligents : des dossiers qui rangent les articles par mots-clés.
//!
//! Un dossier a une liste de mots-clés (un seul suffit : « ou ») cherchés dans le titre des articles,
//! sans tenir compte des majuscules ni des accents, avec l'option « mot entier » (comme les règles
//! d'exclusion). Un article masqué par une règle d'exclusion n'est dans aucun dossier. Un article qui
//! correspond à plusieurs dossiers est dans chacun d'eux.
//!
//! L'appartenance est stockée (`smart_folder_articles`) pour que les listes et les compteurs la
//! lisent en SQL : un article rangé dans un dossier intelligent n'apparaît plus dans son flux ni
//! dans le dossier de son flux (il reste dans « Tous les articles » et « Non lus »).
//! `rebuild` la recalcule en entier ; `filters::apply` l'appelle, donc elle suit les arrivées
//! d'articles, les règles d'exclusion et la synchro.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::feeds::now;
use crate::filters::contains_whole_word;
use crate::query::fold;

/// Dossier intelligent tel que l'interface l'affiche.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartFolder {
    pub uid: String,
    pub name: String,
    pub keywords: Vec<String>,
    pub whole_word: bool,
    pub symbol: Option<String>,
    pub color_name: Option<String>,
    /// Nombre d'articles actuellement rangés dans le dossier.
    pub article_count: i64,
}

/// Mots-clés nettoyés : sans espaces autour, sans vides, sans doublons (à la casse et aux accents près).
pub(crate) fn clean_keywords(keywords: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    keywords
        .iter()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty() && seen.insert(fold(k)))
        .collect()
}

fn matches(folded_title: &str, needles: &[String], whole_word: bool) -> bool {
    needles.iter().any(|n| {
        if whole_word {
            contains_whole_word(folded_title, n)
        } else {
            folded_title.contains(n.as_str())
        }
    })
}

/// Tous les dossiers, du plus ancien au plus récent.
pub fn list(conn: &Connection) -> Result<Vec<SmartFolder>> {
    let mut stmt = conn.prepare(
        "SELECT d.id, d.uid, d.name, d.whole_word, d.symbol, d.color_name,
                (SELECT COUNT(*) FROM smart_folder_articles s WHERE s.folder_id = d.id)
         FROM smart_folders d ORDER BY d.created_at, d.id",
    )?;
    let rows: Vec<(i64, SmartFolder)> = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                SmartFolder {
                    uid: r.get(1)?,
                    name: r.get(2)?,
                    keywords: Vec::new(),
                    whole_word: r.get::<_, i64>(3)? != 0,
                    symbol: r.get(4)?,
                    color_name: r.get(5)?,
                    article_count: r.get(6)?,
                },
            ))
        })?
        .collect::<std::result::Result<_, _>>()?;
    let mut kw = conn.prepare("SELECT keyword FROM smart_folder_keywords WHERE folder_id = ?1 ORDER BY rowid")?;
    let mut out = Vec::with_capacity(rows.len());
    for (id, mut folder) in rows {
        folder.keywords = kw.query_map(params![id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        out.push(folder);
    }
    Ok(out)
}

fn validate(name: &str, keywords: &[String]) -> Result<(String, Vec<String>)> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(Error::Other("nom vide".into()));
    }
    let keywords = clean_keywords(keywords);
    if keywords.is_empty() {
        return Err(Error::Other("aucun mot-clé".into()));
    }
    Ok((name, keywords))
}

pub(crate) fn set_keywords(conn: &Connection, folder_id: i64, keywords: &[String]) -> Result<()> {
    conn.execute("DELETE FROM smart_folder_keywords WHERE folder_id = ?1", params![folder_id])?;
    for keyword in keywords {
        conn.execute(
            "INSERT OR IGNORE INTO smart_folder_keywords (folder_id, keyword) VALUES (?1, ?2)",
            params![folder_id, keyword],
        )?;
    }
    Ok(())
}

/// Crée un dossier et renvoie son uid.
pub fn create(conn: &Connection, name: &str, keywords: &[String], whole_word: bool) -> Result<String> {
    let (name, keywords) = validate(name, keywords)?;
    let uid = uuid::Uuid::new_v4().to_string().to_uppercase();
    conn.execute(
        "INSERT INTO smart_folders (uid, name, whole_word, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![uid, name, whole_word, now()],
    )?;
    set_keywords(conn, conn.last_insert_rowid(), &keywords)?;
    rebuild(conn)?;
    Ok(uid)
}

pub fn update(conn: &Connection, uid: &str, name: &str, keywords: &[String], whole_word: bool) -> Result<()> {
    let (name, keywords) = validate(name, keywords)?;
    let id: i64 = conn
        .query_row("SELECT id FROM smart_folders WHERE uid = ?1", params![uid], |r| r.get(0))
        .optional()?
        .ok_or_else(|| Error::Other("dossier intelligent introuvable".into()))?;
    conn.execute("UPDATE smart_folders SET name = ?2, whole_word = ?3 WHERE id = ?1", params![id, name, whole_word])?;
    set_keywords(conn, id, &keywords)?;
    rebuild(conn)
}

/// Les articles rangés retrouvent leur flux (l'appartenance se supprime avec le dossier).
pub fn delete(conn: &Connection, uid: &str) -> Result<()> {
    conn.execute("DELETE FROM smart_folders WHERE uid = ?1", params![uid])?;
    Ok(())
}

/// Change l'icône et la couleur (None : valeur par défaut), comme pour les dossiers de flux.
pub fn set_style(conn: &Connection, uid: &str, symbol: Option<&str>, color: Option<&str>) -> Result<()> {
    let clean = |v: Option<&str>| v.map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
    let changed = conn.execute(
        "UPDATE smart_folders SET symbol = ?2, color_name = ?3 WHERE uid = ?1",
        params![uid, clean(symbol), clean(color)],
    )?;
    if changed == 0 {
        return Err(Error::Other("dossier intelligent introuvable".into()));
    }
    Ok(())
}

/// Recalcule l'appartenance de tous les articles non masqués (les titres seulement : c'est léger).
pub fn rebuild(conn: &Connection) -> Result<()> {
    let folders: Vec<(i64, Vec<String>, bool)> = {
        let mut stmt = conn.prepare("SELECT id, whole_word FROM smart_folders")?;
        let heads: Vec<(i64, bool)> = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)? != 0)))?
            .collect::<std::result::Result<_, _>>()?;
        let mut kw = conn.prepare("SELECT keyword FROM smart_folder_keywords WHERE folder_id = ?1")?;
        let mut out = Vec::with_capacity(heads.len());
        for (id, whole_word) in heads {
            let words: Vec<String> = kw.query_map(params![id], |r| r.get::<_, String>(0))?.collect::<std::result::Result<_, _>>()?;
            let needles: Vec<String> = words.iter().map(|w| fold(w.trim())).filter(|w| !w.is_empty()).collect();
            out.push((id, needles, whole_word));
        }
        out
    };

    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM smart_folder_articles", [])?;
    if !folders.is_empty() {
        let articles: Vec<(i64, String)> = {
            let mut stmt = tx.prepare("SELECT id, title FROM articles WHERE is_hidden = 0")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let mut insert = tx.prepare("INSERT OR IGNORE INTO smart_folder_articles (folder_id, article_id) VALUES (?1, ?2)")?;
        for (article_id, title) in articles {
            let folded = fold(&title);
            for (folder_id, needles, whole_word) in &folders {
                if matches(&folded, needles, *whole_word) {
                    insert.execute(params![folder_id, article_id])?;
                }
            }
        }
    }
    tx.commit()?;
    Ok(())
}

/// Aperçu d'un dossier pas encore enregistré : nombre d'articles (non masqués) dont le titre correspond.
pub fn preview_count(conn: &Connection, keywords: &[String], whole_word: bool) -> Result<i64> {
    let needles: Vec<String> = clean_keywords(keywords).iter().map(|k| fold(k)).collect();
    if needles.is_empty() {
        return Ok(0);
    }
    let mut stmt = conn.prepare("SELECT title FROM articles WHERE is_hidden = 0")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut count = 0;
    for row in rows {
        if matches(&fold(&row?), &needles, whole_word) {
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
            "INSERT INTO folders (id, uid, name, created_at) VALUES (1, 'F1', 'Actu', 0);
             INSERT INTO feeds (id, url, title, added_at, folder_id) VALUES (1, 'https://a/f', 'A', 0, 1), (2, 'https://b/f', 'B', 0, NULL);
             INSERT INTO articles (id, key, feed_id, feed_url, title, published) VALUES
               (1, 'k1', 1, 'https://a/f', 'Élection : le grand soir', 1),
               (2, 'k2', 1, 'https://a/f', 'Sélections sportives', 2),
               (3, 'k3', 2, 'https://b/f', 'Un nouvel iPhone arrive', 3),
               (4, 'k4', 2, 'https://b/f', 'Météo du week-end', 4);",
        )
        .unwrap();
        c
    }

    fn members(c: &Connection, uid: &str) -> Vec<String> {
        let mut s = c
            .prepare(
                "SELECT a.key FROM smart_folder_articles s JOIN articles a ON a.id = s.article_id
                 JOIN smart_folders d ON d.id = s.folder_id WHERE d.uid = ?1 ORDER BY a.key",
            )
            .unwrap();
        s.query_map(params![uid], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
    }

    fn words(w: &[&str]) -> Vec<String> {
        w.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn any_keyword_matches_ignoring_case_and_accents() {
        let c = setup();
        let uid = create(&c, "Politique", &words(&["ELECTION", "iphone"]), true).unwrap();
        assert_eq!(members(&c, &uid), vec!["k1", "k3"]);
        let folders = list(&c).unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].keywords, words(&["ELECTION", "iphone"]));
        assert_eq!(folders[0].article_count, 2);
    }

    #[test]
    fn whole_word_option_controls_inner_matches() {
        let c = setup();
        let strict = create(&c, "Mot entier", &words(&["election"]), true).unwrap();
        let loose = create(&c, "Partout", &words(&["election"]), false).unwrap();
        assert_eq!(members(&c, &strict), vec!["k1"]);
        assert_eq!(members(&c, &loose), vec!["k1", "k2"]);
    }

    #[test]
    fn an_article_can_be_in_several_folders() {
        let c = setup();
        let a = create(&c, "A", &words(&["election"]), true).unwrap();
        let b = create(&c, "B", &words(&["grand"]), true).unwrap();
        assert_eq!(members(&c, &a), vec!["k1"]);
        assert_eq!(members(&c, &b), vec!["k1"]);
    }

    #[test]
    fn hidden_articles_are_not_in_any_folder() {
        let c = setup();
        let uid = create(&c, "Politique", &words(&["election"]), true).unwrap();
        crate::filters::add(&c, "grand soir", false, "hide", &[]).unwrap(); // masque k1 et relance rebuild
        assert!(members(&c, &uid).is_empty());
        let rule = crate::filters::list(&c).unwrap()[0].uid.clone();
        crate::filters::delete(&c, &rule).unwrap(); // k1 réapparaît, donc revient dans le dossier
        assert_eq!(members(&c, &uid), vec!["k1"]);
    }

    #[test]
    fn update_and_delete_follow_the_keywords() {
        let c = setup();
        let uid = create(&c, "Tech", &words(&["iphone"]), true).unwrap();
        update(&c, &uid, "Météo", &words(&["meteo"]), true).unwrap();
        assert_eq!(members(&c, &uid), vec!["k4"]);
        assert_eq!(list(&c).unwrap()[0].name, "Météo");
        delete(&c, &uid).unwrap();
        assert!(list(&c).unwrap().is_empty());
        let n: i64 = c.query_row("SELECT COUNT(*) FROM smart_folder_articles", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn rejects_empty_name_or_keywords_and_dedupes() {
        let c = setup();
        assert!(create(&c, "  ", &words(&["a"]), true).is_err());
        assert!(create(&c, "Vide", &words(&["  ", ""]), true).is_err());
        let uid = create(&c, "Doublons", &words(&["Météo", "meteo", " METEO "]), true).unwrap();
        assert_eq!(list(&c).unwrap()[0].keywords, words(&["Météo"]));
        assert_eq!(members(&c, &uid), vec!["k4"]);
    }

    #[test]
    fn preview_counts_current_titles() {
        let c = setup();
        assert_eq!(preview_count(&c, &words(&["election", "meteo"]), true).unwrap(), 2);
        assert_eq!(preview_count(&c, &words(&[]), true).unwrap(), 0);
    }
}
