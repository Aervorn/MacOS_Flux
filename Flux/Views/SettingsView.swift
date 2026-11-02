import AppKit
import SwiftUI

struct SettingsView: View {
    @Environment(FeedStore.self) private var store

    @AppStorage(Prefs.autoMarkReadEnabled) private var autoMarkReadEnabled = false
    @AppStorage(Prefs.autoMarkReadDays) private var autoMarkReadDays = 7
    @AppStorage(Prefs.refreshMinutes) private var refreshMinutes = 30
    @AppStorage(Prefs.retentionDays) private var retentionDays = 60
    @AppStorage(Prefs.appearance) private var appearance = AppAppearance.system
    @AppStorage(Prefs.markReadDelay) private var markReadDelay = 0
    @AppStorage(Prefs.showSummary) private var showSummary = true
    @AppStorage(Prefs.readerWidth) private var readerWidth = "narrow"
    @AppStorage(Prefs.readerFontSize) private var readerFontSize = 17
    @AppStorage(Prefs.archiveFavorites) private var archiveFavorites = true
    @AppStorage(Prefs.groupDuplicates) private var groupDuplicates = true
    @AppStorage(Prefs.duplicateSensitivity) private var duplicateSensitivity = 1
    @State private var archiveSize: Int64?
    @State private var pendingArchive = 0
    @AppStorage(Prefs.offlineEnabled) private var offlineEnabled = false
    @AppStorage(Prefs.imageCacheLimitMB) private var imageCacheLimitMB = 1024
    @State private var cacheSize: Int64?

    // « Marquer comme lus les anciens articles » : brouillon, enregistré avec le bouton Appliquer
    @State private var draftAutoRead = false
    @State private var draftAutoReadDays = 7
    @State private var affectedCount = 0

    private var autoReadIsApplied: Bool {
        autoMarkReadEnabled && draftAutoRead && draftAutoReadDays == autoMarkReadDays
    }

    @State private var language = AppLanguage.current
    @State private var languageChanged = false
    @State private var askRelaunch = false

    var body: some View {
        TabView {
            Form {
                Section {
                    Picker("Apparence", selection: $appearance) {
                        Text("Automatique").tag(AppAppearance.system)
                        Text("Clair").tag(AppAppearance.light)
                        Text("Sombre").tag(AppAppearance.dark)
                    }
                    Picker("Langue", selection: $language) {
                        Text("Langue du système").tag(AppLanguage.system)
                        Text(verbatim: "Français").tag("fr")
                        Text(verbatim: "English").tag("en")
                    }
                    if languageChanged {
                        HStack {
                            Text("La nouvelle langue s'appliquera au prochain lancement.")
                                .foregroundStyle(.secondary)
                            Spacer()
                            Button("Relancer Flux") { AppLanguage.relaunch() }
                        }
                    }
                } header: {
                    Text("Général")
                }

                UpdateSettingsSection()

                Section("Affichage") {
                    Toggle("Afficher le résumé sous le titre (mode liste)", isOn: $showSummary)
                    LabeledContent("Taille du texte de l'article") {
                        HStack(spacing: 10) {
                            Image(systemName: "textformat.size.smaller").foregroundStyle(.secondary)
                            Slider(value: Binding(
                                get: { Double(readerFontSize) },
                                set: { readerFontSize = Int($0.rounded()) }
                            ), in: Double(Prefs.readerFontSizes.lowerBound)...Double(Prefs.readerFontSizes.upperBound), step: 1)
                            .frame(width: 160)
                            Image(systemName: "textformat.size.larger").foregroundStyle(.secondary)
                            Text(verbatim: "\(readerFontSize) pt")
                                .monospacedDigit()
                                .foregroundStyle(.secondary)
                                .frame(width: 44, alignment: .trailing)
                        }
                    }
                    Picker("Largeur du texte de l'article", selection: $readerWidth) {
                        Text("Étroite").tag("narrow")
                        Text("Large").tag("wide")
                        Text("Toute la largeur").tag("full")
                    }
                }
            }
            .formStyle(.grouped)
            .settingsTab(idealHeight: languageChanged ? 565 : 520)
            .tabItem { Label("Général", systemImage: "gearshape") }

            Form {
                Section {
                    Picker("Marquer les articles comme lus", selection: $markReadDelay) {
                        Text("Immédiatement").tag(0)
                        Text("Après 5 secondes").tag(5)
                        Text("Après 10 secondes").tag(10)
                        Text("Après 30 secondes").tag(30)
                        Text("Jamais").tag(-1)
                    }
                    Toggle("Marquer automatiquement comme lus les anciens articles", isOn: $draftAutoRead)
                    Picker("Articles publiés il y a plus de", selection: $draftAutoReadDays) {
                        Text("1 jour").tag(1)
                        Text("2 jours").tag(2)
                        Text("3 jours").tag(3)
                        Text("1 semaine").tag(7)
                        Text("2 semaines").tag(14)
                        Text("1 mois").tag(30)
                    }
                    .disabled(!draftAutoRead)
                    HStack {
                        if draftAutoRead && !autoReadIsApplied {
                            Text(affectedCount == 0
                                 ? String(localized: "Aucun article non lu concerné pour l'instant.")
                                 : String(localized: "\(affectedCount) articles non lus seront marqués comme lus."))
                                .foregroundStyle(.secondary)
                        } else if autoReadIsApplied {
                            Label("Appliqué", systemImage: "checkmark.circle.fill")
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button("Appliquer") {
                            autoMarkReadDays = draftAutoReadDays
                            autoMarkReadEnabled = true
                            store.applyAutoMarkRead()
                            updateAffectedCount()
                        }
                        .disabled(!draftAutoRead || autoReadIsApplied)
                    }
                } header: {
                    Text("Lecture")
                } footer: {
                    Text("S'applique aux articles existants et à ceux qui arrivent. Les favoris ne sont jamais marqués automatiquement.")
                        .foregroundStyle(.secondary)
                }

                Section {
                    Toggle("Regrouper les articles similaires de plusieurs sources", isOn: $groupDuplicates)
                    Picker("Sensibilité", selection: $duplicateSensitivity) {
                        ForEach(DuplicateDetector.Sensitivity.allCases) { level in
                            Text(level.label).tag(level.rawValue)
                        }
                    }
                    .disabled(!groupDuplicates)
                } header: {
                    Text("Articles similaires")
                } footer: {
                    Text("Quand plusieurs sources publient la même actualité à moins de 48 heures d'écart, un seul article apparaît, avec « +N » et la liste des autres sources. Le lire marque tout le groupe comme lu. Le regroupement se trompe parfois : clic droit › Séparer des articles similaires.")
                        .foregroundStyle(.secondary)
                }

                Section("Actualisation") {
                    Picker("Actualiser les flux", selection: $refreshMinutes) {
                        Text("Toutes les 15 minutes").tag(15)
                        Text("Toutes les 30 minutes").tag(30)
                        Text("Toutes les heures").tag(60)
                        Text("Toutes les 2 heures").tag(120)
                        Text("Toutes les 6 heures").tag(360)
                    }
                }
            }
            .formStyle(.grouped)
            .settingsTab(idealHeight: 550)
            .tabItem { Label("Lecture", systemImage: "book") }

            FiltersSettingsView()   // hauteur calculée dans la vue (selon le nombre de règles)
                .tabItem { Label("Filtres", systemImage: "line.3.horizontal.decrease.circle") }

            Form {
                Section {
                    Toggle("Préparer les articles pour la lecture hors ligne", isOn: $offlineEnabled)
                    Picker("Taille maximale du cache d'images", selection: $imageCacheLimitMB) {
                        Text("250 Mo").tag(250)
                        Text("500 Mo").tag(500)
                        Text("1 Go").tag(1024)
                        Text("2 Go").tag(2048)
                        Text("5 Go").tag(5120)
                    }
                    HStack {
                        Text("Images en cache")
                        Spacer()
                        if let cacheSize {
                            Text(cacheSize.formatted(.byteCount(style: .file)))
                                .foregroundStyle(.secondary)
                                .monospacedDigit()
                        }
                        Button("Vider") {
                            ImageCache.clear()
                            Task { await updateCacheSize() }
                        }
                    }
                    if let status = store.offlineStatus {
                        HStack(spacing: 8) {
                            ProgressView().controlSize(.small)
                            Text(status).foregroundStyle(.secondary)
                        }
                    } else if offlineEnabled {
                        Button("Préparer maintenant") {
                            Task { await store.prepareOffline() }
                        }
                    }
                } header: {
                    Text("Hors ligne")
                } footer: {
                    Text("Après chaque actualisation, les articles non lus et les favoris sont téléchargés en entier, images comprises. Quand le cache est plein, les images les moins récemment utilisées sont supprimées.")
                        .foregroundStyle(.secondary)
                }

                Section {
                    Toggle("Archiver les favoris (article complet et images)", isOn: $archiveFavorites)
                    HStack {
                        Text("Taille de l'archive")
                        Spacer()
                        if let archiveSize {
                            Text(archiveSize.formatted(.byteCount(style: .file)))
                                .foregroundStyle(.secondary)
                                .monospacedDigit()
                        }
                    }
                    if let status = store.archiveStatus {
                        HStack(spacing: 8) {
                            ProgressView().controlSize(.small)
                            Text(status).foregroundStyle(.secondary)
                        }
                    } else if archiveFavorites && pendingArchive > 0 {
                        HStack {
                            Text("\(pendingArchive) favoris pas encore archivés")
                                .foregroundStyle(.secondary)
                            Spacer()
                            Button("Archiver maintenant") {
                                Task { await store.archiveFavorites() }
                            }
                        }
                    }
                } header: {
                    Text("Archive des favoris")
                } footer: {
                    Text("Quand tu ajoutes un favori, l'article complet et ses images sont rangés dans une archive permanente, indépendante du cache hors ligne : tu peux le relire même si le site le supprime. Retirer un favori libère sa place.")
                        .foregroundStyle(.secondary)
                }

                Section {
                    Picker("Supprimer les articles lus après", selection: $retentionDays) {
                        Text("2 semaines").tag(14)
                        Text("1 mois").tag(30)
                        Text("2 mois").tag(60)
                        Text("3 mois").tag(90)
                        Text("6 mois").tag(180)
                        Text("1 an").tag(365)
                        Text("Jamais").tag(0)
                    }
                } header: {
                    Text("Stockage")
                } footer: {
                    Text("Les articles non lus et les favoris sont toujours conservés. Les articles lus restent retrouvables dans l'Historique et par la recherche jusqu'à leur suppression.")
                        .foregroundStyle(.secondary)
                }
            }
            .formStyle(.grouped)
            .settingsTab(idealHeight: (store.archiveStatus != nil || (archiveFavorites && pendingArchive > 0)) ? 630 : 585)
            .tabItem { Label("Stockage", systemImage: "internaldrive") }

            SyncSettingsView()      // hauteur calculée dans la vue (selon l'état et les autres Mac)
                .tabItem { Label("Synchronisation", systemImage: "icloud") }
        }
        .onChange(of: appearance) {
            AppAppearance.apply(appearance)
        }
        .onChange(of: language) {
            AppLanguage.set(language)
            languageChanged = true
            askRelaunch = true
        }
        .alert("Relancer Flux maintenant ?", isPresented: $askRelaunch) {
            Button("Relancer") { AppLanguage.relaunch() }
                .keyboardShortcut(.defaultAction)
            Button("Plus tard", role: .cancel) {}
        } message: {
            Text("La nouvelle langue sera appliquée au redémarrage de l'app.")
        }
        .task { await updateCacheSize() }
        .onChange(of: groupDuplicates) { store.regroupRecentDuplicates() }
        .onChange(of: duplicateSensitivity) { store.regroupRecentDuplicates() }
        .onChange(of: archiveFavorites) {
            if archiveFavorites { Task { await store.archiveFavorites() } }
        }
        .onChange(of: store.archiveStatus) {
            if store.archiveStatus == nil { Task { await updateCacheSize() } }
        }
        .onChange(of: offlineEnabled) {
            if offlineEnabled { Task { await store.prepareOffline() } }
        }
        .onChange(of: imageCacheLimitMB) {
            let limit = Prefs.imageCacheLimitBytes
            Task {
                await Task.detached { ImageCache.trim(toBytes: limit) }.value
                await updateCacheSize()
            }
        }
        .onChange(of: store.offlineStatus) {
            // Préparation terminée : taille à jour
            if store.offlineStatus == nil { Task { await updateCacheSize() } }
        }
        .onAppear {
            draftAutoRead = autoMarkReadEnabled
            draftAutoReadDays = autoMarkReadDays
            updateAffectedCount()
        }
        .onChange(of: draftAutoRead) {
            // Désactiver est sans risque : pris en compte tout de suite
            if !draftAutoRead { autoMarkReadEnabled = false }
            updateAffectedCount()
        }
        .onChange(of: draftAutoReadDays) { updateAffectedCount() }
    }

    private func updateCacheSize() async {
        cacheSize = await Task.detached { ImageCache.totalSize() }.value
        archiveSize = await Task.detached { ImageCache.archiveSize() }.value
        pendingArchive = store.pendingArchiveCount()
    }

    private func updateAffectedCount() {
        affectedCount = store.unreadCount(olderThanDays: draftAutoReadDays)
    }
}

extension View {
    /// Onglet des réglages : 600 pt de large et la hauteur de son contenu (mesurée sur
    /// captures, plus les lignes qui apparaissent selon l'état). La fenêtre prend la taille
    /// de l'onglet affiché, sans dépasser 85 % de la hauteur de l'écran (au-delà, il défile).
    func settingsTab(idealHeight: CGFloat) -> some View {
        let screen = NSScreen.main?.visibleFrame.height ?? 900
        return frame(width: 600, height: min(idealHeight, screen * 0.85))
    }
}
