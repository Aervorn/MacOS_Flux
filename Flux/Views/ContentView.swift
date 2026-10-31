import AppKit
import UniformTypeIdentifiers
import SwiftData
import SwiftUI

struct ContentView: View {
    @Environment(FeedStore.self) private var store
    @Environment(\.modelContext) private var context
    @Environment(UIState.self) private var ui
    @Environment(KeyboardRouter.self) private var keyboard
    @Environment(SyncEngine.self) private var sync

    @State private var filter: SidebarSelection? = .all
    /// Articles sélectionnés dans la liste (sélection multiple possible)
    @State private var selectedKeys: Set<String> = []
    @State private var searchText = ""
    @State private var importMessage: String?

    /// Largeur de la liste d'articles, mémorisée d'un lancement à l'autre
    @AppStorage("articleListWidth") private var listWidth: Double = 400
    @AppStorage(Prefs.articleLayout) private var articleLayout = "list"
    static let listWidthRange: ClosedRange<Double> = 300...700
    @AppStorage(Prefs.refreshMinutes) private var refreshMinutes = 30
    @AppStorage(Prefs.hideRead) private var hideRead = false
    /// « Masquer les lus » n'a de sens que dans Tous les articles, un dossier ou un flux
    private var canHideRead: Bool {
        switch filter ?? .all {
        case .all, .folder, .feed: true
        default: false
        }
    }
    /// Vignettes en plein écran : la grille occupe liste + lecture, l'article s'ouvre par-dessus
    private var immersiveGrid: Bool { articleLayout == "gridFull" }
    /// Article ouvert par-dessus la grille (mode vignettes plein écran)
    @State private var articleOpen = false

    /// Article affiché en ce moment et depuis quand (temps de lecture pour les statistiques)
    @State private var viewing: (key: String, since: Date)?

    private var readTrigger: ReadTrigger {
        ReadTrigger(key: selectedKeys.count == 1 ? selectedKeys.first : nil, visible: !immersiveGrid || articleOpen)
    }

    private var showsArticleOverGrid: Bool { immersiveGrid && articleOpen && selectedArticle != nil }

    /// Observé pour que la liste se mette à jour quand on déplace un flux de dossier
    @Query private var feeds: [Feed]

    private var folderFeedURLs: [String] {
        guard case .folder(let uid) = filter ?? .all else { return [] }
        return feeds.filter { $0.folder?.uid == uid }.map(\.url)
    }

    /// Dossier pré-sélectionné dans la fenêtre d'ajout
    private var currentFolderUID: String? {
        switch filter ?? .all {
        case .folder(let uid): uid
        case .feed(let url): feeds.first { $0.url == url }?.folder?.uid
        default: nil
        }
    }

    private var selectedArticle: Article? {
        guard selectedKeys.count == 1, let key = selectedKeys.first else { return nil }
        var descriptor = FetchDescriptor<Article>(predicate: #Predicate { $0.key == key })
        descriptor.fetchLimit = 1
        return try? context.fetch(descriptor).first
    }

    private var articleList: some View {
        ArticleListView(
            filter: filter ?? .all,
            selection: $selectedKeys,
            folderFeedURLs: folderFeedURLs,
            searchText: searchText.trimmingCharacters(in: .whitespaces),
            hideRead: hideRead && canHideRead,
            onOpen: { _ in if immersiveGrid { articleOpen = true } }
        )
    }

    var body: some View {
        lifecycle(readingTracking(dialogs(layout)))
    }

    private var layout: some View {
        // Deux colonnes seulement pour NavigationSplitView (barre latérale | reste) :
        // en trois colonnes, macOS ignore la largeur voulue de la liste au lancement
        // et ne la mémorise pas. La liste et l'article sont donc côte à côte dans la
        // colonne de détail, avec une largeur de liste enregistrée dans les réglages.
        NavigationSplitView {
            SidebarView(selection: $filter, showNewFolder: Binding(get: { ui.showNewFolder }, set: { ui.showNewFolder = $0 }))
        } detail: {
            Group {
                if immersiveGrid {
                    // La grille reste en place sous l'article : on retrouve sa position au retour
                    ZStack {
                        articleList
                            .opacity(showsArticleOverGrid ? 0 : 1)
                            .allowsHitTesting(!showsArticleOverGrid)
                        if showsArticleOverGrid, let article = selectedArticle {
                            VStack(spacing: 0) {
                                HStack {
                                    Button {
                                        articleOpen = false
                                    } label: {
                                        Label("Articles", systemImage: "chevron.left")
                                    }
                                    .buttonStyle(.borderless)
                                    .help("Revenir aux vignettes (Échap)")
                                    Spacer()
                                }
                                .padding(.horizontal, 14)
                                .padding(.vertical, 8)
                                Divider()
                                ArticleDetailView(article: article, onSelectArticle: { selectedKeys = [$0] })
                                    .id(article.key)
                            }
                            .background(.background)
                            .transition(.opacity)
                        }
                    }
                    .animation(.easeInOut(duration: 0.15), value: showsArticleOverGrid)
                } else {
                    HStack(spacing: 0) {
                        articleList
                            .frame(width: listWidth)

                        ColumnResizer(width: $listWidth, range: Self.listWidthRange)

                        Group {
                            if let article = selectedArticle {
                                ArticleDetailView(article: article, onSelectArticle: { selectedKeys = [$0] })
                                    .id(article.key)
                            } else if selectedKeys.count > 1 {
                                MultiSelectionView(keys: selectedKeys)
                            } else {
                                ContentUnavailableView("Aucun article sélectionné", systemImage: "newspaper")
                            }
                        }
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                }
            }
            .searchable(text: $searchText, placement: .toolbar, prompt: Text("Rechercher"))
        }
        .toolbar { toolbarContent }
    }

    @ToolbarContentBuilder
    private var toolbarContent: some ToolbarContent {
            // Ajouter un flux, Nouveau dossier et Actualiser sont dans la barre latérale (SidebarView)
            ToolbarItemGroup(placement: .navigation) {
                Toggle(isOn: $hideRead) {
                    Label("Masquer les articles lus",
                          systemImage: hideRead && canHideRead
                              ? "line.3.horizontal.decrease.circle.fill"
                              : "line.3.horizontal.decrease.circle")
                }
                .toggleStyle(.button)
                .disabled(!canHideRead)
                .help(hideRead ? Text("Afficher les articles lus") : Text("Masquer les articles lus"))

                Button {
                    store.markAllRead(filter ?? .all)
                } label: {
                    Label("Tout marquer comme lu", systemImage: "checkmark.circle")
                }
                .keyboardShortcut("k", modifiers: [.command, .shift])
                .help("Tout marquer comme lu (⇧⌘K)")

                Button {
                    store.setRead(true, store.articles(withKeys: selectedKeys))
                } label: {
                    Label("Marquer la sélection comme lue", systemImage: "envelope.open")
                }
                .disabled(selectedKeys.isEmpty)
                .help("Marquer la sélection comme lue")

                Button {
                    store.setRead(false, store.articles(withKeys: selectedKeys))
                } label: {
                    Label("Marquer la sélection comme non lue", systemImage: "envelope.badge")
                }
                .disabled(selectedKeys.isEmpty)
                .help("Marquer la sélection comme non lue")
            }

            ToolbarItem(placement: .navigation) {
                Picker("Présentation", selection: $articleLayout) {
                    Label("Liste", systemImage: "list.bullet").tag("list")
                    Label("Vignettes", systemImage: "square.grid.2x2").tag("grid")
                    Label("Vignettes en plein écran", systemImage: "square.grid.3x3").tag("gridFull")
                }
                .pickerStyle(.segmented)
                .labelStyle(.iconOnly)
                .help("Présentation : liste, vignettes, ou vignettes en plein écran (l'article s'ouvre à la place de la grille)")
            }

            if store.isOffline {
                ToolbarItem(placement: .primaryAction) {
                    Label("Hors ligne", systemImage: "wifi.slash")
                        .labelStyle(.titleAndIcon)
                        .foregroundStyle(.secondary)
                        .help("Pas de connexion : les articles préparés pour la lecture hors ligne restent lisibles.")
                }
            }

            ToolbarItem(placement: .primaryAction) {
                SettingsLink {
                    Label("Réglages", systemImage: "gearshape")
                }
                .help("Réglages (⌘,)")
            }
    }

    /// Import / export OPML, messages, feuille d'ajout
    private func dialogs<V: View>(_ content: V) -> some View {
        content
        .fileImporter(
            isPresented: Binding(get: { ui.showOPMLImport }, set: { ui.showOPMLImport = $0 }),
            allowedContentTypes: [.opml, .xml]
        ) { result in
            guard case .success(let url) = result else { return }
            Task { await importOPML(from: url) }
        }
        .fileExporter(
            isPresented: Binding(get: { ui.showOPMLExport }, set: { ui.showOPMLExport = $0 }),
            document: ui.showOPMLExport ? OPMLDocument(text: store.exportOPML()) : nil,
            contentType: .opml,
            defaultFilename: "Flux.opml"
        ) { _ in }
        .alert(
            "Import OPML",
            isPresented: Binding(get: { importMessage != nil }, set: { if !$0 { importMessage = nil } })
        ) {
            Button("OK") {}
        } message: {
            Text(importMessage ?? "")
        }
        .sheet(isPresented: Binding(get: { ui.showAddFeed }, set: { ui.showAddFeed = $0 })) {
            AddFeedSheet(initialFolderUID: currentFolderUID)
        }
    }

    /// Historique, temps de lecture et marquage comme lu de l'article affiché
    private func readingTracking<V: View>(_ content: V) -> some View {
        content
        .onChange(of: filter) {
            selectedKeys = []
            articleOpen = false
        }
        .task(id: readTrigger) {
            // Historique et statistiques : un article affiché 3 secondes compte comme ouvert
            guard readTrigger.visible, let article = selectedArticle, article.openedAt == nil else { return }
            try? await Task.sleep(for: .seconds(3))
            guard !Task.isCancelled, !article.isDeleted else { return }
            article.openedAt = .now
        }
        .onChange(of: readTrigger, initial: true) {
            finishViewing()
            if readTrigger.visible, let key = readTrigger.key { viewing = (key, .now) }
        }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didResignActiveNotification)) { _ in
            finishViewing()
        }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            if readTrigger.visible, let key = readTrigger.key { viewing = (key, .now) }
        }
        .task(id: readTrigger) {
            // Marque comme lu l'article ouvert, selon le réglage (immédiatement, après un délai, ou jamais).
            // Changer d'article annule la tâche : un article survolé quelques secondes reste non lu.
            // En vignettes plein écran, une carte simplement sélectionnée n'est pas lue.
            guard !immersiveGrid || articleOpen else { return }
            guard let article = selectedArticle, !article.isRead else { return }
            let delay = UserDefaults.standard.integer(forKey: Prefs.markReadDelay)
            guard delay >= 0 else { return }
            if delay > 0 {
                try? await Task.sleep(for: .seconds(delay))
                guard !Task.isCancelled else { return }
            }
            store.setRead(true, [article])   // avec les articles similaires des autres sources
        }
    }

    /// Clavier, fenêtre, actualisations automatiques
    private func lifecycle<V: View>(_ content: V) -> some View {
        content
        .onChange(of: keyboard.event) { _, event in
            if event?.command == .back { articleOpen = false }
        }
        .onChange(of: showsArticleOverGrid, initial: true) { _, open in
            keyboard.handlesEscape = open
            guard open else { return }
            // L'article prend le focus : flèches et espace le font défiler (et non la grille cachée)
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) {
                if let webView = keyboard.readerWebView { webView.window?.makeFirstResponder(webView) }
            }
        }
        .onChange(of: immersiveGrid) { articleOpen = false }
        .background(WindowReader { keyboard.mainWindow = $0 })
        .onAppear {
            keyboard.start()
            sync.start()
        }
        .task {
            // Actualisation au lancement
            await store.refreshAll()
        }
        .task(id: refreshMinutes) {
            // Puis à intervalle régulier ; la boucle redémarre si le réglage change
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(Double(max(refreshMinutes, 5)) * 60))
                guard !Task.isCancelled else { break }
                await store.refreshAll()
            }
        }
    }

    /// Ajoute le temps passé sur l'article qu'on quitte (plafonné à 30 minutes par visite)
    private func finishViewing() {
        guard let current = viewing else { return }
        viewing = nil
        let elapsed = min(Date.now.timeIntervalSince(current.since), 30 * 60)
        guard elapsed >= 1 else { return }
        let key = current.key
        var descriptor = FetchDescriptor<Article>(predicate: #Predicate { $0.key == key })
        descriptor.fetchLimit = 1
        if let article = try? context.fetch(descriptor).first {
            article.readingSeconds += elapsed
        }
    }

    private func importOPML(from url: URL) async {
        let accessing = url.startAccessingSecurityScopedResource()
        defer { if accessing { url.stopAccessingSecurityScopedResource() } }
        guard let data = try? Data(contentsOf: url) else {
            importMessage = String(localized: "Impossible de lire ce fichier.")
            return
        }
        let result = await store.importOPML(data)
        if result.added == 0 && result.skipped == 0 {
            importMessage = String(localized: "Aucun abonnement trouvé dans ce fichier.")
        } else {
            importMessage = String(localized: "\(result.added) flux importés, \(result.skipped) ignorés (déjà présents ou invalides).")
        }
    }
}

/// Séparateur vertical qu'on peut faire glisser pour redimensionner la colonne de gauche.
struct ColumnResizer: View {
    @Binding var width: Double
    let range: ClosedRange<Double>

    @State private var startWidth: Double?

    var body: some View {
        Rectangle()
            .fill(Color(nsColor: .separatorColor))
            .frame(width: 1)
            .frame(maxHeight: .infinity)
            .overlay {
                // Zone de prise plus large que le trait, sans prendre de place dans la mise en page
                Color.clear
                    .frame(width: 9)
                    .contentShape(Rectangle())
                    .onHover { inside in
                        if inside { NSCursor.resizeLeftRight.push() } else { NSCursor.pop() }
                    }
                    .gesture(
                        DragGesture(minimumDistance: 1, coordinateSpace: .global)
                            .onChanged { value in
                                let start = startWidth ?? width
                                startWidth = start
                                width = min(max(start + value.translation.width, range.lowerBound), range.upperBound)
                            }
                            .onEnded { _ in startWidth = nil }
                    )
            }
            .onAppear {
                // Au cas où une valeur enregistrée sortirait des bornes
                width = min(max(width, range.lowerBound), range.upperBound)
            }
    }
}

/// Panneau de droite quand plusieurs articles sont sélectionnés
struct MultiSelectionView: View {
    let keys: Set<String>
    @Environment(FeedStore.self) private var store

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "square.stack.3d.up")
                .font(.system(size: 44, weight: .light))
                .foregroundStyle(.secondary)
            Text("\(keys.count) articles sélectionnés")
                .font(.title2.weight(.semibold))
            VStack(spacing: 8) {
                HStack(spacing: 8) {
                    Button {
                        store.setRead(true, store.articles(withKeys: keys))
                    } label: {
                        Label("Marquer comme lus", systemImage: "envelope.open")
                    }
                    Button {
                        store.setRead(false, store.articles(withKeys: keys))
                    } label: {
                        Label("Marquer comme non lus", systemImage: "envelope.badge")
                    }
                }
                HStack(spacing: 8) {
                    Button {
                        store.setStarred(true, store.articles(withKeys: keys))
                    } label: {
                        Label("Ajouter aux favoris", systemImage: "star")
                    }
                    Button {
                        store.setStarred(false, store.articles(withKeys: keys))
                    } label: {
                        Label("Retirer des favoris", systemImage: "star.slash")
                    }
                }
            }
            .controlSize(.large)
            Text("⌘-clic pour ajouter ou retirer un article, ⇧-clic pour sélectionner une plage.")
                .font(.callout)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.top, 4)
        }
        .padding(32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// Identifie « tel article, visible ou non » pour relancer le marquage comme lu
private struct ReadTrigger: Equatable {
    let key: String?
    let visible: Bool
}


/// Présentation › Masquer les articles lus
struct HideReadMenuItem: View {
    @AppStorage(Prefs.hideRead) private var hideRead = false

    var body: some View {
        Toggle("Masquer les articles lus", isOn: $hideRead)
    }
}
