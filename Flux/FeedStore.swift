import Foundation
import Observation
import SwiftData

@MainActor
@Observable
final class FeedStore {
    private(set) var isRefreshing = false
    /// Pas de connexion lors de la dernière actualisation
    private(set) var isOffline = false
    /// Progression de la préparation hors ligne (nil = rien en cours)
    private(set) var offlineStatus: String?
    @ObservationIgnored let context: ModelContext

    init(context: ModelContext) {
        self.context = context
        migrateFilterScopes()
    }

    /// Règles créées avant le choix de plusieurs flux : l'ancien champ feedURL passe dans feedURLs
    private func migrateFilterScopes() {
        let rules = (try? context.fetch(FetchDescriptor<FilterRule>(
            predicate: #Predicate { $0.feedURL != nil }))) ?? []
        guard !rules.isEmpty else { return }
        for rule in rules {
            if rule.feedURLs.isEmpty, let url = rule.feedURL { rule.feedURLs = [url] }
            rule.feedURL = nil
        }
        try? context.save()
    }

    // MARK: - Ajout

    @discardableResult
    func addFeed(urlString: String, folder: Folder? = nil) async throws -> Feed {
        var s = urlString.trimmingCharacters(in: .whitespacesAndNewlines)
        if !s.contains("://") { s = "https://" + s }
        guard let url = URL(string: s), url.host() != nil else { throw FeedError.invalidURL }

        let (feedURL, parsed) = try await Self.discover(url)
        let key = feedURL.absoluteString

        let existing = try context.fetch(FetchDescriptor<Feed>(predicate: #Predicate { $0.url == key }))
        guard existing.isEmpty else { throw FeedError.alreadyExists }

        let title = parsed.title.map(HTMLUtils.plainText).flatMap { $0.isEmpty ? nil : $0 } ?? url.host() ?? key
        let feed = Feed(url: key, title: title, siteURL: parsed.siteURL)
        context.insert(feed)
        feed.folder = folder
        // Favoris conservés d'une précédente suppression de ce flux : ils le retrouvent
        let sameURL = (try? context.fetch(FetchDescriptor<Article>(
            predicate: #Predicate { $0.feedURL == key }))) ?? []
        for article in sameURL where article.feed == nil { article.feed = feed }
        var added = ingest(parsed, into: feed)
        feed.lastRefreshed = .now
        try? context.save()

        // Flux court (WordPress n'en publie souvent que 10, parfois 5) : on remonte les pages précédentes
        if parsed.items.count < 20 {
            added += await fetchOlderArticles(for: feed)
        }
        groupDuplicates(added)
        Task { await fillMissingImages(added) }
        return feed
    }

    /// Récupère les articles plus anciens d'un flux paginé (`?paged=2`, `?paged=3`… : WordPress
    /// et la plupart des CMS). Les flux non paginés renvoient la même page : rien n'est ajouté.
    @discardableResult
    func fetchOlderArticles(for feed: Feed, maxPages: Int = 8) async -> [Article] {
        guard let base = URLComponents(string: feed.url) else { return [] }
        let cutoff = Prefs.retentionCutoff
        var added: [Article] = []
        for page in 2...max(2, maxPages) {
            var components = base
            var items = components.queryItems ?? []
            items.removeAll { $0.name == "paged" }
            items.append(URLQueryItem(name: "paged", value: String(page)))
            components.queryItems = items
            guard let url = components.url,
                  case .success(let parsed) = await Self.fetchFeed(url.absoluteString),
                  !parsed.items.isEmpty, !feed.isDeleted else { break }
            // Pages d'archive : leur titre (« Page 8 - … ») ne doit pas remplacer celui du flux
            let new = ingest(parsed, into: feed, updateFeedInfo: false)
            added += new
            // Plus rien de nouveau (flux non paginé) ou on dépasse la durée de conservation : on s'arrête
            let oldest = parsed.items.compactMap(\.published).min() ?? .distantPast
            if new.isEmpty || oldest < cutoff { break }
        }
        if !added.isEmpty { try? context.save() }
        return added
    }

    /// Accepte l'URL d'un flux, ou celle d'un site (détection automatique).
    nonisolated static func discover(_ url: URL) async throws -> (URL, ParsedFeed) {
        let (data, finalURL) = try await Net.get(url)
        if let parsed = FeedParser.parse(data) { return (finalURL, parsed) }

        let html = Net.decodeHTML(data)
        for candidate in HTMLUtils.feedLinks(in: html, base: finalURL) {
            if let r = try? await Net.get(candidate), let parsed = FeedParser.parse(r.0) { return (r.1, parsed) }
        }
        for path in ["/feed", "/rss", "/feed.xml", "/rss.xml", "/atom.xml", "/index.xml", "/feed.json"] {
            guard let candidate = URL(string: path, relativeTo: finalURL)?.absoluteURL else { continue }
            if let r = try? await Net.get(candidate), let parsed = FeedParser.parse(r.0) { return (r.1, parsed) }
        }
        throw FeedError.notAFeed
    }

    // MARK: - Actualisation

    func refreshAll() async {
        guard !isRefreshing else { return }
        isRefreshing = true
        defer { isRefreshing = false }

        backfillSearchText()

        let feeds = (try? context.fetch(FetchDescriptor<Feed>())) ?? []
        let urls = feeds.map(\.url)

        let results = await withTaskGroup(of: (String, Result<ParsedFeed, Error>).self) { group in
            for u in urls {
                group.addTask { (u, await Self.fetchFeed(u)) }
            }
            var out: [String: Result<ParsedFeed, Error>] = [:]
            for await (u, r) in group { out[u] = r }
            return out
        }

        // Hors ligne si toutes les requêtes ont échoué faute de connexion
        let failures = results.values.compactMap { r -> Error? in
            if case .failure(let e) = r { return e } else { return nil }
        }
        isOffline = !results.isEmpty && failures.count == results.count && failures.allSatisfy(Self.isOfflineError)

        var added: [Article] = []
        for feed in feeds {
            guard let result = results[feed.url] else { continue }
            added += apply(result, to: feed)
        }
        applyAutoMarkRead(save: false)
        prune()
        if context.hasChanges { try? context.save() }
        groupDuplicates(added)
        guard !isOffline else { return }
        await fillMissingImages(added)
        // En tâche séparée : l'actualisation (et son indicateur) se termine sans attendre
        if Prefs.offline { Task { await prepareOffline() } }
        if Prefs.archive { Task { await archiveFavorites() } }

        // Le cache d'images se remplit aussi en lisant normalement : on le garde sous la limite
        let limit = Prefs.imageCacheLimitBytes
        Task.detached(priority: .background) { ImageCache.trim(toBytes: limit) }
    }

    nonisolated static func isOfflineError(_ error: Error) -> Bool {
        guard let code = (error as? URLError)?.code else { return false }
        return [.notConnectedToInternet, .networkConnectionLost, .dataNotAllowed,
                .internationalRoamingOff, .timedOut].contains(code)
    }

    func refresh(_ feed: Feed) async {
        let result = await Self.fetchFeed(feed.url)
        let added = apply(result, to: feed)
        if context.hasChanges { try? context.save() }
        groupDuplicates(added)
        await fillMissingImages(added)
    }

    nonisolated static func fetchFeed(_ urlString: String) async -> Result<ParsedFeed, Error> {
        guard let url = URL(string: urlString) else { return .failure(FeedError.invalidURL) }
        do {
            let (data, _) = try await Net.get(url)
            guard let parsed = FeedParser.parse(data) else { return .failure(FeedError.notAFeed) }
            return .success(parsed)
        } catch {
            return .failure(error)
        }
    }

    private func apply(_ result: Result<ParsedFeed, Error>, to feed: Feed) -> [Article] {
        guard !feed.isDeleted, feed.modelContext != nil else { return [] }
        switch result {
        case .success(let parsed):
            // On n'écrit que ce qui change, pour ne pas réveiller les vues inutilement
            if feed.lastError != nil { feed.lastError = nil }
            feed.lastRefreshed = .now
            return ingest(parsed, into: feed)
        case .failure(let error):
            // Pas de connexion : ce n'est pas la faute du flux, on ne le signale pas en erreur
            if Self.isOfflineError(error) { return [] }
            let message = error.localizedDescription
            if feed.lastError != message { feed.lastError = message }
            return []
        }
    }

    /// Insère les nouveaux articles d'un flux, sans toucher à ceux déjà connus.
    private func ingest(_ parsed: ParsedFeed, into feed: Feed, updateFeedInfo: Bool = true) -> [Article] {
        if updateFeedInfo {
            if feed.siteURL == nil, let site = parsed.siteURL { feed.siteURL = site }
            // Le nom d'origine suit le flux ; un nom personnalisé (customTitle) reste prioritaire à l'affichage
            if let fresh = parsed.title.map(HTMLUtils.plainText), !fresh.isEmpty, fresh != feed.title {
                feed.title = fresh
            }
        }
        var known = Set(feed.articles.map(\.key))
        let base = URL(string: feed.siteURL ?? feed.url)
        // On n'importe pas les articles plus vieux que la durée de conservation
        let cutoff = Prefs.retentionCutoff
        let autoReadCutoff = Prefs.autoMarkReadCutoff
        let filters = compiledFilters()
        var added: [Article] = []

        for item in parsed.items.prefix(200) {
            if let date = item.published, date < cutoff { continue }

            let link = item.link.flatMap { HTMLUtils.resolve($0, base: base) }
            guard let ident = item.guid ?? link ?? item.title else { continue }
            let key = feed.url + "|" + ident
            guard !known.contains(key) else { continue }
            known.insert(key)

            let html = item.contentHTML ?? item.summaryHTML
            let title = HTMLUtils.plainText(item.title ?? "")
            let summary = String(HTMLUtils.plainText(item.summaryHTML ?? item.contentHTML ?? "").prefix(280))
            let imageBase = link.flatMap { URL(string: $0) } ?? base
            let image = (item.imageURL ?? html.flatMap(HTMLUtils.firstImage))
                .flatMap { HTMLUtils.resolve($0, base: imageBase) }

            let article = Article(
                key: key,
                feedURL: feed.url,
                title: title.isEmpty ? String(localized: "(Sans titre)") : title,
                link: link,
                summary: summary,
                contentHTML: html,
                imageURL: image,
                author: item.author.map(HTMLUtils.plainText),
                published: item.published ?? .now
            )
            if let autoReadCutoff, article.published < autoReadCutoff, !article.isStarred {
                article.isRead = true
            }
            applyFilters(filters, to: article)
            context.insert(article)
            article.feed = feed
            added.append(article)
        }
        return added
    }

    /// Pour les articles sans miniature : récupère l'og:image de la page.
    private func fillMissingImages(_ articles: [Article]) async {
        let targets = articles
            .filter { $0.imageURL == nil }
            .compactMap { a in a.link.map { (a.key, $0) } }
            .prefix(40)
        guard !targets.isEmpty else { return }

        let list = Array(targets)
        var found: [String: String] = [:]
        for start in stride(from: 0, to: list.count, by: 6) {
            let chunk = list[start..<min(start + 6, list.count)]
            let partial = await withTaskGroup(of: (String, String?).self) { group in
                for (key, link) in chunk {
                    group.addTask { (key, await Self.ogImage(for: link)) }
                }
                var out: [String: String] = [:]
                for await (key, image) in group {
                    if let image { out[key] = image }
                }
                return out
            }
            found.merge(partial) { a, _ in a }
        }

        for article in articles where !article.isDeleted {
            if let image = found[article.key] { article.imageURL = image }
        }
        try? context.save()
    }

    nonisolated static func ogImage(for link: String) async -> String? {
        guard let url = URL(string: link), let r = try? await Net.get(url) else { return nil }
        return HTMLUtils.ogImage(in: Net.decodeHTML(r.0)).flatMap { HTMLUtils.resolve($0, base: r.1) }
    }

    /// Calcule le texte de recherche des articles enregistrés avant la recherche plein texte
    /// (par lots, pour ne pas bloquer l'interface)
    private func backfillSearchText() {
        var descriptor = FetchDescriptor<Article>(predicate: #Predicate { $0.searchText == "" })
        descriptor.fetchLimit = 500
        for article in (try? context.fetch(descriptor)) ?? [] {
            article.updateSearchText()
        }
    }

    // MARK: - Gestion

    func markAllRead(_ filter: SidebarSelection) {
        let predicate: Predicate<Article>
        switch filter {
        case .feed(let url):
            predicate = #Predicate { !$0.isRead && $0.feedURL == url }
        case .starred:
            predicate = #Predicate { !$0.isRead && $0.isStarred }
        case .folder(let uid):
            let urls = feedURLs(inFolder: uid)
            predicate = #Predicate { !$0.isRead && urls.contains($0.feedURL) }
        case .history:
            predicate = #Predicate { !$0.isRead && $0.openedAt != nil }
        case .tag(let uid):
            let needle = Article.tagNeedle(uid)
            predicate = #Predicate { !$0.isRead && $0.isStarred && $0.tagsRaw.localizedStandardContains(needle) }
        case .all, .unread:
            predicate = #Predicate { !$0.isRead }
        }
        for article in (try? context.fetch(FetchDescriptor(predicate: predicate))) ?? [] {
            article.isRead = true
        }
        try? context.save()
    }

    /// Marque comme lu / non lu, avec les articles similaires des autres sources
    func setRead(_ read: Bool, _ articles: [Article]) {
        var targets = articles
        var seenClusters = Set<String>()
        for article in articles {
            guard let cluster = article.clusterID, seenClusters.insert(cluster).inserted else { continue }
            targets += clusterMembers(cluster)
        }
        for article in targets where article.isRead != read { article.isRead = read }
        try? context.save()
    }

    func setStarred(_ starred: Bool, _ articles: [Article]) {
        let changed = articles.filter { $0.isStarred != starred }
        for article in changed { article.isStarred = starred }
        try? context.save()
        guard !changed.isEmpty else { return }
        if starred {
            Task { await archiveFavorites(changed) }
        } else {
            unarchive(changed)
        }
    }

    // MARK: - Tags des favoris

    @discardableResult
    func createTag(named name: String) -> FavoriteTag? {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let tags = (try? context.fetch(FetchDescriptor<FavoriteTag>())) ?? []
        if let existing = tags.first(where: { $0.name.localizedCaseInsensitiveCompare(trimmed) == .orderedSame }) {
            return existing
        }
        let tag = FavoriteTag(name: trimmed)
        context.insert(tag)
        try? context.save()
        return tag
    }

    func rename(_ tag: FavoriteTag, to name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed != tag.name else { return }
        tag.name = trimmed
        try? context.save()
    }

    /// Supprime le tag ; les favoris restent, sans ce tag
    func delete(_ tag: FavoriteTag) {
        removeTagFromArticles(tag.uid)
        context.delete(tag)
        try? context.save()
    }

    func removeTagFromArticles(_ uid: String) {
        let needle = Article.tagNeedle(uid)
        let tagged = (try? context.fetch(FetchDescriptor<Article>(
            predicate: #Predicate { $0.tagsRaw.localizedStandardContains(needle) }))) ?? []
        for article in tagged { article.tagUIDs.removeAll { $0 == uid } }
    }

    /// Ajoute ou retire un tag. Un tag range un favori : l'ajouter à un article
    /// qui n'en est pas un l'ajoute aux favoris.
    func setTag(_ uid: String, _ on: Bool, _ articles: [Article]) {
        for article in articles {
            if on {
                if !article.tagUIDs.contains(uid) { article.tagUIDs.append(uid) }
            } else {
                article.tagUIDs.removeAll { $0 == uid }
            }
        }
        if on { setStarred(true, articles) } else { try? context.save() }
    }

    func articles(withKeys keys: Set<String>) -> [Article] {
        let list = Array(keys)
        return (try? context.fetch(FetchDescriptor<Article>(predicate: #Predicate { list.contains($0.key) }))) ?? []
    }

    /// Renomme un flux ; un nom vide rétablit le nom d'origine
    func rename(_ feed: Feed, to name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        feed.customTitle = (trimmed.isEmpty || trimmed == feed.title) ? nil : trimmed
        try? context.save()
    }

    // MARK: - Changement d'adresse

    /// Change l'adresse d'un flux (ex. un site qui propose un flux plus complet) en gardant
    /// ses articles, leur état lu/favori, son nom, son dossier et ses règles de filtrage.
    func changeAddress(of feed: Feed, to urlString: String) async throws {
        var s = urlString.trimmingCharacters(in: .whitespacesAndNewlines)
        if !s.contains("://") { s = "https://" + s }
        guard let url = URL(string: s), url.host() != nil else { throw FeedError.invalidURL }

        let (feedURL, parsed) = try await Self.discover(url)
        let newURL = feedURL.absoluteString
        guard !feed.isDeleted, newURL != feed.url else { return }
        let existing = try context.fetch(FetchDescriptor<Feed>(predicate: #Predicate { $0.url == newURL }))
        guard existing.isEmpty else { throw FeedError.alreadyExists }

        migrate(feed, to: newURL)
        if let site = parsed.siteURL { feed.siteURL = site }
        feed.lastError = nil
        // Les articles communs aux deux adresses (même guid) sont reconnus : pas de doublons
        var added = ingest(parsed, into: feed)
        feed.lastRefreshed = .now
        try? context.save()
        if parsed.items.count < 20 {
            added += await fetchOlderArticles(for: feed)
        }
        groupDuplicates(added)
        Task { await fillMissingImages(added) }
    }

    /// Réécrit les données locales d'un flux sous une nouvelle adresse, sans réseau.
    /// Sert aussi à la synchro, quand l'adresse a été changée sur un autre Mac.
    func migrate(_ feed: Feed, to newURL: String) {
        let oldURL = feed.url
        guard newURL != oldURL, !feed.isDeleted else { return }
        let oldPrefix = oldURL + "|"

        // Favoris gardés d'une ancienne suppression d'un flux à la nouvelle adresse
        let already = (try? context.fetch(FetchDescriptor<Article>(
            predicate: #Predicate { $0.feedURL == newURL }))) ?? []
        var byKey = Dictionary(already.map { ($0.key, $0) }, uniquingKeysWith: { a, _ in a })
        let articles = (try? context.fetch(FetchDescriptor<Article>(
            predicate: #Predicate { $0.feedURL == oldURL }))) ?? []

        feed.url = newURL
        feed.previousURL = oldURL
        for article in articles {
            let ident = article.key.hasPrefix(oldPrefix) ? String(article.key.dropFirst(oldPrefix.count)) : article.key
            let newKey = newURL + "|" + ident
            if let twin = byKey[newKey], twin !== article {
                // Déjà présent sous la nouvelle adresse : on garde un seul exemplaire
                if article.isRead && !twin.isRead { twin.isRead = true }
                if article.isStarred && !twin.isStarred { twin.isStarred = true }
                context.delete(article)
                continue
            }
            article.key = newKey
            article.feedURL = newURL
            if article.feed == nil { article.feed = feed }
            byKey[newKey] = article
        }
        for article in already where article.feed == nil { article.feed = feed }

        let rules = (try? context.fetch(FetchDescriptor<FilterRule>())) ?? []
        for rule in rules where rule.feedURLs.contains(oldURL) {
            rule.feedURLs = Array(Set(rule.feedURLs.map { $0 == oldURL ? newURL : $0 })).sorted()
        }
        try? context.save()
    }

    /// Supprime un flux et ses articles, sauf les favoris : ils restent dans Favoris
    /// (et reviennent dans le flux si on l'ajoute à nouveau)
    func delete(_ feed: Feed) {
        let name = feed.displayTitle
        for article in Array(feed.articles) where article.isStarred {
            article.sourceTitle = name
            article.feed = nil
        }
        context.delete(feed)
        try? context.save()
    }

    func favoriteCount(of feed: Feed) -> Int {
        feed.articles.filter(\.isStarred).count
    }

    // MARK: - Filtres

    func compiledFilters() -> [CompiledFilter] {
        ((try? context.fetch(FetchDescriptor<FilterRule>())) ?? []).map(CompiledFilter.init)
    }

    /// Applique la première règle qui correspond (les favoris ne sont jamais filtrés)
    private func applyFilters(_ filters: [CompiledFilter], to article: Article) {
        guard !article.isStarred, article.filteredBy == nil,
              let rule = filters.first(where: { $0.matches(title: article.title, feedURL: article.feedURL) }) else { return }
        article.filteredBy = rule.uid
        switch rule.action {
        case .hide: article.isHidden = true
        case .markRead: article.isRead = true
        }
    }

    /// Repasse toutes les règles sur les articles qui ne sont filtrés par aucune
    func applyFiltersToExisting() {
        let filters = compiledFilters()
        guard !filters.isEmpty else { return }
        let predicate = #Predicate<Article> { $0.filteredBy == nil && !$0.isStarred }
        for article in (try? context.fetch(FetchDescriptor(predicate: predicate))) ?? [] {
            applyFilters(filters, to: article)
        }
        try? context.save()
    }

    /// Libère les articles filtrés par une règle (ils réapparaissent ; ceux déjà marqués comme lus le restent)
    private func releaseArticles(of uid: String) {
        let predicate = #Predicate<Article> { $0.filteredBy == uid }
        for article in (try? context.fetch(FetchDescriptor(predicate: predicate))) ?? [] {
            article.filteredBy = nil
            article.isHidden = false
        }
    }

    func addFilter(keyword: String, wholeWord: Bool, action: FilterAction, feedURLs: [String]) {
        let trimmed = keyword.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        context.insert(FilterRule(keyword: trimmed, wholeWord: wholeWord, action: action, feedURLs: feedURLs))
        try? context.save()
        applyFiltersToExisting()
    }

    func updateFilter(_ rule: FilterRule, keyword: String, wholeWord: Bool, action: FilterAction, feedURLs: [String]) {
        let trimmed = keyword.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        releaseArticles(of: rule.uid)
        rule.keyword = trimmed
        rule.wholeWord = wholeWord
        rule.action = action.rawValue
        rule.feedURLs = feedURLs.sorted()
        try? context.save()
        applyFiltersToExisting()
    }

    func deleteFilter(_ rule: FilterRule) {
        releaseArticles(of: rule.uid)
        context.delete(rule)
        try? context.save()
        applyFiltersToExisting()   // une autre règle peut encore s'appliquer
    }

    /// Aperçu d'une règle pas encore enregistrée : nombre d'articles (hors favoris) dont le titre correspond
    func previewMatchCount(_ filter: CompiledFilter) -> Int {
        var descriptor = FetchDescriptor<Article>(predicate: #Predicate { !$0.isStarred })
        descriptor.propertiesToFetch = [\.title, \.feedURL]
        let articles = (try? context.fetch(descriptor)) ?? []
        return articles.reduce(0) { $0 + (filter.matches(title: $1.title, feedURL: $1.feedURL) ? 1 : 0) }
    }

    /// Nombre d'articles actuellement touchés par une règle
    func matchCount(of rule: FilterRule) -> Int {
        let uid = rule.uid
        return (try? context.fetchCount(FetchDescriptor<Article>(predicate: #Predicate { $0.filteredBy == uid }))) ?? 0
    }

    // MARK: - Articles similaires

    func clusterMembers(_ cluster: String) -> [Article] {
        let predicate = #Predicate<Article> { $0.clusterID == cluster }
        return (try? context.fetch(FetchDescriptor(predicate: predicate,
                                                   sortBy: [SortDescriptor(\.published)]))) ?? []
    }

    /// Autres articles du même groupe que `article`
    func similarArticles(to article: Article) -> [Article] {
        guard let cluster = article.clusterID else { return [] }
        return clusterMembers(cluster).filter { $0.key != article.key }
    }

    /// Rattache les nouveaux articles aux articles similaires des autres sources (48 h autour)
    func groupDuplicates(_ newArticles: [Article]) {
        guard UserDefaults.standard.bool(forKey: Prefs.groupDuplicates) else { return }
        // Première fois, ou algorithme amélioré depuis : on refait tous les groupes
        if UserDefaults.standard.integer(forKey: Prefs.duplicatesVersion) < DuplicateDetector.version {
            UserDefaults.standard.set(DuplicateDetector.version, forKey: Prefs.duplicatesVersion)
            regroupRecentDuplicates()
            return
        }
        let candidates = newArticles.filter { !$0.isDeleted && !$0.isHidden && !$0.clusterOptOut && $0.clusterID == nil }
        guard !candidates.isEmpty else { return }

        let earliest = (candidates.map(\.published).min() ?? .now).addingTimeInterval(-DuplicateDetector.window)
        let poolPredicate = #Predicate<Article> { $0.published >= earliest && !$0.isHidden && !$0.clusterOptOut }
        let pool = (try? context.fetch(FetchDescriptor(predicate: poolPredicate))) ?? []
        cluster(candidates, within: pool)
    }

    /// Recalcule les groupes : tous défaits, puis les 7 derniers jours regroupés
    /// (réglages modifiés, nouvelle version de l'algorithme)
    func regroupRecentDuplicates() {
        // Option désactivée : on défait tous les groupes, anciens compris
        guard UserDefaults.standard.bool(forKey: Prefs.groupDuplicates) else {
            let clustered = (try? context.fetch(FetchDescriptor<Article>(
                predicate: #Predicate { $0.clusterID != nil }))) ?? []
            for article in clustered {
                article.clusterID = nil
                article.isDuplicate = false
            }
            try? context.save()
            return
        }
        let clustered = (try? context.fetch(FetchDescriptor<Article>(
            predicate: #Predicate { $0.clusterID != nil }))) ?? []
        for article in clustered {
            article.clusterID = nil
            article.isDuplicate = false
        }
        let since = Date.now.addingTimeInterval(-7 * 86_400)
        let recent = (try? context.fetch(FetchDescriptor<Article>(
            predicate: #Predicate { $0.published >= since }))) ?? []
        if UserDefaults.standard.bool(forKey: Prefs.groupDuplicates) {
            let usable = recent.filter { !$0.isHidden && !$0.clusterOptOut }
            cluster(usable.sorted { $0.published < $1.published }, within: usable)
        }
        try? context.save()
    }

    private func cluster(_ candidates: [Article], within pool: [Article]) {
        let sensitivity = DuplicateDetector.Sensitivity(
            rawValue: UserDefaults.standard.integer(forKey: Prefs.duplicateSensitivity)) ?? .normal
        let detector = DuplicateDetector(sensitivity: sensitivity)

        // Signatures et index « mot → articles » pour ne comparer que ce qui partage des mots
        var signatures: [String: DuplicateDetector.Signature] = [:]
        var byWord: [String: [Article]] = [:]
        var byLink: [String: [Article]] = [:]
        for article in pool {
            let signature = detector.signature(title: article.title, link: article.link)
            signatures[article.key] = signature
            for word in signature.words { byWord[word, default: []].append(article) }
            if let link = signature.link { byLink[link, default: []].append(article) }
        }
        detector.prepare(Array(signatures.values))

        // Groupes existants : leurs membres (pour connaître les sources déjà présentes et le premier article)
        var members: [String: [Article]] = [:]
        for article in pool { if let id = article.clusterID { members[id, default: []].append(article) } }
        func groupMembers(_ id: String) -> [Article] {
            if let known = members[id] { return known }
            let fetched = clusterMembers(id)
            members[id] = fetched
            return fetched
        }

        var touched = Set<String>()
        for article in candidates where article.clusterID == nil {
            guard let signature = signatures[article.key] else { continue }
            var possible: [String: Article] = [:]
            for word in signature.words { for other in byWord[word] ?? [] { possible[other.key] = other } }
            if let link = signature.link { for other in byLink[link] ?? [] { possible[other.key] = other } }

            var best: (article: Article, score: Double)?
            for other in possible.values {
                guard other.key != article.key, other.feedURL != article.feedURL,
                      abs(other.published.timeIntervalSince(article.published)) <= DuplicateDetector.window,
                      let otherSignature = signatures[other.key],
                      let score = detector.score(signature, otherSignature) else { continue }
                if let id = other.clusterID {
                    let group = groupMembers(id)
                    // Une source ne publie pas deux fois la même actualité : une seule place par source
                    guard !group.contains(where: { $0.feedURL == article.feedURL }) else { continue }
                    // Pas de réaction en chaîne : il faut aussi ressembler au premier article du groupe
                    if let lead = group.min(by: { $0.published < $1.published }), lead.key != other.key {
                        let leadSignature = signatures[lead.key] ?? detector.signature(title: lead.title, link: lead.link)
                        guard detector.score(signature, leadSignature) != nil else { continue }
                    }
                }
                if score > (best?.score ?? 0) { best = (other, score) }
            }
            guard let match = best?.article else { continue }
            let id = match.clusterID ?? UUID().uuidString
            if match.clusterID == nil {
                match.clusterID = id
                members[id] = [match]
            }
            article.clusterID = id
            members[id, default: []].append(article)
            touched.insert(id)
        }

        // Dans chaque groupe, le premier publié représente l'actualité ; les autres suivent son état de lecture
        for id in touched {
            let group = clusterMembers(id)
            guard let lead = group.first else { continue }
            for member in group {
                member.isDuplicate = member.key != lead.key
                if member.isDuplicate, lead.isRead, !member.isRead { member.isRead = true }
            }
        }
        if !touched.isEmpty { try? context.save() }
    }

    /// Sort un article de son groupe ; il ne sera plus regroupé
    func detachFromCluster(_ article: Article) {
        guard let cluster = article.clusterID else { return }
        article.clusterID = nil
        article.isDuplicate = false
        article.clusterOptOut = true
        let remaining = clusterMembers(cluster)
        if remaining.count <= 1 {
            for member in remaining {
                member.clusterID = nil
                member.isDuplicate = false
            }
        } else {
            for (index, member) in remaining.enumerated() { member.isDuplicate = index > 0 }
        }
        try? context.save()
    }

    // MARK: - Archive des favoris

    /// Progression de l'archivage des favoris (nil = rien en cours)
    private(set) var archiveStatus: String?

    /// Range dans l'archive permanente l'article complet et les images des favoris
    /// (par défaut : tous les favoris pas encore archivés)
    func archiveFavorites(_ articles: [Article]? = nil) async {
        guard Prefs.archive, !isOffline, archiveStatus == nil else { return }
        let targets = (articles ?? ((try? context.fetch(FetchDescriptor<Article>(
            predicate: #Predicate { $0.isStarred && !$0.isArchived }))) ?? []))
            .filter { $0.isStarred && !$0.isArchived }
        guard !targets.isEmpty else { return }
        archiveStatus = String(localized: "Archivage…")
        defer { archiveStatus = nil }

        for (index, article) in targets.enumerated() {
            guard !article.isDeleted, article.isStarred, Prefs.archive, !isOffline else { continue }
            archiveStatus = String(localized: "Favoris : \(index + 1) sur \(targets.count)")

            // 1. Article complet, si le flux ne donne qu'un extrait
            if article.fullHTML == nil, HTMLUtils.looksLikeExcerpt(article.contentHTML),
               let link = article.link, let url = URL(string: link) {
                do {
                    if let content = try await FullTextLoader.load(url) {
                        article.fullHTML = content
                        article.updateSearchText()
                    }
                } catch {
                    if let e = error as? URLError, Self.isOfflineError(e) { return }   // on réessaiera plus tard
                }
            }

            // 2. Images (miniature + images du texte, 50 max)
            let base = article.link.flatMap { URL(string: $0) }
            var urls = Array(HTMLUtils.imageURLs(in: article.fullHTML ?? article.contentHTML ?? "", base: base).prefix(50))
            if let thumb = article.imageURL.flatMap({ URL(string: $0) }) { urls.insert(thumb, at: 0) }
            await withTaskGroup(of: Void.self) { group in
                for url in urls {
                    group.addTask { _ = await ImageCache.archive(url) }
                }
            }
            article.isArchived = true
            try? context.save()
        }
    }

    /// Favoris retirés : leurs images quittent l'archive (retour dans le cache normal)
    private func unarchive(_ articles: [Article]) {
        for article in articles { article.isArchived = false }
        try? context.save()
        let favorites = (try? context.fetch(FetchDescriptor<Article>(predicate: #Predicate { $0.isStarred }))) ?? []
        var keep = Set<URL>()
        for article in favorites {
            let base = article.link.flatMap { URL(string: $0) }
            keep.formUnion(HTMLUtils.imageURLs(in: article.fullHTML ?? article.contentHTML ?? "", base: base).prefix(50))
            if let thumb = article.imageURL.flatMap({ URL(string: $0) }) { keep.insert(thumb) }
        }
        Task.detached(priority: .utility) { ImageCache.pruneArchive(keeping: keep) }
    }

    func pendingArchiveCount() -> Int {
        (try? context.fetchCount(FetchDescriptor<Article>(predicate: #Predicate { $0.isStarred && !$0.isArchived }))) ?? 0
    }

    // MARK: - Hors ligne

    /// Prépare la lecture hors ligne des articles non lus et des favoris :
    /// récupère l'article complet quand le flux ne donne qu'un extrait,
    /// puis télécharge toutes leurs images dans le cache disque.
    func prepareOffline() async {
        guard offlineStatus == nil else { return }
        offlineStatus = String(localized: "Préparation…")
        defer { offlineStatus = nil }

        var descriptor = FetchDescriptor<Article>(
            predicate: #Predicate { (!$0.isRead && !$0.isHidden && !$0.isDuplicate) || $0.isStarred },
            sortBy: [SortDescriptor(\.published, order: .reverse)]
        )
        descriptor.fetchLimit = 400
        let articles = (try? context.fetch(descriptor)) ?? []

        // 1. Articles complets (téléchargement 4 par 4, extraction une par une)
        let needFull = articles.filter {
            $0.fullHTML == nil && $0.link != nil && HTMLUtils.looksLikeExcerpt($0.contentHTML)
        }
        var done = 0
        for start in stride(from: 0, to: needFull.count, by: 4) {
            guard Prefs.offline, !isOffline else { return }
            let chunk = Array(needFull[start..<min(start + 4, needFull.count)])
            let pages = await withTaskGroup(of: (String, String, URL)?.self) { group in
                for article in chunk {
                    guard let link = article.link, let url = URL(string: link) else { continue }
                    let key = article.key
                    group.addTask {
                        guard let r = try? await Net.get(url) else { return nil }
                        return (key, Net.decodeHTML(r.0), r.1)
                    }
                }
                var out: [(String, String, URL)] = []
                for await page in group { if let page { out.append(page) } }
                return out
            }
            for (key, html, finalURL) in pages {
                guard let article = chunk.first(where: { $0.key == key }), !article.isDeleted,
                      article.fullHTML == nil,
                      let content = (try? await ArticleExtractor.shared.extract(html: html, url: finalURL))?.content
                else { continue }
                article.fullHTML = content
                article.updateSearchText()
                if article.imageURL == nil, let first = HTMLUtils.imageURLs(in: content, base: finalURL).first {
                    article.imageURL = first.absoluteString
                }
            }
            done += chunk.count
            offlineStatus = String(localized: "Articles : \(done) sur \(needFull.count)")
            if context.hasChanges { try? context.save() }
        }

        // 2. Images (miniature + images du texte, 25 max par article)
        var imageURLs: [URL] = []
        var seen = Set<URL>()
        for article in articles where !article.isDeleted {
            let base = article.link.flatMap { URL(string: $0) }
            var urls = HTMLUtils.imageURLs(in: article.fullHTML ?? article.contentHTML ?? "", base: base).prefix(25).map { $0 }
            if let thumb = article.imageURL.flatMap({ URL(string: $0) }) { urls.insert(thumb, at: 0) }
            for url in urls where seen.insert(url).inserted { imageURLs.append(url) }
        }
        let missing = imageURLs.filter { !ImageCache.isCached($0) }
        for start in stride(from: 0, to: missing.count, by: 24) {
            guard Prefs.offline, !isOffline else { return }
            offlineStatus = String(localized: "Images : \(start) sur \(missing.count)")
            await ImageCache.prefetch(Array(missing[start..<min(start + 24, missing.count)]))
        }
        ImageCache.trim(toBytes: Prefs.imageCacheLimitBytes)
    }

    // MARK: - OPML

    struct ImportResult {
        var added = 0
        var skipped = 0
    }

    /// Importe les abonnements d'un fichier OPML : crée les dossiers manquants,
    /// ignore les flux déjà présents, puis actualise pour récupérer les articles.
    func importOPML(_ data: Data) async -> ImportResult {
        var result = ImportResult()
        let entries = OPML.parse(data)
        guard !entries.isEmpty else { return result }

        let feeds = (try? context.fetch(FetchDescriptor<Feed>())) ?? []
        var knownURLs = Set(feeds.map(\.url))
        var foldersByName: [String: Folder] = [:]
        for folder in (try? context.fetch(FetchDescriptor<Folder>())) ?? [] {
            foldersByName[folder.name.lowercased()] = folder
        }

        for entry in entries {
            guard let url = URL(string: entry.xmlURL), url.host() != nil else {
                result.skipped += 1
                continue
            }
            let key = url.absoluteString
            guard !knownURLs.contains(key) else {
                result.skipped += 1
                continue
            }
            knownURLs.insert(key)

            var folder: Folder?
            if let name = entry.folder {
                if let existing = foldersByName[name.lowercased()] {
                    folder = existing
                } else {
                    let created = Folder(name: name)
                    context.insert(created)
                    foldersByName[name.lowercased()] = created
                    folder = created
                }
            }

            let title = entry.title.isEmpty ? (url.host() ?? key) : entry.title
            let feed = Feed(url: key, title: title, siteURL: entry.htmlURL)
            context.insert(feed)
            feed.folder = folder
            result.added += 1
        }

        try? context.save()
        if result.added > 0 {
            // Si une actualisation est déjà en cours, on attend sa fin pour inclure les nouveaux flux
            while isRefreshing { try? await Task.sleep(for: .milliseconds(300)) }
            await refreshAll()
        }
        return result
    }

    func exportOPML() -> String {
        OPML.export(feeds: (try? context.fetch(FetchDescriptor<Feed>())) ?? [])
    }

    // MARK: - Dossiers

    func folder(uid: String) -> Folder? {
        var descriptor = FetchDescriptor<Folder>(predicate: #Predicate { $0.uid == uid })
        descriptor.fetchLimit = 1
        return try? context.fetch(descriptor).first
    }

    func feedURLs(inFolder uid: String) -> [String] {
        folder(uid: uid)?.feeds.map(\.url) ?? []
    }

    func createFolder(named name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        context.insert(Folder(name: trimmed))
        try? context.save()
    }

    func update(_ folder: Folder, name: String, symbol: String?, colorName: String?) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty { folder.name = trimmed }
        folder.symbol = (symbol == "folder") ? nil : symbol
        folder.colorName = colorName
        try? context.save()
    }

    /// Les flux du dossier reviennent à la racine.
    func delete(_ folder: Folder) {
        for feed in folder.feeds { feed.folder = nil }
        context.delete(folder)
        try? context.save()
    }

    func move(_ feed: Feed, to folder: Folder?) {
        feed.folder = folder
        try? context.save()
    }

    /// Utilisé par le glisser-déposer (on transporte l'URL du flux).
    func move(feedURLs urls: [String], to folder: Folder?) {
        for url in urls {
            let descriptor = FetchDescriptor<Feed>(predicate: #Predicate { $0.url == url })
            if let feed = try? context.fetch(descriptor).first {
                feed.folder = folder
            }
        }
        try? context.save()
    }

    /// Réglage « marquer automatiquement comme lus les articles de plus de X jours »
    /// (les favoris ne sont pas concernés)
    func applyAutoMarkRead(save: Bool = true) {
        guard let cutoff = Prefs.autoMarkReadCutoff else { return }
        let predicate = #Predicate<Article> { !$0.isRead && !$0.isStarred && $0.published < cutoff }
        for article in (try? context.fetch(FetchDescriptor(predicate: predicate))) ?? [] {
            article.isRead = true
        }
        if save, context.hasChanges { try? context.save() }
    }

    /// Nombre d'articles non lus (hors favoris) publiés il y a plus de `days` jours
    func unreadCount(olderThanDays days: Int) -> Int {
        let cutoff = Date.now.addingTimeInterval(-Double(max(days, 1)) * 86_400)
        let predicate = #Predicate<Article> { !$0.isRead && !$0.isHidden && !$0.isStarred && $0.published < cutoff }
        return (try? context.fetchCount(FetchDescriptor(predicate: predicate))) ?? 0
    }

    private func prune() {
        let cutoff = Prefs.retentionCutoff
        let predicate = #Predicate<Article> { $0.isRead && !$0.isStarred && $0.published < cutoff }
        for article in (try? context.fetch(FetchDescriptor(predicate: predicate))) ?? [] {
            context.delete(article)
        }
    }
}
