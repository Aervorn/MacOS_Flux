import AppKit
import SwiftData
import SwiftUI

struct ArticleListView: View {
    @Query private var articles: [Article]
    /// Articles similaires masqués : pour afficher « +N » sur l'article qui représente le groupe
    @Query(filter: #Predicate<Article> { $0.isDuplicate }) private var duplicates: [Article]
    /// Clés des articles sélectionnés (sélection multiple)
    @Binding var selection: Set<String>
    private let filter: SidebarSelection

    private let isSearching: Bool
    /// Articles lus masqués (Tous les articles, dossiers et flux)
    private let hidesRead: Bool

    /// Point de départ d'une sélection étendue (⇧-clic, ⇧↑/↓)
    @State private var anchor: String?
    /// Articles à qui ajouter le tag créé par « Nouveau tag… »
    @State private var tagTargets: [Article]?
    @Query(sort: \FavoriteTag.name) private var tags: [FavoriteTag]
    /// Dernier article atteint au clavier ou à la souris
    @State private var cursor: String?

    /// `folderFeedURLs` : adresses des flux du dossier quand `filter` est `.folder`
    /// `searchText` : recherche dans le titre et le texte complet (vide = pas de recherche)
    /// Appelé quand on ouvre un article (clic simple, ou Entrée)
    private let onOpen: ((Article) -> Void)?

    /// `hideRead` : masque les articles lus (sauf ceux sélectionnés) dans Tous les articles, les dossiers et les flux
    init(filter: SidebarSelection, selection: Binding<Set<String>>, folderFeedURLs: [String] = [], searchText: String = "",
         hideRead: Bool = false, onOpen: ((Article) -> Void)? = nil) {
        self.onOpen = onOpen
        self.hidesRead = hideRead
        self.filter = filter
        self._selection = selection
        self.isSearching = !searchText.isEmpty

        // Les articles sélectionnés restent visibles dans "Non lus" même une fois marqués comme lus
        let sticky = Array(selection.wrappedValue)
        let urls = folderFeedURLs
        let q = searchText
        let predicate: Predicate<Article>?
        // « Masquer les lus » : Tous les articles se comporte comme Non lus.
        // Prédicats écrits en entier plutôt qu'avec un booléen capturé, que SwiftData traduit mal.
        let effective: SidebarSelection = (hideRead && filter == .all) ? .unread : filter
        let unreadOnly = hideRead

        if q.isEmpty {
            switch effective {
            case .feed(let url) where unreadOnly:
                predicate = #Predicate { !$0.isHidden && $0.feedURL == url && (!$0.isRead || sticky.contains($0.key)) }
            case .folder where unreadOnly:
                predicate = #Predicate {
                    !$0.isHidden && !$0.isDuplicate && urls.contains($0.feedURL) && (!$0.isRead || sticky.contains($0.key))
                }
            case .all:
                predicate = #Predicate { !$0.isHidden && !$0.isDuplicate }
            case .unread:
                predicate = #Predicate { !$0.isHidden && !$0.isDuplicate && (!$0.isRead || sticky.contains($0.key)) }
            case .starred:
                predicate = #Predicate { $0.isStarred }
            case .history:
                predicate = #Predicate { $0.openedAt != nil }
            case .tag(let uid):
                let needle = Article.tagNeedle(uid)
                predicate = #Predicate { $0.isStarred && $0.tagsRaw.localizedStandardContains(needle) }
            case .feed(let url):
                predicate = #Predicate { !$0.isHidden && $0.feedURL == url }
            case .folder:
                predicate = #Predicate { !$0.isHidden && !$0.isDuplicate && urls.contains($0.feedURL) }
            }
        } else {
            // Même filtres, restreints aux articles dont le titre ou le texte contient la recherche
            switch effective {
            case .feed(let url) where unreadOnly:
                predicate = #Predicate {
                    !$0.isHidden && $0.feedURL == url && (!$0.isRead || sticky.contains($0.key))
                        && ($0.title.localizedStandardContains(q) || $0.searchText.localizedStandardContains(q))
                }
            case .folder where unreadOnly:
                predicate = #Predicate {
                    !$0.isHidden && !$0.isDuplicate && urls.contains($0.feedURL) && (!$0.isRead || sticky.contains($0.key))
                        && ($0.title.localizedStandardContains(q) || $0.searchText.localizedStandardContains(q))
                }
            case .all:
                predicate = #Predicate {
                    !$0.isHidden && !$0.isDuplicate
                        && ($0.title.localizedStandardContains(q) || $0.searchText.localizedStandardContains(q))
                }
            case .unread:
                predicate = #Predicate {
                    !$0.isHidden && !$0.isDuplicate && (!$0.isRead || sticky.contains($0.key))
                        && ($0.title.localizedStandardContains(q) || $0.searchText.localizedStandardContains(q))
                }
            case .starred:
                predicate = #Predicate {
                    $0.isStarred
                        && ($0.title.localizedStandardContains(q) || $0.searchText.localizedStandardContains(q))
                }
            case .history:
                predicate = #Predicate {
                    $0.openedAt != nil
                        && ($0.title.localizedStandardContains(q) || $0.searchText.localizedStandardContains(q))
                }
            case .tag(let uid):
                let needle = Article.tagNeedle(uid)
                predicate = #Predicate {
                    $0.isStarred && $0.tagsRaw.localizedStandardContains(needle)
                        && ($0.title.localizedStandardContains(q) || $0.searchText.localizedStandardContains(q))
                }
            case .feed(let url):
                predicate = #Predicate {
                    !$0.isHidden && $0.feedURL == url
                        && ($0.title.localizedStandardContains(q) || $0.searchText.localizedStandardContains(q))
                }
            case .folder:
                predicate = #Predicate {
                    !$0.isHidden && !$0.isDuplicate && urls.contains($0.feedURL)
                        && ($0.title.localizedStandardContains(q) || $0.searchText.localizedStandardContains(q))
                }
            }
        }
        var descriptor = FetchDescriptor<Article>(
            predicate: predicate,
            // Historique : dernier article ouvert en premier
            sortBy: filter == .history
                ? [SortDescriptor<Article>(\.openedAt, order: .reverse)]
                : [SortDescriptor<Article>(\.published, order: .reverse)]
        )
        descriptor.fetchLimit = 1000
        // Pas d'animation : une mise à jour animée pendant le défilement fait sauter la liste
        _articles = Query(descriptor)
    }

    /// Hauteur fixe de chaque ligne. Toutes les lignes faisant exactement la même
    /// hauteur, la hauteur totale de la liste est connue d'avance : l'ascenseur
    /// ne peut plus « trembler » pendant le défilement.
    static let rowHeight: CGFloat = 100
    /// Hauteur sans le résumé (miniature de 69 pt + marges)
    static let compactRowHeight: CGFloat = 84
    @AppStorage(Prefs.showSummary) private var showSummary = true

    @FocusState private var isFocused: Bool
    @AppStorage(Prefs.articleLayout) private var layout = "list"
    @State private var availableWidth: CGFloat = 400

    private var isGrid: Bool { layout == "grid" || layout == "gridFull" }

    // Vignettes : largeur minimale d'une carte et espacements
    private static let cardMinWidth: CGFloat = 150
    private static let gridSpacing: CGFloat = 12
    private static let gridPadding: CGFloat = 12

    /// Nombre de colonnes en mode vignettes, selon la largeur de la liste
    private var columnCount: Int {
        let usable = availableWidth - 2 * Self.gridPadding + Self.gridSpacing
        return max(1, Int(usable / (Self.cardMinWidth + Self.gridSpacing)))
    }
    @Environment(KeyboardRouter.self) private var keyboard
    @Environment(FeedStore.self) private var store

    /// Article seul sélectionné (nil si aucun ou plusieurs)
    private var single: Article? {
        guard selection.count == 1, let key = selection.first else { return nil }
        return articles.first { $0.key == key }
    }

    private var selectedArticles: [Article] {
        articles.filter { selection.contains($0.key) }
    }

    // Une ScrollView + LazyVStack plutôt qu'une List : sur macOS, List estime la
    // hauteur des lignes jamais affichées avec une valeur par défaut, puis la
    // corrige au défilement, ce qui faisait sauter la liste en bas.
    var body: some View {
        let counts = otherSourceCounts
        return ScrollViewReader { proxy in
            ScrollView {
                if isGrid {
                    // Toutes les cartes ont la même taille : la hauteur totale reste stable
                    LazyVGrid(
                        columns: Array(repeating: GridItem(.flexible(), spacing: Self.gridSpacing), count: columnCount),
                        spacing: Self.gridSpacing
                    ) {
                        ForEach(articles) { article in
                            card(for: article, counts: counts)
                        }
                    }
                    .padding(Self.gridPadding)
                } else {
                    LazyVStack(spacing: 0) {
                        ForEach(articles) { article in
                            row(for: article, counts: counts)
                        }
                    }
                    .padding(.vertical, 6)
                }
            }
            .background {
                GeometryReader { geo in
                    Color.clear
                        .onAppear { availableWidth = geo.size.width }
                        .onChange(of: geo.size.width) { _, width in availableWidth = width }
                }
            }
            .focusable()
            .focused($isFocused)
            .focusEffectDisabled()
            .onKeyPress(.downArrow, phases: [.down, .repeat]) { press in
                // En vignettes, ↓ descend d'une rangée
                moveSelection(by: isGrid ? columnCount : 1, extend: press.modifiers.contains(.shift), proxy: proxy)
                return .handled
            }
            .onKeyPress(.upArrow, phases: [.down, .repeat]) { press in
                moveSelection(by: isGrid ? -columnCount : -1, extend: press.modifiers.contains(.shift), proxy: proxy)
                return .handled
            }
            .onKeyPress(.rightArrow, phases: [.down, .repeat]) { press in
                guard isGrid else { return .ignored }
                moveSelection(by: 1, extend: press.modifiers.contains(.shift), proxy: proxy)
                return .handled
            }
            .onKeyPress(.leftArrow, phases: [.down, .repeat]) { press in
                guard isGrid else { return .ignored }
                moveSelection(by: -1, extend: press.modifiers.contains(.shift), proxy: proxy)
                return .handled
            }
            .onChange(of: keyboard.event) { _, event in
                if let event { perform(event.command, proxy: proxy) }
            }
        }
        .overlay {
            if articles.isEmpty {
                ContentUnavailableView(emptyTitle, systemImage: isSearching ? "magnifyingglass" : "tray")
            }
        }
        .newTagAlert(for: $tagTargets)
    }

    private var otherSourceCounts: [String: Int] {
        Dictionary(grouping: duplicates.compactMap(\.clusterID), by: { $0 }).mapValues(\.count)
    }

    /// Nombre d'autres sources pour l'article qui représente un groupe
    private func otherSources(_ article: Article, _ counts: [String: Int]) -> Int {
        guard !article.isDuplicate, let cluster = article.clusterID else { return 0 }
        return counts[cluster] ?? 0
    }

    private func row(for article: Article, counts: [String: Int]) -> some View {
        let isSelected = selection.contains(article.key)
        return ArticleRow(article: article, showSummary: showSummary, otherSources: otherSources(article, counts))
            .padding(.horizontal, 10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .frame(height: showSummary ? Self.rowHeight : Self.compactRowHeight)
            .background {
                if isSelected {
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(Color.accentColor.opacity(isFocused ? 0.22 : 0.12))
                }
            }
            .overlay(alignment: .bottom) {
                if !isSelected {
                    Divider().padding(.leading, 114).padding(.trailing, 10)
                }
            }
            .padding(.horizontal, 8)
            .contentShape(Rectangle())
            .onTapGesture { click(article) }
            .contextMenu { menu(for: article) }
            .id(article.key)
    }

    private func card(for article: Article, counts: [String: Int]) -> some View {
        let isSelected = selection.contains(article.key)
        return ArticleCard(article: article, otherSources: otherSources(article, counts))
            .padding(6)
            .background {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(isSelected ? Color.accentColor.opacity(isFocused ? 0.22 : 0.12) : .clear)
            }
            .contentShape(Rectangle())
            .onTapGesture { click(article) }
            .contextMenu { menu(for: article) }
            .id(article.key)
    }

    // MARK: - Sélection

    /// Clic : sélection simple ; ⌘-clic : ajoute / retire ; ⇧-clic : plage depuis l'ancre
    private func click(_ article: Article) {
        let key = article.key
        let modifiers = NSEvent.modifierFlags
        if modifiers.contains(.shift), let anchor, let range = indexRange(anchor, key) {
            selection = Set(articles[range].map(\.key))
        } else if modifiers.contains(.command) {
            if selection.contains(key) { selection.remove(key) } else { selection.insert(key) }
            anchor = key
        } else {
            selection = [key]
            anchor = key
            onOpen?(article)
        }
        cursor = key
        isFocused = true
    }

    private func indexRange(_ a: String, _ b: String) -> ClosedRange<Int>? {
        guard let i = articles.firstIndex(where: { $0.key == a }),
              let j = articles.firstIndex(where: { $0.key == b }) else { return nil }
        return min(i, j)...max(i, j)
    }

    /// ↑ / ↓ (et j / k) : article précédent / suivant ; avec ⇧, étend la sélection
    private func moveSelection(by delta: Int, extend: Bool = false, proxy: ScrollViewProxy) {
        guard !articles.isEmpty else { return }
        let from = cursor.flatMap { c in articles.firstIndex { $0.key == c } }
            ?? articles.firstIndex { selection.contains($0.key) }
        let next = from.map { min(max($0 + delta, 0), articles.count - 1) } ?? 0
        let key = articles[next].key

        if extend, let anchor, let range = indexRange(anchor, key) {
            selection = Set(articles[range].map(\.key))
        } else {
            selection = [key]
            anchor = key
        }
        cursor = key
        proxy.scrollTo(key)   // sans ancre : défile juste assez pour rendre la ligne visible
    }

    private func selectAll() {
        selection = Set(articles.map(\.key))
        anchor = articles.first?.key
        cursor = articles.last?.key
    }

    // MARK: - Raccourcis

    /// Raccourcis façon Reeder (voir KeyboardRouter)
    private func perform(_ command: KeyCommand, proxy: ScrollViewProxy) {
        switch command {
        case .next:
            moveSelection(by: 1, proxy: proxy)
        case .previous:
            moveSelection(by: -1, proxy: proxy)
        case .pageDown:
            guard single != nil else {
                if selection.isEmpty { moveSelection(by: 1, proxy: proxy) }
                return
            }
            Task {
                // Une fois en bas de l'article, espace passe au suivant
                if await keyboard.scrollReader(down: true) {
                    moveSelection(by: 1, proxy: proxy)
                }
            }
        case .pageUp:
            Task { _ = await keyboard.scrollReader(down: false) }
        case .toggleRead:
            // Si tout est lu, on repasse en non lu ; sinon tout devient lu
            let targets = selectedArticles
            store.setRead(!targets.allSatisfy(\.isRead), targets)
        case .toggleStar:
            let targets = selectedArticles
            store.setStarred(!targets.allSatisfy(\.isStarred), targets)
        case .openInBrowser:
            if let link = single?.link, let url = URL(string: link) {
                NSWorkspace.shared.open(url)
            }
        case .selectAll:
            selectAll()
        case .open:
            if let single { onOpen?(single) }
        case .back:
            isFocused = true   // retour aux vignettes : les flèches reviennent à la grille
        }
    }

    private var emptyTitle: String {
        if isSearching { return String(localized: "Aucun résultat") }
        if hidesRead { return String(localized: "Tout est lu") }
        switch filter {
        case .unread: return String(localized: "Tout est lu")
        case .starred: return String(localized: "Aucun favori")
        case .tag: return String(localized: "Aucun favori avec ce tag")
        case .history: return String(localized: "Aucun article ouvert pour l'instant")
        default: return String(localized: "Aucun article")
        }
    }

    // MARK: - Menu contextuel

    /// S'applique à toute la sélection si on fait un clic droit sur un article sélectionné
    @ViewBuilder
    private func menu(for article: Article) -> some View {
        let targets = selection.contains(article.key) ? selectedArticles : [article]
        let allRead = targets.allSatisfy(\.isRead)
        let allStarred = targets.allSatisfy(\.isStarred)

        Button(allRead ? String(localized: "Marquer comme non lu") : String(localized: "Marquer comme lu")) {
            store.setRead(!allRead, targets)
        }
        Button(allStarred ? String(localized: "Retirer des favoris") : String(localized: "Ajouter aux favoris")) {
            store.setStarred(!allStarred, targets)
        }
        Menu("Tags") {
            TagMenuContent(tags: tags, articles: targets, store: store) { tagTargets = targets }
        }
        if targets.count == 1, article.clusterID != nil {
            Divider()
            Button("Séparer des articles similaires") { store.detachFromCluster(article) }
        }
        if targets.count == 1, let link = article.link, let url = URL(string: link) {
            Divider()
            Button("Ouvrir dans le navigateur") { NSWorkspace.shared.open(url) }
            Button("Copier le lien") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(link, forType: .string)
            }
        }
    }
}

struct ArticleRow: View {
    let article: Article
    var showSummary = true
    /// Autres sources ayant publié la même actualité
    var otherSources = 0

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Thumbnail(url: article.imageURL.flatMap { URL(string: $0) })

            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    if !article.isRead {
                        Circle().fill(.tint).frame(width: 7, height: 7)
                    }
                    Text(article.sourceName)
                        .lineLimit(1)
                    if otherSources > 0 {
                        OtherSourcesBadge(count: otherSources)
                    }
                    Spacer(minLength: 4)
                    if article.isStarred {
                        Image(systemName: "star.fill").foregroundStyle(.yellow)
                    }
                    Text(article.published.formatted(.relative(presentation: .named).locale(.app)))
                        .lineLimit(1)
                }
                .font(.caption)
                .foregroundStyle(.secondary)

                // Hauteur réservée même si le texte est plus court
                Text(article.title)
                    .font(.system(size: 14, weight: article.isRead ? .regular : .semibold))
                    .foregroundStyle(article.isRead ? .secondary : .primary)
                    .lineLimit(2, reservesSpace: true)

                if showSummary {
                    Text(article.summary)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .lineLimit(2, reservesSpace: true)
                }
            }
        }
    }
}

struct Thumbnail: View {
    let url: URL?
    var width: CGFloat = 92
    var height: CGFloat = 69

    var body: some View {
        // Le cadre est fixé AVANT l'image : l'overlay ne peut pas agrandir la vue,
        // et le clipShape recadre l'image en scaledToFill à la bonne taille.
        RoundedRectangle(cornerRadius: 8, style: .continuous)
            .fill(.quaternary)
            .frame(width: width, height: height)
            .overlay {
                if let url {
                    CachedImage(url: url) { placeholder }
                } else {
                    placeholder
                }
            }
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    private var placeholder: some View {
        Image(systemName: "newspaper")
            .font(.title3)
            .foregroundStyle(.tertiary)
    }
}

/// Carte du mode vignettes : l'image, et le titre dessous
struct ArticleCard: View {
    let article: Article
    var otherSources = 0

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            // Cadre au format 16:10 fixé avant l'image, qui est recadrée dedans
            RoundedRectangle(cornerRadius: 9, style: .continuous)
                .fill(.quaternary)
                .aspectRatio(16 / 10, contentMode: .fit)
                .overlay {
                    if let url = article.imageURL.flatMap({ URL(string: $0) }) {
                        CachedImage(url: url) { placeholder }
                    } else {
                        placeholder
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
                .overlay(alignment: .topLeading) {
                    if otherSources > 0 {
                        OtherSourcesBadge(count: otherSources)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 3)
                            .background(.ultraThinMaterial, in: Capsule())
                            .padding(6)
                    }
                }
                .overlay(alignment: .topTrailing) {
                    if article.isStarred {
                        Image(systemName: "star.fill")
                            .font(.caption)
                            .foregroundStyle(.yellow)
                            .padding(5)
                            .background(.ultraThinMaterial, in: Circle())
                            .padding(6)
                    }
                }

            HStack(alignment: .firstTextBaseline, spacing: 5) {
                if !article.isRead {
                    Circle().fill(.tint).frame(width: 7, height: 7)
                }
                Text(article.title)
                    .font(.system(size: 13, weight: article.isRead ? .regular : .semibold))
                    .foregroundStyle(article.isRead ? .secondary : .primary)
                    .lineLimit(3, reservesSpace: true)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .help(article.sourceName)
    }

    private var placeholder: some View {
        Image(systemName: "newspaper")
            .font(.title2)
            .foregroundStyle(.tertiary)
    }
}

/// « +2 » : la même actualité a aussi été publiée par d'autres sources
struct OtherSourcesBadge: View {
    let count: Int

    var body: some View {
        Label {
            Text(verbatim: "+\(count)")
        } icon: {
            Image(systemName: "square.stack")
        }
            .labelStyle(.titleAndIcon)
            .font(.caption2.weight(.semibold))
            .foregroundStyle(.secondary)
            .help(count == 1
                  ? String(localized: "Aussi publié par une autre source")
                  : String(localized: "Aussi publié par \(count) autres sources"))
    }
}

