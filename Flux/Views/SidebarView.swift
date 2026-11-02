import AppKit
import SwiftData
import SwiftUI

enum SidebarSelection: Hashable {
    case all
    case unread
    case starred
    /// Articles déjà ouverts, du plus récent au plus ancien
    case history
    case folder(String)   // Folder.uid
    case feed(String)     // Feed.url
}

struct SidebarView: View {
    @Binding var selection: SidebarSelection?
    @Binding var showNewFolder: Bool
    @Environment(FeedStore.self) private var store
    @Environment(UIState.self) private var ui

    @Query private var allFeeds: [Feed]
    /// Triés par nom affiché (le nom personnalisé s'il y en a un)
    private var feeds: [Feed] {
        allFeeds.sorted { $0.displayTitle.localizedStandardCompare($1.displayTitle) == .orderedAscending }
    }
    @Query(sort: \Folder.name) private var folders: [Folder]
    @Query(filter: #Predicate<Article> { !$0.isRead && !$0.isHidden }) private var unread: [Article]

    @State private var newFolderName = ""
    @State private var editingFolder: Folder?
    @State private var feedToDelete: Feed?
    @State private var renamingFeed: Feed?
    @State private var editingAddress: Feed?
    /// Flux pour lequel on crée une règle de filtrage depuis le menu contextuel
    @State private var newRuleFeed: Feed?
    @State private var feedName = ""

    var body: some View {
        let counts = Dictionary(grouping: unread, by: \.feedURL).mapValues(\.count)

        List(selection: $selection) {
            Section {
                // Chaque vue a son propre menu contextuel : sans lui, c'est celui de la liste
                // (« Nouveau dossier de flux… ») qui s'afficherait, ce qui prête à confusion ici
                Label("Tous les articles", systemImage: "tray.full")
                    .tag(SidebarSelection.all)
                    .contextMenu { smartMenu(.all) }
                Label("Non lus", systemImage: "circle.inset.filled")
                    .badge(unread.filter { !$0.isDuplicate }.count)
                    .tag(SidebarSelection.unread)
                    .contextMenu { smartMenu(.unread) }
                Label("Favoris", systemImage: "star")
                    .tag(SidebarSelection.starred)
                    .contextMenu { smartMenu(.starred) }
                Label("Historique", systemImage: "clock.arrow.circlepath")
                    .tag(SidebarSelection.history)
                    .contextMenu { smartMenu(.history) }
            }

            Section {
                if feeds.isEmpty && folders.isEmpty {
                    Text("Ajoute un flux avec le bouton +, ou importe tes abonnements depuis le menu Fichier.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }

                ForEach(folders) { folder in
                    folderGroup(folder, counts: counts)
                }

                ForEach(feeds.filter { $0.folder == nil }) { feed in
                    feedRow(feed, counts: counts)
                }
            } header: {
                Text("Flux")
                    // Déposer un flux sur l'en-tête le sort de son dossier
                    .dropDestination(for: String.self) { urls, _ in
                        store.move(feedURLs: urls, to: nil)
                        return true
                    }
            }
        }
        .listStyle(.sidebar)
        .navigationSplitViewColumnWidth(min: 190, ideal: 230)
        .toolbar { sidebarToolbar }
        .contextMenu {
            Button("Nouveau dossier de flux…") { showNewFolder = true }
        }
        .alert("Nouveau dossier", isPresented: $showNewFolder) {
            TextField("Nom du dossier", text: $newFolderName)
            Button("Créer") {
                store.createFolder(named: newFolderName)
                newFolderName = ""
            }
            Button("Annuler", role: .cancel) { newFolderName = "" }
        }
        .alert("Renommer le flux", isPresented: Binding(
            get: { renamingFeed != nil },
            set: { if !$0 { renamingFeed = nil } }
        )) {
            TextField("Nom du flux", text: $feedName)
            Button("Renommer") {
                if let feed = renamingFeed { store.rename(feed, to: feedName) }
            }
            .keyboardShortcut(.defaultAction)
            if renamingFeed?.customTitle != nil {
                Button("Nom d'origine") {
                    if let feed = renamingFeed { store.rename(feed, to: "") }
                }
            }
            Button("Annuler", role: .cancel) {}
        } message: {
            if let feed = renamingFeed {
                Text("Nom d'origine : \(feed.title)")
            }
        }
        .alert(
            Text("Supprimer « \(feedToDelete?.displayTitle ?? "") » ?"),
            isPresented: Binding(get: { feedToDelete != nil }, set: { if !$0 { feedToDelete = nil } })
        ) {
            Button("Supprimer", role: .destructive) {
                if let feed = feedToDelete {
                    if selection == .feed(feed.url) { selection = .all }
                    store.delete(feed)
                }
            }
            Button("Annuler", role: .cancel) {}
        } message: {
            if let feed = feedToDelete {
                let favorites = store.favoriteCount(of: feed)
                Text(favorites == 0
                     ? String(localized: "Ses articles seront supprimés.")
                     : String(localized: "Ses articles seront supprimés, sauf ses \(favorites) favoris, qui restent dans Favoris."))
            }
        }
        .sheet(item: $editingFolder) { folder in
            FolderEditSheet(folder: folder)
        }
        .sheet(item: $newRuleFeed) { feed in
            FilterRuleSheet(rule: nil, feeds: feeds, initialFeedURL: feed.url) { keyword, wholeWord, action, feedURLs in
                store.addFilter(keyword: keyword, wholeWord: wholeWord, action: action, feedURLs: feedURLs)
            }
        }
        .sheet(item: $editingAddress) { feed in
            FeedAddressSheet(feed: feed) { oldURL, newURL in
                if selection == .feed(oldURL) { selection = .feed(newURL) }
            }
        }
    }

    // MARK: - Vues intelligentes

    @ViewBuilder
    private func smartMenu(_ view: SidebarSelection) -> some View {
        Button("Tout marquer comme lu") { store.markAllRead(view) }
    }

    // MARK: - Barre d'outils de la barre latérale

    /// Les raccourcis (⌘N, ⇧⌘N, ⌘R) sont portés par les menus Fichier et Présentation :
    /// ils restent disponibles quand la barre latérale est masquée.
    @ToolbarContentBuilder
    private var sidebarToolbar: some ToolbarContent {
        ToolbarItemGroup {
            Button {
                ui.showAddFeed = true
            } label: {
                Label("Ajouter un flux", systemImage: "plus")
            }
            .help("Ajouter un flux (⌘N)")

            Button {
                showNewFolder = true
            } label: {
                Label("Nouveau dossier", systemImage: "folder.badge.plus")
            }
            .help("Nouveau dossier (⇧⌘N)")

            Button {
                Task { await store.refreshAll() }
            } label: {
                if store.isRefreshing {
                    ProgressView().controlSize(.small)
                } else {
                    Label("Actualiser", systemImage: "arrow.clockwise")
                }
            }
            .disabled(store.isRefreshing)
            .help("Actualiser tous les flux (⌘R)")
        }
    }

    // MARK: - Dossier

    private func folderGroup(_ folder: Folder, counts: [String: Int]) -> some View {
        let folderFeeds = feeds.filter { $0.folder?.uid == folder.uid }
        let unreadInFolder = folderFeeds.reduce(0) { $0 + (counts[$1.url] ?? 0) }

        return DisclosureGroup(isExpanded: Binding(
            get: { folder.isExpanded },
            set: { folder.isExpanded = $0 }
        )) {
            ForEach(folderFeeds) { feed in
                feedRow(feed, counts: counts)
            }
        } label: {
            Label {
                Text(folder.name)
            } icon: {
                Image(systemName: folder.symbolName)
                    .foregroundStyle(folder.tint ?? Color.accentColor)
            }
                .badge(unreadInFolder)
                .tag(SidebarSelection.folder(folder.uid))
                .dropDestination(for: String.self) { urls, _ in
                    store.move(feedURLs: urls, to: folder)
                    folder.isExpanded = true
                    return true
                }
                .contextMenu {
                    Button("Tout marquer comme lu") { store.markAllRead(.folder(folder.uid)) }
                    Button("Actualiser") {
                        Task { for feed in folderFeeds { await store.refresh(feed) } }
                    }
                    Divider()
                    Button("Personnaliser…") { editingFolder = folder }
                    Button("Nouveau dossier de flux…") { showNewFolder = true }
                    Divider()
                    Button("Supprimer le dossier", role: .destructive) {
                        if selection == .folder(folder.uid) { selection = .all }
                        store.delete(folder)
                    }
                }
        }
    }

    // MARK: - Flux

    private func feedRow(_ feed: Feed, counts: [String: Int]) -> some View {
        FeedRow(feed: feed)
            .badge(counts[feed.url] ?? 0)
            .tag(SidebarSelection.feed(feed.url))
            .draggable(feed.url)
            .contextMenu {
                Button("Tout marquer comme lu") { store.markAllRead(.feed(feed.url)) }
                Button("Actualiser") { Task { await store.refresh(feed) } }
                Button("Récupérer les articles plus anciens") {
                    Task {
                        let added = await store.fetchOlderArticles(for: feed)
                        store.groupDuplicates(added)
                    }
                }
                Button("Renommer…") {
                    feedName = feed.displayTitle
                    renamingFeed = feed
                }
                Button("Modifier l'adresse…") { editingAddress = feed }
                Button("Nouvelle règle de filtrage…") { newRuleFeed = feed }

                Menu("Déplacer vers") {
                    ForEach(folders) { folder in
                        Button(folder.name) { store.move(feed, to: folder) }
                            .disabled(feed.folder?.uid == folder.uid)
                    }
                    if !folders.isEmpty { Divider() }
                    Button("Hors dossier") { store.move(feed, to: nil) }
                        .disabled(feed.folder == nil)
                }

                Divider()
                if let site = feed.siteURL, let url = URL(string: site) {
                    Button("Ouvrir le site") { NSWorkspace.shared.open(url) }
                }
                Button("Copier l'adresse du flux") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(feed.url, forType: .string)
                }
                Divider()
                Button("Supprimer le flux…", role: .destructive) {
                    feedToDelete = feed
                }
            }
    }
}

struct FeedRow: View {
    let feed: Feed

    var body: some View {
        Label {
            HStack(spacing: 4) {
                Text(feed.displayTitle).lineLimit(1)
                if let error = feed.lastError {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(.orange)
                        .help(error)
                }
            }
        } icon: {
            AsyncImage(url: feed.faviconURL) { phase in
                if let image = phase.image {
                    image.resizable().scaledToFit()
                } else {
                    Image(systemName: "dot.radiowaves.up.forward")
                }
            }
            .frame(width: 16, height: 16)
            .clipShape(RoundedRectangle(cornerRadius: 3))
        }
    }
}

/// Changer l'adresse d'un flux sans perdre ses articles
struct FeedAddressSheet: View {
    let feed: Feed
    /// Appelé avec l'ancienne et la nouvelle adresse une fois le changement fait
    let onChanged: (String, String) -> Void

    @Environment(\.dismiss) private var dismiss
    @Environment(FeedStore.self) private var store

    @State private var address: String
    @State private var working = false
    @State private var errorMessage: String?

    init(feed: Feed, onChanged: @escaping (String, String) -> Void) {
        self.feed = feed
        self.onChanged = onChanged
        _address = State(initialValue: feed.url)
    }

    private var canSubmit: Bool {
        let trimmed = address.trimmingCharacters(in: .whitespacesAndNewlines)
        return !trimmed.isEmpty && trimmed != feed.url && !working
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Modifier l'adresse du flux")
                .font(.title2.bold())
            Text("Les articles déjà récupérés, leur état (lu, favori), le nom et le dossier du flux sont conservés.")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            TextField("https://exemple.fr/feed.xml", text: $address)
                .textFieldStyle(.roundedBorder)
                .onSubmit { if canSubmit { save() } }
                .disabled(working)

            if let errorMessage {
                Text(errorMessage)
                    .font(.callout)
                    .foregroundStyle(.red)
            }

            HStack {
                Spacer()
                Button("Annuler", role: .cancel) { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Button {
                    save()
                } label: {
                    if working {
                        ProgressView().controlSize(.small).frame(width: 70)
                    } else {
                        Text("Enregistrer").frame(width: 70)
                    }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(!canSubmit)
            }
        }
        .padding(20)
        .frame(width: 460)
    }

    private func save() {
        let oldURL = feed.url
        Task {
            working = true
            errorMessage = nil
            do {
                try await store.changeAddress(of: feed, to: address)
                onChanged(oldURL, feed.url)
                dismiss()
            } catch {
                errorMessage = error.localizedDescription
            }
            working = false
        }
    }
}
