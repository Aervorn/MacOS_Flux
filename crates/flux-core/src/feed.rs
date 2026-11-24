//! Lecture des flux RSS 2.0, RSS 1.0 (RDF), Atom et JSON Feed (même comportement que `FeedParser.swift`).

use std::borrow::Cow;

use chrono::{DateTime, NaiveDate, NaiveDateTime};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use serde_json::Value;

use crate::html::{decode_entities, plain_text};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedItem {
    pub guid: Option<String>,
    pub title: Option<String>,
    pub link: Option<String>,
    pub summary_html: Option<String>,
    pub content_html: Option<String>,
    pub image_url: Option<String>,
    pub author: Option<String>,
    /// Secondes Unix
    pub published: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedFeed {
    pub title: Option<String>,
    pub site_url: Option<String>,
    pub items: Vec<ParsedItem>,
}

/// RSS, Atom ou JSON Feed (détecté automatiquement). None si ce n'est pas un flux.
pub fn parse(data: &[u8]) -> Option<ParsedFeed> {
    let first = data.iter().find(|b| !matches!(**b, b' ' | b'\n' | b'\r' | b'\t' | 0xEF | 0xBB | 0xBF));
    if first == Some(&b'{') {
        return parse_json(data);
    }
    parse_xml(data)
}

// --- XML ---------------------------------------------------------------------------------------

struct XmlState {
    feed: ParsedFeed,
    item: Option<ParsedItem>,
    buffers: Vec<String>,
    saw_root: bool,
}

fn looks_like_image(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url).to_lowercase();
    [".jpg", ".jpeg", ".png", ".webp", ".gif", ".avif"].iter().any(|ext| path.ends_with(ext))
}

fn attr(attrs: &[(String, String)], name: &str) -> Option<String> {
    attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
}

impl XmlState {
    fn set_image(&mut self, url: &str) {
        if let Some(item) = self.item.as_mut() {
            if item.image_url.is_none() {
                item.image_url = Some(url.to_string());
            }
        }
    }

    fn start(&mut self, name: &str, attrs: &[(String, String)]) {
        self.buffers.push(String::new());
        match name {
            "rss" | "rdf:rdf" | "feed" => self.saw_root = true,
            "item" | "entry" => self.item = Some(ParsedItem::default()),
            "link" => {
                // Atom : <link rel="alternate" href="…"/>
                let Some(href) = attr(attrs, "href") else { return };
                let rel = attr(attrs, "rel").unwrap_or_else(|| "alternate".into());
                if let Some(item) = self.item.as_mut() {
                    if rel == "alternate" {
                        if item.link.is_none() {
                            item.link = Some(href);
                        }
                    } else if rel == "enclosure" && attr(attrs, "type").unwrap_or_default().starts_with("image") {
                        self.set_image(&href);
                    }
                } else if rel == "alternate" && self.feed.site_url.is_none() {
                    self.feed.site_url = Some(href);
                }
            }
            "media:thumbnail" => {
                if let Some(u) = attr(attrs, "url") {
                    self.set_image(&u);
                }
            }
            "media:content" => {
                if let Some(u) = attr(attrs, "url") {
                    let is_image = attr(attrs, "medium").as_deref() == Some("image")
                        || attr(attrs, "type").unwrap_or_default().starts_with("image")
                        || looks_like_image(&u);
                    if is_image {
                        self.set_image(&u);
                    }
                }
            }
            "enclosure" => {
                if let Some(u) = attr(attrs, "url") {
                    if attr(attrs, "type").unwrap_or_default().starts_with("image") || looks_like_image(&u) {
                        self.set_image(&u);
                    }
                }
            }
            "itunes:image" => {
                if let Some(u) = attr(attrs, "href") {
                    self.set_image(&u);
                }
            }
            _ => {}
        }
    }

    fn text(&mut self, s: &str) {
        if let Some(last) = self.buffers.last_mut() {
            last.push_str(s);
        }
    }

    fn end(&mut self, name: &str) {
        let raw = self.buffers.pop().unwrap_or_default();
        // Le texte des enfants remonte au parent (utile pour <author><name>…</name></author>)
        if let Some(parent) = self.buffers.last_mut() {
            parent.push_str(&raw);
        }
        let value = raw.trim().to_string();

        if let Some(item) = self.item.as_mut() {
            match name {
                "item" | "entry" => {
                    if let Some(done) = self.item.take() {
                        self.feed.items.push(done);
                    }
                }
                "title" => {
                    if item.title.is_none() {
                        item.title = Some(value);
                    }
                }
                "link" => {
                    if !value.is_empty() && item.link.is_none() {
                        item.link = Some(value);
                    }
                }
                "guid" | "id" => {
                    if !value.is_empty() {
                        item.guid = Some(value);
                    }
                }
                "description" | "summary" => {
                    if !value.is_empty() && item.summary_html.is_none() {
                        item.summary_html = Some(value);
                    }
                }
                "content:encoded" | "content" => {
                    if !value.is_empty() {
                        item.content_html = Some(value);
                    }
                }
                "pubdate" | "published" | "issued" => {
                    item.published = parse_date(&value).or(item.published);
                }
                "updated" | "modified" | "dc:date" => {
                    if item.published.is_none() {
                        item.published = parse_date(&value);
                    }
                }
                "author" | "dc:creator" => {
                    if !value.is_empty() && item.author.is_none() {
                        item.author = Some(value);
                    }
                }
                _ => {}
            }
        } else {
            match name {
                "title" => {
                    if self.feed.title.is_none() && !value.is_empty() {
                        self.feed.title = Some(value);
                    }
                }
                "link" => {
                    if self.feed.site_url.is_none() && !value.is_empty() {
                        self.feed.site_url = Some(value);
                    }
                }
                _ => {}
            }
        }
    }
}

fn element(e: &BytesStart<'_>) -> (String, Vec<(String, String)>) {
    let name = String::from_utf8_lossy(e.name().as_ref()).to_lowercase();
    let attrs = e
        .attributes()
        .flatten()
        .map(|a| {
            let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
            let value = a
                .unescape_value()
                .map(|v| v.into_owned())
                .unwrap_or_else(|_| String::from_utf8_lossy(&a.value).into_owned());
            (key, value)
        })
        .collect();
    (name, attrs)
}

fn parse_xml(data: &[u8]) -> Option<ParsedFeed> {
    // Flux en ISO Latin 1 : on convertit en UTF-8 avant la lecture.
    let text: Cow<'_, str> = match std::str::from_utf8(data) {
        Ok(s) => Cow::Borrowed(s),
        Err(_) => Cow::Owned(data.iter().map(|&b| b as char).collect()),
    };
    let mut reader = Reader::from_str(&text);
    reader.config_mut().check_end_names = false;

    let mut state = XmlState { feed: ParsedFeed::default(), item: None, buffers: Vec::new(), saw_root: false };
    let mut finished_cleanly = true;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let (name, attrs) = element(&e);
                state.start(&name, &attrs);
            }
            Ok(Event::Empty(e)) => {
                let (name, attrs) = element(&e);
                state.start(&name, &attrs);
                state.end(&name);
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_lowercase();
                state.end(&name);
            }
            Ok(Event::Text(e)) => {
                let s = match e.unescape() {
                    Ok(s) => s.into_owned(),
                    // entité inconnue (&nbsp; dans un flux mal formé) : on garde le texte brut
                    Err(_) => String::from_utf8_lossy(&e).into_owned(),
                };
                state.text(&s);
            }
            Ok(Event::CData(e)) => state.text(&String::from_utf8_lossy(&e)),
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => {
                finished_cleanly = false;
                break;
            }
        }
    }
    if state.saw_root && (finished_cleanly || !state.feed.items.is_empty()) {
        Some(state.feed)
    } else {
        None
    }
}

// --- JSON Feed ---------------------------------------------------------------------------------

fn json_string(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Certains flux mettent un nombre dans "id" au lieu d'une chaîne.
fn json_id(v: &Value) -> Option<String> {
    match v.get("id")? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn parse_json(data: &[u8]) -> Option<ParsedFeed> {
    let root: Value = serde_json::from_slice(data).ok()?;
    let is_jsonfeed = root.get("version").and_then(Value::as_str).is_some_and(|v| v.contains("jsonfeed.org"));
    let items = root.get("items").and_then(Value::as_array);
    if !is_jsonfeed && items.is_none() {
        return None;
    }
    let mut feed = ParsedFeed {
        title: json_string(&root, "title"),
        site_url: json_string(&root, "home_page_url"),
        items: Vec::new(),
    };
    for item in items.into_iter().flatten() {
        // Texte brut seul : on l'habille en paragraphes HTML
        let text_as_html = json_string(item, "content_text").map(|text| {
            text.split("\n\n")
                .map(|p| format!("<p>{}</p>", escape_html(p).replace('\n', "<br>")))
                .collect::<String>()
        });
        let attachment_image = item.get("attachments").and_then(Value::as_array).and_then(|list| {
            list.iter()
                .find(|a| json_string(a, "mime_type").unwrap_or_default().starts_with("image"))
                .and_then(|a| json_string(a, "url"))
        });
        let names: Vec<String> = item
            .get("authors")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(|a| json_string(a, "name")).collect())
            .unwrap_or_default();
        let author = if names.is_empty() {
            item.get("author").and_then(|a| json_string(a, "name"))
        } else {
            Some(names.join(", "))
        };
        feed.items.push(ParsedItem {
            guid: json_id(item),
            title: json_string(item, "title"),
            link: json_string(item, "url").or_else(|| json_string(item, "external_url")),
            summary_html: json_string(item, "summary"),
            content_html: json_string(item, "content_html").or(text_as_html),
            image_url: json_string(item, "image").or_else(|| json_string(item, "banner_image")).or(attachment_image),
            author,
            published: json_string(item, "date_published")
                .or_else(|| json_string(item, "date_modified"))
                .and_then(|d| parse_date(&d)),
        });
    }
    Some(feed)
}

pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

// --- Dates -------------------------------------------------------------------------------------

/// Remplace un fuseau écrit en lettres (GMT, EST, CEST…) par un décalage numérique.
fn numeric_zone(s: &str) -> String {
    let Some((head, zone)) = s.rsplit_once(' ') else { return s.to_string() };
    let offset = match zone.to_uppercase().as_str() {
        "GMT" | "UT" | "UTC" | "Z" => "+0000",
        "EST" => "-0500",
        "EDT" => "-0400",
        "CST" => "-0600",
        "CDT" => "-0500",
        "MST" => "-0700",
        "MDT" => "-0600",
        "PST" => "-0800",
        "PDT" => "-0700",
        "CET" => "+0100",
        "CEST" => "+0200",
        _ => return s.to_string(),
    };
    format!("{head} {offset}")
}

/// Date d'un flux (RFC 822 / ISO 8601 et variantes courantes), en secondes Unix.
pub fn parse_date(raw: &str) -> Option<f64> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let ts = |d: DateTime<chrono::FixedOffset>| d.timestamp() as f64 + f64::from(d.timestamp_subsec_millis()) / 1000.0;

    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return Some(ts(d));
    }

    // Jour de la semaine retiré : s'il ne correspond pas à la date, chrono refuserait toute la date.
    let without_weekday = {
        let mut chars = s.splitn(2, ',');
        match (chars.next(), chars.next()) {
            (Some(day), Some(rest)) if day.trim().chars().all(char::is_alphabetic) => rest.trim().to_string(),
            _ => s.to_string(),
        }
    };
    let zoned = numeric_zone(&without_weekday);
    for format in [
        "%d %b %Y %H:%M:%S %z",
        "%d %b %Y %H:%M %z",
        "%Y-%m-%dT%H:%M:%S%.f%z",
        "%Y-%m-%dT%H:%M:%S%z",
    ] {
        if let Ok(d) = DateTime::parse_from_str(&zoned, format) {
            return Some(ts(d));
        }
    }
    // Sans fuseau : UTC
    for format in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(d) = NaiveDateTime::parse_from_str(s, format) {
            return Some(d.and_utc().timestamp() as f64);
        }
    }
    for format in ["%Y-%m-%d", "%d %b %Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(&without_weekday, format) {
            return d.and_hms_opt(0, 0, 0).map(|d| d.and_utc().timestamp() as f64);
        }
    }
    None
}

/// Titre ou texte d'un flux sans balises ni entités (utilisé pour les noms de flux).
pub fn clean_title(raw: &str) -> String {
    plain_text(&decode_entities(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:content="http://purl.org/rss/1.0/modules/content/" xmlns:media="http://search.yahoo.com/mrss/" xmlns:dc="http://purl.org/dc/elements/1.1/">
<channel>
  <title>Mon blog</title>
  <link>https://blog.example/</link>
  <item>
    <title>Premier &amp; dernier</title>
    <link>https://blog.example/1</link>
    <guid>g1</guid>
    <pubDate>Wed, 02 Oct 2024 13:00:00 GMT</pubDate>
    <dc:creator>Alice</dc:creator>
    <description><![CDATA[<p>Résumé</p>]]></description>
    <content:encoded><![CDATA[<p>Corps <b>complet</b></p>]]></content:encoded>
    <media:thumbnail url="https://blog.example/t.jpg"/>
  </item>
  <item><title>Sans date</title><enclosure url="https://blog.example/photo.png" type="image/png"/></item>
</channel></rss>"#;

    #[test]
    fn parses_rss_items() {
        let feed = parse(RSS.as_bytes()).unwrap();
        assert_eq!(feed.title.as_deref(), Some("Mon blog"));
        assert_eq!(feed.site_url.as_deref(), Some("https://blog.example/"));
        assert_eq!(feed.items.len(), 2);
        let a = &feed.items[0];
        assert_eq!(a.title.as_deref(), Some("Premier & dernier"));
        assert_eq!(a.guid.as_deref(), Some("g1"));
        assert_eq!(a.author.as_deref(), Some("Alice"));
        assert_eq!(a.summary_html.as_deref(), Some("<p>Résumé</p>"));
        assert_eq!(a.content_html.as_deref(), Some("<p>Corps <b>complet</b></p>"));
        assert_eq!(a.image_url.as_deref(), Some("https://blog.example/t.jpg"));
        assert_eq!(a.published, Some(1_727_874_000.0));
        assert_eq!(feed.items[1].published, None);
        assert_eq!(feed.items[1].image_url.as_deref(), Some("https://blog.example/photo.png"));
    }

    #[test]
    fn parses_atom() {
        let atom = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>Atom</title>
            <link rel="alternate" href="https://a.example/"/><link rel="self" href="https://a.example/feed"/>
            <entry><title>E1</title><id>urn:1</id><link rel="alternate" href="https://a.example/e1"/>
              <updated>2024-10-02T13:00:00+02:00</updated><author><name>Bob</name></author>
              <summary>Court</summary></entry></feed>"#;
        let feed = parse(atom.as_bytes()).unwrap();
        assert_eq!(feed.site_url.as_deref(), Some("https://a.example/"));
        let e = &feed.items[0];
        assert_eq!(e.link.as_deref(), Some("https://a.example/e1"));
        assert_eq!(e.guid.as_deref(), Some("urn:1"));
        assert_eq!(e.author.as_deref(), Some("Bob"));
        assert_eq!(e.published, Some(1_727_866_800.0));
    }

    #[test]
    fn parses_json_feed_and_numeric_ids() {
        let json = r#"{"version":"https://jsonfeed.org/version/1.1","title":"J","home_page_url":"https://j.example/",
            "items":[{"id":42,"url":"https://j.example/1","title":"T","content_text":"a\n\nb","date_published":"2024-10-02T13:00:00Z",
                      "authors":[{"name":"X"},{"name":"Y"}],"image":"https://j.example/i.jpg"}]}"#;
        let feed = parse(json.as_bytes()).unwrap();
        let i = &feed.items[0];
        assert_eq!(i.guid.as_deref(), Some("42"));
        assert_eq!(i.content_html.as_deref(), Some("<p>a</p><p>b</p>"));
        assert_eq!(i.author.as_deref(), Some("X, Y"));
        assert_eq!(i.published, Some(1_727_874_000.0));
    }

    #[test]
    fn rejects_non_feeds_and_reads_latin1() {
        assert!(parse(b"<html><body>Pas un flux</body></html>").is_none());
        assert!(parse(b"{\"foo\": 1}").is_none());
        assert!(parse(b"n'importe quoi").is_none());
        let mut latin1 = b"<rss><channel><title>Caf".to_vec();
        latin1.push(0xE9);
        latin1.extend_from_slice(b"</title></channel></rss>");
        assert_eq!(parse(&latin1).unwrap().title.as_deref(), Some("Café"));
    }

    #[test]
    fn parses_common_date_formats() {
        let expected = Some(1_727_874_000.0); // 2024-10-02 13:00:00 UTC
        assert_eq!(parse_date("Wed, 02 Oct 2024 13:00:00 GMT"), expected);
        assert_eq!(parse_date("Wed, 02 Oct 2024 13:00:00 +0000"), expected);
        assert_eq!(parse_date("Tue, 02 Oct 2024 15:00:00 +0200"), expected); // mauvais jour de semaine toléré
        assert_eq!(parse_date("Wed, 02 Oct 2024 13:00 GMT"), expected);
        assert_eq!(parse_date("2024-10-02T13:00:00Z"), expected);
        assert_eq!(parse_date("2024-10-02T13:00:00"), expected);
        assert_eq!(parse_date("2024-10-02 13:00:00"), expected);
        assert_eq!(parse_date("2024-10-02"), Some(1_727_827_200.0));
        assert_eq!(parse_date("Wed, 02 Oct 2024 08:00:00 EST"), expected);
        assert_eq!(parse_date("n'importe quoi"), None);
        assert_eq!(parse_date(""), None);
    }
}
