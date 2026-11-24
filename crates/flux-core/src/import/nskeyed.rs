//! Décodage d'un `NSArray<NSString>` archivé par NSKeyedArchiver (plist binaire).
//!
//! C'est le format de `ZFEEDURLS` dans le store SwiftData : un dictionnaire
//! `{ $top: { root: UID(1) }, $objects: [ "$null", { NS.objects: [UID…] }, "url1", … ] }`.
//! Un tableau vide (`NS.objects` vide) signifie « tous les flux ».

use std::io::Cursor;

use plist::Value;

use crate::error::{Error, Result};

pub fn decode_string_array(blob: &[u8]) -> Result<Vec<String>> {
    let value = Value::from_reader(Cursor::new(blob))?;
    let bad = |what: &str| Error::Other(format!("archive NSKeyedArchiver inattendue : {what}"));

    let dict = value.as_dictionary().ok_or_else(|| bad("racine"))?;
    let objects = dict
        .get("$objects")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("$objects"))?;
    let root_uid = dict
        .get("$top")
        .and_then(Value::as_dictionary)
        .and_then(|top| top.get("root"))
        .and_then(Value::as_uid)
        .ok_or_else(|| bad("$top.root"))?;
    let root = objects
        .get(root_uid.get() as usize)
        .and_then(Value::as_dictionary)
        .ok_or_else(|| bad("objet racine"))?;

    let Some(items) = root.get("NS.objects").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };

    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let uid = item.as_uid().ok_or_else(|| bad("élément sans UID"))?;
        let s = objects
            .get(uid.get() as usize)
            .and_then(Value::as_string)
            .ok_or_else(|| bad("élément qui n'est pas du texte"))?;
        out.push(s.to_string());
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) fn encode_string_array(items: &[&str]) -> Vec<u8> {
    use plist::{Dictionary, Uid};

    let mut root = Dictionary::new();
    let uids = (0..items.len()).map(|i| Value::Uid(Uid::new(2 + i as u64))).collect();
    root.insert("NS.objects".into(), Value::Array(uids));

    let mut objects = vec![Value::String("$null".into()), Value::Dictionary(root)];
    objects.extend(items.iter().map(|s| Value::String((*s).into())));

    let mut top = Dictionary::new();
    top.insert("root".into(), Value::Uid(Uid::new(1)));

    let mut archive = Dictionary::new();
    archive.insert("$top".into(), Value::Dictionary(top));
    archive.insert("$objects".into(), Value::Array(objects));

    let mut buf = Vec::new();
    plist::to_writer_binary(&mut buf, &Value::Dictionary(archive)).unwrap();
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_urls_in_order() {
        let blob = encode_string_array(&["https://a.example/feed", "https://b.example/rss"]);
        assert_eq!(
            decode_string_array(&blob).unwrap(),
            vec!["https://a.example/feed".to_string(), "https://b.example/rss".to_string()]
        );
    }

    #[test]
    fn empty_array_means_no_url() {
        let blob = encode_string_array(&[]);
        assert!(decode_string_array(&blob).unwrap().is_empty());
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(decode_string_array(b"pas un plist").is_err());
    }
}
