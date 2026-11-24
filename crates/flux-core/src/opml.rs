//! Lecture et écriture de fichiers OPML, le format standard d'échange d'abonnements
//! entre lecteurs RSS (NetNewsWire, Reeder, Feedly…). Reprend `OPML.swift`.

use std::collections::{HashMap, HashSet};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use rusqlite::Connection;
use serde::Serialize;
use url::Url;

use crate::error::Result;
use crate::feeds::add_feed_row;
use crate::manage::create_folder;
use crate::query::fold;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub title: String,
    pub xml_url: String,
    pub html_url: Option<String>,
    /// Nom du dossier parent (les dossiers imbriqués sont aplatis sur le plus proche).
    pub folder: Option<String>,
}

/// Attributs d'un `<outline>`, clés en minuscules (la casse varie : xmlUrl, xmlURL…).
fn attributes(e: &BytesStart<'_>) -> HashMap<String, String> {
    e.attributes()
        .flatten()
        .map(|a| {
            let key = String::from_utf8_lossy(a.key.as_ref()).to_lowercase();
            let value = a
                .unescape_value()
                .map(|v| v.into_owned())
                .unwrap_or_else(|_| String::from_utf8_lossy(&a.value).into_owned());
            (key, value)
        })
        .collect()
}

pub fn parse(data: &[u8]) -> Vec<Entry> {
    let text = String::from_utf8_lossy(data);
    let mut reader = Reader::from_str(&text);
    reader.config_mut().check_end_names = false;

    let mut entries = Vec::new();
    // Pile des <outline> ouverts : nom de dossier, ou None pour un flux (ou un dossier sans nom).
    let mut stack: Vec<Option<String>> = Vec::new();

    let mut outline = |e: &BytesStart<'_>, stack: &mut Vec<Option<String>>, open: bool| {
        if !e.name().as_ref().eq_ignore_ascii_case(b"outline") {
            return false;
        }
        let a = attributes(e);
        let name = a.get("title").or_else(|| a.get("text")).map(|s| s.trim().to_string()).unwrap_or_default();
        match a.get("xmlurl").map(|s| s.trim()).filter(|s| !s.is_empty()) {
            Some(xml) => {
                let folder = stack.iter().rev().find_map(|f| f.clone());
                entries.push(Entry {
                    title: name,
                    xml_url: xml.to_string(),
                    html_url: a.get("htmlurl").cloned().filter(|s| !s.is_empty()),
                    folder,
                });
                if open {
                    stack.push(None);
                }
            }
            None if open => stack.push((!name.is_empty()).then_some(name)),
            None => {}
        }
        open
    };

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                outline(&e, &mut stack, true);
            }
            Ok(Event::Empty(e)) => {
                outline(&e, &mut stack, false);
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref().eq_ignore_ascii_case(b"outline") {
                    stack.pop();
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    entries
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub added: usize,
    /// Déjà présents ou adresse invalide.
    pub skipped: usize,
}

/// Ajoute les flux du fichier (sans les télécharger) : crée les dossiers manquants et ignore
/// les flux déjà présents. Les articles arrivent à la prochaine actualisation.
pub fn import(conn: &Connection, data: &[u8]) -> Result<ImportResult> {
    let mut result = ImportResult::default();
    let mut known: HashSet<String> = {
        let mut stmt = conn.prepare("SELECT url FROM feeds")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.collect::<std::result::Result<_, _>>()?
    };
    let mut folders: HashMap<String, String> = {
        let mut stmt = conn.prepare("SELECT name, uid FROM folders")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?.into_iter().map(|(n, u)| (n.to_lowercase(), u)).collect()
    };

    let tx = conn.unchecked_transaction()?;
    for entry in parse(data) {
        let Ok(url) = Url::parse(entry.xml_url.trim()) else {
            result.skipped += 1;
            continue;
        };
        if url.host_str().is_none() || !matches!(url.scheme(), "http" | "https") {
            result.skipped += 1;
            continue;
        }
        let key = url.to_string();
        if !known.insert(key.clone()) {
            result.skipped += 1;
            continue;
        }

        let folder_uid = match entry.folder.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            Some(name) => match folders.get(&name.to_lowercase()) {
                Some(uid) => Some(uid.clone()),
                None => {
                    let created = create_folder(&tx, name)?;
                    if let Some(uid) = &created {
                        folders.insert(name.to_lowercase(), uid.clone());
                    }
                    created
                }
            },
            None => None,
        };
        let title = if entry.title.is_empty() { url.host_str().unwrap_or(&key).to_string() } else { entry.title.clone() };
        add_feed_row(&tx, &key, &title, entry.html_url.as_deref(), folder_uid.as_deref())?;
        result.added += 1;
    }
    tx.commit()?;
    Ok(result)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Tous les abonnements : dossiers d'abord (par nom), puis les flux sans dossier.
pub fn export(conn: &Connection) -> Result<String> {
    struct Row {
        title: String,
        url: String,
        site: Option<String>,
        folder: Option<String>,
    }
    let mut rows: Vec<Row> = {
        let mut stmt = conn.prepare(
            "SELECT COALESCE(NULLIF(f.custom_title, ''), f.title), f.url, f.site_url, d.name
             FROM feeds f LEFT JOIN folders d ON d.id = f.folder_id",
        )?;
        let mapped = stmt.query_map([], |r| {
            Ok(Row { title: r.get(0)?, url: r.get(1)?, site: r.get(2)?, folder: r.get(3)? })
        })?;
        mapped.collect::<std::result::Result<_, _>>()?
    };
    rows.sort_by_key(|r| fold(&r.title));

    let line = |r: &Row, indent: &str| {
        let site = r.site.as_deref().map(|s| format!(" htmlUrl=\"{}\"", esc(s))).unwrap_or_default();
        format!(
            "{indent}<outline type=\"rss\" text=\"{t}\" title=\"{t}\" xmlUrl=\"{u}\"{site}/>",
            t = esc(&r.title),
            u = esc(&r.url)
        )
    };

    let mut names: Vec<&str> = rows.iter().filter_map(|r| r.folder.as_deref()).collect::<HashSet<_>>().into_iter().collect();
    names.sort_by_key(|n| fold(n));

    let mut body: Vec<String> = Vec::new();
    for name in names {
        body.push(format!("    <outline text=\"{n}\" title=\"{n}\">", n = esc(name)));
        body.extend(rows.iter().filter(|r| r.folder.as_deref() == Some(name)).map(|r| line(r, "      ")));
        body.push("    </outline>".to_string());
    }
    body.extend(rows.iter().filter(|r| r.folder.is_none()).map(|r| line(r, "    ")));

    let date = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opml version=\"2.0\">\n  <head>\n    <title>Abonnements Flux</title>\n    <dateCreated>{date}</dateCreated>\n  </head>\n  <body>\n{}\n  </body>\n</opml>\n",
        body.join("\n")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<opml version="2.0"><body>
  <outline text="Tech">
    <outline type="rss" text="Un &amp; Deux" xmlURL="https://a.example/feed" htmlUrl="https://a.example"/>
    <outline text="Sous-dossier"><outline type="rss" title="B" XMLURL="https://b.example/rss"/></outline>
  </outline>
  <outline type="rss" text="Seul" xmlUrl="https://c.example/atom"/>
  <outline type="rss" text="Cassé" xmlUrl="pas une adresse"/>
</body></opml>"#;

    #[test]
    fn parses_folders_flattened_and_attribute_case() {
        let e = parse(SAMPLE.as_bytes());
        assert_eq!(e.len(), 4);
        assert_eq!(e[0].title, "Un & Deux");
        assert_eq!(e[0].folder.as_deref(), Some("Tech"));
        assert_eq!(e[0].html_url.as_deref(), Some("https://a.example"));
        assert_eq!(e[1].folder.as_deref(), Some("Sous-dossier")); // le dossier le plus proche
        assert_eq!(e[2].folder, None);
    }

    #[test]
    fn import_creates_folders_skips_known_and_invalid() {
        let c = crate::db::open_in_memory().unwrap();
        c.execute("INSERT INTO folders (uid, name, created_at) VALUES ('U1', 'tech', 0)", []).unwrap();
        add_feed_row(&c, "https://c.example/atom", "Déjà là", None, None).unwrap();
        let r = import(&c, SAMPLE.as_bytes()).unwrap();
        assert_eq!(r, ImportResult { added: 2, skipped: 2 });
        // le dossier « Tech » existait déjà (casse ignorée) ; « Sous-dossier » est créé
        let folders: i64 = c.query_row("SELECT COUNT(*) FROM folders", [], |r| r.get(0)).unwrap();
        assert_eq!(folders, 2);
        let in_tech: i64 = c
            .query_row("SELECT COUNT(*) FROM feeds WHERE folder_id = (SELECT id FROM folders WHERE uid = 'U1')", [], |r| r.get(0))
            .unwrap();
        assert_eq!(in_tech, 1);
        // second import : tout est déjà présent
        assert_eq!(import(&c, SAMPLE.as_bytes()).unwrap().added, 0);
    }

    #[test]
    fn export_then_import_round_trips() {
        let c = crate::db::open_in_memory().unwrap();
        import(&c, SAMPLE.as_bytes()).unwrap();
        c.execute("UPDATE feeds SET custom_title = 'Renommé <ok>' WHERE url = 'https://c.example/atom'", []).unwrap();
        let xml = export(&c).unwrap();
        assert!(xml.contains("text=\"Renommé &lt;ok&gt;\""));
        assert!(xml.contains("<outline text=\"Tech\" title=\"Tech\">"));

        let d = crate::db::open_in_memory().unwrap();
        let r = import(&d, xml.as_bytes()).unwrap();
        assert_eq!(r.added, 3);
        let title: String = d.query_row("SELECT title FROM feeds WHERE url = 'https://c.example/atom'", [], |r| r.get(0)).unwrap();
        assert_eq!(title, "Renommé <ok>");
    }
}
