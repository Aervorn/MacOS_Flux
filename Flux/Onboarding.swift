import AppKit
import SwiftData
import SwiftUI

// MARK: - Contenu

/// Une page de la découverte de Flux
struct OnboardingPage: Identifiable {
    let id: Int
    let symbol: String
    let color: Color
    let title: String
    let text: String
    /// Astuces affichées sous le texte (symbole, texte)
    var tips: [(symbol: String, text: String)] = []
    /// Dernière page : options iCloud (synchro et sauvegarde)
    var showsICloudOptions = false

    static var all: [OnboardingPage] {
        [
            OnboardingPage(
                id: 0, symbol: "dot.radiowaves.up.forward", color: .orange,
                title: String(localized: "Bienvenue dans Flux"),
                text: String(localized: "Un lecteur RSS pour Mac, sans algorithme ni annuaire : tu choisis tes sources, Flux te les présente proprement."),
                tips: []),
            OnboardingPage(
                id: 1, symbol: "plus.circle", color: .blue,
                title: String(localized: "Ajoute tes sources"),
                text: String(localized: "Colle l'adresse d'un site ou de son flux : Flux trouve le flux tout seul."),
                tips: [
                    ("plus", String(localized: "Bouton + ou ⌘N pour ajouter un flux")),
                    ("folder", String(localized: "Range tes flux dans des dossiers, par glisser-déposer")),
                    ("square.and.arrow.down", String(localized: "Tu viens d'un autre lecteur ? Fichier › Importer des abonnements (OPML)")),
                ]),
            OnboardingPage(
                id: 2, symbol: "doc.richtext", color: .indigo,
                title: String(localized: "Lis sans quitter l'app"),
                text: String(localized: "Quand un flux ne donne qu'un extrait, Flux récupère l'article complet et l'affiche dans une mise en page lisible."),
                tips: [
                    ("square.stack", String(localized: "La même actualité publiée par plusieurs sources n'apparaît qu'une fois")),
                    ("keyboard", String(localized: "j / k : article suivant ou précédent ; espace : faire défiler")),
                    ("wifi.slash", String(localized: "Lecture hors ligne possible (Réglages › Stockage)")),
                ]),
            OnboardingPage(
                id: 3, symbol: "line.3.horizontal.decrease.circle", color: .teal,
                title: String(localized: "Fais le tri"),
                text: String(localized: "Les règles de filtrage masquent ou marquent comme lus les articles dont le titre contient un mot-clé, sur tous tes flux ou seulement certains."),
                tips: [
                    ("cursorarrow.click.2", String(localized: "Clic droit sur un flux › Nouvelle règle de filtrage")),
                    ("eye.slash", String(localized: "Présentation › Masquer les articles lus")),
                ]),
            OnboardingPage(
                id: 4, symbol: "star", color: .yellow,
                title: String(localized: "Garde l'essentiel"),
                text: String(localized: "Les favoris sont archivés en entier, images comprises : tu peux les relire même si le site les supprime."),
                tips: [
                    ("tag", String(localized: "Range tes favoris avec des tags (taguer un article l'ajoute aux favoris)")),
                    ("arrow.down.doc", String(localized: "Exporte un article en PDF ou imprime-le (⌥⌘E, ⌘P)")),
                ]),
            OnboardingPage(
                id: 5, symbol: "icloud", color: .cyan,
                title: String(localized: "Synchronise et sauvegarde"),
                text: String(localized: "Avec iCloud Drive, tes flux, favoris et articles lus suivent sur tes autres Mac, et une copie complète de Flux est sauvegardée chaque jour."),
                showsICloudOptions: true),
        ]
    }
}

/// Nouveautés présentées au premier lancement après une mise à jour
enum WhatsNew {
    struct Item: Identifiable {
        var id: String { symbol + title }
        let symbol: String
        let title: String
        let text: String
    }

    struct Release: Identifiable {
        var id: String { version }
        let version: String
        let items: [Item]
    }

    /// Du plus récent au plus ancien. Une version absente d'ici (correctif) n'affiche rien.
    static var releases: [Release] {
        [
            Release(version: "0.5.0", items: [
                Item(symbol: "tag", title: String(localized: "Tags pour les favoris"),
                     text: String(localized: "Range tes favoris avec des tags, depuis le clic droit ou la barre d'outils. Ils apparaissent sous Favoris dans la barre latérale.")),
                Item(symbol: "line.3.horizontal.decrease.circle", title: String(localized: "Règles de filtrage par flux"),
                     text: String(localized: "Une règle peut maintenant viser plusieurs flux. Raccourci : clic droit sur un flux › Nouvelle règle de filtrage.")),
                Item(symbol: "externaldrive.badge.icloud", title: String(localized: "Sauvegarde dans iCloud Drive"),
                     text: String(localized: "Une copie complète de Flux chaque jour, et la restauration en un clic. À activer dans Réglages › iCloud.")),
                Item(symbol: "arrow.down.doc", title: String(localized: "Export PDF et impression"),
                     text: String(localized: "Bouton dans la barre d'outils de l'article, ⌥⌘E ou ⌘P.")),
                Item(symbol: "link", title: String(localized: "Changer l'adresse d'un flux"),
                     text: String(localized: "Clic droit sur un flux › Modifier l'adresse : ses articles, lus et favoris sont conservés.")),
            ]),
        ]
    }

    static func release(for version: String) -> Release? {
        releases.first { $0.version == version }
    }

    static var latest: Release? { releases.first }
}

extension Bundle {
    var shortVersion: String {
        object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "?"
    }
}

// MARK: - Au lancement

enum StartupScreens {
    /// Première installation : découverte de Flux. Mise à jour : nouveautés de la version.
    @MainActor
    static func decide(context: ModelContext, ui: UIState) {
        let defaults = UserDefaults.standard
        let current = Bundle.main.shortVersion
        let lastSeen = defaults.string(forKey: Prefs.lastSeenVersion)
        defaults.set(current, forKey: Prefs.lastSeenVersion)

        if !defaults.bool(forKey: Prefs.onboardingSeen) {
            defaults.set(true, forKey: Prefs.onboardingSeen)
            // Déjà des flux : l'app était installée avant que la découverte existe
            let hasFeeds = ((try? context.fetchCount(FetchDescriptor<Feed>())) ?? 0) > 0
            if hasFeeds {
                ui.whatsNew = WhatsNew.release(for: current)
            } else {
                ui.showOnboarding = true
            }
            return
        }
        if lastSeen != current {
            ui.whatsNew = WhatsNew.release(for: current)
        }
    }
}

// MARK: - Découverte

struct OnboardingView: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(SyncEngine.self) private var sync
    @Environment(BackupManager.self) private var backup
    @AppStorage(Prefs.syncEnabled) private var syncEnabled = false
    @AppStorage(Prefs.backupEnabled) private var backupEnabled = false

    @State private var index = 0
    private let pages = OnboardingPage.all

    private var page: OnboardingPage { pages[index] }
    private var isLast: Bool { index == pages.count - 1 }

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Spacer()
                Button("Passer") { dismiss() }
                    .buttonStyle(.plain)
                    .foregroundStyle(.secondary)
                    .keyboardShortcut(.cancelAction)
                    .opacity(isLast ? 0 : 1)
                    .disabled(isLast)
            }
            .padding([.top, .horizontal], 18)

            pageView(page)
                .id(page.id)
                .transition(.opacity)
                .frame(maxWidth: .infinity, maxHeight: .infinity)

            HStack(spacing: 7) {
                ForEach(pages) { p in
                    Circle()
                        .fill(p.id == index ? Color.accentColor : Color.secondary.opacity(0.3))
                        .frame(width: 7, height: 7)
                        .onTapGesture { go(to: p.id) }
                }
            }
            .padding(.bottom, 16)

            HStack {
                Button("Précédent") { go(to: index - 1) }
                    .keyboardShortcut(.leftArrow, modifiers: [])
                    .opacity(index == 0 ? 0 : 1)
                    .disabled(index == 0)
                Spacer()
                if isLast {
                    Button("Commencer") { dismiss() }
                        .keyboardShortcut(.defaultAction)
                } else {
                    Button("Suivant") { go(to: index + 1) }
                        .keyboardShortcut(.defaultAction)
                }
            }
            .controlSize(.large)
            .padding([.horizontal, .bottom], 20)
        }
        .frame(width: 580, height: 500)
        .onChange(of: syncEnabled) {
            if syncEnabled { Task { await sync.sync() } } else { sync.disable() }
        }
        .onChange(of: backupEnabled) {
            if backupEnabled, backup.lastBackup == nil { Task { await backup.backupNow() } }
        }
    }

    private func go(to newIndex: Int) {
        guard pages.indices.contains(newIndex) else { return }
        withAnimation(.easeInOut(duration: 0.2)) { index = newIndex }
    }

    @ViewBuilder
    private func pageView(_ page: OnboardingPage) -> some View {
        VStack(spacing: 16) {
            Image(systemName: page.symbol)
                .font(.system(size: 38, weight: .medium))
                .foregroundStyle(.white)
                .frame(width: 84, height: 84)
                .background(page.color.gradient, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
                .shadow(color: page.color.opacity(0.35), radius: 10, y: 4)

            Text(page.title)
                .font(.title.bold())
                .multilineTextAlignment(.center)

            Text(page.text)
                .font(.body)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: 440)

            if !page.tips.isEmpty {
                VStack(alignment: .leading, spacing: 9) {
                    ForEach(page.tips.indices, id: \.self) { i in
                        Label {
                            Text(page.tips[i].text)
                        } icon: {
                            Image(systemName: page.tips[i].symbol)
                                .foregroundStyle(page.color)
                                .frame(width: 20)
                        }
                        .font(.callout)
                    }
                }
                .padding(.top, 4)
                .frame(maxWidth: 440, alignment: .leading)
            }

            if page.showsICloudOptions {
                iCloudOptions
            }
        }
        .padding(.horizontal, 40)
    }

    @ViewBuilder
    private var iCloudOptions: some View {
        if SyncEngine.isICloudDriveAvailable {
            VStack(alignment: .leading, spacing: 10) {
                Toggle("Synchroniser avec mes autres Mac", isOn: $syncEnabled)
                Toggle("Sauvegarder Flux chaque jour dans iCloud Drive", isOn: $backupEnabled)
                Text("Modifiable à tout moment dans Réglages › iCloud.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .toggleStyle(.switch)
            .padding(14)
            .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 10))
            .frame(maxWidth: 440)
            .padding(.top, 4)
        } else {
            Label("iCloud Drive n'est pas activé sur ce Mac. Tu pourras activer ces options plus tard dans Réglages › iCloud.",
                  systemImage: "exclamationmark.triangle")
                .font(.callout)
                .foregroundStyle(.orange)
                .frame(maxWidth: 440)
        }
    }
}

// MARK: - Nouveautés

struct WhatsNewView: View {
    let release: WhatsNew.Release
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            VStack(spacing: 10) {
                Image(nsImage: NSApp.applicationIconImage)
                    .resizable()
                    .frame(width: 72, height: 72)
                Text("Nouveautés de Flux \(release.version)")
                    .font(.title.bold())
            }
            .padding(.top, 28)
            .padding(.bottom, 20)

            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    ForEach(release.items) { item in
                        HStack(alignment: .top, spacing: 14) {
                            Image(systemName: item.symbol)
                                .font(.title2)
                                .foregroundStyle(Color.accentColor)
                                .frame(width: 32)
                            VStack(alignment: .leading, spacing: 3) {
                                Text(item.title).font(.headline)
                                Text(item.text)
                                    .foregroundStyle(.secondary)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                        }
                    }
                }
                .padding(.horizontal, 36)
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            Button("Continuer") { dismiss() }
                .keyboardShortcut(.defaultAction)
                .controlSize(.large)
                .padding(20)
        }
        .frame(width: 520, height: 540)
    }
}
