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
    @State private var feedName = ""

    var body: some View {
        let counts = Dictionary(grouping: unread, by: \.feedURL).mapValues(\.count)

        List(selection: $selection) {
            Section {
                Label("Tous les articles", systemImage: "tray.full")
                    .tag(SidebarSelection.all)
                Label("Non lus", systemImage: "circle.inset.filled")
                    .badge(unread.filter { !$0.isDuplicate }.count)
                    .tag(SidebarSelection.unread)
                Label("Favoris", systemImage: "star")
                    .tag(SidebarSelection.starred)
                Label("Historique", systemImage: "clock.arrow.circlepath")
                    .tag(SidebarSelection.history)
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
            Button("Nouveau dossier…") { showNewFolder = true }
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
                    Button("Nouveau dossier…") { showNewFolder = true }
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
