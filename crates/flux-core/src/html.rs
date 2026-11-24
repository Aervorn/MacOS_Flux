//! Petits outils HTML sans dépendance lourde (expressions régulières), comme `HTMLUtils.swift`.

use std::sync::OnceLock;

use regex::{Captures, Regex};
use url::Url;

fn cached(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("expression régulière valide"))
}

fn re_script_style() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    cached(&R, r"(?is)<script\b[^>]*>.*?</script\s*>|<style\b[^>]*>.*?</style\s*>|<noscript\b[^>]*>.*?</noscript\s*>")
}
fn re_block_breaks() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    cached(&R, r"(?i)<(br|/p|/div|/li|/h[1-6]|/blockquote)\b[^>]*>")
}
fn re_tags() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    cached(&R, r"(?s)<[^>]+>")
}
fn re_spaces() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    cached(&R, r"\s+")
}
fn re_entity() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    cached(&R, r"&(#[xX][0-9a-fA-F]+|#[0-9]+|[a-zA-Z][a-zA-Z0-9]*);")
}
fn re_img() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    cached(&R, r"(?is)<img\b[^>]*>")
}
fn re_meta() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    cached(&R, r"(?is)<meta\b[^>]*>")
}
fn re_link() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    cached(&R, r"(?is)<link\b[^>]*>")
}

fn named_entity(name: &str) -> Option<&'static str> {
    Some(match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{00A0}",
        "hellip" => "…",
        "mdash" => "—",
        "ndash" => "–",
        "lsquo" => "‘",
        "rsquo" => "’",
        "ldquo" => "“",
        "rdquo" => "”",
        "laquo" => "«",
        "raquo" => "»",
        "bull" => "•",
        "middot" => "·",
        "copy" => "©",
        "reg" => "®",
        "trade" => "™",
        "euro" => "€",
        "deg" => "°",
        "times" => "×",
        "shy" | "zwnj" => "",
        "thinsp" => "\u{2009}",
        "eacute" => "é",
        "egrave" => "è",
        "ecirc" => "ê",
        "euml" => "ë",
        "agrave" => "à",
        "acirc" => "â",
        "auml" => "ä",
        "ccedil" => "ç",
        "icirc" => "î",
        "iuml" => "ï",
        "ocirc" => "ô",
        "ouml" => "ö",
        "ugrave" => "ù",
        "ucirc" => "û",
        "uuml" => "ü",
        "oelig" => "œ",
        "aelig" => "æ",
        "ntilde" => "ñ",
        "szlig" => "ß",
        "Eacute" => "É",
        "Egrave" => "È",
        "Ecirc" => "Ê",
        "Agrave" => "À",
        "Acirc" => "Â",
        "Ccedil" => "Ç",
        "Ocirc" => "Ô",
        "Ugrave" => "Ù",
        "OElig" => "Œ",
        "Icirc" => "Î",
        _ => return None,
    })
}

/// Remplace les entités HTML connues (`&amp;`, `&#233;`, `&eacute;`…) ; les autres restent telles quelles.
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    re_entity()
        .replace_all(s, |caps: &Captures| {
            let body = &caps[1];
            let decoded = if let Some(hex) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32).map(String::from)
            } else if let Some(dec) = body.strip_prefix('#') {
                dec.parse::<u32>().ok().and_then(char::from_u32).map(String::from)
            } else {
                named_entity(body).map(String::from)
            };
            decoded.unwrap_or_else(|| caps[0].to_string())
        })
        .into_owned()
}

/// Texte brut d'un fragment HTML.
pub fn plain_text(html: &str) -> String {
    if html.is_empty() {
        return String::new();
    }
    let s = re_script_style().replace_all(html, " ");
    let s = re_block_breaks().replace_all(&s, " ");
    let s = re_tags().replace_all(&s, "");
    let s = decode_entities(&s);
    re_spaces().replace_all(&s, " ").trim().to_string()
}

/// Valeur d'un attribut dans une balise (`<img src="…">`), guillemets simples, doubles ou absents.
pub fn attribute(name: &str, tag: &str) -> Option<String> {
    let pattern = format!(r#"(?is)\s{}\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+))"#, regex::escape(name));
    let re = Regex::new(&pattern).ok()?;
    let caps = re.captures(tag)?;
    (1..=3).find_map(|i| caps.get(i)).map(|m| m.as_str().to_string())
}

/// Première image « réelle » d'un fragment HTML (ignore les pixels espions).
pub fn first_image(html: &str) -> Option<String> {
    for m in re_img().find_iter(html) {
        let tag = m.as_str();
        if attribute("width", tag).as_deref() == Some("1") || attribute("height", tag).as_deref() == Some("1") {
            continue;
        }
        let Some(src) = attribute("src", tag).or_else(|| attribute("data-src", tag)) else { continue };
        let lower = src.to_lowercase();
        if lower.starts_with("data:")
            || ["feedburner", "pixel", "/1x1", "tracking", "gravatar"].iter().any(|bad| lower.contains(bad))
        {
            continue;
        }
        return Some(src);
    }
    None
}

/// Adresses absolues (http, https) des images d'un fragment HTML, telles qu'elles y figurent
/// (c'est sous ce nom que le lecteur les cherche dans le cache).
pub fn img_sources(html: &str) -> Vec<String> {
    re_img()
        .find_iter(html)
        .filter_map(|m| attribute("src", m.as_str()))
        .map(|s| decode_entities(s.trim()))
        .filter(|s| s.starts_with("http://") || s.starts_with("https://"))
        .collect()
}

/// og:image (ou twitter:image) d'une page.
pub fn og_image(html: &str) -> Option<String> {
    let mut found: Vec<(String, String)> = Vec::new();
    for m in re_meta().find_iter(html) {
        let tag = m.as_str();
        let Some(prop) = attribute("property", tag).or_else(|| attribute("name", tag)) else { continue };
        let Some(content) = attribute("content", tag).filter(|c| !c.is_empty()) else { continue };
        let prop = prop.to_lowercase();
        if !found.iter().any(|(p, _)| *p == prop) {
            found.push((prop, content));
        }
    }
    ["og:image", "og:image:url", "twitter:image", "twitter:image:src"]
        .iter()
        .find_map(|wanted| found.iter().find(|(p, _)| p == wanted).map(|(_, c)| c.clone()))
}

/// Adresses des flux annoncés par une page (`<link rel="alternate" type="application/rss+xml">`…).
pub fn feed_links(html: &str, base: &Url) -> Vec<Url> {
    let mut out = Vec::new();
    for m in re_link().find_iter(html) {
        let tag = m.as_str();
        let Some(rel) = attribute("rel", tag).map(|r| r.to_lowercase()) else { continue };
        let Some(kind) = attribute("type", tag).map(|t| t.to_lowercase()) else { continue };
        if !rel.contains("alternate") || !(kind.contains("rss") || kind.contains("atom") || kind.contains("feed+json")) {
            continue;
        }
        let Some(href) = attribute("href", tag) else { continue };
        if let Some(resolved) = resolve(&href, Some(base)).and_then(|r| Url::parse(&r).ok()) {
            out.push(resolved);
        }
    }
    out
}

/// Adresse absolue à partir d'une adresse brute (relative, `//hôte/…`, avec entités ou espaces).
pub fn resolve(raw: &str, base: Option<&Url>) -> Option<String> {
    let mut s = decode_entities(raw.trim());
    if s.is_empty() {
        return None;
    }
    if s.starts_with("//") {
        s = format!("https:{s}");
    }
    let s = s.replace(' ', "%20");
    let url = match base {
        Some(base) => base.join(&s).ok()?,
        None => Url::parse(&s).ok()?,
    };
    Some(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_strips_tags_scripts_and_entities() {
        assert_eq!(plain_text("<p>Caf&eacute; &amp; cr&#232;me</p><script>x()</script><p>Fin</p>"), "Café & crème Fin");
        assert_eq!(plain_text("a<br>b"), "a b");
        assert_eq!(plain_text(""), "");
        assert_eq!(plain_text("&inconnu; &#x41;"), "&inconnu; A");
    }

    #[test]
    fn reads_attributes_in_all_quote_styles() {
        let tag = r#"<img src="a.jpg" alt='x y' width=1>"#;
        assert_eq!(attribute("src", tag).as_deref(), Some("a.jpg"));
        assert_eq!(attribute("alt", tag).as_deref(), Some("x y"));
        assert_eq!(attribute("width", tag).as_deref(), Some("1"));
        assert_eq!(attribute("height", tag), None);
    }

    #[test]
    fn first_image_skips_trackers() {
        let html = r#"<img src="https://t/pixel.gif"><img width="1" src="https://a/b.jpg"><img data-src="https://ok/c.jpg">"#;
        assert_eq!(first_image(html).as_deref(), Some("https://ok/c.jpg"));
        assert_eq!(first_image("<p>rien</p>"), None);
    }

    #[test]
    fn finds_og_image_and_feed_links() {
        let html = r#"<head><meta property="og:image" content="https://x/og.jpg">
            <link rel="alternate" type="application/rss+xml" href="/feed.xml">
            <link rel="stylesheet" href="/a.css"></head>"#;
        assert_eq!(og_image(html).as_deref(), Some("https://x/og.jpg"));
        let base = Url::parse("https://site.example/blog/").unwrap();
        let links = feed_links(html, &base);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].as_str(), "https://site.example/feed.xml");
    }

    #[test]
    fn resolves_relative_and_protocol_relative_urls() {
        let base = Url::parse("https://site.example/a/b").unwrap();
        assert_eq!(resolve("../img.png", Some(&base)).as_deref(), Some("https://site.example/img.png"));
        assert_eq!(resolve("//cdn.example/x.jpg", Some(&base)).as_deref(), Some("https://cdn.example/x.jpg"));
        assert_eq!(resolve("a b.png", Some(&base)).as_deref(), Some("https://site.example/a/a%20b.png"));
        assert_eq!(resolve("  ", Some(&base)), None);
        assert_eq!(resolve("https://x.example", None).as_deref(), Some("https://x.example/"));
    }
}
