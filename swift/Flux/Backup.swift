import AppKit
import Foundation
import Observation
import SQLite3

/// Sauvegardes complètes dans iCloud Drive.
///
/// Contrairement à la synchronisation (qui propage aussi les suppressions et les erreurs),
/// une sauvegarde est une photo figée : une archive .zip par jour, rangée dans
///
///     iCloud Drive/Flux/Sauvegardes/<nom du Mac>/Flux AAAA-MM-JJ HH.mm.zip
///
/// Elle contient la base (flux, dossiers, articles, favoris, règles, historique),
/// les réglages et, en option, l'archive des favoris. Le cache d'images n'y est pas :
/// il se retélécharge.
///
/// Restaurer ne remplace pas la base pendant que l'app tourne : la sauvegarde est
/// décompressée à côté, puis appliquée au lancement suivant (`applyPendingRestore`),
/// avant l'ouverture de la base.
@MainActor
@Observable
final class BackupManager {
    private(set) var isWorking = false
    private(set) var status: String?
    private(set) var lastError: String?
    private(set) var lastBackup: Date?

    @ObservationIgnored private let store: FeedStore
    @ObservationIgnored private var loopTask: Task<Void, Never>?

    private static let interval: TimeInterval = 86_400

    init(store: FeedStore) {
        self.store = store
        let stamp = UserDefaults.standard.double(forKey: Prefs.lastBackup)
        lastBackup = stamp > 0 ? Date(timeIntervalSince1970: stamp) : nil
    }

    // MARK: - Emplacements

    nonisolated private static var appSupport: URL {
        URL.applicationSupportDirectory.appending(path: "Flux", directoryHint: .isDirectory)
    }
    nonisolated private static var storeURL: URL { appSupport.appending(path: "Flux.store") }
    nonisolated private static var pendingRestoreDirectory: URL {
        appSupport.appending(path: "Restauration en attente", directoryHint: .isDirectory)
    }
    /// Base et archive remplacées par la dernière restauration (au cas où)
    nonisolated static var beforeRestoreDirectory: URL {
        appSupport.appending(path: "Avant restauration", directoryHint: .isDirectory)
    }

    /// Dossier commun à tous les Mac (pour « Restaurer… »)
    nonisolated static var backupsRoot: URL {
        SyncEngine.iCloudDrive.appending(path: "Flux", directoryHint: .isDirectory)
            .appending(path: "Sauvegardes", directoryHint: .isDirectory)
    }

    /// Un sous-dossier par Mac : la rotation d'un Mac n'efface pas les sauvegardes de l'autre
    nonisolated static var deviceDirectory: URL {
        let name = SyncEngine.deviceName
            .components(separatedBy: CharacterSet(charactersIn: "/:\\")).joined(separator: "-")
        return backupsRoot.appending(path: name, directoryHint: .isDirectory)
    }

    static var isEnabled: Bool { UserDefaults.standard.bool(forKey: Prefs.backupEnabled) }

    // MARK: - Sauvegarde automatique

    /// Vérifie au lancement (après une minute) puis toutes les heures si une sauvegarde est due
    func start() {
        guard loopTask == nil else { return }
        loopTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(60))
            while !Task.isCancelled {
                if let self, Self.isEnabled, SyncEngine.isICloudDriveAvailable, !self.isWorking,
                   Date.now.timeIntervalSince(self.lastBackup ?? .distantPast) >= Self.interval {
                    await self.backupNow()
                }
                try? await Task.sleep(for: .seconds(3600))
            }
        }
    }

    // MARK: - Sauvegarde

    func backupNow() async {
        guard !isWorking else { return }
        guard SyncEngine.isICloudDriveAvailable else {
            lastError = String(localized: "iCloud Drive n'est pas activé sur ce Mac.")
            return
        }
        isWorking = true
        status = String(localized: "Sauvegarde en cours…")
        lastError = nil
        defer { isWorking = false; status = nil }

        try? store.context.save()
        let includeArchive = UserDefaults.standard.bool(forKey: Prefs.backupIncludeArchive)
        let keep = max(UserDefaults.standard.integer(forKey: Prefs.backupKeep), 1)
        let settings = Self.exportableSettings()
        let deviceName = SyncEngine.deviceName

        do {
            try await Task.detached(priority: .utility) {
                try Self.makeBackup(settings: settings, includeArchive: includeArchive,
                                    deviceName: deviceName, keep: keep)
            }.value
            let now = Date.now
            lastBackup = now
            UserDefaults.standard.set(now.timeIntervalSince1970, forKey: Prefs.lastBackup)
        } catch {
            lastError = error.localizedDescription
        }
    }

    nonisolated private static func makeBackup(settings: [String: Any], includeArchive: Bool,
                                               deviceName: String, keep: Int) throws {
        let fm = FileManager.default
        let staging = fm.temporaryDirectory.appending(path: "Flux-backup-" + UUID().uuidString, directoryHint: .isDirectory)
        try fm.createDirectory(at: staging, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: staging) }

        // 1. Base : copie cohérente même pendant une écriture (API de sauvegarde de SQLite)
        try copyDatabase(from: storeURL, to: staging.appending(path: "Flux.store"))

        // 2. Réglages
        let plist = try PropertyListSerialization.data(fromPropertyList: settings, format: .binary, options: 0)
        try plist.write(to: staging.appending(path: "Réglages.plist"))

        // 3. Archive des favoris
        if includeArchive, fm.fileExists(atPath: ImageCache.archiveDirectory.path) {
            try fm.copyItem(at: ImageCache.archiveDirectory, to: staging.appending(path: "Archive", directoryHint: .isDirectory))
        }

        // 4. Informations
        let info: [String: Any] = [
            "format": 1,
            "date": ISO8601DateFormatter().string(from: .now),
            "device": deviceName,
            "appVersion": Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "?",
        ]
        try JSONSerialization.data(withJSONObject: info, options: [.prettyPrinted, .sortedKeys])
            .write(to: staging.appending(path: "Infos.json"))

        // 5. Compression puis dépôt dans iCloud Drive
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd HH.mm"
        let name = "Flux \(formatter.string(from: .now)).zip"
        let zip = fm.temporaryDirectory.appending(path: UUID().uuidString + ".zip")
        defer { try? fm.removeItem(at: zip) }
        try ditto(["-c", "-k", "--sequesterRsrc", staging.path, zip.path])

        let destination = deviceDirectory
        try fm.createDirectory(at: destination, withIntermediateDirectories: true)
        let target = destination.appending(path: name)
        if fm.fileExists(atPath: target.path) { try fm.removeItem(at: target) }
        try fm.moveItem(at: zip, to: target)

        // 6. Rotation : on garde les `keep` plus récentes de ce Mac
        rotate(in: destination, keep: keep)
    }

    /// Supprime les plus anciennes (les noms datés se trient chronologiquement).
    /// Une sauvegarde pas téléchargée sur ce Mac apparaît comme « .Flux ….zip.icloud ».
    nonisolated private static func rotate(in directory: URL, keep: Int) {
        let fm = FileManager.default
        let files = (try? fm.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)) ?? []
        let backups = files.compactMap { url -> (name: String, url: URL)? in
            var name = url.lastPathComponent
            if name.hasPrefix("."), name.hasSuffix(".icloud") { name = String(name.dropFirst().dropLast(7)) }
            guard name.hasPrefix("Flux "), name.hasSuffix(".zip") else { return nil }
            return (name, url)
        }
        .sorted { $0.name > $1.name }
        for old in backups.dropFirst(keep) {
            try? fm.removeItem(at: old.url)
        }
    }

    // MARK: - Restauration

    /// Prépare la restauration d'une sauvegarde, puis relance Flux pour l'appliquer
    func restore(from zip: URL) async {
        guard !isWorking else { return }
        isWorking = true
        status = String(localized: "Préparation de la restauration…")
        lastError = nil
        defer { isWorking = false; status = nil }

        do {
            try await Task.detached(priority: .userInitiated) {
                try Self.prepareRestore(from: zip)
            }.value
            AppLanguage.relaunch()
        } catch {
            lastError = error.localizedDescription
        }
    }

    nonisolated private static func prepareRestore(from zip: URL) throws {
        let fm = FileManager.default
        let pending = pendingRestoreDirectory
        try? fm.removeItem(at: pending)
        try fm.createDirectory(at: pending, withIntermediateDirectories: true)

        // Lecture coordonnée : iCloud Drive télécharge le fichier s'il n'est pas sur ce Mac
        let local = fm.temporaryDirectory.appending(path: UUID().uuidString + ".zip")
        defer { try? fm.removeItem(at: local) }
        var coordinationError: NSError?
        var copyError: Error?
        NSFileCoordinator().coordinate(readingItemAt: zip, options: [], error: &coordinationError) { url in
            do { try fm.copyItem(at: url, to: local) } catch { copyError = error }
        }
        if let error = coordinationError ?? copyError { throw error }

        try ditto(["-x", "-k", local.path, pending.path])
        guard fm.fileExists(atPath: pending.appending(path: "Flux.store").path) else {
            try? fm.removeItem(at: pending)
            throw BackupError.notABackup
        }
    }

    /// Au lancement, avant d'ouvrir la base : applique une restauration préparée
    nonisolated static func applyPendingRestore() {
        let fm = FileManager.default
        let pending = pendingRestoreDirectory
        let restoredStore = pending.appending(path: "Flux.store")
        guard fm.fileExists(atPath: restoredStore.path) else { return }

        // Base et archive actuelles mises de côté (remplace la mise de côté précédente)
        let aside = beforeRestoreDirectory
        try? fm.removeItem(at: aside)
        try? fm.createDirectory(at: aside, withIntermediateDirectories: true)
        for suffix in ["", "-wal", "-shm"] {
            let file = appSupport.appending(path: "Flux.store" + suffix)
            if fm.fileExists(atPath: file.path) {
                try? fm.moveItem(at: file, to: aside.appending(path: "Flux.store" + suffix))
            }
        }
        do {
            try fm.moveItem(at: restoredStore, to: storeURL)
        } catch {
            // Échec : on remet la base d'origine
            for suffix in ["", "-wal", "-shm"] {
                try? fm.moveItem(at: aside.appending(path: "Flux.store" + suffix),
                                 to: appSupport.appending(path: "Flux.store" + suffix))
            }
            return
        }

        let restoredArchive = pending.appending(path: "Archive", directoryHint: .isDirectory)
        if fm.fileExists(atPath: restoredArchive.path) {
            try? fm.moveItem(at: ImageCache.archiveDirectory, to: aside.appending(path: "Archive", directoryHint: .isDirectory))
            try? fm.moveItem(at: restoredArchive, to: ImageCache.archiveDirectory)
        }

        if let data = try? Data(contentsOf: pending.appending(path: "Réglages.plist")),
           let settings = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any] {
            for (key, value) in settings where isExportableSetting(key) {
                UserDefaults.standard.set(value, forKey: key)
            }
        }

        // Synchro : sans mémoire locale, tout le contenu restauré est daté de maintenant.
        // Il l'emporte donc sur l'état des autres Mac (y compris des suppressions) et leur
        // est envoyé ; ce qu'ils ont ajouté depuis la sauvegarde est récupéré en plus.
        try? fm.removeItem(at: appSupport.appending(path: "sync-state.json"))
        try? fm.removeItem(at: pending)
    }

    // MARK: - Réglages

    /// Réglages de Flux, sans ce qui est propre à ce Mac ou au système
    private static func exportableSettings() -> [String: Any] {
        guard let id = Bundle.main.bundleIdentifier,
              let domain = UserDefaults.standard.persistentDomain(forName: id) else { return [:] }
        return domain.filter { isExportableSetting($0.key) }
    }

    nonisolated private static func isExportableSetting(_ key: String) -> Bool {
        if key == "AppleLanguages" { return true }
        if ["syncDeviceID", Prefs.lastBackup].contains(key) { return false }
        // Fenêtres, Sparkle, système
        return !["NS", "SU", "Apple", "com.apple.", "WebKit", "_"].contains { key.hasPrefix($0) }
    }

    // MARK: - Outils

    nonisolated private static func copyDatabase(from source: URL, to destination: URL) throws {
        var src: OpaquePointer?
        var dst: OpaquePointer?
        guard sqlite3_open_v2(source.path, &src, SQLITE_OPEN_READONLY, nil) == SQLITE_OK else {
            sqlite3_close(src)
            throw BackupError.database
        }
        defer { sqlite3_close(src) }
        guard sqlite3_open(destination.path, &dst) == SQLITE_OK else {
            sqlite3_close(dst)
            throw BackupError.database
        }
        defer { sqlite3_close(dst) }
        guard let backup = sqlite3_backup_init(dst, "main", src, "main") else { throw BackupError.database }
        var rc: Int32
        var retries = 0
        repeat {
            rc = sqlite3_backup_step(backup, -1)
            if rc == SQLITE_BUSY || rc == SQLITE_LOCKED {
                retries += 1
                sqlite3_sleep(100)
            }
        } while (rc == SQLITE_OK || rc == SQLITE_BUSY || rc == SQLITE_LOCKED) && retries < 100
        sqlite3_backup_finish(backup)
        guard rc == SQLITE_DONE else { throw BackupError.database }
    }

    nonisolated private static func ditto(_ arguments: [String]) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/ditto")
        process.arguments = arguments
        try process.run()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else { throw BackupError.archive }
    }
}

enum BackupError: LocalizedError {
    case database
    case archive
    case notABackup

    var errorDescription: String? {
        switch self {
        case .database: String(localized: "Impossible de copier la base de Flux.")
        case .archive: String(localized: "Impossible de créer ou d'ouvrir l'archive de la sauvegarde.")
        case .notABackup: String(localized: "Ce fichier n'est pas une sauvegarde de Flux.")
        }
    }
}
