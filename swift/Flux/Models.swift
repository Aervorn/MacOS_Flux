import Foundation
import SwiftData
import SwiftUI

@Model
final class Feed {
    @Attribute(.unique) var url: String
    var title: String
    var siteURL: String?
    var addedAt: Date
    var lastRefreshed: Date?
    var lastError: String?
    var folder: Folder?
    /// Nom choisi par l'utilisateur (nil = nom fourni par le flux)
    var customTitle: String?
    /// Ancienne adresse, après un changement d'adresse (la synchro s'en sert pour
    /// déplacer le flux sur les autres Mac au lieu de le supprimer et de le recréer)
    var previousURL: String?

    @Relationship(deleteRule: .cascade, inverse: \Article.feed)
    var articles: [Article] = []

    init(url: String, title: String, siteURL: String?) {
        self.url = url
        self.title = title
        self.siteURL = siteURL
        self.addedAt = .now
    }

    /// Nom affiché partout dans l'app
    var displayTitle: String { customTitle ?? title }

    var faviconURL: URL? {
        guard let host = URL(string: siteURL ?? url)?.host() else { return nil }
        return URL(string: "https://\(host)/favicon.ico")
    }
}

@Model
final class Folder {
    @Attribute(.unique) var uid: String
    var name: String
    var isExpanded: Bool
    var createdAt: Date
    /// Nom d'un symbole SF Symbols (nil = "folder")
    var symbol: String?
    /// Nom d'une FolderColor (nil = couleur d'accent)
    var colorName: String?

    /// Supprimer un dossier ne supprime pas ses flux : ils reviennent à la racine.
    @Relationship(deleteRule: .nullify, inverse: \Feed.folder)
    var feeds: [Feed] = []

    init(name: String) {
        self.uid = UUID().uuidString
        self.name = name
        self.isExpanded = true
        self.createdAt = .now
    }
}

/// Règle d'exclusion par mot-clé, appliquée au titre des articles
@Model
final class FilterRule {
    @Attribute(.unique) var uid: String
    var keyword: String
    /// Mot entier (« Apple » ne touche pas « Applevision ») ou n'importe où dans le titre
    var wholeWord: Bool
    /// FilterAction.rawValue
    var action: String
    /// Ancien champ (un seul flux, nil = tous), remplacé par feedURLs.
    /// Gardé pour migrer les règles des versions précédentes (FeedStore.migrateFilterScopes).
    var feedURL: String?
    /// Flux concernés, triés (vide = tous les flux)
    var feedURLs: [String] = []
    var createdAt: Date

    init(keyword: String, wholeWord: Bool, action: FilterAction, feedURLs: [String]) {
        self.uid = UUID().uuidString
        self.keyword = keyword
        self.wholeWord = wholeWord
        self.action = action.rawValue
        self.feedURLs = feedURLs.sorted()
        self.createdAt = .now
    }

    var filterAction: FilterAction { FilterAction(rawValue: action) ?? .hide }
}

/// Tag pour ranger les favoris (un favori peut en avoir plusieurs)
@Model
final class FavoriteTag {
    @Attribute(.unique) var uid: String
    var name: String
    var createdAt: Date

    init(name: String) {
        self.uid = UUID().uuidString
        self.name = name
        self.createdAt = .now
    }
}

enum FilterAction: String, CaseIterable, Identifiable {
    case hide
    case markRead

    var id: String { rawValue }

    var label: String {
        switch self {
        case .hide: String(localized: "Masquer l'article")
        case .markRead: String(localized: "Marquer comme lu")
        }
    }
}

extension Folder {
    var symbolName: String { symbol ?? "folder" }
    var tint: Color? { colorName.flatMap(FolderColor.init(rawValue:))?.color }
}

enum FolderColor: String, CaseIterable, Identifiable {
    case red, orange, yellow, green, mint, teal, cyan, blue, indigo, purple, pink, brown, gray

    var id: String { rawValue }

    var label: String {
        switch self {
        case .red: String(localized: "Rouge")
        case .orange: String(localized: "Orange")
        case .yellow: String(localized: "Jaune")
        case .green: String(localized: "Vert")
        case .mint: String(localized: "Menthe")
        case .teal: String(localized: "Bleu canard")
        case .cyan: String(localized: "Cyan")
        case .blue: String(localized: "Bleu")
        case .indigo: String(localized: "Indigo")
        case .purple: String(localized: "Violet")
        case .pink: String(localized: "Rose")
        case .brown: String(localized: "Marron")
        case .gray: String(localized: "Gris")
        }
    }

    var color: Color {
        switch self {
        case .red: .red
        case .orange: .orange
        case .yellow: .yellow
        case .green: .green
        case .mint: .mint
        case .teal: .teal
        case .cyan: .cyan
        case .blue: .blue
        case .indigo: .indigo
        case .purple: .purple
        case .pink: .pink
        case .brown: .brown
        case .gray: .gray
        }
    }
}

extension Locale {
    /// Langue de l'interface (celle que macOS a choisie pour l'app), pour formater les dates
    static var app: Locale {
        Locale(identifier: Bundle.main.preferredLocalizations.first ?? "en")
    }
}

@Model
final class Article {
    /// feedURL + "|" + guid (ou lien, ou titre)
    @Attribute(.unique) var key: String
    var feedURL: String
    var title: String
    var link: String?
    /// Extrait en texte brut, pour la liste
    var summary: String
    /// HTML fourni par le flux (content:encoded, description…)
    var contentHTML: String?
    /// HTML extrait de la page web par Readability (mis en cache)
    var fullHTML: String?
    /// Titre + résumé + contenu en texte brut, pour la recherche plein texte
    /// ("" = pas encore calculé pour les articles importés avant cette version)
    var searchText: String = ""
    /// Masqué par une règle de filtrage (n'apparaît nulle part, ne compte pas dans les non-lus)
    var isHidden: Bool = false
    /// Règle de filtrage (FilterRule.uid) qui a masqué ou marqué comme lu cet article
    var filteredBy: String?
    /// Favori archivé : article complet et images rangés dans l'archive permanente
    var isArchived: Bool = false
    /// Première ouverture de l'article (historique et statistiques)
    var openedAt: Date?
    /// Temps total passé avec l'article affiché, en secondes
    var readingSeconds: Double = 0
    /// Nombre de mots du texte le plus complet disponible (temps de lecture estimé)
    var wordCount: Int = 0
    /// Groupe d'articles similaires (même actualité dans plusieurs sources)
    var clusterID: String?
    /// Article du groupe masqué dans les vues générales (seul le premier publié y apparaît)
    var isDuplicate: Bool = false
    /// Séparé à la main de son groupe : ne sera plus jamais regroupé
    var clusterOptOut: Bool = false
    /// Nom de la source, gardé quand son flux est supprimé (favoris conservés)
    var sourceTitle: String?
    /// Tags (FavoriteTag.uid) sous la forme « ,uid1,uid2, » ("" = aucun) : une simple chaîne
    /// se filtre dans un #Predicate (une liste ne se traduit pas en requête). Voir `tagUIDs`.
    var tagsRaw: String = ""
    var imageURL: String?
    var author: String?
    var published: Date
    var isRead: Bool
    var isStarred: Bool
    var feed: Feed?

    init(key: String, feedURL: String, title: String, link: String?, summary: String,
         contentHTML: String?, imageURL: String?, author: String?, published: Date) {
        self.key = key
        self.feedURL = feedURL
        self.title = title
        self.link = link
        self.summary = summary
        self.contentHTML = contentHTML
        self.fullHTML = nil
        self.searchText = Article.makeSearchText(title: title, summary: summary, html: contentHTML)
        self.wordCount = Article.countWords(contentHTML)
        self.imageURL = imageURL
        self.author = author
        self.published = published
        self.isRead = false
        self.isStarred = false
    }
}

extension Article {
    /// Tags de l'article, triés
    var tagUIDs: [String] {
        get { tagsRaw.split(separator: ",").map(String.init) }
        set {
            let uids = Set(newValue.filter { !$0.isEmpty }).sorted()
            let raw = uids.isEmpty ? "" : "," + uids.joined(separator: ",") + ","
            if raw != tagsRaw { tagsRaw = raw }
        }
    }

    /// Motif à chercher dans `tagsRaw` pour un tag
    static func tagNeedle(_ uid: String) -> String { "," + uid + "," }

    /// Limite pour ne pas alourdir la base avec des articles très longs
    private static let searchTextLimit = 30_000

    static func makeSearchText(title: String, summary: String, html: String?) -> String {
        let body = html.map(HTMLUtils.plainText) ?? ""
        return String((title + "\n" + summary + "\n" + body).prefix(searchTextLimit))
    }

    /// À appeler quand le contenu change (article complet récupéré)
    func updateSearchText() {
        let html = fullHTML ?? contentHTML
        searchText = Article.makeSearchText(title: title, summary: summary, html: html)
        wordCount = Article.countWords(html)
    }

    /// Nom de la source à afficher (même si le flux a été supprimé)
    var sourceName: String {
        feed?.displayTitle ?? sourceTitle ?? URL(string: feedURL)?.host() ?? feedURL
    }

    static func countWords(_ html: String?) -> Int {
        HTMLUtils.plainText(html ?? "").split(whereSeparator: { $0.isWhitespace }).count
    }

    /// Temps de lecture estimé (230 mots par minute), en secondes
    var estimatedReadingSeconds: Double {
        let words = wordCount > 0 ? wordCount : Article.countWords(fullHTML ?? contentHTML)
        return Double(words) / 230 * 60
    }
}
