// Autorisation des notifications de Flux dans macOS (Réglages Système › Notifications).
//
// Le module de notifications de Tauri ne sait pas lire cette autorisation : on interroge ici
// l'interface actuelle d'Apple (UserNotifications), et on peut demander l'autorisation.

import Foundation
import UserNotifications

private final class StatusBox: @unchecked Sendable {
    var value: Int32 = -1
}

/// UNUserNotificationCenter plante hors d'une app empaquetée (binaire lancé seul, `tauri dev`).
private func insideAppBundle() -> Bool {
    Bundle.main.bundleIdentifier != nil && Bundle.main.bundleURL.pathExtension == "app"
}

/// -1 inconnu (hors app empaquetée), 0 pas encore demandé, 1 refusé, 2 autorisé.
/// Bloque jusqu'à la réponse : à appeler hors du fil principal.
@_cdecl("flux_notify_status")
public func fluxNotifyStatus() -> Int32 {
    guard insideAppBundle() else { return -1 }
    let box = StatusBox()
    let done = DispatchSemaphore(value: 0)
    UNUserNotificationCenter.current().getNotificationSettings { settings in
        switch settings.authorizationStatus {
        case .notDetermined: box.value = 0
        case .denied: box.value = 1
        default: box.value = 2 // autorisé, provisoire…
        }
        done.signal()
    }
    done.wait()
    return box.value
}

/// Affiche la demande d'autorisation de macOS (une seule fois : ensuite, seuls les Réglages
/// Système permettent de changer d'avis), puis renvoie l'état comme `flux_notify_status`.
@_cdecl("flux_notify_request")
public func fluxNotifyRequest() -> Int32 {
    guard insideAppBundle() else { return -1 }
    let done = DispatchSemaphore(value: 0)
    UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge]) { _, _ in
        done.signal()
    }
    done.wait()
    return fluxNotifyStatus()
}
