import AppKit
import SwiftData
import SwiftUI

@main
struct FluxApp: App {
    private let container: ModelContainer
    @State private var store: FeedStore
    @State private var sync: SyncEngine
    @State private var backup: BackupManager
    @State private var ui = UIState()
    @State private var keyboard = KeyboardRouter()

    init() {
        Prefs.register()
        AppAppearance.apply()
        _ = AppUpdater.shared   // démarre Sparkle (recherche planifiée des mises à jour)

        // Cache disque généreux pour les miniatures (AsyncImage passe par URLCache.shared)
        URLCache.shared = URLCache(memoryCapacity: 64 << 20, diskCapacity: 512 << 20)

        // Base dans ~/Library/Application Support/Flux/ (l'app n'est pas sandboxée,
        // on évite le "default.store" partagé avec d'autres apps)
        // Restauration demandée avant la relance : appliquée avant d'ouvrir la base
        BackupManager.applyPendingRestore()

        let directory = URL.applicationSupportDirectory.appending(path: "Flux", directoryHint: .isDirectory)
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let configuration = ModelConfiguration(url: directory.appending(path: "Flux.store"))

        let modelContainer: ModelContainer
        do {
            modelContainer = try ModelContainer(for: Feed.self, Article.self, Folder.self, FilterRule.self, FavoriteTag.self, configurations: configuration)
        } catch {
            fatalError("Impossible d'ouvrir la base : \(error)")
        }
        container = modelContainer
        let feedStore = FeedStore(context: modelContainer.mainContext)
        _store = State(initialValue: feedStore)
        _sync = State(initialValue: SyncEngine(store: feedStore))
        let backupManager = BackupManager(store: feedStore)
        backupManager.start()
        _backup = State(initialValue: backupManager)
    }

    var body: some Scene {
        Window("Flux", id: "main") {
            ContentView()
                .environment(store)
                .environment(sync)
                .environment(backup)
                .environment(ui)
                .environment(keyboard)
                .frame(minWidth: 1000, minHeight: 560)
        }
        .defaultSize(width: 1280, height: 820)
        .modelContainer(container)
        .commands {
            CommandGroup(replacing: .appInfo) {
                AboutMenuItem()
                CheckForUpdatesMenuItem()
            }
            CommandGroup(replacing: .newItem) {
                Button("Ajouter un flux…") { ui.showAddFeed = true }
                    .keyboardShortcut("n")
                Button("Nouveau dossier…") { ui.showNewFolder = true }
                    .keyboardShortcut("n", modifiers: [.command, .shift])
            }
            CommandGroup(after: .toolbar) {
                Divider()
                Button("Actualiser tous les flux") { Task { await store.refreshAll() } }
                    .keyboardShortcut("r")
                    .disabled(store.isRefreshing)
                HideReadMenuItem()
                Divider()
                ReaderTextSizeCommands()
                Divider()
                StatsMenuItem()
            }
            CommandGroup(replacing: .help) {
                Button("Découvrir Flux") { ui.showOnboarding = true }
                Button("Nouveautés de Flux") { ui.whatsNew = WhatsNew.latest }
                    .disabled(WhatsNew.latest == nil)
            }
            CommandGroup(replacing: .printItem) {
                ArticleExportCommands()
            }
            CommandGroup(after: .newItem) {
                Divider()
                Button("Importer des abonnements (OPML)…") { ui.showOPMLImport = true }
                    .keyboardShortcut("i", modifiers: [.command, .shift])
                Button("Exporter les abonnements (OPML)…") { ui.showOPMLExport = true }
                    .keyboardShortcut("e", modifiers: [.command, .shift])
            }
        }

        Window("Statistiques de lecture", id: "stats") {
            StatsView()
        }
        .modelContainer(container)
        .defaultSize(width: 760, height: 680)

        Window("À propos de Flux", id: "about") {
            AboutView()
        }
        .windowStyle(.hiddenTitleBar)
        .windowResizability(.contentSize)
        .defaultPosition(.center)

        Settings {
            SettingsView()
                .environment(store)
                .environment(sync)
                .environment(backup)
                .modelContainer(container)
        }
        .windowResizability(.contentSize)
    }
}

/// Lien entre la fenêtre principale et celle des réglages.
/// Fermer la fenêtre principale (ou quitter Flux) ferme aussi les réglages, qui sinon
/// restaient seuls à l'écran ; ils ne sont pas non plus rouverts au lancement suivant.
@MainActor
enum AppWindows {
    private static weak var main: NSWindow?
    private static weak var settings: NSWindow?
    private static var observers: [Any] = []

    static func registerMain(_ window: NSWindow?) {
        guard let window else { return }
        main = window
        startObserving()
    }

    static func registerSettings(_ window: NSWindow?) {
        guard let window else { return }
        settings = window
        window.isRestorable = false
        startObserving()
    }

    private static func startObserving() {
        guard observers.isEmpty else { return }
        let center = NotificationCenter.default
        observers.append(center.addObserver(forName: NSWindow.willCloseNotification, object: nil, queue: .main) { note in
            let closing = note.object as? NSWindow
            MainActor.assumeIsolated {
                if let closing, closing === main { closeSettings() }
            }
        })
        observers.append(center.addObserver(forName: NSApplication.willTerminateNotification, object: nil, queue: .main) { _ in
            MainActor.assumeIsolated { closeSettings() }
        })
    }

    private static func closeSettings() {
        guard let settings, settings.isVisible else { return }
        settings.isRestorable = false
        settings.close()
    }
}
