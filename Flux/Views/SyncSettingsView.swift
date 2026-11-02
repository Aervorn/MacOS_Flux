import AppKit
import SwiftUI
import UniformTypeIdentifiers

/// Onglet Réglages › iCloud : synchronisation et sauvegarde
struct SyncSettingsView: View {
    @Environment(SyncEngine.self) private var sync
    @AppStorage(Prefs.syncEnabled) private var enabled = false
    @AppStorage(Prefs.backupEnabled) private var backupEnabled = false

    /// Hauteur selon l'état : option seule, ou avec la ligne d'état et la liste des autres Mac
    private var idealHeight: CGFloat {
        guard SyncEngine.isICloudDriveAvailable else { return 235 }
        let syncHeight: CGFloat = enabled ? 185 + 52 + 70 + CGFloat(max(sync.otherDevices.count, 1)) * 44 : 185
        // Section Sauvegarde : option seule, ou avec ses réglages et la ligne d'état
        return syncHeight + (backupEnabled ? 300 : 150)
    }

    var body: some View {
        Form {
            Section {
                Toggle("Synchroniser avec mes autres Mac via iCloud Drive", isOn: $enabled)
                    .disabled(!SyncEngine.isICloudDriveAvailable && !enabled)

                if !SyncEngine.isICloudDriveAvailable {
                    Label("iCloud Drive n'est pas activé sur ce Mac (Réglages Système › Identifiant Apple › iCloud › iCloud Drive).",
                          systemImage: "exclamationmark.triangle")
                        .foregroundStyle(.orange)
                } else if enabled {
                    HStack {
                        if sync.isSyncing {
                            ProgressView().controlSize(.small)
                            Text("Synchronisation…").foregroundStyle(.secondary)
                        } else if let error = sync.lastError {
                            Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(.orange)
                        } else if let last = sync.lastSync {
                            Label {
                                Text("Synchronisé \(last.formatted(.relative(presentation: .named).locale(.app)))")
                            } icon: {
                                Image(systemName: "checkmark.icloud")
                            }
                            .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button("Synchroniser maintenant") {
                            Task { await sync.sync() }
                        }
                        .disabled(sync.isSyncing)
                    }
                }
            } header: {
                Text("iCloud Drive")
            } footer: {
                Text("Synchronisés : flux, dossiers, noms personnalisés, règles de filtrage, articles lus et favoris. Non synchronisés : réglages, historique, statistiques et images (chaque Mac télécharge les siennes). Active l'option sur chaque Mac, avec le même compte iCloud. La première fois, macOS peut demander l'autorisation d'accéder à iCloud Drive.")
                    .foregroundStyle(.secondary)
            }

            if SyncEngine.isICloudDriveAvailable {
                BackupSettingsSection()
            }

            if enabled {
                Section("Autres Mac") {
                    if sync.otherDevices.isEmpty {
                        Text("Aucun autre Mac pour l'instant. Active la synchronisation sur ton autre Mac : il apparaîtra ici.")
                            .foregroundStyle(.secondary)
                    }
                    ForEach(sync.otherDevices.indices, id: \.self) { index in
                        let device = sync.otherDevices[index]
                        LabeledContent {
                            Text(device.updated.formatted(.relative(presentation: .named).locale(.app)))
                                .foregroundStyle(.secondary)
                        } label: {
                            Label(device.name, systemImage: "laptopcomputer")
                        }
                    }
                }
            }
        }
        .formStyle(.grouped)
        .settingsTab(idealHeight: idealHeight)
        .onChange(of: enabled) {
            if enabled {
                Task { await sync.sync() }
            } else {
                sync.disable()
            }
        }
    }
}

/// Section Sauvegarde de l'onglet iCloud
struct BackupSettingsSection: View {
    @Environment(BackupManager.self) private var backup
    @AppStorage(Prefs.backupEnabled) private var enabled = false
    @AppStorage(Prefs.backupKeep) private var keep = 7
    @AppStorage(Prefs.backupIncludeArchive) private var includeArchive = true

    @State private var restoreCandidate: URL?

    var body: some View {
        Section {
            Toggle("Sauvegarder Flux chaque jour dans iCloud Drive", isOn: $enabled)
            if enabled {
                Toggle("Inclure l'archive des favoris (articles complets et images)", isOn: $includeArchive)
                Picker("Sauvegardes conservées", selection: $keep) {
                    Text("3 dernières").tag(3)
                    Text("7 dernières").tag(7)
                    Text("14 dernières").tag(14)
                    Text("30 dernières").tag(30)
                }
            }
            HStack {
                if let status = backup.status {
                    ProgressView().controlSize(.small)
                    Text(status).foregroundStyle(.secondary)
                } else if let error = backup.lastError {
                    Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(.orange)
                } else if let last = backup.lastBackup {
                    Text("Dernière sauvegarde \(last.formatted(.relative(presentation: .named).locale(.app)))")
                        .foregroundStyle(.secondary)
                } else {
                    Text("Aucune sauvegarde pour l'instant.").foregroundStyle(.secondary)
                }
                Spacer()
                Button("Sauvegarder maintenant") {
                    Task { await backup.backupNow() }
                }
                .disabled(backup.isWorking)
            }
            HStack {
                Button("Afficher dans le Finder") { showInFinder() }
                Spacer()
                Button("Restaurer…") { chooseBackup() }
                    .disabled(backup.isWorking)
            }
        } header: {
            Text("Sauvegarde")
        } footer: {
            Text("Copie complète de Flux (flux, dossiers, articles, favoris, règles, historique et réglages) rangée dans iCloud Drive › Flux › Sauvegardes. Contrairement à la synchronisation, une suppression n'efface pas les sauvegardes déjà faites. Le cache d'images n'est pas sauvegardé : il se retélécharge.")
                .foregroundStyle(.secondary)
        }
        .onChange(of: enabled) {
            if enabled, backup.lastBackup == nil { Task { await backup.backupNow() } }
        }
        .alert(
            Text("Restaurer « \(restoreCandidate?.deletingPathExtension().lastPathComponent ?? "") » ?"),
            isPresented: Binding(get: { restoreCandidate != nil }, set: { if !$0 { restoreCandidate = nil } })
        ) {
            Button("Restaurer et relancer", role: .destructive) {
                if let url = restoreCandidate { Task { await backup.restore(from: url) } }
            }
            Button("Annuler", role: .cancel) {}
        } message: {
            Text("Flux va redémarrer avec le contenu de cette sauvegarde. Le contenu actuel est mis de côté dans le dossier « Avant restauration ». Si la synchronisation est activée, le contenu restauré sera aussi envoyé à tes autres Mac.")
        }
    }

    private func showInFinder() {
        let folder = FileManager.default.fileExists(atPath: BackupManager.deviceDirectory.path)
            ? BackupManager.deviceDirectory : BackupManager.backupsRoot
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        NSWorkspace.shared.activateFileViewerSelecting([folder])
    }

    private func chooseBackup() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.zip]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.directoryURL = FileManager.default.fileExists(atPath: BackupManager.deviceDirectory.path)
            ? BackupManager.deviceDirectory : BackupManager.backupsRoot
        panel.message = String(localized: "Choisis la sauvegarde à restaurer")
        panel.prompt = String(localized: "Choisir")
        guard panel.runModal() == .OK, let url = panel.url else { return }
        restoreCandidate = url
    }
}
