import Foundation
import SwiftUI
import UniformTypeIdentifiers

extension UTType {
    static let opml = UTType(filenameExtension: "opml", conformingTo: .xml) ?? .xml
}

/// Lecture et écriture de fichiers OPML (le format standard d'échange
/// d'abonnements entre lecteurs RSS : NetNewsWire, Reeder, Feedly…).
enum OPML {
    struct Entry {
        var title: String
        var xmlURL: String
        var htmlURL: String?
        /// Nom du dossier parent (les dossiers imbriqués sont aplatis sur le plus proche)
        var folder: String?
    }

    static func parse(_ data: Data) -> [Entry] {
        let delegate = Parser()
        let parser = XMLParser(data: data)
        parser.delegate = delegate
        parser.shouldResolveExternalEntities = false
        parser.parse()
        return delegate.entries
    }

    private final class Parser: NSObject, XMLParserDelegate {
        var entries: [Entry] = []
        /// Pile des <outline> ouverts : nom de dossier, ou nil pour un flux
        private var stack: [String?] = []

        func parser(_ parser: XMLParser, didStartElement elementName: String, namespaceURI: String?,
                    qualifiedName qName: String?, attributes: [String: String] = [:]) {
            guard elementName.lowercased() == "outline" else { return }
            // Les attributs varient en casse selon les apps (xmlUrl, xmlURL…)
            var a: [String: String] = [:]
            for (k, v) in attributes { a[k.lowercased()] = v }
            let name = (a["title"] ?? a["text"] ?? "").trimmingCharacters(in: .whitespacesAndNewlines)

            if let xml = a["xmlurl"]?.trimmingCharacters(in: .whitespacesAndNewlines), !xml.isEmpty {
                let folder = stack.last(where: { $0 != nil }) ?? nil
                entries.append(Entry(title: name, xmlURL: xml, htmlURL: a["htmlurl"], folder: folder))
                stack.append(nil)
            } else {
                stack.append(name.isEmpty ? nil : name)
            }
        }

        func parser(_ parser: XMLParser, didEndElement elementName: String, namespaceURI: String?, qualifiedName qName: String?) {
            if elementName.lowercased() == "outline", !stack.isEmpty { stack.removeLast() }
        }
    }

    static func export(feeds: [Feed]) -> String {
        func esc(_ s: String) -> String {
            s.replacingOccurrences(of: "&", with: "&amp;")
                .replacingOccurrences(of: "<", with: "&lt;")
                .replacingOccurrences(of: ">", with: "&gt;")
                .replacingOccurrences(of: "\"", with: "&quot;")
        }
        func line(_ feed: Feed, indent: String) -> String {
            var attrs = "type=\"rss\" text=\"\(esc(feed.displayTitle))\" title=\"\(esc(feed.displayTitle))\" xmlUrl=\"\(esc(feed.url))\""
            if let site = feed.siteURL { attrs += " htmlUrl=\"\(esc(site))\"" }
            return "\(indent)<outline \(attrs)/>"
        }

        var body: [String] = []
        let grouped = Dictionary(grouping: feeds.filter { $0.folder != nil }) { $0.folder!.name }
        for name in grouped.keys.sorted(by: { $0.localizedStandardCompare($1) == .orderedAscending }) {
            body.append("    <outline text=\"\(esc(name))\" title=\"\(esc(name))\">")
            for feed in grouped[name]!.sorted(by: { $0.displayTitle.localizedStandardCompare($1.displayTitle) == .orderedAscending }) {
                body.append(line(feed, indent: "      "))
            }
            body.append("    </outline>")
        }
        for feed in feeds.filter({ $0.folder == nil }).sorted(by: { $0.displayTitle.localizedStandardCompare($1.displayTitle) == .orderedAscending }) {
            body.append(line(feed, indent: "    "))
        }

        let date = Date.now.formatted(.iso8601)
        return """
        <?xml version="1.0" encoding="UTF-8"?>
        <opml version="2.0">
          <head>
            <title>Abonnements Flux</title>
            <dateCreated>\(date)</dateCreated>
          </head>
          <body>
        \(body.joined(separator: "\n"))
          </body>
        </opml>

        """
    }
}

/// Document pour l'export (fileExporter)
struct OPMLDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.opml] }
    var text: String

    init(text: String) { self.text = text }

    init(configuration: ReadConfiguration) throws {
        text = String(decoding: configuration.file.regularFileContents ?? Data(), as: UTF8.self)
    }

    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: Data(text.utf8))
    }
}

/// États d'interface partagés entre les menus de l'app et la fenêtre principale
@MainActor
@Observable
final class UIState {
    var showOPMLImport = false
    var showOPMLExport = false
    var showAddFeed = false
    var showNewFolder = false
    /// Découverte de Flux (première installation, ou menu Aide)
    var showOnboarding = false
    /// Nouveautés d'une version (après une mise à jour, ou menu Aide)
    var whatsNew: WhatsNew.Release?
}
