import Foundation

struct ParsedItem: Sendable {
    var guid: String?
    var title: String?
    var link: String?
    var summaryHTML: String?
    var contentHTML: String?
    var imageURL: String?
    var author: String?
    var published: Date?
}

struct ParsedFeed: Sendable {
    var title: String?
    var siteURL: String?
    var items: [ParsedItem] = []
}

/// Parser RSS 2.0 / RSS 1.0 (RDF) / Atom basé sur XMLParser.
final class FeedParser: NSObject, XMLParserDelegate {
    private var feed = ParsedFeed()
    private var item: ParsedItem?
    private var buffers: [String] = []
    private var sawRoot = false

    /// RSS, Atom ou JSON Feed (détecté automatiquement)
    static func parse(_ data: Data) -> ParsedFeed? {
        if let first = data.first(where: { !($0 == 0x20 || $0 == 0x0A || $0 == 0x0D || $0 == 0x09 || $0 == 0xEF || $0 == 0xBB || $0 == 0xBF) }),
           first == UInt8(ascii: "{") {
            return JSONFeedParser.parse(data)
        }
        let delegate = FeedParser()
        let parser = XMLParser(data: data)
        parser.delegate = delegate
        parser.shouldResolveExternalEntities = false
        let ok = parser.parse()
        guard delegate.sawRoot, ok || !delegate.feed.items.isEmpty else { return nil }
        return delegate.feed
    }

    func parser(_ parser: XMLParser, didStartElement elementName: String, namespaceURI: String?,
                qualifiedName qName: String?, attributes a: [String: String] = [:]) {
        let name = elementName.lowercased()
        buffers.append("")

        switch name {
        case "rss", "rdf:rdf", "feed":
            sawRoot = true
        case "item", "entry":
            item = ParsedItem()
        case "link":
            // Atom : <link rel="alternate" href="…"/>
            guard let href = a["href"] else { break }
            let rel = a["rel"] ?? "alternate"
            if item != nil {
                if rel == "alternate", item?.link == nil { item?.link = href }
                else if rel == "enclosure", (a["type"] ?? "").hasPrefix("image") { setImage(href) }
            } else if rel == "alternate", feed.siteURL == nil {
                feed.siteURL = href
            }
        case "media:thumbnail":
            if let u = a["url"] { setImage(u) }
        case "media:content":
            if let u = a["url"], a["medium"] == "image" || (a["type"] ?? "").hasPrefix("image") || looksLikeImage(u) {
                setImage(u)
            }
        case "enclosure":
            if let u = a["url"], (a["type"] ?? "").hasPrefix("image") || looksLikeImage(u) { setImage(u) }
        case "itunes:image":
            if let u = a["href"] { setImage(u) }
        default:
            break
        }
    }

    func parser(_ parser: XMLParser, foundCharacters string: String) {
        guard !buffers.isEmpty else { return }
        buffers[buffers.count - 1] += string
    }

    func parser(_ parser: XMLParser, foundCDATA CDATABlock: Data) {
        guard !buffers.isEmpty else { return }
        buffers[buffers.count - 1] += String(decoding: CDATABlock, as: UTF8.self)
    }

    func parser(_ parser: XMLParser, didEndElement elementName: String, namespaceURI: String?, qualifiedName qName: String?) {
        let name = elementName.lowercased()
        let raw = buffers.popLast() ?? ""
        // Le texte des enfants remonte au parent (utile pour <author><name>…</name></author>)
        if !buffers.isEmpty { buffers[buffers.count - 1] += raw }
        let value = raw.trimmingCharacters(in: .whitespacesAndNewlines)

        if item != nil {
            switch name {
            case "item", "entry":
                if let i = item { feed.items.append(i) }
                item = nil
            case "title":
                if item?.title == nil { item?.title = value }
            case "link":
                if !value.isEmpty, item?.link == nil { item?.link = value }
            case "guid", "id":
                if !value.isEmpty { item?.guid = value }
            case "description", "summary":
                if !value.isEmpty, item?.summaryHTML == nil { item?.summaryHTML = value }
            case "content:encoded", "content":
                if !value.isEmpty { item?.contentHTML = value }
            case "pubdate", "published", "issued":
                item?.published = DateParser.parse(value) ?? item?.published
            case "updated", "modified", "dc:date":
                if item?.published == nil { item?.published = DateParser.parse(value) }
            case "author", "dc:creator":
                if !value.isEmpty, item?.author == nil { item?.author = value }
            default:
                break
            }
        } else {
            switch name {
            case "title":
                if feed.title == nil, !value.isEmpty { feed.title = value }
            case "link":
                if feed.siteURL == nil, !value.isEmpty { feed.siteURL = value }
            default:
                break
            }
        }
    }

    private func setImage(_ url: String) {
        guard item != nil, item?.imageURL == nil else { return }
        item?.imageURL = url
    }

    private func looksLikeImage(_ url: String) -> Bool {
        let path = (URL(string: url)?.path ?? url).lowercased()
        return [".jpg", ".jpeg", ".png", ".webp", ".gif", ".avif"].contains { path.hasSuffix($0) }
    }
}

enum DateParser {
    private static let iso: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime]
        return f
    }()

    private static let isoFractional: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()

    private static let formatters: [DateFormatter] = [
        "EEE, dd MMM yyyy HH:mm:ss zzz",
        "EEE, dd MMM yyyy HH:mm:ss Z",
        "EEE, dd MMM yyyy HH:mm zzz",
        "EEE, dd MMM yyyy HH:mm Z",
        "dd MMM yyyy HH:mm:ss zzz",
        "dd MMM yyyy HH:mm:ss Z",
        "EEE, dd MMM yyyy",
        "yyyy-MM-dd'T'HH:mm:ssZZZZZ",
        "yyyy-MM-dd'T'HH:mm:ss.SSSZZZZZ",
        "yyyy-MM-dd'T'HH:mm:ss",
        "yyyy-MM-dd HH:mm:ss",
        "yyyy-MM-dd",
    ].map { format in
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = TimeZone(secondsFromGMT: 0)
        f.dateFormat = format
        return f
    }

    static func parse(_ raw: String) -> Date? {
        let s = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !s.isEmpty else { return nil }
        if let d = iso.date(from: s) ?? isoFractional.date(from: s) { return d }
        for f in formatters {
            if let d = f.date(from: s) { return d }
        }
        return nil
    }
}

/// JSON Feed 1.0 / 1.1 (https://www.jsonfeed.org)
enum JSONFeedParser {
    private struct Feed: Decodable {
        var version: String?
        var title: String?
        var home_page_url: String?
        var items: [Item]?
    }

    private struct Author: Decodable {
        var name: String?
    }

    private struct Attachment: Decodable {
        var url: String?
        var mime_type: String?
    }

    private struct Item: Decodable {
        var id: LossyString?
        var url: String?
        var external_url: String?
        var title: String?
        var content_html: String?
        var content_text: String?
        var summary: String?
        var image: String?
        var banner_image: String?
        var date_published: String?
        var date_modified: String?
        var author: Author?
        var authors: [Author]?
        var attachments: [Attachment]?
    }

    /// Certains flux mettent un nombre dans "id" au lieu d'une chaîne
    private struct LossyString: Decodable {
        var value: String
        init(from decoder: Decoder) throws {
            let c = try decoder.singleValueContainer()
            if let s = try? c.decode(String.self) { value = s }
            else if let i = try? c.decode(Int.self) { value = String(i) }
            else if let d = try? c.decode(Double.self) { value = String(d) }
            else { value = "" }
        }
    }

    static func parse(_ data: Data) -> ParsedFeed? {
        guard let feed = try? JSONDecoder().decode(Feed.self, from: data),
              feed.version?.contains("jsonfeed.org") == true || feed.items != nil else { return nil }

        var result = ParsedFeed(title: feed.title, siteURL: feed.home_page_url)
        for item in feed.items ?? [] {
            // Texte brut seul : on l'habille en paragraphes HTML
            let textAsHTML = item.content_text.map { text in
                text.components(separatedBy: "\n\n")
                    .map { "<p>\(HTMLUtils.escape($0).replacingOccurrences(of: "\n", with: "<br>"))</p>" }
                    .joined()
            }
            let imageAttachment = item.attachments?.first { ($0.mime_type ?? "").hasPrefix("image") }?.url
            result.items.append(ParsedItem(
                guid: item.id?.value.isEmpty == false ? item.id?.value : nil,
                title: item.title,
                link: item.url ?? item.external_url,
                summaryHTML: item.summary,
                contentHTML: item.content_html ?? textAsHTML,
                imageURL: item.image ?? item.banner_image ?? imageAttachment,
                author: item.authors?.compactMap(\.name).joined(separator: ", ").nilIfEmpty ?? item.author?.name,
                published: (item.date_published ?? item.date_modified).flatMap(DateParser.parse)
            ))
        }
        return result
    }
}

private extension String {
    var nilIfEmpty: String? { isEmpty ? nil : self }
}

