//! Import de `Flux.store` (SwiftData / Core Data) vers la base SQLite de Flux.
//!
//! Le store Swift n'est jamais modifié : il est ouvert en lecture seule.
//! Rappels sur le format :
//! - tables `ZARTICLE`, `ZFEED`, `ZFOLDER`, `ZFILTERRULE`, `ZFAVORITETAG` ;
//! - relations par entier (`Z_PK`), conservés ici comme identifiants ;
//! - dates en secondes depuis le 1er janvier 2001 (converties en secondes Unix) ;
//! - tags d'un article dans `ZTAGSRAW`, sous la forme `,uid1,uid2,` ;
//! - portée d'une règle de filtre dans `ZFEEDURLS` (NSKeyedArchiver, voir `nskeyed`),
//!   avec `ZFEEDURL` comme ancien champ à un seul flux.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection, OpenFlags};

use super::{nskeyed, ImportReport};
use crate::error::{Error, Result};

/// Secondes entre 1970-01-01 et 2001-01-01.
const CORE_DATA_EPOCH: f64 = 978_307_200.0;

fn unix(v: Option<f64>) -> Option<f64> {
    v.map(|s| s + CORE_DATA_EPOCH)
}

fn flag(v: Option<i64>) -> i64 {
    i64::from(v.unwrap_or(0) != 0)
}

/// Importe tout le contenu du store dans `dst`, qui doit être vide.
pub fn import_store(src_path: &Path, dst: &mut Connection) -> Result<ImportReport> {
    let src = Connection::open_with_flags(src_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;

    let has_articles: i64 = src.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'ZARTICLE'",
        [],
        |r| r.get(0),
    )?;
    if has_articles == 0 {
        return Err(Error::Other("ce fichier n'est pas un Flux.store (table ZARTICLE absente)".into()));
    }

    let existing: i64 = dst.query_row(
        "SELECT (SELECT COUNT(*) FROM feeds) + (SELECT COUNT(*) FROM articles)",
        [],
        |r| r.get(0),
    )?;
    if existing > 0 {
        return Err(Error::Other("la base de destination contient déjà des données".into()));
    }

    let tx = dst.transaction()?;
    let mut report = ImportReport::default();

    // --- Dossiers ---------------------------------------------------------
    let mut folder_ids: HashSet<i64> = HashSet::new();
    {
        let mut stmt = src.prepare(
            "SELECT Z_PK, ZUID, ZNAME, ZISEXPANDED, ZCREATEDAT, ZSYMBOL, ZCOLORNAME FROM ZFOLDER ORDER BY Z_PK",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let uid = row.get::<_, Option<String>>(1)?.unwrap_or_else(|| format!("imported-folder-{id}"));
            let name = row.get::<_, Option<String>>(2)?.unwrap_or_default();
            let expanded = flag(row.get::<_, Option<i64>>(3)?);
            let created = unix(row.get::<_, Option<f64>>(4)?).unwrap_or(0.0);
            let symbol: Option<String> = row.get(5)?;
            let color: Option<String> = row.get(6)?;
            tx.execute(
                "INSERT INTO folders (id, uid, name, is_expanded, created_at, symbol, color_name)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id, uid, name, expanded, created, symbol, color],
            )?;
            folder_ids.insert(id);
            report.folders += 1;
        }
    }

    // --- Flux -------------------------------------------------------------
    let mut feed_ids: HashSet<i64> = HashSet::new();
    {
        let mut stmt = src.prepare(
            "SELECT Z_PK, ZURL, ZTITLE, ZSITEURL, ZADDEDAT, ZLASTREFRESHED, ZLASTERROR, ZFOLDER,
                    ZCUSTOMTITLE, ZPREVIOUSURL
             FROM ZFEED ORDER BY Z_PK",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let Some(url) = row.get::<_, Option<String>>(1)? else {
                report.warnings.push(format!("flux {id} sans adresse : ignoré"));
                continue;
            };
            let title = row.get::<_, Option<String>>(2)?.unwrap_or_default();
            let site_url: Option<String> = row.get(3)?;
            let added = unix(row.get::<_, Option<f64>>(4)?).unwrap_or(0.0);
            let refreshed = unix(row.get::<_, Option<f64>>(5)?);
            let last_error: Option<String> = row.get(6)?;
            let mut folder: Option<i64> = row.get(7)?;
            if let Some(f) = folder {
                if !folder_ids.contains(&f) {
                    report.warnings.push(format!("flux « {url} » : dossier {f} introuvable, remis à la racine"));
                    folder = None;
                }
            }
            let custom_title: Option<String> = row.get(8)?;
            let previous_url: Option<String> = row.get(9)?;
            tx.execute(
                "INSERT INTO feeds (id, url, title, site_url, added_at, last_refreshed, last_error,
                                    folder_id, custom_title, previous_url)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![id, url, title, site_url, added, refreshed, last_error, folder, custom_title, previous_url],
            )?;
            feed_ids.insert(id);
            report.feeds += 1;
        }
    }

    // --- Tags de favoris --------------------------------------------------
    let mut tag_ids: HashMap<String, i64> = HashMap::new();
    {
        let mut stmt = src.prepare("SELECT Z_PK, ZUID, ZNAME, ZCREATEDAT FROM ZFAVORITETAG ORDER BY Z_PK")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let Some(uid) = row.get::<_, Option<String>>(1)? else {
                report.warnings.push(format!("tag {id} sans identifiant : ignoré"));
                continue;
            };
            let name = row.get::<_, Option<String>>(2)?.unwrap_or_default();
            let created = unix(row.get::<_, Option<f64>>(3)?).unwrap_or(0.0);
            tx.execute(
                "INSERT INTO favorite_tags (id, uid, name, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![id, uid, name, created],
            )?;
            tag_ids.insert(uid, id);
            report.favorite_tags += 1;
        }
    }

    // --- Articles ---------------------------------------------------------
    {
        let mut stmt = src.prepare(
            "SELECT Z_PK, ZKEY, ZFEED, ZFEEDURL, ZTITLE, ZLINK, ZSUMMARY, ZCONTENTHTML, ZFULLHTML,
                    ZSEARCHTEXT, ZISHIDDEN, ZFILTEREDBY, ZISARCHIVED, ZOPENEDAT, ZREADINGSECONDS,
                    ZWORDCOUNT, ZCLUSTERID, ZISDUPLICATE, ZCLUSTEROPTOUT, ZSOURCETITLE, ZIMAGEURL,
                    ZAUTHOR, ZPUBLISHED, ZISREAD, ZISSTARRED, ZTAGSRAW
             FROM ZARTICLE ORDER BY Z_PK",
        )?;
        let mut insert = tx.prepare(
            "INSERT INTO articles (id, key, feed_id, feed_url, title, link, summary, content_html, full_html,
                                  search_text, is_hidden, filtered_by, is_archived, opened_at, reading_seconds,
                                  word_count, cluster_id, is_duplicate, cluster_opt_out, source_title, image_url,
                                  author, published, is_read, is_starred)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
                     ?20, ?21, ?22, ?23, ?24, ?25)",
        )?;
        let mut insert_tag = tx.prepare("INSERT OR IGNORE INTO article_tags (article_id, tag_id) VALUES (?1, ?2)")?;

        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let Some(key) = row.get::<_, Option<String>>(1)? else {
                report.warnings.push(format!("article {id} sans clé : ignoré"));
                continue;
            };
            let mut feed: Option<i64> = row.get(2)?;
            if let Some(f) = feed {
                if !feed_ids.contains(&f) {
                    report.warnings.push(format!("article {id} : flux {f} introuvable, détaché"));
                    feed = None;
                }
            }
            let feed_url = row.get::<_, Option<String>>(3)?.unwrap_or_default();
            let title = row.get::<_, Option<String>>(4)?.unwrap_or_default();
            let link: Option<String> = row.get(5)?;
            let summary = row.get::<_, Option<String>>(6)?.unwrap_or_default();
            let content_html: Option<String> = row.get(7)?;
            let full_html: Option<String> = row.get(8)?;
            let search_text = row.get::<_, Option<String>>(9)?.unwrap_or_default();
            let hidden = flag(row.get::<_, Option<i64>>(10)?);
            let filtered_by: Option<String> = row.get(11)?;
            let archived = flag(row.get::<_, Option<i64>>(12)?);
            let opened = unix(row.get::<_, Option<f64>>(13)?);
            let reading = row.get::<_, Option<f64>>(14)?.unwrap_or(0.0);
            let words = row.get::<_, Option<i64>>(15)?.unwrap_or(0);
            let cluster: Option<String> = row.get(16)?;
            let duplicate = flag(row.get::<_, Option<i64>>(17)?);
            let opt_out = flag(row.get::<_, Option<i64>>(18)?);
            let source_title: Option<String> = row.get(19)?;
            let image_url: Option<String> = row.get(20)?;
            let author: Option<String> = row.get(21)?;
            let published = unix(row.get::<_, Option<f64>>(22)?).unwrap_or(0.0);
            let read = flag(row.get::<_, Option<i64>>(23)?);
            let starred = flag(row.get::<_, Option<i64>>(24)?);
            let tags_raw = row.get::<_, Option<String>>(25)?.unwrap_or_default();

            insert.execute(params![
                id, key, feed, feed_url, title, link, summary, content_html, full_html, search_text, hidden,
                filtered_by, archived, opened, reading, words, cluster, duplicate, opt_out, source_title,
                image_url, author, published, read, starred
            ])?;
            report.articles += 1;

            for uid in tags_raw.split(',').filter(|s| !s.is_empty()) {
                match tag_ids.get(uid) {
                    Some(tag_id) => {
                        insert_tag.execute(params![id, tag_id])?;
                        report.article_tags += 1;
                    }
                    None => report.warnings.push(format!("article {id} : tag « {uid} » inconnu, ignoré")),
                }
            }
        }
    }

    // --- Règles de filtrage -----------------------------------------------
    {
        let mut stmt = src.prepare(
            "SELECT Z_PK, ZUID, ZKEYWORD, ZWHOLEWORD, ZACTION, ZCREATEDAT, ZFEEDURL, ZFEEDURLS
             FROM ZFILTERRULE ORDER BY Z_PK",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let uid = row.get::<_, Option<String>>(1)?.unwrap_or_else(|| format!("imported-rule-{id}"));
            let keyword = row.get::<_, Option<String>>(2)?.unwrap_or_default();
            let whole_word = flag(row.get::<_, Option<i64>>(3)?);
            let action = row.get::<_, Option<String>>(4)?.unwrap_or_else(|| "hide".into());
            let created = unix(row.get::<_, Option<f64>>(5)?).unwrap_or(0.0);
            let legacy_url: Option<String> = row.get(6)?;
            let blob: Option<Vec<u8>> = row.get(7)?;

            // Portée : liste de flux (vide = tous). Si elle est vide, l'ancien champ à un
            // seul flux sert de repli (même migration que FeedStore.migrateFilterScopes).
            let mut scope: Vec<String> = match blob.as_deref() {
                Some(bytes) if !bytes.is_empty() => match nskeyed::decode_string_array(bytes) {
                    Ok(urls) => urls,
                    Err(e) => {
                        report.warnings.push(format!(
                            "règle « {keyword} » : portée illisible ({e}), appliquée à tous les flux"
                        ));
                        Vec::new()
                    }
                },
                _ => Vec::new(),
            };
            if scope.is_empty() {
                if let Some(url) = legacy_url.filter(|u| !u.is_empty()) {
                    scope.push(url);
                }
            }

            tx.execute(
                "INSERT INTO filter_rules (id, uid, keyword, whole_word, action, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, uid, keyword, whole_word, action, created],
            )?;
            for url in scope {
                tx.execute(
                    "INSERT OR IGNORE INTO filter_rule_feeds (rule_id, feed_url) VALUES (?1, ?2)",
                    params![id, url],
                )?;
            }
            report.filter_rules += 1;
        }
    }

    tx.commit()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    /// Store minimal, avec les mêmes tables que SwiftData (voir schema.txt de l'analyse).
    fn fake_store(path: &Path) {
        let c = Connection::open(path).unwrap();
        c.execute_batch(
            "CREATE TABLE ZARTICLE ( Z_PK INTEGER PRIMARY KEY, Z_ENT INTEGER, Z_OPT INTEGER, ZCLUSTEROPTOUT INTEGER, ZISARCHIVED INTEGER, ZISDUPLICATE INTEGER, ZISHIDDEN INTEGER, ZISREAD INTEGER, ZISSTARRED INTEGER, ZWORDCOUNT INTEGER, ZFEED INTEGER, ZOPENEDAT TIMESTAMP, ZPUBLISHED TIMESTAMP, ZREADINGSECONDS FLOAT, ZAUTHOR VARCHAR, ZCLUSTERID VARCHAR, ZCONTENTHTML VARCHAR, ZFEEDURL VARCHAR, ZFILTEREDBY VARCHAR, ZFULLHTML VARCHAR, ZIMAGEURL VARCHAR, ZKEY VARCHAR, ZLINK VARCHAR, ZSEARCHTEXT VARCHAR, ZSOURCETITLE VARCHAR, ZSUMMARY VARCHAR, ZTITLE VARCHAR, ZTAGSRAW VARCHAR);
             CREATE TABLE ZFEED ( Z_PK INTEGER PRIMARY KEY, Z_ENT INTEGER, Z_OPT INTEGER, ZFOLDER INTEGER, ZADDEDAT TIMESTAMP, ZLASTREFRESHED TIMESTAMP, ZCUSTOMTITLE VARCHAR, ZLASTERROR VARCHAR, ZSITEURL VARCHAR, ZTITLE VARCHAR, ZURL VARCHAR, ZPREVIOUSURL VARCHAR);
             CREATE TABLE ZFOLDER ( Z_PK INTEGER PRIMARY KEY, Z_ENT INTEGER, Z_OPT INTEGER, ZISEXPANDED INTEGER, ZCREATEDAT TIMESTAMP, ZNAME VARCHAR, ZUID VARCHAR, ZCOLORNAME VARCHAR, ZSYMBOL VARCHAR);
             CREATE TABLE ZFILTERRULE ( Z_PK INTEGER PRIMARY KEY, Z_ENT INTEGER, Z_OPT INTEGER, ZWHOLEWORD INTEGER, ZCREATEDAT TIMESTAMP, ZACTION VARCHAR, ZFEEDURL VARCHAR, ZKEYWORD VARCHAR, ZUID VARCHAR, ZFEEDURLS BLOB);
             CREATE TABLE ZFAVORITETAG ( Z_PK INTEGER PRIMARY KEY, Z_ENT INTEGER, Z_OPT INTEGER, ZCREATEDAT TIMESTAMP, ZNAME VARCHAR, ZUID VARCHAR);

             INSERT INTO ZFOLDER (Z_PK, ZISEXPANDED, ZCREATEDAT, ZNAME, ZUID, ZCOLORNAME, ZSYMBOL)
                 VALUES (1, 1, 1000, 'Tech', 'F-1', 'blue', 'desktopcomputer');
             INSERT INTO ZFEED (Z_PK, ZFOLDER, ZADDEDAT, ZTITLE, ZURL, ZSITEURL)
                 VALUES (1, 1, 2000, 'Korben', 'https://korben.info/feed', 'https://korben.info'),
                        (2, NULL, 2000.5, 'Autre', 'https://autre.example/rss', NULL);
             INSERT INTO ZFAVORITETAG (Z_PK, ZCREATEDAT, ZNAME, ZUID) VALUES (1, 3000, 'IA', 'T-1');
             INSERT INTO ZARTICLE (Z_PK, ZKEY, ZFEED, ZFEEDURL, ZTITLE, ZSUMMARY, ZPUBLISHED, ZISREAD, ZISSTARRED,
                                   ZISARCHIVED, ZWORDCOUNT, ZREADINGSECONDS, ZTAGSRAW)
                 VALUES (1, 'https://korben.info/feed|a', 1, 'https://korben.info/feed', 'Titre A', 'Résumé', 4000, 1, 1, 1, 120, 33.5, ',T-1,'),
                        (2, 'https://autre.example/rss|b', 2, 'https://autre.example/rss', 'Titre B', 'Résumé', 4001, 0, 0, 0, 0, 0, '');
             INSERT INTO ZARTICLE (Z_PK, ZKEY, ZFEED, ZFEEDURL, ZTITLE, ZPUBLISHED, ZTAGSRAW)
                 VALUES (3, 'orphan|c', 99, 'orphan', 'Orpheline', 4002, ',T-inconnu,');",
        )
        .unwrap();

        let scoped = nskeyed::encode_string_array(&["https://korben.info/feed", "https://autre.example/rss"]);
        let all = nskeyed::encode_string_array(&[]);
        c.execute(
            "INSERT INTO ZFILTERRULE (Z_PK, ZWHOLEWORD, ZCREATEDAT, ZACTION, ZKEYWORD, ZUID, ZFEEDURLS)
             VALUES (1, 1, 5000, 'hide', 'Apple Watch', 'R-1', ?1), (2, 0, 5001, 'markRead', 'Promotion', 'R-2', ?2)",
            params![scoped, all],
        )
        .unwrap();
        c.execute(
            "INSERT INTO ZFILTERRULE (Z_PK, ZWHOLEWORD, ZCREATEDAT, ZACTION, ZKEYWORD, ZUID, ZFEEDURL)
             VALUES (3, 1, 5002, 'hide', 'Ancien', 'R-3', 'https://korben.info/feed')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn imports_a_whole_store() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("Flux.store");
        fake_store(&src);
        let mut dst = db::open_in_memory().unwrap();

        let r = import_store(&src, &mut dst).unwrap();
        assert_eq!((r.folders, r.feeds, r.favorite_tags, r.articles, r.article_tags, r.filter_rules), (1, 2, 1, 3, 1, 3));

        // Dates : secondes depuis 2001 → secondes Unix
        let added: f64 = dst.query_row("SELECT added_at FROM feeds WHERE id = 2", [], |r| r.get(0)).unwrap();
        assert!((added - (2000.5 + CORE_DATA_EPOCH)).abs() < 1e-6);

        // Relations conservées
        let folder: Option<i64> = dst.query_row("SELECT folder_id FROM feeds WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(folder, Some(1));

        // Article favori, archivé, avec son tag
        let (starred, archived, words): (i64, i64, i64) = dst
            .query_row("SELECT is_starred, is_archived, word_count FROM articles WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!((starred, archived, words), (1, 1, 120));
        let tag: String = dst
            .query_row(
                "SELECT t.name FROM article_tags a JOIN favorite_tags t ON t.id = a.tag_id WHERE a.article_id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tag, "IA");

        // Article orphelin : détaché, tag inconnu ignoré, avertissements
        let feed: Option<i64> = dst.query_row("SELECT feed_id FROM articles WHERE id = 3", [], |r| r.get(0)).unwrap();
        assert_eq!(feed, None);
        assert_eq!(r.warnings.len(), 2);
    }

    #[test]
    fn filter_rule_scopes() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("Flux.store");
        fake_store(&src);
        let mut dst = db::open_in_memory().unwrap();
        import_store(&src, &mut dst).unwrap();

        let count = |rule: i64| -> i64 {
            dst.query_row("SELECT COUNT(*) FROM filter_rule_feeds WHERE rule_id = ?1", [rule], |r| r.get(0)).unwrap()
        };
        assert_eq!(count(1), 2); // deux flux
        assert_eq!(count(2), 0); // tableau vide = tous les flux
        assert_eq!(count(3), 1); // ancien champ à un seul flux
    }

    #[test]
    fn refuses_a_non_empty_destination_and_a_foreign_file() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("Flux.store");
        fake_store(&src);
        let mut dst = db::open_in_memory().unwrap();
        import_store(&src, &mut dst).unwrap();
        assert!(import_store(&src, &mut dst).is_err());

        let other = dir.path().join("autre.sqlite");
        Connection::open(&other).unwrap().execute_batch("CREATE TABLE t (x)").unwrap();
        let mut fresh = db::open_in_memory().unwrap();
        assert!(import_store(&other, &mut fresh).is_err());
    }

    /// Test sur le vrai store (copie) : à lancer à la main.
    /// FLUX_STORE_COPY=~/Developer/flux-analysis/Flux-copy.store \
    ///   cargo test -p flux-core real_store -- --ignored --nocapture
    #[test]
    #[ignore]
    fn real_store() {
        let path = std::env::var("FLUX_STORE_COPY").expect("définir FLUX_STORE_COPY");
        let mut dst = db::open_in_memory().unwrap();
        let r = import_store(Path::new(&path), &mut dst).unwrap();
        println!("{r:#?}");
        assert_eq!((r.folders, r.feeds, r.favorite_tags, r.articles, r.filter_rules), (4, 10, 3, 259, 2));
        assert!(r.warnings.is_empty(), "avertissements : {:?}", r.warnings);
        let starred: i64 = dst.query_row("SELECT COUNT(*) FROM articles WHERE is_starred = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(starred, 11);
    }
}
