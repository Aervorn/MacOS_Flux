import AppKit
import Foundation

/// Clés des réglages (UserDefaults / @AppStorage) et leurs valeurs par défaut.
enum Prefs {
    static let autoMarkReadEnabled = "autoMarkReadEnabled"
    static let autoMarkReadDays = "autoMarkReadDays"
    static let refreshMinutes = "refreshMinutes"
    static let retentionDays = "retentionDays"
    static let appearance = "appearance"
    /// Délai avant de marquer comme lu l'article ouvert, en secondes (0 = immédiatement, -1 = jamais)
    static let markReadDelay = "markReadDelay"
    /// Présentation de la liste d'articles : "list", "grid" ou "gridFull"
    /// (vignettes en plein écran : la grille prend toute la place, l'article s'ouvre par-dessus)
    static let articleLayout = "articleLayout"
    /// Lecture hors ligne : télécharger articles complets et images des non lus et favoris
    static let offlineEnabled = "offlineEnabled"
    /// Taille maximale du cache d'images, en Mo
    static let imageCacheLimitMB = "imageCacheLimitMB"
    /// Largeur du texte dans la vue de lecture : "narrow", "wide" ou "full"
    static let readerWidth = "readerWidth"
    /// Afficher le résumé sous le titre en mode liste
    static let showSummary = "showSummary"
    /// Taille du texte de lecture, en points
    static let readerFontSize = "readerFontSize"
    static let readerFontSizes: ClosedRange<Int> = 13...28
    /// Ancien réglage (remplacé par articleLayout = "gridFull"), lu une fois pour la migration
    static let gridFullWidth = "gridFullWidth"
    /// Archiver les favoris (article complet + images) dans un stockage permanent
    static let archiveFavorites = "archiveFavorites"
    /// Regrouper les articles similaires publiés par plusieurs sources
    static let groupDuplicates = "groupDuplicates"
    /// DuplicateDetector.Sensitivity.rawValue
    static let duplicateSensitivity = "duplicateSensitivity"
    /// Version de l'algorithme qui a calculé les groupes actuels
    static let duplicatesVersion = "duplicatesVersion"
    /// Synchronisation entre Mac par iCloud Drive
    static let syncEnabled = "syncEnabled"
    /// Masquer les articles lus dans Tous les articles, les dossiers et les flux
    static let hideRead = "hideRead"
    /// Favoris : masquer les articles qui ont un tag (ils restent dans la vue de leur(s) tag(s))
    static let hideTaggedFavorites = "hideTaggedFavorites"
    /// Sauvegarde quotidienne dans iCloud Drive (BackupManager)
    static let backupEnabled = "backupEnabled"
    /// Nombre de sauvegardes gardées pour ce Mac
    static let backupKeep = "backupKeep"
    /// Inclure l'archive des favoris (articles complets et images)
    static let backupIncludeArchive = "backupIncludeArchive"
    /// Date de la dernière sauvegarde (secondes depuis 1970), propre à ce Mac
    static let lastBackup = "lastBackup"
    /// Découverte de Flux déjà présentée sur ce Mac
    static let onboardingSeen = "onboardingSeen"
    /// Dernière version lancée sur ce Mac (pour afficher les nouveautés après une mise à jour)
    static let lastSeenVersion = "lastSeenVersion"

    static let defaults: [String: Any] = [
        autoMarkReadEnabled: false,
        autoMarkReadDays: 7,
        refreshMinutes: 30,
        retentionDays: 60,
        appearance: AppAppearance.system,
        markReadDelay: 0,
        offlineEnabled: false,
        imageCacheLimitMB: 1024,
        readerWidth: "narrow",
        showSummary: true,
        readerFontSize: 17,
        archiveFavorites: true,
        groupDuplicates: true,
        duplicateSensitivity: 1,
        hideRead: false,
        hideTaggedFavorites: false,
        backupEnabled: false,
        backupKeep: 7,
        backupIncludeArchive: true,
    ]

    static func register() {
        let d = UserDefaults.standard
        d.register(defaults: defaults)
        // Migration : l'ancienne case « Vignettes en plein écran » devient un choix de présentation
        if d.object(forKey: gridFullWidth) != nil {
            if d.bool(forKey: gridFullWidth), d.string(forKey: articleLayout) == "grid" {
                d.set("gridFull", forKey: articleLayout)
            }
            d.removeObject(forKey: gridFullWidth)
        }
    }

    /// Date avant laquelle un article est automatiquement marqué comme lu (nil = option désactivée)
    static var autoMarkReadCutoff: Date? {
        let d = UserDefaults.standard
        guard d.bool(forKey: autoMarkReadEnabled) else { return nil }
        let days = max(d.integer(forKey: autoMarkReadDays), 1)
        return Date.now.addingTimeInterval(-Double(days) * 86_400)
    }

    static var offline: Bool { UserDefaults.standard.bool(forKey: offlineEnabled) }

    static var imageCacheLimitBytes: Int64 {
        Int64(max(UserDefaults.standard.integer(forKey: imageCacheLimitMB), 100)) << 20
    }

    static var archive: Bool { UserDefaults.standard.bool(forKey: archiveFavorites) }

    /// Les articles lus (et non favoris) plus vieux que ça sont supprimés (0 = jamais)
    static var retentionCutoff: Date {
        let days = UserDefaults.standard.integer(forKey: retentionDays)
        guard days > 0 else { return .distantPast }
        return Date.now.addingTimeInterval(-Double(max(days, 7)) * 86_400)
    }
}

/// Apparence de l'app : celle de macOS, ou forcée en clair / sombre.
/// Appliquée à toute l'app (fenêtres, réglages et vue de lecture, dont le CSS
/// suit `prefers-color-scheme`).
enum AppAppearance {
    static let system = "system"
    static let light = "light"
    static let dark = "dark"

    @MainActor
    static func apply(_ value: String? = nil) {
        let v = value ?? UserDefaults.standard.string(forKey: Prefs.appearance) ?? system
        switch v {
        case light: NSApplication.shared.appearance = NSAppearance(named: .aqua)
        case dark: NSApplication.shared.appearance = NSAppearance(named: .darkAqua)
        default: NSApplication.shared.appearance = nil   // suit macOS
        }
    }
}

/// Langue de l'interface. Par défaut, celle de macOS ; on peut la forcer
/// (équivaut au réglage par app de Réglages Système › Langue et région).
enum AppLanguage {
    static let system = "system"
    private static let key = "AppleLanguages"

    /// "system", "fr" ou "en"
    static var current: String {
        let domain = UserDefaults.standard.persistentDomain(forName: Bundle.main.bundleIdentifier ?? "") ?? [:]
        guard let first = (domain[key] as? [String])?.first else { return system }
        return first.hasPrefix("fr") ? "fr" : first.hasPrefix("en") ? "en" : system
    }

    static func set(_ code: String) {
        if code == system {
            UserDefaults.standard.removeObject(forKey: key)
        } else {
            UserDefaults.standard.set([code], forKey: key)
        }
    }

    /// Relance l'app pour appliquer la nouvelle langue
    @MainActor
    static func relaunch() {
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.createsNewApplicationInstance = true
        NSWorkspace.shared.openApplication(at: Bundle.main.bundleURL, configuration: configuration) { _, _ in
            DispatchQueue.main.async { NSApp.terminate(nil) }
        }
    }
}
