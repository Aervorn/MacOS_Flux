//! Repère les articles qui parlent de la même actualité dans plusieurs sources.
//!
//! Reprend `DuplicateDetector.swift` et le regroupement de `FeedStore.swift`, sans la comparaison
//! « par le sens » (NLEmbedding n'existe pas hors macOS) : seuls restent les liens identiques et
//! le recouvrement des mots importants des titres. Deux articles de flux différents, publiés à
//! moins de 48 h d'écart, sont regroupés :
//! - s'ils pointent vers le même lien (republication) ;
//! - ou s'ils partagent au moins deux mots importants (hors mots du moment) et que leurs titres
//!   se recouvrent assez, les mots rares comptant davantage.
//!
//! Le résultat n'est jamais parfait : la sensibilité règle le compromis.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection};
use url::Url;

use crate::error::Result;
use crate::feeds::now;
use crate::prefs::Prefs;
use crate::query::fold;

/// Fenêtre de comparaison entre deux articles (48 h).
pub const WINDOW: f64 = 48.0 * 3600.0;
/// Période regroupée quand on recalcule tout (7 jours).
const REGROUP_PERIOD: f64 = 7.0 * 86_400.0;

/// Recouvrement minimal des mots des titres, pondérés par leur rareté (faible, normale, forte).
fn word_threshold(sensitivity: u32) -> f64 {
    [0.5, 0.4, 0.32][sensitivity.min(2) as usize]
}

/// Part minimale du titre le plus court retrouvée dans l'autre (au moins trois mots rares en commun) :
/// rattrape les titres dont l'un est bien plus long que l'autre (« Michel Dumas, chef star des réseaux est mort »
/// contre « On pensait avoir beaucoup de temps… Michel Dumas, chef star des réseaux, est décédé »).
fn coverage_threshold(sensitivity: u32) -> f64 {
    [0.7, 0.55, 0.45][sensitivity.min(2) as usize]
}

#[derive(Debug, Clone)]
struct Item {
    key: String,
    feed_url: String,
    title: String,
    link: Option<String>,
    published: f64,
    cluster_id: Option<String>,
    is_duplicate: bool,
    is_read: bool,
}

struct Signature {
    words: HashSet<String>,
    link: Option<String>,
}

fn signature(title: &str, link: Option<&str>) -> Signature {
    Signature { words: significant_words(title), link: link.and_then(normalized_link) }
}

/// Mots du titre sans majuscules, accents, mots courants ni pluriels simples.
pub fn significant_words(text: &str) -> HashSet<String> {
    let folded = fold(text);
    let mut words = HashSet::new();
    for raw in folded.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        // On garde les nombres (« iPhone 18 », « 2026 ») même courts.
        let is_number = raw.chars().all(char::is_numeric);
        let len = raw.chars().count();
        if !(is_number || len >= 3) || STOP_WORDS.contains(&raw) {
            continue;
        }
        let mut word = raw.to_string();
        if !is_number && len > 4 && (word.ends_with('s') || word.ends_with('x')) {
            word.pop();
        }
        words.insert(word);
    }
    words
}

/// Lien sans protocole, www, paramètres ni barre finale.
pub fn normalized_link(link: &str) -> Option<String> {
    let url = Url::parse(link).ok()?;
    let host = url.host_str()?.to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    Some(format!("{host}{}", url.path().trim_end_matches('/')))
}

/// Mesure la fréquence des mots sur la période : un mot présent dans beaucoup de titres
/// (« iPhone », « Apple » une semaine de keynote) ne prouve rien.
struct Detector {
    threshold: f64,
    coverage: f64,
    document_count: usize,
    document_frequency: HashMap<String, usize>,
}

impl Detector {
    fn new(sensitivity: u32, signatures: &[Signature]) -> Self {
        let mut document_frequency: HashMap<String, usize> = HashMap::new();
        for s in signatures {
            for w in &s.words {
                *document_frequency.entry(w.clone()).or_default() += 1;
            }
        }
        Detector { threshold: word_threshold(sensitivity), coverage: coverage_threshold(sensitivity), document_count: signatures.len().max(1), document_frequency }
    }

    fn frequency(&self, word: &str) -> usize {
        self.document_frequency.get(word).copied().unwrap_or(0)
    }

    /// Mot trop courant sur la période pour distinguer une actualité d'une autre.
    fn is_common(&self, word: &str) -> bool {
        self.frequency(word) > 4usize.max(self.document_count / 50)
    }

    /// Poids d'un mot : d'autant plus fort qu'il est rare.
    fn weight(&self, word: &str) -> f64 {
        ((self.document_count + 1) as f64 / (self.frequency(word) + 1) as f64).ln() + 0.05
    }

    /// Score de similarité (None = pas un doublon).
    fn score(&self, a: &Signature, b: &Signature) -> Option<f64> {
        if let (Some(la), Some(lb)) = (&a.link, &b.link) {
            if la == lb {
                return Some(3.0);
            }
        }
        let shared: Vec<&String> = a.words.intersection(&b.words).collect();
        // Au moins deux mots partagés qui ne sont pas des mots du moment.
        let strong = shared.iter().filter(|w| !self.is_common(w)).count();
        if strong < 2 {
            return None;
        }
        let shared_weight: f64 = shared.iter().map(|w| self.weight(w)).sum();
        let union_weight: f64 = a.words.union(&b.words).map(|w| self.weight(w)).sum();
        let overlap = shared_weight / union_weight.max(0.0001);
        if overlap >= self.threshold {
            return Some(1.0 + overlap);
        }
        // Titres de longueurs très différentes : le plus court est-il presque entièrement repris ?
        if strong >= 3 {
            let weight_of = |s: &Signature| s.words.iter().map(|w| self.weight(w)).sum::<f64>();
            let shortest = weight_of(a).min(weight_of(b)).max(0.0001);
            if shared_weight / shortest >= self.coverage {
                return Some(1.0 + overlap);
            }
        }
        None
    }
}

fn load_items(conn: &Connection, since: f64) -> Result<Vec<Item>> {
    let mut stmt = conn.prepare(
        "SELECT key, feed_url, title, link, published, cluster_id, is_duplicate, is_read
         FROM articles
         WHERE cluster_opt_out = 0 AND is_hidden = 0
           AND (published >= ?1 OR cluster_id IN
                (SELECT cluster_id FROM articles WHERE published >= ?1 AND cluster_id IS NOT NULL))",
    )?;
    let rows = stmt.query_map(params![since], |r| {
        Ok(Item {
            key: r.get(0)?,
            feed_url: r.get(1)?,
            title: r.get(2)?,
            link: r.get(3)?,
            published: r.get(4)?,
            cluster_id: r.get(5)?,
            is_duplicate: r.get::<_, i64>(6)? != 0,
            is_read: r.get::<_, i64>(7)? != 0,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Rattache les candidats aux articles similaires des autres sources ; renvoie les indices modifiés.
fn cluster(items: &mut [Item], mut candidates: Vec<usize>, sensitivity: u32) -> Vec<usize> {
    let signatures: Vec<Signature> = items.iter().map(|i| signature(&i.title, i.link.as_deref())).collect();
    let detector = Detector::new(sensitivity, &signatures);

    let mut by_word: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut by_link: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, s) in signatures.iter().enumerate() {
        for w in &s.words {
            by_word.entry(w.as_str()).or_default().push(index);
        }
        if let Some(l) = &s.link {
            by_link.entry(l.as_str()).or_default().push(index);
        }
    }
    let mut members: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, item) in items.iter().enumerate() {
        if let Some(id) = &item.cluster_id {
            members.entry(id.clone()).or_default().push(index);
        }
    }

    candidates.sort_by(|&a, &b| items[a].published.total_cmp(&items[b].published));
    let mut touched: HashSet<String> = HashSet::new();
    let mut dirty: HashSet<usize> = HashSet::new();

    for &index in &candidates {
        if items[index].cluster_id.is_some() {
            continue;
        }
        let sig = &signatures[index];
        let mut possible: HashSet<usize> = HashSet::new();
        for w in &sig.words {
            possible.extend(by_word.get(w.as_str()).into_iter().flatten());
        }
        if let Some(l) = &sig.link {
            possible.extend(by_link.get(l.as_str()).into_iter().flatten());
        }

        let mut best: Option<(usize, f64)> = None;
        for &other in &possible {
            if other == index
                || items[other].feed_url == items[index].feed_url
                || (items[other].published - items[index].published).abs() > WINDOW
            {
                continue;
            }
            let Some(score) = detector.score(sig, &signatures[other]) else { continue };
            if let Some(id) = &items[other].cluster_id {
                let group = &members[id];
                // Une source ne publie pas deux fois la même actualité : une seule place par source.
                if group.iter().any(|&m| items[m].feed_url == items[index].feed_url) {
                    continue;
                }
                // Pas de réaction en chaîne : il faut aussi ressembler au premier article du groupe.
                let lead = *group
                    .iter()
                    .min_by(|&&a, &&b| items[a].published.total_cmp(&items[b].published))
                    .expect("groupe non vide");
                if lead != other && detector.score(sig, &signatures[lead]).is_none() {
                    continue;
                }
            }
            if score > best.map_or(0.0, |(_, s)| s) {
                best = Some((other, score));
            }
        }
        let Some((matched, _)) = best else { continue };
        let id = match items[matched].cluster_id.clone() {
            Some(id) => id,
            None => {
                let id = uuid::Uuid::new_v4().to_string().to_uppercase();
                items[matched].cluster_id = Some(id.clone());
                members.insert(id.clone(), vec![matched]);
                dirty.insert(matched);
                id
            }
        };
        items[index].cluster_id = Some(id.clone());
        members.entry(id.clone()).or_default().push(index);
        touched.insert(id);
        dirty.insert(index);
    }

    // Dans chaque groupe, le premier publié représente l'actualité ; les autres suivent son état de lecture.
    for id in &touched {
        let mut group = members[id].clone();
        group.sort_by(|&a, &b| items[a].published.total_cmp(&items[b].published).then_with(|| items[a].key.cmp(&items[b].key)));
        let lead = group[0];
        let lead_read = items[lead].is_read;
        for &m in &group {
            let duplicate = m != lead;
            let read = items[m].is_read || (duplicate && lead_read);
            if items[m].is_duplicate != duplicate || items[m].is_read != read {
                items[m].is_duplicate = duplicate;
                items[m].is_read = read;
                dirty.insert(m);
            }
        }
    }
    dirty.into_iter().collect()
}

fn save(conn: &Connection, items: &[Item], dirty: &[usize]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for &i in dirty {
        let item = &items[i];
        tx.execute(
            "UPDATE articles SET cluster_id = ?2, is_duplicate = ?3, is_read = ?4 WHERE key = ?1",
            params![item.key, item.cluster_id, item.is_duplicate, item.is_read],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Rattache les nouveaux articles aux articles similaires des autres sources (48 h autour).
pub fn group_new(conn: &Connection, new_keys: &[String], prefs: &Prefs) -> Result<()> {
    if !prefs.group_duplicates || new_keys.is_empty() {
        return Ok(());
    }
    let wanted: HashSet<&str> = new_keys.iter().map(String::as_str).collect();
    let mut earliest = f64::INFINITY;
    {
        let mut stmt = conn.prepare("SELECT published FROM articles WHERE key = ?1")?;
        for key in new_keys {
            if let Ok(p) = stmt.query_row(params![key], |r| r.get::<_, f64>(0)) {
                earliest = earliest.min(p);
            }
        }
    }
    if !earliest.is_finite() {
        return Ok(());
    }
    let mut items = load_items(conn, earliest - WINDOW)?;
    let candidates: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, i)| wanted.contains(i.key.as_str()) && i.cluster_id.is_none())
        .map(|(n, _)| n)
        .collect();
    if candidates.is_empty() {
        return Ok(());
    }
    let dirty = cluster(&mut items, candidates, prefs.duplicate_sensitivity);
    save(conn, &items, &dirty)
}

/// Recalcule les groupes : tous défaits, puis les 7 derniers jours regroupés
/// (réglages modifiés). Option désactivée : tous les groupes sont défaits.
pub fn regroup_recent(conn: &Connection, prefs: &Prefs) -> Result<()> {
    conn.execute("UPDATE articles SET cluster_id = NULL, is_duplicate = 0 WHERE cluster_id IS NOT NULL", [])?;
    if !prefs.group_duplicates {
        return Ok(());
    }
    let mut items = load_items(conn, now() - REGROUP_PERIOD)?;
    let candidates: Vec<usize> = (0..items.len()).collect();
    let dirty = cluster(&mut items, candidates, prefs.duplicate_sensitivity);
    save(conn, &items, &dirty)
}

/// Sort un article de son groupe ; il ne sera plus regroupé.
pub fn detach(conn: &Connection, key: &str) -> Result<()> {
    let cluster: Option<String> =
        conn.query_row("SELECT cluster_id FROM articles WHERE key = ?1", params![key], |r| r.get(0))?;
    let Some(cluster) = cluster else { return Ok(()) };
    conn.execute(
        "UPDATE articles SET cluster_id = NULL, is_duplicate = 0, cluster_opt_out = 1 WHERE key = ?1",
        params![key],
    )?;
    let remaining: Vec<String> = {
        let mut stmt = conn.prepare("SELECT key FROM articles WHERE cluster_id = ?1 ORDER BY published, key")?;
        let rows = stmt.query_map(params![cluster], |r| r.get(0))?;
        rows.collect::<std::result::Result<_, _>>()?
    };
    for (index, member) in remaining.iter().enumerate() {
        if remaining.len() <= 1 {
            conn.execute("UPDATE articles SET cluster_id = NULL, is_duplicate = 0 WHERE key = ?1", params![member])?;
        } else {
            conn.execute("UPDATE articles SET is_duplicate = ?2 WHERE key = ?1", params![member, index > 0])?;
        }
    }
    Ok(())
}

/// Après l'arrivée de nouveaux articles : filtres d'abord (un article masqué n'est pas regroupé), puis doublons.
pub fn process_new(conn: &Connection, new_keys: &[String], prefs: &Prefs) -> Result<()> {
    crate::filters::apply(conn, Some(new_keys))?;
    group_new(conn, new_keys, prefs)
}

// Mots courants (français et anglais), sans accents ni majuscules.
const STOP_WORDS: &[&str] = &[
    // français
    "les", "des", "une", "est", "son", "sa", "ses", "aux", "par", "pour", "dans", "sur", "avec", "sans", "qui", "que",
    "quoi", "dont", "mais", "plus", "moins", "tres", "tout", "tous", "toute", "toutes", "cette", "ces", "cet", "leur",
    "leurs", "nous", "vous", "ils", "elles", "elle", "lui", "eux", "pas", "peu", "deja", "encore", "aussi", "comme",
    "entre", "vers", "chez", "apres", "avant", "depuis", "pendant", "selon", "contre", "sous", "fait", "faire", "font",
    "sont", "ont", "avoir", "etre", "ete", "sera", "seront", "peut", "peuvent", "va", "vont", "voici", "voila",
    "enfin", "bien", "quand", "comment", "pourquoi", "ou", "lors", "notre", "votre", "nos", "vos", "mon", "ton", "mes",
    "tes", "ceux", "celle", "celles", "celui", "ici", "la", "le", "un", "et", "de", "du", "au", "en", "ne", "se", "ce",
    "il", "on", "nouveau", "nouvelle", "nouveaux", "nouvelles", "officiel", "officiellement", "savoir", "faut",
    // anglais
    "the", "and", "for", "with", "from", "that", "this", "these", "those", "are", "was", "were", "has", "have", "had",
    "will", "can", "its", "it's", "your", "you", "our", "their", "they", "but", "not", "now", "new", "more", "than",
    "about", "into", "over", "after", "before", "just", "what", "how", "why", "when", "who", "here", "there", "all",
    "out", "get", "gets", "got", "finally", "officially", "announces", "announced", "launches", "launched", "says",
    "said",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(rows: &[(&str, &str, &str, &str, f64)]) -> Connection {
        let c = crate::db::open_in_memory().unwrap();
        c.execute_batch("INSERT INTO feeds (id, url, title, added_at) VALUES (1,'f1','F1',0),(2,'f2','F2',0),(3,'f3','F3',0);").unwrap();
        for (key, feed, title, link, published) in rows {
            let id: i64 = feed.trim_start_matches('f').parse().unwrap();
            c.execute(
                "INSERT INTO articles (key, feed_id, feed_url, title, link, published) VALUES (?1,?2,?3,?4,?5,?6)",
                params![key, id, feed, title, link, published],
            )
            .unwrap();
        }
        c
    }

    fn state(c: &Connection, key: &str) -> (Option<String>, bool) {
        c.query_row("SELECT cluster_id, is_duplicate FROM articles WHERE key = ?1", params![key], |r| {
            Ok((r.get(0)?, r.get::<_, i64>(1)? != 0))
        })
        .unwrap()
    }

    /// Articles sans rapport : donnent à la période un nombre de titres réaliste (rareté des mots).
    fn add_fillers(c: &Connection, t: f64, n: usize) {
        for (i, f) in fillers(n, t).iter().enumerate() {
            c.execute(
                "INSERT INTO articles (key, feed_id, feed_url, title, link, published) VALUES (?1, 3, 'f3', ?2, ?3, ?4)",
                params![format!("fill{i}"), f.2, f.3, f.4],
            )
            .unwrap();
        }
    }

    fn fillers(n: usize, t: f64) -> Vec<(String, String, String, String, f64)> {
        let topics = ["tarte citron meringuee", "marathon paris record", "volcan islande eruption", "budget senat vote",
            "tennis finale gagnant", "meteo orage alerte", "cinema palme festival", "retraites reforme debat",
            "cyclisme etape sprint", "livre prix goncourt", "pluie inondation region", "football match nul"];
        (0..n)
            .map(|i| (format!("fill{i}"), "f3".to_string(), format!("{} {i}", topics[i % topics.len()]), format!("https://z.example/{i}"), t))
            .collect()
    }

    #[test]
    fn words_ignore_case_accents_plurals_and_stop_words() {
        let w = significant_words("Les Écrans géants d'Apple : iPhone 18 !");
        assert!(w.contains("ecran") && w.contains("geant") && w.contains("apple") && w.contains("iphone") && w.contains("18"));
        assert!(!w.contains("les"));
    }

    #[test]
    fn link_is_normalized() {
        assert_eq!(normalized_link("https://www.Example.com/a/b/?utm=1#x").as_deref(), Some("example.com/a/b"));
        assert_eq!(normalized_link("pas un lien"), None);
    }

    #[test]
    fn same_story_in_two_sources_is_grouped() {
        let t = now();
        let c = setup(&[
            ("a", "f1", "Apple dévoile iPhone pliable écran géant", "https://one.example/a", t - 100.0),
            ("b", "f2", "Apple dévoile iPhone pliable écran immense", "https://two.example/b", t),
            ("c", "f2", "Recette de la tarte aux pommes", "https://two.example/c", t),
        ]);
        add_fillers(&c, t, 10);
        let prefs = Prefs::default();
        regroup_recent(&c, &prefs).unwrap();
        let (ca, da) = state(&c, "a");
        let (cb, db) = state(&c, "b");
        assert!(ca.is_some() && ca == cb);
        assert!(!da && db); // le premier publié représente l'actualité
        assert_eq!(state(&c, "c").0, None);
    }

    #[test]
    fn long_and_short_titles_about_the_same_person_are_grouped() {
        let t = now();
        let c = setup(&[
            ("a", "f1", "« On pensait avoir encore beaucoup de temps »… Michel Dumas, le chef star des réseaux sociaux, est décédé", "https://one.example/a", t - 100.0),
            ("b", "f2", "Michel Dumas, le chef français star des réseaux sociaux, est mort à l'âge de 72 ans", "https://two.example/b", t),
        ]);
        add_fillers(&c, t, 100);
        regroup_recent(&c, &Prefs::default()).unwrap();
        assert!(state(&c, "a").0.is_some() && state(&c, "a").0 == state(&c, "b").0);
    }

    #[test]
    fn same_link_groups_and_same_feed_does_not() {
        let t = now();
        let c = setup(&[
            ("a", "f1", "Titre totalement différent", "https://www.site.example/x/", t - 10.0),
            ("b", "f2", "Un autre titre sans rapport", "https://site.example/x?utm=1", t),
            ("c", "f1", "Titre totalement différent bis", "https://site.example/x", t),
        ]);
        regroup_recent(&c, &Prefs::default()).unwrap();
        assert_eq!(state(&c, "a").0, state(&c, "b").0);
        assert!(state(&c, "a").0.is_some());
        // « c » est du même flux que « a » : pas de deuxième place pour la même source
        assert!(state(&c, "c").0.is_none());
    }

    #[test]
    fn group_new_follows_read_state_and_detach_opts_out() {
        let t = now();
        let c = setup(&[
            ("a", "f1", "Séisme majeur secoue Japon côte", "https://one.example/a", t - 100.0),
            ("b", "f2", "Séisme majeur secoue côte Japon aujourd'hui", "https://two.example/b", t),
        ]);
        add_fillers(&c, t, 10);
        c.execute("UPDATE articles SET is_read = 1 WHERE key = 'a'", []).unwrap();
        group_new(&c, &["b".to_string()], &Prefs::default()).unwrap();
        assert_eq!(state(&c, "a").0, state(&c, "b").0);
        let read: i64 = c.query_row("SELECT is_read FROM articles WHERE key = 'b'", [], |r| r.get(0)).unwrap();
        assert_eq!(read, 1);

        detach(&c, "b").unwrap();
        assert_eq!(state(&c, "a"), (None, false)); // il ne reste plus qu'un article : plus de groupe
        assert_eq!(state(&c, "b"), (None, false));
        regroup_recent(&c, &Prefs::default()).unwrap();
        assert_eq!(state(&c, "b").0, None); // sorti pour de bon
    }

    #[test]
    fn disabled_option_clears_groups() {
        let t = now();
        let c = setup(&[
            ("a", "f1", "x", "https://s.example/x", t),
            ("b", "f2", "y", "https://s.example/x", t),
        ]);
        regroup_recent(&c, &Prefs::default()).unwrap();
        assert!(state(&c, "a").0.is_some());
        regroup_recent(&c, &Prefs { group_duplicates: false, ..Prefs::default() }).unwrap();
        assert!(state(&c, "a").0.is_none());
    }
}
