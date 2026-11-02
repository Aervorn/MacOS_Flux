import AppKit
import Foundation
import Observation
import SwiftData

/// Synchronisation entre Mac par iCloud Drive.
///
/// CloudKit demanderait un compte développeur Apple payant ; Flux n'étant pas
/// sandboxée, elle écrit directement dans iCloud Drive :
///
///     iCloud Drive/Flux/devices/<identifiant du Mac>.json
///
/// Chaque Mac écrit uniquement son propre fichier (pas de conflit d'écriture) et lit
/// ceux des autres. Chaque élément (flux, dossier, règle, état d'un article) porte la
/// date de sa dernière modification : à la fusion, la plus récente l'emporte.
/// Une suppression est une « pierre tombale » datée, gardée 60 jours.
enum SyncModel {
    struct Snapshot: Codable {
        var version = 1
        var device = ""
        var deviceName = ""
        var updatedAt: Double = 0
        var feeds: [String: FeedEntry] = [:]        // clé : URL du flux
        var folders: [String: FolderEntry] = [:]    // clé : uid
        var filters: [String: FilterEntry] = [:]    // clé : uid
        var articles: [String: ArticleEntry] = [:]  // clé : Article.key
    }

    struct FeedEntry: Codable {
        var title: String               // nom d'origine (pour créer le flux ailleurs, pas comparé)
        var siteURL: String?            // idem
        var customTitle: String?
        var folder: String?
        /// Ancienne adresse du flux (changement d'adresse) ; absente des fichiers des anciennes versions
        var previousURL: String?
        var deleted = false
        var modified: Double

        func sameValue(as other: FeedEntry) -> Bool {
            customTitle == other.customTitle && folder == other.folder && deleted == other.deleted
        }
    }

    struct FolderEntry: Codable {
        var name: String
        var symbol: String?
        var color: String?
        var deleted = false
        var modified: Double

        func sameValue(as other: FolderEntry) -> Bool {
            name == other.name && symbol == other.symbol && color == other.color && deleted == other.deleted
        }
    }

    struct FilterEntry: Codable {
        var keyword: String
        var wholeWord: Bool
        var action: String
        /// Ancien format (un seul flux). Encore écrit pour les versions précédentes : le premier
        /// flux de la liste (elles appliquent alors la règle à ce seul flux plutôt qu'à tous)
        var feedURL: String?
        /// Flux concernés, triés (vide = tous) ; absent des fichiers des anciennes versions
        var feedURLs: [String]?
        var deleted = false
        var modified: Double

        init(keyword: String, wholeWord: Bool, action: String, feedURLs: [String], modified: Double) {
            self.keyword = keyword
            self.wholeWord = wholeWord
            self.action = action
            self.feedURLs = feedURLs.sorted()
            self.feedURL = self.feedURLs?.first
            self.modified = modified
        }

        /// Flux concernés, quel que soit le format du fichier (vide = tous)
        var scope: [String] { (feedURLs ?? feedURL.map { [$0] } ?? []).sorted() }

        func sameValue(as other: FilterEntry) -> Bool {
            keyword == other.keyword && wholeWord == other.wholeWord && action == other.action
                && scope == other.scope && deleted == other.deleted
        }
    }

    /// État d'un article ; « non lu et pas favori » est l'état par défaut
    struct ArticleEntry: Codable {
        var read: Bool
        var starred: Bool
        var published: Double
        var modified: Double
        /// De quoi recréer un favori sur un Mac qui n'a pas (ou plus) l'article
        var meta: ArticleMeta?

        func sameValue(as other: ArticleEntry) -> Bool {
            read == other.read && starred == other.starred
        }

        var isDefault: Bool { !read && !starred }
    }

    struct ArticleMeta: Codable {
        var feedURL: String
        var title: String
        var link: String?
        var summary: String
        var imageURL: String?
        var author: String?
        var source: String?
    }

    /// Mémoire locale entre deux synchronisations
    struct LocalState: Codable {
        var lastSnapshot = Snapshot()
        /// Articles présents lors de la dernière synchro : un article absent de cette liste
        /// vient d'arriver, il reçoit l'état synchronisé au lieu de l'écraser
        var knownArticles: Set<String> = []
    }
}

@MainActor
@Observable
final class SyncEngine {
    private(set) var isSyncing = false
    private(set) var lastSync: Date?
    private(set) var lastError: String?
    /// Autres Mac vus dans iCloud Drive (nom, dernière mise à jour)
    private(set) var otherDevices: [(name: String, updated: Date)] = []

    @ObservationIgnored private let store: FeedStore
    @ObservationIgnored private var context: ModelContext { store.context }
    @ObservationIgnored private var saveObserver: NSObjectProtocol?
    @ObservationIgnored private var activeObserver: NSObjectProtocol?
    @ObservationIgnored private var pollTask: Task<Void, Never>?
    @ObservationIgnored private var debounceTask: Task<Void, Never>?
    @ObservationIgnored private var remoteStamps: [String: Date] = [:]

    private static let tombstoneLifetime: TimeInterval = 60 * 86_400

    init(store: FeedStore) {
        self.store = store
    }

    // MARK: - Emplacements

    static var iCloudDrive: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appending(path: "Library/Mobile Documents/com~apple~CloudDocs", directoryHint: .isDirectory)
    }

    static var isICloudDriveAvailable: Bool {
        FileManager.default.fileExists(atPath: iCloudDrive.path)
    }

    private static var devicesDirectory: URL {
        iCloudDrive.appending(path: "Flux", directoryHint: .isDirectory)
            .appending(path: "devices", directoryHint: .isDirectory)
    }

    private static var localStateURL: URL {
        URL.applicationSupportDirectory.appending(path: "Flux", directoryHint: .isDirectory)
            .appending(path: "sync-state.json")
    }

    static var deviceID: String {
        let key = "syncDeviceID"
        if let id = UserDefaults.standard.string(forKey: key) { return id }
        let id = UUID().uuidString
        UserDefaults.standard.set(id, forKey: key)
        return id
    }

    static var deviceName: String {
        Host.current().localizedName ?? "Mac"
    }

    static var isEnabled: Bool { UserDefaults.standard.bool(forKey: Prefs.syncEnabled) }

    // MARK: - Déclencheurs

    /// Synchronise après chaque modification (5 s d'attente), au retour dans l'app,
    /// et vérifie toutes les 30 s si un autre Mac a écrit du nouveau.
    func start() {
        guard saveObserver == nil else { return }
        saveObserver = NotificationCenter.default.addObserver(
            forName: ModelContext.didSave, object: nil, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.scheduleSync() }
        }
        activeObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didBecomeActiveNotification, object: nil, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.scheduleSync(delay: 1) }
        }
        pollTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(30))
                guard let self, Self.isEnabled else { continue }
                if self.othersChanged() { await self.sync() }
            }
        }
        scheduleSync(delay: 2)
    }

    func scheduleSync(delay: Double = 5) {
        guard Self.isEnabled, !isSyncing else { return }
        debounceTask?.cancel()
        debounceTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(delay))
            guard !Task.isCancelled else { return }
            await self?.sync()
        }
    }

    /// Un fichier d'un autre Mac a-t-il changé depuis la dernière lecture ?
    private func othersChanged() -> Bool {
        for (url, date) in remoteFiles() where remoteStamps[url.lastPathComponent] != date {
            return true
        }
        return false
    }

    private func remoteFiles() -> [(URL, Date)] {
        let fm = FileManager.default
        let keys: [URLResourceKey] = [.contentModificationDateKey]
        let files = (try? fm.contentsOfDirectory(at: Self.devicesDirectory, includingPropertiesForKeys: keys)) ?? []
        var result: [(URL, Date)] = []
        for file in files {
            let name = file.lastPathComponent
            // Fichier pas encore téléchargé par iCloud Drive (« .xxx.json.icloud ») : on le demande
            if name.hasPrefix("."), name.hasSuffix(".icloud") {
                try? fm.startDownloadingUbiquitousItem(at: file)
                continue
            }
            guard file.pathExtension == "json", name != Self.deviceID + ".json" else { continue }
            let date = (try? file.resourceValues(forKeys: Set(keys)))?.contentModificationDate ?? .distantPast
            result.append((file, date))
        }
        return result
    }

    // MARK: - Synchronisation

    func sync() async {
        guard Self.isEnabled, !isSyncing else { return }
        guard Self.isICloudDriveAvailable else {
            lastError = String(localized: "iCloud Drive n'est pas activé sur ce Mac.")
            return
        }
        isSyncing = true
        defer { isSyncing = false }

        do {
            try FileManager.default.createDirectory(at: Self.devicesDirectory, withIntermediateDirectories: true)
            var state = loadLocalState()
            let now = Date.now.timeIntervalSince1970

            // 1. État local, horodaté par rapport au dernier instantané
            applyPendingArticleStates(state)
            var current = buildSnapshot(previous: state.lastSnapshot, now: now)

            // 2. Fusion des autres Mac : la modification la plus récente l'emporte
            var others: [(name: String, updated: Date)] = []
            var newFeeds: [Feed] = []
            for (url, date) in remoteFiles() {
                remoteStamps[url.lastPathComponent] = date
                guard let data = try? Data(contentsOf: url),
                      let remote = try? JSONDecoder().decode(SyncModel.Snapshot.self, from: data),
                      remote.version == 1 else { continue }
                others.append((remote.deviceName, Date(timeIntervalSince1970: remote.updatedAt)))
                newFeeds += merge(remote, into: &current)
            }
            otherDevices = others.sorted { $0.updated > $1.updated }
            if context.hasChanges { try context.save() }

            // 3. On publie notre état, seulement s'il a changé : sinon les Mac se relanceraient
            //    l'un l'autre en boucle à chaque lecture
            current.device = Self.deviceID
            current.deviceName = Self.deviceName
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.sortedKeys]
            var previous = state.lastSnapshot
            previous.updatedAt = 0
            current.updatedAt = 0
            let unchanged = (try? encoder.encode(previous)) == (try? encoder.encode(current))
            let ownFile = Self.devicesDirectory.appending(path: Self.deviceID + ".json")
            if unchanged && FileManager.default.fileExists(atPath: ownFile.path) {
                current.updatedAt = state.lastSnapshot.updatedAt
            } else {
                current.updatedAt = now
                try encoder.encode(current).write(to: ownFile, options: .atomic)
            }

            state.lastSnapshot = current
            state.knownArticles = Set(allArticles().map(\.key))
            saveLocalState(state)

            lastSync = .now
            lastError = nil

            // Flux ajoutés depuis un autre Mac : on récupère leurs articles
            if !newFeeds.isEmpty {
                Task { for feed in newFeeds { await store.refresh(feed) } }
            }
        } catch {
            lastError = error.localizedDescription
        }
    }

    // MARK: - Instantané local

    private func allArticles() -> [Article] {
        (try? context.fetch(FetchDescriptor<Article>())) ?? []
    }

    private func buildSnapshot(previous: SyncModel.Snapshot, now: Double) -> SyncModel.Snapshot {
        var snap = SyncModel.Snapshot()
        let tombstoneLimit = now - Self.tombstoneLifetime

        // Dossiers
        let folders = (try? context.fetch(FetchDescriptor<Folder>())) ?? []
        for folder in folders {
            var entry = SyncModel.FolderEntry(name: folder.name, symbol: folder.symbol, color: folder.colorName, modified: now)
            if let old = previous.folders[folder.uid], old.sameValue(as: entry) { entry.modified = old.modified }
            snap.folders[folder.uid] = entry
        }
        for (uid, old) in previous.folders where snap.folders[uid] == nil {
            if old.deleted { if old.modified > tombstoneLimit { snap.folders[uid] = old } }
            else { var gone = old; gone.deleted = true; gone.modified = now; snap.folders[uid] = gone }
        }

        // Flux
        let feeds = (try? context.fetch(FetchDescriptor<Feed>())) ?? []
        for feed in feeds {
            var entry = SyncModel.FeedEntry(title: feed.title, siteURL: feed.siteURL, customTitle: feed.customTitle,
                                            folder: feed.folder?.uid, previousURL: feed.previousURL, modified: now)
            if let old = previous.feeds[feed.url], old.sameValue(as: entry) { entry.modified = old.modified }
            snap.feeds[feed.url] = entry
        }
        for (url, old) in previous.feeds where snap.feeds[url] == nil {
            if old.deleted { if old.modified > tombstoneLimit { snap.feeds[url] = old } }
            else { var gone = old; gone.deleted = true; gone.modified = now; snap.feeds[url] = gone }
        }

        // Règles de filtrage
        let rules = (try? context.fetch(FetchDescriptor<FilterRule>())) ?? []
        for rule in rules {
            var entry = SyncModel.FilterEntry(keyword: rule.keyword, wholeWord: rule.wholeWord, action: rule.action,
                                              feedURLs: rule.feedURLs, modified: now)
            if let old = previous.filters[rule.uid], old.sameValue(as: entry) { entry.modified = old.modified }
            snap.filters[rule.uid] = entry
        }
        for (uid, old) in previous.filters where snap.filters[uid] == nil {
            if old.deleted { if old.modified > tombstoneLimit { snap.filters[uid] = old } }
            else { var gone = old; gone.deleted = true; gone.modified = now; snap.filters[uid] = gone }
        }

        // Articles : seuls les états différents du défaut sont publiés
        let retention = Prefs.retentionCutoff.timeIntervalSince1970
        var present = Set<String>()
        for article in allArticles() {
            present.insert(article.key)
            var entry = SyncModel.ArticleEntry(read: article.isRead, starred: article.isStarred,
                                               published: article.published.timeIntervalSince1970, modified: now)
            if article.isStarred {
                entry.meta = SyncModel.ArticleMeta(feedURL: article.feedURL, title: article.title, link: article.link,
                                                   summary: article.summary, imageURL: article.imageURL,
                                                   author: article.author, source: article.sourceName)
            }
            if let old = previous.articles[article.key] {
                if old.sameValue(as: entry) { entry.modified = old.modified }
                snap.articles[article.key] = entry   // y compris un retour à « non lu », daté
            } else if !entry.isDefault {
                snap.articles[article.key] = entry
            }
        }
        // Articles disparus d'ici (supprimés par l'ancienneté) : on garde leur état sans le modifier
        for (key, old) in previous.articles where !present.contains(key) {
            if old.starred || (old.published > retention && old.modified > tombstoneLimit) {
                snap.articles[key] = old
            }
        }
        // Inutile de traîner éternellement des articles revenus à l'état par défaut
        snap.articles = snap.articles.filter { !$0.value.isDefault || $0.value.modified > tombstoneLimit }
        return snap
    }

    /// Articles arrivés depuis la dernière synchro : ils prennent l'état synchronisé
    /// (lu ou favori sur l'autre Mac) au lieu d'être vus comme « remis à non lu »
    private func applyPendingArticleStates(_ state: SyncModel.LocalState) {
        guard !state.lastSnapshot.articles.isEmpty else { return }
        for article in allArticles() where !state.knownArticles.contains(article.key) {
            guard let entry = state.lastSnapshot.articles[article.key] else { continue }
            if article.isRead != entry.read { article.isRead = entry.read }
            if article.isStarred != entry.starred { store.setStarred(entry.starred, [article]) }
        }
    }

    // MARK: - Fusion

    /// Applique les éléments plus récents d'un autre Mac. Renvoie les flux créés.
    private func merge(_ remote: SyncModel.Snapshot, into current: inout SyncModel.Snapshot) -> [Feed] {
        var createdFeeds: [Feed] = []

        // Dossiers d'abord (les flux y sont rangés)
        var folders = Dictionary(((try? context.fetch(FetchDescriptor<Folder>())) ?? []).map { ($0.uid, $0) },
                                 uniquingKeysWith: { a, _ in a })
        for (uid, entry) in remote.folders {
            if let local = current.folders[uid], local.modified >= entry.modified { continue }
            if entry.deleted {
                if let folder = folders[uid] { store.delete(folder); folders[uid] = nil }
            } else if let folder = folders[uid] {
                folder.name = entry.name
                folder.symbol = entry.symbol
                folder.colorName = entry.color
            } else if let twin = folders.values.first(where: {
                $0.name.lowercased() == entry.name.lowercased() && remote.folders[$0.uid] == nil
            }) {
                // Même dossier créé séparément sur les deux Mac : on les réunit sous un seul identifiant
                let oldUID = twin.uid
                folders[oldUID] = nil
                current.folders[oldUID] = nil
                for (feedURL, var feedEntry) in current.feeds where feedEntry.folder == oldUID {
                    feedEntry.folder = uid
                    current.feeds[feedURL] = feedEntry
                }
                twin.uid = uid
                twin.symbol = entry.symbol
                twin.colorName = entry.color
                folders[uid] = twin
            } else {
                let folder = Folder(name: entry.name)
                folder.uid = uid
                folder.symbol = entry.symbol
                folder.colorName = entry.color
                context.insert(folder)
                folders[uid] = folder
            }
            current.folders[uid] = entry
        }

        // Flux
        var feeds = Dictionary(((try? context.fetch(FetchDescriptor<Feed>())) ?? []).map { ($0.url, $0) },
                               uniquingKeysWith: { a, _ in a })
        // Adresse changée sur un autre Mac : on déplace le flux local (articles, lus, favoris)
        // au lieu de le supprimer puis de le recréer
        for (url, entry) in remote.feeds where !entry.deleted && feeds[url] == nil {
            guard let oldURL = entry.previousURL, let feed = feeds[oldURL] else { continue }
            store.migrate(feed, to: url)
            feeds[oldURL] = nil
            feeds[url] = feed
            if var gone = current.feeds[oldURL] {
                gone.deleted = true
                gone.modified = entry.modified
                current.feeds[oldURL] = gone
            }
            createdFeeds.append(feed)
        }

        for (url, entry) in remote.feeds {
            if let local = current.feeds[url], local.modified >= entry.modified { continue }
            if entry.deleted {
                if let feed = feeds[url] { store.delete(feed); feeds[url] = nil }
            } else {
                let feed: Feed
                if let existing = feeds[url] {
                    feed = existing
                } else {
                    feed = Feed(url: url, title: entry.title, siteURL: entry.siteURL)
                    context.insert(feed)
                    feeds[url] = feed
                    createdFeeds.append(feed)
                    // Favoris conservés d'une ancienne suppression de ce flux
                    let orphans = (try? context.fetch(FetchDescriptor<Article>(
                        predicate: #Predicate { $0.feedURL == url }))) ?? []
                    for article in orphans where article.feed == nil { article.feed = feed }
                }
                feed.customTitle = entry.customTitle
                feed.folder = entry.folder.flatMap { folders[$0] }
            }
            current.feeds[url] = entry
        }

        // Règles de filtrage
        let rules = (try? context.fetch(FetchDescriptor<FilterRule>())) ?? []
        var rulesByUID = Dictionary(rules.map { ($0.uid, $0) }, uniquingKeysWith: { a, _ in a })
        var filtersChanged = false
        for (uid, entry) in remote.filters {
            if let local = current.filters[uid], local.modified >= entry.modified { continue }
            filtersChanged = true
            if entry.deleted {
                if let rule = rulesByUID[uid] { store.deleteFilter(rule); rulesByUID[uid] = nil }
            } else if let rule = rulesByUID[uid] {
                store.updateFilter(rule, keyword: entry.keyword, wholeWord: entry.wholeWord,
                                   action: FilterAction(rawValue: entry.action) ?? .hide, feedURLs: entry.scope)
            } else if let twin = rulesByUID.values.first(where: {
                CompiledFilter.fold($0.keyword) == CompiledFilter.fold(entry.keyword)
                    && $0.feedURLs.sorted() == entry.scope && remote.filters[$0.uid] == nil
            }) {
                // Même règle créée sur les deux Mac : un seul identifiant
                rulesByUID[twin.uid] = nil
                current.filters[twin.uid] = nil
                twin.uid = uid
                rulesByUID[uid] = twin
            } else {
                let rule = FilterRule(keyword: entry.keyword, wholeWord: entry.wholeWord,
                                      action: FilterAction(rawValue: entry.action) ?? .hide, feedURLs: entry.scope)
                rule.uid = uid
                context.insert(rule)
                rulesByUID[uid] = rule
            }
            current.filters[uid] = entry
        }
        if filtersChanged { store.applyFiltersToExisting() }

        // États des articles
        let articles = Dictionary(allArticles().map { ($0.key, $0) }, uniquingKeysWith: { a, _ in a })
        var toStar: [Article] = []
        var toUnstar: [Article] = []
        for (key, entry) in remote.articles {
            if let local = current.articles[key], local.modified >= entry.modified { continue }
            if let article = articles[key] {
                if article.isRead != entry.read { article.isRead = entry.read }
                if article.isStarred != entry.starred {
                    if entry.starred { toStar.append(article) } else { toUnstar.append(article) }
                }
            } else if entry.starred, let meta = entry.meta {
                // Favori que ce Mac n'a pas (article plus ancien que son flux) : on le recrée
                let article = Article(key: key, feedURL: meta.feedURL, title: meta.title, link: meta.link,
                                      summary: meta.summary, contentHTML: nil, imageURL: meta.imageURL,
                                      author: meta.author, published: Date(timeIntervalSince1970: entry.published))
                article.isRead = entry.read
                article.sourceTitle = meta.source
                context.insert(article)
                article.feed = feeds[meta.feedURL]
                toStar.append(article)
            }
            // Article absent et simple « lu » : l'état est gardé, il s'appliquera à son arrivée
            current.articles[key] = entry
        }
        if !toStar.isEmpty { store.setStarred(true, toStar) }
        if !toUnstar.isEmpty { store.setStarred(false, toUnstar) }

        return createdFeeds
    }

    // MARK: - Mémoire locale

    private func loadLocalState() -> SyncModel.LocalState {
        guard let data = try? Data(contentsOf: Self.localStateURL),
              let state = try? JSONDecoder().decode(SyncModel.LocalState.self, from: data) else {
            return SyncModel.LocalState()
        }
        return state
    }

    private func saveLocalState(_ state: SyncModel.LocalState) {
        if let data = try? JSONEncoder().encode(state) {
            try? data.write(to: Self.localStateURL, options: .atomic)
        }
    }

    /// Désactivation : on retire ce Mac de la synchronisation (son fichier) et on oublie l'état local
    func disable() {
        debounceTask?.cancel()
        try? FileManager.default.removeItem(at: Self.devicesDirectory.appending(path: Self.deviceID + ".json"))
        try? FileManager.default.removeItem(at: Self.localStateURL)
        otherDevices = []
        lastSync = nil
        lastError = nil
    }
}
