//! Synchronisation entre Mac par iCloud Drive.
//!
//! Format de fichiers JSON v2 (depuis la 0.8.5, pour synchroniser les dossiers intelligents).
//! Il remplace le v1 de l'app Swift et des versions 0.8.0 et 0.8.1 : un Mac qui lit un fichier d'une
//! autre version l'ignore et le signale (`SyncReport::incompatible`), il faut donc mettre les deux
//! Mac à jour. Les fichiers v1 restent dans le dossier, sans effet.
//!
//! ```text
//! iCloud Drive/Flux/devices/<identifiant du Mac>.json
//! ```
//!
//! Chaque Mac écrit uniquement son propre fichier (pas de conflit d'écriture) et lit ceux des
//! autres. Chaque élément (flux, dossier, tag, règle, état d'un article) porte la date de sa
//! dernière modification : à la fusion, la plus récente l'emporte. Une suppression est une
//! « pierre tombale » datée, gardée 60 jours.
//!
//! Les changements locaux ne sont pas suivis un par un : à chaque synchro on compare la base
//! à l'instantané précédent, et seuls les éléments qui ont changé reçoivent une nouvelle date.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::{actions, db, filters, manage, smart};

/// Version du format des fichiers. La 2 ajoute `smartFolders` ; les fichiers d'une autre version sont ignorés.
pub const SYNC_VERSION: u32 = 2;

const TOMBSTONE_LIFETIME: f64 = 60.0 * 86_400.0;
const STATE_KEY: &str = "syncState";
const DEVICE_KEY: &str = "syncDeviceId";

fn io(e: std::io::Error) -> Error {
    Error::Other(e.to_string())
}

// ---------------------------------------------------------------------------
// Format des fichiers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Snapshot {
    pub version: u32,
    pub device: String,
    #[serde(rename = "deviceName")]
    pub device_name: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: f64,
    /// Clé : adresse du flux
    pub feeds: BTreeMap<String, FeedEntry>,
    /// Clé : uid
    pub folders: BTreeMap<String, FolderEntry>,
    /// Clé : uid
    pub filters: BTreeMap<String, FilterEntry>,
    /// Clé : clé de l'article ; seuls les états différents du défaut sont publiés
    pub articles: BTreeMap<String, ArticleEntry>,
    /// Tags des favoris (clé : uid) ; absent des fichiers des anciennes versions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<BTreeMap<String, TagEntry>>,
    /// Dossiers intelligents (clé : uid), depuis le format v2
    #[serde(rename = "smartFolders")]
    pub smart_folders: BTreeMap<String, SmartFolderEntry>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Snapshot {
            version: SYNC_VERSION,
            device: String::new(),
            device_name: String::new(),
            updated_at: 0.0,
            feeds: BTreeMap::new(),
            folders: BTreeMap::new(),
            filters: BTreeMap::new(),
            articles: BTreeMap::new(),
            tags: None,
            smart_folders: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedEntry {
    /// Nom d'origine (pour créer le flux ailleurs, pas comparé)
    pub title: String,
    #[serde(rename = "siteURL", default, skip_serializing_if = "Option::is_none")]
    pub site_url: Option<String>,
    #[serde(rename = "customTitle", default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// Ancienne adresse du flux (changement d'adresse)
    #[serde(rename = "previousURL", default, skip_serializing_if = "Option::is_none")]
    pub previous_url: Option<String>,
    #[serde(default)]
    pub deleted: bool,
    pub modified: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default)]
    pub deleted: bool,
    pub modified: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartFolderEntry {
    pub name: String,
    /// Mots-clés, dans l'ordre choisi (un seul suffit)
    pub keywords: Vec<String>,
    #[serde(rename = "wholeWord")]
    pub whole_word: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default)]
    pub deleted: bool,
    pub modified: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagEntry {
    pub name: String,
    #[serde(default)]
    pub deleted: bool,
    pub modified: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterEntry {
    pub keyword: String,
    #[serde(rename = "wholeWord")]
    pub whole_word: bool,
    pub action: String,
    /// Ancien format (un seul flux) : le premier flux de la liste, pour les anciennes versions
    #[serde(rename = "feedURL", default, skip_serializing_if = "Option::is_none")]
    pub feed_url: Option<String>,
    /// Flux concernés, triés (vide = tous)
    #[serde(rename = "feedURLs", default, skip_serializing_if = "Option::is_none")]
    pub feed_urls: Option<Vec<String>>,
    #[serde(default)]
    pub deleted: bool,
    pub modified: f64,
}

impl FilterEntry {
    fn new(keyword: String, whole_word: bool, action: String, mut feed_urls: Vec<String>, modified: f64) -> Self {
        feed_urls.sort();
        FilterEntry {
            keyword,
            whole_word,
            action,
            feed_url: feed_urls.first().cloned(),
            feed_urls: Some(feed_urls),
            deleted: false,
            modified,
        }
    }

    /// Flux concernés, quel que soit le format du fichier (vide = tous)
    fn scope(&self) -> Vec<String> {
        let mut v = match (&self.feed_urls, &self.feed_url) {
            (Some(list), _) => list.clone(),
            (None, Some(one)) => vec![one.clone()],
            (None, None) => Vec::new(),
        };
        v.sort();
        v
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArticleEntry {
    pub read: bool,
    pub starred: bool,
    pub published: f64,
    pub modified: f64,
    /// De quoi recréer un favori sur un Mac qui n'a pas (ou plus) l'article
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<ArticleMeta>,
    /// Tags (uid), triés
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

impl ArticleEntry {
    fn tag_list(&self) -> Vec<String> {
        self.tags.clone().unwrap_or_default()
    }

    /// « Non lu et pas favori » est l'état par défaut
    fn is_default(&self) -> bool {
        !self.read && !self.starred && self.tag_list().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArticleMeta {
    #[serde(rename = "feedURL")]
    pub feed_url: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(rename = "imageURL", default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Ce que chaque type d'élément sait faire pour la comparaison et les pierres tombales.
trait Entry: Clone {
    fn deleted(&self) -> bool;
    fn modified(&self) -> f64;
    fn set_modified(&mut self, m: f64);
    fn tombstone(&mut self, now: f64);
    fn same_value(&self, other: &Self) -> bool;
}

macro_rules! entry_common {
    () => {
        fn deleted(&self) -> bool {
            self.deleted
        }
        fn modified(&self) -> f64 {
            self.modified
        }
        fn set_modified(&mut self, m: f64) {
            self.modified = m;
        }
        fn tombstone(&mut self, now: f64) {
            self.deleted = true;
            self.modified = now;
        }
    };
}

impl Entry for FeedEntry {
    entry_common!();
    fn same_value(&self, o: &Self) -> bool {
        self.custom_title == o.custom_title && self.folder == o.folder && self.deleted == o.deleted
    }
}

impl Entry for FolderEntry {
    entry_common!();
    fn same_value(&self, o: &Self) -> bool {
        self.name == o.name && self.symbol == o.symbol && self.color == o.color && self.deleted == o.deleted
    }
}

impl Entry for SmartFolderEntry {
    entry_common!();
    fn same_value(&self, o: &Self) -> bool {
        self.name == o.name
            && self.keywords == o.keywords
            && self.whole_word == o.whole_word
            && self.symbol == o.symbol
            && self.color == o.color
            && self.deleted == o.deleted
    }
}

impl Entry for TagEntry {
    entry_common!();
    fn same_value(&self, o: &Self) -> bool {
        self.name == o.name && self.deleted == o.deleted
    }
}

impl Entry for FilterEntry {
    entry_common!();
    fn same_value(&self, o: &Self) -> bool {
        self.keyword == o.keyword
            && self.whole_word == o.whole_word
            && self.action == o.action
            && self.scope() == o.scope()
            && self.deleted == o.deleted
    }
}

/// Mémoire locale entre deux synchronisations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct LocalState {
    last_snapshot: Snapshot,
    /// Articles présents lors de la dernière synchro : un article absent de cette liste vient
    /// d'arriver, il reçoit l'état synchronisé au lieu de l'écraser.
    known_articles: HashSet<String>,
}

// ---------------------------------------------------------------------------
// Emplacements et identité du Mac
// ---------------------------------------------------------------------------

pub fn icloud_drive() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Library/Mobile Documents/com~apple~CloudDocs")
}

pub fn is_icloud_available() -> bool {
    icloud_drive().is_dir()
}

pub fn devices_dir() -> PathBuf {
    icloud_drive().join("Flux").join("devices")
}

#[derive(Debug, Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
}

/// Nom du Mac tel que l'affichent les Réglages Système (« MacBook Pro de … »).
fn computer_name() -> String {
    std::process::Command::new("scutil")
        .args(["--get", "ComputerName"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Mac".to_string())
}

/// Identifiant de ce Mac (créé au premier appel) et son nom.
pub fn device(conn: &Connection) -> Result<Device> {
    let id = match db::load_setting::<String>(conn, DEVICE_KEY)? {
        Some(id) => id,
        None => {
            let id = uuid::Uuid::new_v4().to_string().to_uppercase();
            db::save_setting(conn, DEVICE_KEY, &id)?;
            id
        }
    };
    Ok(Device { id, name: computer_name() })
}

/// Fichiers des autres Mac et leur date de modification.
pub fn remote_files(dir: &Path, own_id: &str) -> Vec<(PathBuf, f64)> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let own = format!("{own_id}.json");
    let mut out = Vec::new();
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // Fichier pas encore téléchargé par iCloud Drive (« .xxx.json.icloud ») : on le demande
        if name.starts_with('.') && name.ends_with(".icloud") {
            let real = name.trim_start_matches('.').trim_end_matches(".icloud").to_string();
            let _ = std::process::Command::new("brctl").arg("download").arg(dir.join(real)).status();
            continue;
        }
        if !name.ends_with(".json") || name == own {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        out.push((entry.path(), modified));
    }
    out
}

// ---------------------------------------------------------------------------
// Synchronisation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OtherDevice {
    pub name: String,
    /// Secondes Unix
    pub updated: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub others: Vec<OtherDevice>,
    /// Flux créés depuis un autre Mac : leurs articles restent à récupérer.
    pub new_feeds: Vec<String>,
    /// La base a changé à cause de la fusion (l'interface doit se recharger).
    pub changed: bool,
    /// Notre fichier a été réécrit.
    pub published: bool,
    /// Fichiers d'un autre Mac dans un format d'une autre version (à mettre à jour) : ignorés.
    pub incompatible: Vec<OtherDevice>,
}

/// Une synchro complète : état local → fusion des autres Mac → publication.
/// `now` est passé en paramètre pour pouvoir tester les conflits.
pub fn sync(conn: &Connection, dir: &Path, dev: &Device, retention_days: u32, now: f64) -> Result<SyncReport> {
    std::fs::create_dir_all(dir).map_err(io)?;
    let mut state: LocalState = db::load_setting(conn, STATE_KEY)?.unwrap_or_default();
    let mut report = SyncReport::default();

    // 1. État local, horodaté par rapport au dernier instantané
    apply_pending_article_states(conn, &state)?;
    let retention = now - f64::from(retention_days) * 86_400.0;
    let mut current = build_snapshot(conn, &state.last_snapshot, now, retention)?;

    // 2. Fusion des autres Mac : la modification la plus récente l'emporte
    let mut others = Vec::new();
    for (path, _) in remote_files(dir, &dev.id) {
        let Ok(data) = std::fs::read(&path) else { continue };
        let Ok(remote) = serde_json::from_slice::<Snapshot>(&data) else { continue };
        if remote.version != SYNC_VERSION {
            report.incompatible.push(OtherDevice { name: remote.device_name.clone(), updated: remote.updated_at });
            continue;
        }
        others.push(OtherDevice { name: remote.device_name.clone(), updated: remote.updated_at });
        let merged = merge(conn, &remote, &mut current, now)?;
        report.new_feeds.extend(merged.created_feeds);
        report.changed |= merged.changed;
    }
    others.sort_by(|a, b| b.updated.total_cmp(&a.updated));
    report.others = others;

    // 3. On publie notre état, seulement s'il a changé : sinon les Mac se relanceraient
    //    l'un l'autre en boucle à chaque lecture
    current.device = dev.id.clone();
    current.device_name = dev.name.clone();
    let mut before = state.last_snapshot.clone();
    before.updated_at = 0.0;
    let mut after = current.clone();
    after.updated_at = 0.0;
    let unchanged = serde_json::to_string(&before)? == serde_json::to_string(&after)?;
    let own_file = dir.join(format!("{}.json", dev.id));
    if unchanged && own_file.exists() {
        current.updated_at = state.last_snapshot.updated_at;
    } else {
        current.updated_at = now;
        let tmp = dir.join(format!("{}.json.tmp", dev.id));
        std::fs::write(&tmp, serde_json::to_vec(&current)?).map_err(io)?;
        std::fs::rename(&tmp, &own_file).map_err(io)?;
        report.published = true;
    }

    state.last_snapshot = current;
    state.known_articles = all_article_keys(conn)?;
    db::save_setting(conn, STATE_KEY, &state)?;
    Ok(report)
}

/// Désactivation : on retire ce Mac de la synchronisation (son fichier) et on oublie l'état local.
pub fn disable(conn: &Connection, dir: &Path, dev: &Device) -> Result<()> {
    let _ = std::fs::remove_file(dir.join(format!("{}.json", dev.id)));
    conn.execute("DELETE FROM settings WHERE key = ?1", params![STATE_KEY])?;
    Ok(())
}

fn all_article_keys(conn: &Connection) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare("SELECT key FROM articles")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

// ---------------------------------------------------------------------------
// Instantané local
// ---------------------------------------------------------------------------

/// Garde la date de l'ancienne entrée si rien n'a changé depuis.
fn stamp<E: Entry>(prev: &BTreeMap<String, E>, key: &str, mut entry: E) -> E {
    if let Some(old) = prev.get(key) {
        if old.same_value(&entry) {
            entry.set_modified(old.modified());
        }
    }
    entry
}

/// Les éléments disparus d'ici deviennent des pierres tombales (gardées 60 jours).
fn carry_deleted<E: Entry>(prev: &BTreeMap<String, E>, snap: &mut BTreeMap<String, E>, now: f64, limit: f64) {
    for (key, old) in prev {
        if snap.contains_key(key) {
            continue;
        }
        if old.deleted() {
            if old.modified() > limit {
                snap.insert(key.clone(), old.clone());
            }
        } else {
            let mut gone = old.clone();
            gone.tombstone(now);
            snap.insert(key.clone(), gone);
        }
    }
}

fn build_snapshot(conn: &Connection, prev: &Snapshot, now: f64, retention: f64) -> Result<Snapshot> {
    let mut snap = Snapshot::default();
    let limit = now - TOMBSTONE_LIFETIME;

    // Dossiers
    {
        let mut stmt = conn.prepare("SELECT uid, name, symbol, color_name FROM folders")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?))
        })?;
        for row in rows {
            let (uid, name, symbol, color) = row?;
            let entry = FolderEntry { name, symbol, color, deleted: false, modified: now };
            snap.folders.insert(uid.clone(), stamp(&prev.folders, &uid, entry));
        }
    }
    carry_deleted(&prev.folders, &mut snap.folders, now, limit);

    // Flux
    {
        let mut stmt = conn.prepare(
            "SELECT f.url, f.title, f.site_url, f.custom_title, d.uid, f.previous_url
             FROM feeds f LEFT JOIN folders d ON d.id = f.folder_id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })?;
        for row in rows {
            let (url, title, site_url, custom_title, folder, previous_url) = row?;
            let entry = FeedEntry { title, site_url, custom_title, folder, previous_url, deleted: false, modified: now };
            snap.feeds.insert(url.clone(), stamp(&prev.feeds, &url, entry));
        }
    }
    carry_deleted(&prev.feeds, &mut snap.feeds, now, limit);

    // Tags des favoris
    {
        let prev_tags = prev.tags.clone().unwrap_or_default();
        let mut tags: BTreeMap<String, TagEntry> = BTreeMap::new();
        let mut stmt = conn.prepare("SELECT uid, name FROM favorite_tags")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (uid, name) = row?;
            let entry = TagEntry { name, deleted: false, modified: now };
            tags.insert(uid.clone(), stamp(&prev_tags, &uid, entry));
        }
        carry_deleted(&prev_tags, &mut tags, now, limit);
        snap.tags = if tags.is_empty() { None } else { Some(tags) };
    }

    // Dossiers intelligents
    for folder in smart::list(conn)? {
        let entry = SmartFolderEntry {
            name: folder.name,
            keywords: folder.keywords,
            whole_word: folder.whole_word,
            symbol: folder.symbol,
            color: folder.color_name,
            deleted: false,
            modified: now,
        };
        snap.smart_folders.insert(folder.uid.clone(), stamp(&prev.smart_folders, &folder.uid, entry));
    }
    carry_deleted(&prev.smart_folders, &mut snap.smart_folders, now, limit);

    // Règles de filtrage
    for rule in filters::list(conn)? {
        let entry = FilterEntry::new(rule.keyword, rule.whole_word, rule.action, rule.feed_urls, now);
        snap.filters.insert(rule.uid.clone(), stamp(&prev.filters, &rule.uid, entry));
    }
    carry_deleted(&prev.filters, &mut snap.filters, now, limit);

    // Articles : seuls les états différents du défaut sont publiés
    let mut tags_by_article: HashMap<String, Vec<String>> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT a.key, t.uid FROM article_tags x
             JOIN articles a ON a.id = x.article_id JOIN favorite_tags t ON t.id = x.tag_id",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (key, uid) = row?;
            tags_by_article.entry(key).or_default().push(uid);
        }
        for list in tags_by_article.values_mut() {
            list.sort();
        }
    }
    let mut present: HashSet<String> = HashSet::new();
    {
        let mut stmt = conn.prepare(
            "SELECT a.key, a.is_read, a.is_starred, a.published, a.feed_url, a.title, a.link,
                    CASE WHEN a.is_starred = 1 THEN a.summary ELSE '' END,
                    a.image_url, a.author, a.source_title, COALESCE(f.custom_title, f.title)
             FROM articles a LEFT JOIN feeds f ON f.id = a.feed_id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)? != 0,
                r.get::<_, i64>(2)? != 0,
                r.get::<_, f64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, Option<String>>(11)?,
            ))
        })?;
        for row in rows {
            let (key, read, starred, published, feed_url, title, link, summary, image_url, author, source_title, feed_title) = row?;
            present.insert(key.clone());
            let tags = tags_by_article.remove(&key).filter(|t| !t.is_empty());
            let meta = if starred {
                let source = crate::query::source_name(feed_title, source_title, &feed_url);
                Some(ArticleMeta { feed_url, title, link, summary, image_url, author, source: Some(source) })
            } else {
                None
            };
            let mut entry = ArticleEntry { read, starred, published, modified: now, meta, tags };
            if let Some(old) = prev.articles.get(&key) {
                if old.read == entry.read && old.starred == entry.starred && old.tag_list() == entry.tag_list() {
                    entry.modified = old.modified;
                }
                snap.articles.insert(key, entry); // y compris un retour à « non lu », daté
            } else if !entry.is_default() {
                snap.articles.insert(key, entry);
            }
        }
    }
    // Articles disparus d'ici (supprimés par l'ancienneté) : on garde leur état sans le modifier
    for (key, old) in &prev.articles {
        if !present.contains(key) && (old.starred || (old.published > retention && old.modified > limit)) {
            snap.articles.insert(key.clone(), old.clone());
        }
    }
    // Inutile de traîner éternellement des articles revenus à l'état par défaut
    snap.articles.retain(|_, v| !v.is_default() || v.modified > limit);
    Ok(snap)
}

fn article_tag_uids(conn: &Connection, key: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.uid FROM article_tags x
         JOIN articles a ON a.id = x.article_id JOIN favorite_tags t ON t.id = x.tag_id
         WHERE a.key = ?1 ORDER BY t.uid",
    )?;
    let rows = stmt.query_map(params![key], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Remplace les tags d'un article (les tags inconnus de ce Mac sont ignorés).
fn set_article_tags(conn: &Connection, key: &str, uids: &[String]) -> Result<()> {
    let mut wanted = uids.to_vec();
    wanted.sort();
    if article_tag_uids(conn, key)? == wanted {
        return Ok(());
    }
    let Some(article_id) = conn
        .query_row("SELECT id FROM articles WHERE key = ?1", params![key], |r| r.get::<_, i64>(0))
        .optional()?
    else {
        return Ok(());
    };
    conn.execute("DELETE FROM article_tags WHERE article_id = ?1", params![article_id])?;
    for uid in uids {
        conn.execute(
            "INSERT OR IGNORE INTO article_tags (article_id, tag_id)
             SELECT ?1, id FROM favorite_tags WHERE uid = ?2",
            params![article_id, uid],
        )?;
    }
    Ok(())
}

/// Articles arrivés depuis la dernière synchro : ils prennent l'état synchronisé
/// (lu ou favori sur l'autre Mac) au lieu d'être vus comme « remis à non lu ».
fn apply_pending_article_states(conn: &Connection, state: &LocalState) -> Result<()> {
    if state.last_snapshot.articles.is_empty() {
        return Ok(());
    }
    let keys: Vec<String> = all_article_keys(conn)?.into_iter().filter(|k| !state.known_articles.contains(k)).collect();
    for key in keys {
        let Some(entry) = state.last_snapshot.articles.get(&key) else { continue };
        conn.execute("UPDATE articles SET is_read = ?2 WHERE key = ?1", params![key, entry.read])?;
        actions::set_starred(conn, std::slice::from_ref(&key), entry.starred)?;
        set_article_tags(conn, &key, &entry.tag_list())?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Fusion
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Merged {
    created_feeds: Vec<String>,
    changed: bool,
}

fn folder_id(conn: &Connection, uid: &str) -> Result<Option<i64>> {
    Ok(conn.query_row("SELECT id FROM folders WHERE uid = ?1", params![uid], |r| r.get(0)).optional()?)
}

/// Applique les éléments plus récents d'un autre Mac.
fn merge(conn: &Connection, remote: &Snapshot, current: &mut Snapshot, now: f64) -> Result<Merged> {
    let mut out = Merged::default();

    // Dossiers d'abord (les flux y sont rangés)
    for (uid, entry) in &remote.folders {
        if current.folders.get(uid).is_some_and(|local| local.modified >= entry.modified) {
            continue;
        }
        out.changed = true;
        let exists = folder_id(conn, uid)?.is_some();
        if entry.deleted {
            if exists {
                manage::delete_folder(conn, uid)?;
            }
        } else if exists {
            conn.execute(
                "UPDATE folders SET name = ?2, symbol = ?3, color_name = ?4 WHERE uid = ?1",
                params![uid, entry.name, entry.symbol, entry.color],
            )?;
        } else if let Some(twin) = twin_folder(conn, &entry.name, &remote.folders)? {
            // Même dossier créé séparément sur les deux Mac : on les réunit sous un seul identifiant
            conn.execute(
                "UPDATE folders SET uid = ?1, symbol = ?2, color_name = ?3 WHERE uid = ?4",
                params![uid, entry.symbol, entry.color, twin],
            )?;
            current.folders.remove(&twin);
            for feed in current.feeds.values_mut() {
                if feed.folder.as_deref() == Some(twin.as_str()) {
                    feed.folder = Some(uid.clone());
                }
            }
        } else {
            conn.execute(
                "INSERT INTO folders (uid, name, created_at, symbol, color_name) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![uid, entry.name, now, entry.symbol, entry.color],
            )?;
        }
        current.folders.insert(uid.clone(), entry.clone());
    }

    // Tags des favoris (avant les articles, qui y font référence)
    let remote_tags = remote.tags.clone().unwrap_or_default();
    let mut current_tags = current.tags.clone().unwrap_or_default();
    for (uid, entry) in &remote_tags {
        if current_tags.get(uid).is_some_and(|local| local.modified >= entry.modified) {
            continue;
        }
        out.changed = true;
        let exists: bool = conn
            .query_row("SELECT 1 FROM favorite_tags WHERE uid = ?1", params![uid], |_| Ok(()))
            .optional()?
            .is_some();
        if entry.deleted {
            if exists {
                manage::delete_tag(conn, uid)?;
            }
        } else if exists {
            conn.execute("UPDATE favorite_tags SET name = ?2 WHERE uid = ?1", params![uid, entry.name])?;
        } else if let Some(twin) = twin_tag(conn, &entry.name, &remote_tags)? {
            // Même tag créé séparément sur les deux Mac : un seul identifiant
            conn.execute("UPDATE favorite_tags SET uid = ?1 WHERE uid = ?2", params![uid, twin])?;
            current_tags.remove(&twin);
        } else {
            conn.execute(
                "INSERT INTO favorite_tags (uid, name, created_at) VALUES (?1, ?2, ?3)",
                params![uid, entry.name, now],
            )?;
        }
        current_tags.insert(uid.clone(), entry.clone());
    }
    current.tags = if current_tags.is_empty() { None } else { Some(current_tags) };

    // Changement d'adresse fait sur un autre Mac (`previousURL`) : on fait la même chose ici, pour
    // garder les articles, leur état lu / favori, leurs tags et les règles de filtrage. Cela doit
    // précéder la boucle suivante, qui supprimerait sinon l'ancien flux (son entrée est une pierre
    // tombale) avant que ses articles aient pu être rattachés à la nouvelle adresse.
    for (url, entry) in &remote.feeds {
        let Some(old) = entry.previous_url.as_deref() else { continue };
        if entry.deleted || old == url {
            continue;
        }
        if current.feeds.get(url).is_some_and(|local| local.modified >= entry.modified) {
            continue;
        }
        let has = |u: &str| -> Result<bool> {
            Ok(conn.query_row("SELECT COUNT(*) FROM feeds WHERE url = ?1", params![u], |r| r.get::<_, i64>(0))? > 0)
        };
        if has(old)? && !has(url)? {
            manage::migrate_feed(conn, old, url)?;
            out.changed = true;
        }
    }

    // Flux
    for (url, entry) in &remote.feeds {
        if current.feeds.get(url).is_some_and(|local| local.modified >= entry.modified) {
            continue;
        }
        out.changed = true;
        let existing: Option<i64> =
            conn.query_row("SELECT id FROM feeds WHERE url = ?1", params![url], |r| r.get(0)).optional()?;
        if entry.deleted {
            if existing.is_some() {
                manage::delete_feed(conn, url)?;
            }
        } else {
            let feed_id = match existing {
                Some(id) => id,
                None => {
                    conn.execute(
                        "INSERT INTO feeds (url, title, site_url, added_at) VALUES (?1, ?2, ?3, ?4)",
                        params![url, entry.title, entry.site_url, now],
                    )?;
                    let id = conn.last_insert_rowid();
                    out.created_feeds.push(url.clone());
                    // Favoris conservés d'une ancienne suppression de ce flux
                    conn.execute(
                        "UPDATE articles SET feed_id = ?1 WHERE feed_url = ?2 AND feed_id IS NULL",
                        params![id, url],
                    )?;
                    id
                }
            };
            let folder = match &entry.folder {
                Some(uid) => folder_id(conn, uid)?,
                None => None,
            };
            conn.execute(
                "UPDATE feeds SET custom_title = ?2, folder_id = ?3 WHERE id = ?1",
                params![feed_id, entry.custom_title, folder],
            )?;
        }
        current.feeds.insert(url.clone(), entry.clone());
    }

    // Règles de filtrage
    let mut filters_changed = false;
    for (uid, entry) in &remote.filters {
        if current.filters.get(uid).is_some_and(|local| local.modified >= entry.modified) {
            continue;
        }
        out.changed = true;
        filters_changed = true;
        let id: Option<i64> =
            conn.query_row("SELECT id FROM filter_rules WHERE uid = ?1", params![uid], |r| r.get(0)).optional()?;
        let scope = entry.scope();
        if entry.deleted {
            if id.is_some() {
                filters::release(conn, uid)?;
                conn.execute("DELETE FROM filter_rules WHERE uid = ?1", params![uid])?;
            }
        } else if filters::Action::parse(&entry.action).is_none() {
            // Action inconnue de cette version : on garde l'entrée sans l'appliquer
        } else if let Some(id) = id {
            filters::release(conn, uid)?;
            conn.execute(
                "UPDATE filter_rules SET keyword = ?2, whole_word = ?3, action = ?4 WHERE id = ?1",
                params![id, entry.keyword, entry.whole_word, entry.action],
            )?;
            filters::set_feeds(conn, id, &scope)?;
        } else if let Some(twin) = twin_filter(conn, entry, &scope, &remote.filters)? {
            // Même règle créée sur les deux Mac : un seul identifiant
            conn.execute("UPDATE filter_rules SET uid = ?1 WHERE uid = ?2", params![uid, twin])?;
            current.filters.remove(&twin);
        } else {
            conn.execute(
                "INSERT INTO filter_rules (uid, keyword, whole_word, action, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![uid, entry.keyword, entry.whole_word, entry.action, now],
            )?;
            filters::set_feeds(conn, conn.last_insert_rowid(), &scope)?;
        }
        current.filters.insert(uid.clone(), entry.clone());
    }
    if filters_changed {
        filters::apply(conn, None)?;
    }

    // Dossiers intelligents
    let mut smart_changed = false;
    for (uid, entry) in &remote.smart_folders {
        if current.smart_folders.get(uid).is_some_and(|local| local.modified >= entry.modified) {
            continue;
        }
        out.changed = true;
        smart_changed = true;
        let id: Option<i64> =
            conn.query_row("SELECT id FROM smart_folders WHERE uid = ?1", params![uid], |r| r.get(0)).optional()?;
        let keywords = smart::clean_keywords(&entry.keywords);
        if entry.deleted {
            if id.is_some() {
                smart::delete(conn, uid)?;
            }
        } else if keywords.is_empty() || entry.name.trim().is_empty() {
            // Entrée inutilisable : on la garde sans l'appliquer
        } else if let Some(id) = id {
            conn.execute(
                "UPDATE smart_folders SET name = ?2, whole_word = ?3, symbol = ?4, color_name = ?5 WHERE id = ?1",
                params![id, entry.name, entry.whole_word, entry.symbol, entry.color],
            )?;
            smart::set_keywords(conn, id, &keywords)?;
        } else if let Some(twin) = twin_smart_folder(conn, &entry.name, &remote.smart_folders)? {
            // Même dossier créé séparément sur les deux Mac : un seul identifiant, le contenu le plus récent
            conn.execute(
                "UPDATE smart_folders SET uid = ?1, name = ?2, whole_word = ?3, symbol = ?4, color_name = ?5 WHERE uid = ?6",
                params![uid, entry.name, entry.whole_word, entry.symbol, entry.color, twin],
            )?;
            let twin_id: i64 = conn.query_row("SELECT id FROM smart_folders WHERE uid = ?1", params![uid], |r| r.get(0))?;
            smart::set_keywords(conn, twin_id, &keywords)?;
            current.smart_folders.remove(&twin);
        } else {
            conn.execute(
                "INSERT INTO smart_folders (uid, name, whole_word, symbol, color_name, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![uid, entry.name, entry.whole_word, entry.symbol, entry.color, now],
            )?;
            smart::set_keywords(conn, conn.last_insert_rowid(), &keywords)?;
        }
        current.smart_folders.insert(uid.clone(), entry.clone());
    }
    if smart_changed {
        smart::rebuild(conn)?;
    }

    // États des articles
    for (key, entry) in &remote.articles {
        if current.articles.get(key).is_some_and(|local| local.modified >= entry.modified) {
            continue;
        }
        let id: Option<i64> =
            conn.query_row("SELECT id FROM articles WHERE key = ?1", params![key], |r| r.get(0)).optional()?;
        if id.is_some() {
            out.changed = true;
            conn.execute("UPDATE articles SET is_read = ?2 WHERE key = ?1", params![key, entry.read])?;
            set_article_tags(conn, key, &entry.tag_list())?;
            actions::set_starred(conn, std::slice::from_ref(key), entry.starred)?;
        } else if let (true, Some(meta)) = (entry.starred, &entry.meta) {
            // Favori que ce Mac n'a pas (article plus ancien que son flux) : on le recrée
            out.changed = true;
            let feed_id: Option<i64> = conn
                .query_row("SELECT id FROM feeds WHERE url = ?1", params![meta.feed_url], |r| r.get(0))
                .optional()?;
            conn.execute(
                "INSERT INTO articles (key, feed_id, feed_url, title, link, summary, image_url, author,
                                       published, is_read, is_starred, source_title)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11)",
                params![
                    key,
                    feed_id,
                    meta.feed_url,
                    meta.title,
                    meta.link,
                    meta.summary,
                    meta.image_url,
                    meta.author,
                    entry.published,
                    entry.read,
                    meta.source
                ],
            )?;
            set_article_tags(conn, key, &entry.tag_list())?;
        }
        // Article absent et simple « lu » : l'état est gardé, il s'appliquera à son arrivée
        current.articles.insert(key.clone(), entry.clone());
    }
    Ok(out)
}

fn twin_folder(conn: &Connection, name: &str, remote: &BTreeMap<String, FolderEntry>) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT uid, name FROM folders")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows
        .into_iter()
        .find(|(uid, n)| n.to_lowercase() == name.to_lowercase() && !remote.contains_key(uid))
        .map(|(uid, _)| uid))
}

fn twin_tag(conn: &Connection, name: &str, remote: &BTreeMap<String, TagEntry>) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT uid, name FROM favorite_tags")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows
        .into_iter()
        .find(|(uid, n)| n.to_lowercase() == name.to_lowercase() && !remote.contains_key(uid))
        .map(|(uid, _)| uid))
}

fn twin_smart_folder(conn: &Connection, name: &str, remote: &BTreeMap<String, SmartFolderEntry>) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT uid, name FROM smart_folders")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows
        .into_iter()
        .find(|(uid, n)| n.to_lowercase() == name.to_lowercase() && !remote.contains_key(uid))
        .map(|(uid, _)| uid))
}

fn twin_filter(
    conn: &Connection,
    entry: &FilterEntry,
    scope: &[String],
    remote: &BTreeMap<String, FilterEntry>,
) -> Result<Option<String>> {
    let wanted = crate::query::fold(&entry.keyword);
    for rule in filters::list(conn)? {
        let mut feeds = rule.feed_urls.clone();
        feeds.sort();
        if crate::query::fold(&rule.keyword) == wanted && feeds == scope && !remote.contains_key(&rule.uid) {
            return Ok(Some(rule.uid));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mac(id: &str) -> (Connection, Device) {
        (db::open_in_memory().unwrap(), Device { id: id.into(), name: format!("Mac {id}") })
    }

    fn run(c: &(Connection, Device), dir: &Path, now: f64) -> SyncReport {
        sync(&c.0, dir, &c.1, 60, now).unwrap()
    }

    fn count(c: &Connection, sql: &str) -> i64 {
        c.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn folders_feeds_tags_and_read_state_travel() {
        let dir = tempfile::tempdir().unwrap();
        let a = mac("A");
        let b = mac("B");
        a.0.execute_batch(
            "INSERT INTO folders (id, uid, name, created_at) VALUES (1, 'F1', 'Tech', 0);
             INSERT INTO feeds (id, url, title, added_at, folder_id) VALUES (1, 'u1', 'Korben', 0, 1);
             INSERT INTO favorite_tags (id, uid, name, created_at) VALUES (1, 'T1', 'IA', 0);
             INSERT INTO articles (id, key, feed_id, feed_url, title, published, is_read) VALUES (1, 'u1|1', 1, 'u1', 'A', 10, 0);",
        )
        .unwrap();
        run(&a, dir.path(), 1000.0); // état de départ de A
        a.0.execute("UPDATE articles SET is_read = 1", []).unwrap();
        let r = run(&a, dir.path(), 1100.0);
        assert!(r.published);

        let r = run(&b, dir.path(), 1200.0);
        assert!(r.changed);
        assert_eq!(r.new_feeds, vec!["u1".to_string()]);
        assert_eq!(count(&b.0, "SELECT COUNT(*) FROM folders WHERE name = 'Tech'"), 1);
        assert_eq!(count(&b.0, "SELECT COUNT(*) FROM feeds f JOIN folders d ON d.id = f.folder_id WHERE f.url = 'u1'"), 1);
        assert_eq!(count(&b.0, "SELECT COUNT(*) FROM favorite_tags WHERE name = 'IA'"), 1);

        // L'article arrive sur B après l'actualisation : il prend l'état « lu » de A
        b.0.execute(
            "INSERT INTO articles (key, feed_id, feed_url, title, published) VALUES ('u1|1', (SELECT id FROM feeds WHERE url = 'u1'), 'u1', 'A', 10)",
            [],
        )
        .unwrap();
        run(&b, dir.path(), 1300.0);
        assert_eq!(count(&b.0, "SELECT is_read FROM articles WHERE key = 'u1|1'"), 1);
    }

    #[test]
    fn smart_folders_travel_and_follow_edits_and_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let a = mac("A");
        let b = mac("B");
        a.0.execute_batch(
            "INSERT INTO feeds (id, url, title, added_at) VALUES (1, 'u1', 'Korben', 0);
             INSERT INTO articles (id, key, feed_id, feed_url, title, published) VALUES
               (1, 'u1|1', 1, 'u1', 'Nouvel iPhone', 10), (2, 'u1|2', 1, 'u1', 'Météo', 20);",
        )
        .unwrap();
        let uid = smart::create(&a.0, "Tech", &["iphone".to_string(), "mac".to_string()], true).unwrap();
        smart::set_style(&a.0, &uid, Some("cpu"), Some("blue")).unwrap();
        run(&a, dir.path(), 1000.0);

        // B reçoit le dossier avec ses mots-clés et son style ; ses articles y sont rangés une fois présents
        b.0.execute_batch(
            "INSERT INTO feeds (id, url, title, added_at) VALUES (1, 'u1', 'Korben', 0);
             INSERT INTO articles (id, key, feed_id, feed_url, title, published) VALUES (1, 'u1|1', 1, 'u1', 'Nouvel iPhone', 10);",
        )
        .unwrap();
        let r = run(&b, dir.path(), 1100.0);
        assert!(r.changed && r.incompatible.is_empty());
        let folders = smart::list(&b.0).unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].uid, uid);
        assert_eq!(folders[0].keywords, vec!["iphone".to_string(), "mac".to_string()]);
        assert_eq!((folders[0].symbol.as_deref(), folders[0].color_name.as_deref()), (Some("cpu"), Some("blue")));
        assert_eq!(folders[0].article_count, 1);

        // Modification sur B, puis suppression sur A
        smart::update(&b.0, &uid, "Technologie", &["iphone".to_string()], false).unwrap();
        run(&b, dir.path(), 1200.0);
        run(&a, dir.path(), 1300.0);
        let on_a = smart::list(&a.0).unwrap();
        assert_eq!((on_a[0].name.as_str(), on_a[0].keywords.len(), on_a[0].whole_word), ("Technologie", 1, false));
        smart::delete(&a.0, &uid).unwrap();
        run(&a, dir.path(), 1400.0);
        run(&b, dir.path(), 1500.0);
        assert!(smart::list(&b.0).unwrap().is_empty());
    }

    #[test]
    fn same_smart_folder_created_on_both_macs_is_merged() {
        let dir = tempfile::tempdir().unwrap();
        let a = mac("A");
        let b = mac("B");
        smart::create(&a.0, "Tech", &["iphone".to_string()], true).unwrap();
        smart::create(&b.0, "tech", &["iphone".to_string()], true).unwrap();
        run(&a, dir.path(), 1000.0);
        run(&b, dir.path(), 1100.0);
        run(&a, dir.path(), 1200.0);
        assert_eq!(smart::list(&a.0).unwrap().len(), 1);
        assert_eq!(smart::list(&b.0).unwrap().len(), 1);
    }

    #[test]
    fn files_of_another_format_version_are_ignored_and_reported() {
        let dir = tempfile::tempdir().unwrap();
        let a = mac("A");
        let old = Snapshot {
            version: 1,
            device: "OLD".into(),
            device_name: "Vieux Mac".into(),
            updated_at: 5.0,
            folders: BTreeMap::from([(
                "F9".to_string(),
                FolderEntry { name: "Ancien".into(), symbol: None, color: None, deleted: false, modified: 5.0 },
            )]),
            ..Snapshot::default()
        };
        std::fs::write(dir.path().join("OLD.json"), serde_json::to_vec(&old).unwrap()).unwrap();
        let r = run(&a, dir.path(), 1000.0);
        assert_eq!(r.incompatible.len(), 1);
        assert_eq!(r.incompatible[0].name, "Vieux Mac");
        assert!(r.others.is_empty() && !r.changed);
        assert_eq!(count(&a.0, "SELECT COUNT(*) FROM folders"), 0);
        // Notre fichier est bien au nouveau format
        let own: Snapshot = serde_json::from_slice(&std::fs::read(dir.path().join("A.json")).unwrap()).unwrap();
        assert_eq!(own.version, SYNC_VERSION);
    }

    #[test]
    fn address_change_on_one_mac_keeps_articles_on_the_other() {
        let dir = tempfile::tempdir().unwrap();
        let a = mac("A");
        let b = mac("B");
        a.0.execute_batch(
            "INSERT INTO feeds (id, url, title, added_at) VALUES (1, 'u1', 'Korben', 0);
             INSERT INTO articles (id, key, feed_id, feed_url, title, published) VALUES (1, 'u1|1', 1, 'u1', 'A', 10);",
        )
        .unwrap();
        run(&a, dir.path(), 1000.0);
        run(&b, dir.path(), 1100.0);
        // B reçoit l'article et le met en favori
        b.0.execute(
            "INSERT INTO articles (key, feed_id, feed_url, title, published, is_starred) VALUES ('u1|1', (SELECT id FROM feeds WHERE url = 'u1'), 'u1', 'A', 10, 1)",
            [],
        )
        .unwrap();
        let article_id: i64 = count(&b.0, "SELECT id FROM articles WHERE key = 'u1|1'");

        // A change l'adresse du flux, puis B se synchronise
        manage::migrate_feed(&a.0, "u1", "u2").unwrap();
        run(&a, dir.path(), 1200.0);
        let r = run(&b, dir.path(), 1300.0);

        assert!(r.changed);
        assert!(r.new_feeds.is_empty(), "le flux n'est pas recréé : {:?}", r.new_feeds);
        assert_eq!(count(&b.0, "SELECT COUNT(*) FROM feeds WHERE url = 'u1'"), 0);
        assert_eq!(count(&b.0, "SELECT COUNT(*) FROM feeds WHERE url = 'u2' AND previous_url = 'u1'"), 1);
        assert_eq!(count(&b.0, "SELECT id FROM articles WHERE key = 'u2|1' AND is_starred = 1"), article_id);
    }

    #[test]
    fn starred_article_is_recreated_and_second_sync_is_quiet() {
        let dir = tempfile::tempdir().unwrap();
        let a = mac("A");
        let b = mac("B");
        a.0.execute_batch(
            "INSERT INTO feeds (id, url, title, added_at) VALUES (1, 'u1', 'Korben', 0);
             INSERT INTO articles (id, key, feed_id, feed_url, title, summary, published, is_starred) VALUES (1, 'u1|1', 1, 'u1', 'Titre', 'Résumé', 10, 1);",
        )
        .unwrap();
        run(&a, dir.path(), 1000.0);
        run(&b, dir.path(), 1100.0);
        assert_eq!(count(&b.0, "SELECT COUNT(*) FROM articles WHERE key = 'u1|1' AND is_starred = 1"), 1);
        // Rien n'a changé : plus de réécriture, pas de rechargement
        let again = run(&b, dir.path(), 1200.0);
        assert!(!again.changed);
        assert!(!run(&a, dir.path(), 1300.0).changed);
    }

    #[test]
    fn deletion_propagates_and_latest_change_wins() {
        let dir = tempfile::tempdir().unwrap();
        let a = mac("A");
        let b = mac("B");
        a.0.execute_batch(
            "INSERT INTO feeds (id, url, title, added_at) VALUES (1, 'u1', 'Korben', 0), (2, 'u2', 'Autre', 0);",
        )
        .unwrap();
        run(&a, dir.path(), 1000.0);
        run(&b, dir.path(), 1100.0);
        run(&a, dir.path(), 1150.0);

        // A supprime u1 ; B renomme u2 plus tard que A
        manage::delete_feed(&a.0, "u1").unwrap();
        manage::rename_feed(&a.0, "u2", "Nom de A").unwrap();
        run(&a, dir.path(), 2000.0);
        manage::rename_feed(&b.0, "u2", "Nom de B").unwrap();
        run(&b, dir.path(), 3000.0); // B voit la suppression et son renommage est plus récent
        assert_eq!(count(&b.0, "SELECT COUNT(*) FROM feeds WHERE url = 'u1'"), 0);
        assert_eq!(count(&b.0, "SELECT COUNT(*) FROM feeds WHERE custom_title = 'Nom de B'"), 1);

        run(&a, dir.path(), 4000.0);
        assert_eq!(count(&a.0, "SELECT COUNT(*) FROM feeds WHERE custom_title = 'Nom de B'"), 1);
    }

    #[test]
    fn filter_rules_travel_and_apply() {
        let dir = tempfile::tempdir().unwrap();
        let a = mac("A");
        let b = mac("B");
        a.0.execute_batch("INSERT INTO feeds (id, url, title, added_at) VALUES (1, 'u1', 'Korben', 0);").unwrap();
        filters::add(&a.0, "football", true, "hide", &[]).unwrap();
        run(&a, dir.path(), 1000.0);
        b.0.execute_batch(
            "INSERT INTO feeds (id, url, title, added_at) VALUES (1, 'u1', 'Korben', 0);
             INSERT INTO articles (id, key, feed_id, feed_url, title, published) VALUES (1, 'u1|1', 1, 'u1', 'Le football en direct', 10);",
        )
        .unwrap();
        run(&b, dir.path(), 1100.0);
        assert_eq!(count(&b.0, "SELECT COUNT(*) FROM filter_rules WHERE keyword = 'football'"), 1);
        assert_eq!(count(&b.0, "SELECT is_hidden FROM articles WHERE key = 'u1|1'"), 1);
    }

    #[test]
    fn disable_removes_our_file_and_state() {
        let dir = tempfile::tempdir().unwrap();
        let a = mac("A");
        run(&a, dir.path(), 1000.0);
        assert!(dir.path().join("A.json").exists());
        disable(&a.0, dir.path(), &a.1).unwrap();
        assert!(!dir.path().join("A.json").exists());
        assert_eq!(count(&a.0, "SELECT COUNT(*) FROM settings WHERE key = 'syncState'"), 0);
    }
}
