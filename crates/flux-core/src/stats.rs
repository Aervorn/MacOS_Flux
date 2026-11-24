//! Statistiques de lecture : articles ouverts, temps passé, sources les plus lues.
//!
//! Les dates sont en secondes (UNIX). Le découpage par jour tient compte du fuseau
//! horaire de l'appelant (`tz_offset`, en secondes, positif à l'est de Greenwich).

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::Result;
use crate::query::source_name;

/// Vitesse de lecture utilisée pour le « temps estimé » (mots par minute), comme côté Swift.
const WORDS_PER_MINUTE: f64 = 230.0;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCount {
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// Articles ouverts sur la période
    pub read_count: u32,
    /// Articles publiés sur la période (hors masqués)
    pub received_count: u32,
    pub reading_seconds: f64,
    pub estimated_seconds: f64,
    pub per_day: f64,
    /// Un nombre d'articles ouverts par jour, le dernier élément étant aujourd'hui
    pub daily: Vec<u32>,
    pub most_read: Vec<SourceCount>,
    pub most_active: Vec<SourceCount>,
    /// Date (secondes) de la toute première ouverture enregistrée
    pub tracking_since: Option<f64>,
}

pub fn compute(conn: &Connection, days: u32, now: f64, tz_offset: i64) -> Result<Stats> {
    let days = days.max(1);
    let today = ((now as i64) + tz_offset).div_euclid(86_400);
    let start_day = today - i64::from(days) + 1;
    let start = (start_day * 86_400 - tz_offset) as f64;

    let mut s = Stats { daily: vec![0; days as usize], ..Default::default() };

    // Ouverts sur la période
    let mut stmt = conn.prepare(
        "SELECT a.opened_at, a.reading_seconds, a.word_count, a.feed_url, f.title, f.custom_title, a.source_title
         FROM articles a LEFT JOIN feeds f ON f.id = a.feed_id
         WHERE a.opened_at >= ?1",
    )?;
    let mut read: std::collections::HashMap<String, u32> = Default::default();
    let rows = stmt.query_map(params![start], |r| {
        Ok((
            r.get::<_, f64>(0)?,
            r.get::<_, f64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<String>>(5)?,
        ))
    })?;
    for row in rows {
        let (opened, seconds, words, feed_url, title, custom) = row?;
        s.read_count += 1;
        s.reading_seconds += seconds;
        s.estimated_seconds += words as f64 / WORDS_PER_MINUTE * 60.0;
        let idx = ((opened as i64) + tz_offset).div_euclid(86_400) - start_day;
        if (0..i64::from(days)).contains(&idx) {
            s.daily[idx as usize] += 1;
        }
        *read.entry(feed_name(&feed_url, custom.or(title))).or_default() += 1;
    }
    s.per_day = f64::from(s.read_count) / f64::from(days);

    // Publiés sur la période
    let mut stmt = conn.prepare(
        "SELECT a.feed_url, f.title, f.custom_title FROM articles a LEFT JOIN feeds f ON f.id = a.feed_id
         WHERE a.published >= ?1 AND a.is_hidden = 0",
    )?;
    let mut active: std::collections::HashMap<String, u32> = Default::default();
    let rows = stmt.query_map(params![start], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?))
    })?;
    for row in rows {
        let (feed_url, title, custom) = row?;
        s.received_count += 1;
        *active.entry(feed_name(&feed_url, custom.or(title))).or_default() += 1;
    }

    s.most_read = top(read);
    s.most_active = top(active);
    s.tracking_since =
        conn.query_row("SELECT MIN(opened_at) FROM articles WHERE opened_at IS NOT NULL", [], |r| r.get(0))?;
    Ok(s)
}

fn feed_name(feed_url: &str, title: Option<String>) -> String {
    source_name(title, None, feed_url)
}

fn top(map: std::collections::HashMap<String, u32>) -> Vec<SourceCount> {
    let mut v: Vec<SourceCount> = map.into_iter().map(|(name, count)| SourceCount { name, count }).collect();
    v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    v.truncate(8);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn counts_reads_by_day_and_source() {
        let c = db::open_in_memory().unwrap();
        c.execute_batch(
            "INSERT INTO feeds (id, url, title, added_at) VALUES (1, 'u1', 'Alpha', 0), (2, 'u2', 'Beta', 0);
             INSERT INTO articles (id, key, feed_id, feed_url, title, published, opened_at, reading_seconds, word_count) VALUES
               (1, 'a', 1, 'u1', 'A', 990000, 1000000, 60, 230),
               (2, 'b', 1, 'u1', 'B', 913600, 913600, 30, 460),
               (3, 'c', 2, 'u2', 'C', 999000, NULL, 0, 100);",
        )
        .unwrap();
        // « maintenant » = 1 000 000 s ; fuseau UTC ; 3 jours (il y a 2 jours = 827 200 s…)
        let s = compute(&c, 3, 1_000_000.0, 0).unwrap();
        assert_eq!(s.read_count, 2);
        assert_eq!(s.received_count, 3);
        assert_eq!(s.daily.iter().sum::<u32>(), 2);
        assert_eq!(*s.daily.last().unwrap(), 1);
        assert_eq!(s.reading_seconds, 90.0);
        assert_eq!(s.estimated_seconds, 180.0);
        assert_eq!(s.most_read[0].name, "Alpha");
        assert_eq!(s.most_read[0].count, 2);
        assert_eq!(s.tracking_since, Some(913_600.0));
        // Période d'un jour : seul l'article d'aujourd'hui compte
        assert_eq!(compute(&c, 1, 1_000_000.0, 0).unwrap().read_count, 1);
    }
}
