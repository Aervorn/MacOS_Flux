import Combine
import Sparkle
import SwiftUI

/// Mises à jour automatiques avec Sparkle.
///
/// Le flux des versions (`appcast.xml`, à la racine du dépôt) est indiqué par `SUFeedURL`
/// dans Info.plist ; chaque DMG est signé avec la clé EdDSA dont la partie publique est
/// `SUPublicEDKey` (voir scripts/sparkle-keys.sh et scripts/make-dmg.sh).
@MainActor
final class AppUpdater: ObservableObject {
    static let shared = AppUpdater()

    private let controller: SPUStandardUpdaterController
    var updater: SPUUpdater { controller.updater }

    /// Faux pendant une recherche en cours : grise l'entrée du menu
    @Published private(set) var canCheckForUpdates = false
    @Published var automaticallyChecks: Bool {
        didSet { updater.automaticallyChecksForUpdates = automaticallyChecks }
    }
    @Published var automaticallyDownloads: Bool {
        didSet { updater.automaticallyDownloadsUpdates = automaticallyDownloads }
    }

    private init() {
        controller = SPUStandardUpdaterController(startingUpdater: true, updaterDelegate: nil, userDriverDelegate: nil)
        automaticallyChecks = controller.updater.automaticallyChecksForUpdates
        automaticallyDownloads = controller.updater.automaticallyDownloadsUpdates
        controller.updater.publisher(for: \.canCheckForUpdates)
            .receive(on: DispatchQueue.main)
            .assign(to: &$canCheckForUpdates)
    }

    func checkForUpdates() {
        controller.checkForUpdates(nil)
    }

    var lastCheck: Date? { updater.lastUpdateCheckDate }
}

/// Entrée « Rechercher les mises à jour… » du menu de l'application
struct CheckForUpdatesMenuItem: View {
    @ObservedObject private var updater = AppUpdater.shared

    var body: some View {
        Button("Rechercher les mises à jour…") { updater.checkForUpdates() }
            .disabled(!updater.canCheckForUpdates)
    }
}

/// Section « Mises à jour » des réglages
struct UpdateSettingsSection: View {
    @ObservedObject private var updater = AppUpdater.shared

    var body: some View {
        Section("Mises à jour") {
            Toggle("Rechercher automatiquement les mises à jour", isOn: $updater.automaticallyChecks)
            Toggle("Télécharger et installer automatiquement", isOn: $updater.automaticallyDownloads)
                .disabled(!updater.automaticallyChecks)
            HStack {
                if let date = updater.lastCheck {
                    Text("Dernière recherche : \(date.formatted(date: .abbreviated, time: .shortened))")
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Button("Rechercher maintenant") { updater.checkForUpdates() }
                    .disabled(!updater.canCheckForUpdates)
            }
        }
    }
}
