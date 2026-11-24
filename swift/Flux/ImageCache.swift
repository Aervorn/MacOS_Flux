import AppKit
import CryptoKit
import Foundation
import SwiftUI
import WebKit

/// Cache d'images sur disque, utilisé par la vue de lecture et les miniatures.
/// Une image = un fichier nommé d'après le SHA-256 de son URL, dans
/// ~/Library/Application Support/Flux/Images. Quand le cache dépasse la taille
/// maximale, les images les moins récemment utilisées sont supprimées.
enum ImageCache {
    static let directory: URL = {
        let dir = URL.applicationSupportDirectory
            .appending(path: "Flux", directoryHint: .isDirectory)
            .appending(path: "Images", directoryHint: .isDirectory)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }()

    /// Archive permanente des images des favoris : jamais vidée automatiquement,
    /// ni par la limite de taille, ni par le bouton « Vider » du cache
    static let archiveDirectory: URL = {
        let dir = URL.applicationSupportDirectory
            .appending(path: "Flux", directoryHint: .isDirectory)
            .appending(path: "Archive", directoryHint: .isDirectory)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }()

    /// Taille max d'une image téléchargée
    private static let maxImageBytes = 15 << 20

    static func fileName(for url: URL) -> String {
        SHA256.hash(data: Data(url.absoluteString.utf8)).map { String(format: "%02x", $0) }.joined()
    }

    static func fileURL(for url: URL) -> URL {
        directory.appending(path: fileName(for: url))
    }

    static func archiveURL(for url: URL) -> URL {
        archiveDirectory.appending(path: fileName(for: url))
    }

    static func isArchived(_ url: URL) -> Bool {
        FileManager.default.fileExists(atPath: archiveURL(for: url).path)
    }

    static func isCached(_ url: URL) -> Bool {
        isArchived(url) || FileManager.default.fileExists(atPath: fileURL(for: url).path)
    }

    /// Image en cache ou dans l'archive (et la marque comme récemment utilisée)
    static func cachedData(for url: URL) -> Data? {
        if let data = try? Data(contentsOf: archiveURL(for: url)) { return data }
        let file = fileURL(for: url)
        guard let data = try? Data(contentsOf: file) else { return nil }
        try? FileManager.default.setAttributes([.modificationDate: Date.now], ofItemAtPath: file.path)
        return data
    }

    /// Image depuis le cache, sinon téléchargée (puis mise en cache)
    static func data(for url: URL) async -> Data? {
        if let data = cachedData(for: url) { return data }
        return await download(url)
    }

    @discardableResult
    static func download(_ url: URL) async -> Data? {
        guard let scheme = url.scheme?.lowercased(), scheme == "http" || scheme == "https",
              let result = try? await Net.get(url) else { return nil }
        let data = result.0
        guard !data.isEmpty, data.count <= maxImageBytes, mimeType(for: data) != nil else { return nil }
        try? data.write(to: fileURL(for: url), options: .atomic)
        return data
    }

    /// Télécharge les images absentes du cache (6 à la fois)
    static func prefetch(_ urls: [URL]) async {
        let missing = urls.filter { !isCached($0) }
        for start in stride(from: 0, to: missing.count, by: 6) {
            let chunk = missing[start..<min(start + 6, missing.count)]
            await withTaskGroup(of: Void.self) { group in
                for url in chunk {
                    group.addTask { _ = await download(url) }
                }
            }
        }
    }

    /// Range une image dans l'archive permanente (depuis le cache, sinon téléchargée)
    @discardableResult
    static func archive(_ url: URL) async -> Bool {
        if isArchived(url) { return true }
        let cached = fileURL(for: url)
        if FileManager.default.fileExists(atPath: cached.path) {
            return (try? FileManager.default.moveItem(at: cached, to: archiveURL(for: url))) != nil
        }
        guard await download(url) != nil else { return false }
        return (try? FileManager.default.moveItem(at: cached, to: archiveURL(for: url))) != nil
    }

    /// Sort de l'archive les images qui ne servent plus à aucun favori :
    /// elles retournent dans le cache normal (et sa limite de taille)
    static func pruneArchive(keeping urls: Set<URL>) {
        let keep = Set(urls.map(fileName(for:)))
        let files = (try? FileManager.default.contentsOfDirectory(at: archiveDirectory, includingPropertiesForKeys: nil)) ?? []
        for file in files where !keep.contains(file.lastPathComponent) {
            let destination = directory.appending(path: file.lastPathComponent)
            try? FileManager.default.removeItem(at: destination)
            try? FileManager.default.moveItem(at: file, to: destination)
        }
    }

    static func archiveSize() -> Int64 {
        size(of: archiveDirectory)
    }

    static func totalSize() -> Int64 {
        size(of: directory)
    }

    private static func size(of directory: URL) -> Int64 {
        let keys: [URLResourceKey] = [.fileSizeKey]
        let files = (try? FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: keys)) ?? []
        return files.reduce(0) { $0 + Int64((try? $1.resourceValues(forKeys: Set(keys)))?.fileSize ?? 0) }
    }

    /// Supprime les images les moins récemment utilisées jusqu'à passer sous la limite
    static func trim(toBytes limit: Int64) {
        let keys: [URLResourceKey] = [.fileSizeKey, .contentModificationDateKey]
        let files = (try? FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: keys)) ?? []
        var entries = files.map { file -> (URL, Int64, Date) in
            let values = try? file.resourceValues(forKeys: Set(keys))
            return (file, Int64(values?.fileSize ?? 0), values?.contentModificationDate ?? .distantPast)
        }
        var total = entries.reduce(0) { $0 + $1.1 }
        guard total > limit else { return }
        entries.sort { $0.2 < $1.2 }   // les plus anciennes d'abord
        for (file, size, _) in entries where total > limit {
            try? FileManager.default.removeItem(at: file)
            total -= size
        }
    }

    static func clear() {
        let files = (try? FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)) ?? []
        for file in files { try? FileManager.default.removeItem(at: file) }
    }

    /// Type MIME d'après les premiers octets (nil si ce n'est pas une image)
    static func mimeType(for data: Data) -> String? {
        let b = [UInt8](data.prefix(16))
        guard b.count >= 4 else { return nil }
        if b[0] == 0xFF, b[1] == 0xD8, b[2] == 0xFF { return "image/jpeg" }
        if b[0] == 0x89, b[1] == 0x50, b[2] == 0x4E, b[3] == 0x47 { return "image/png" }
        if b[0] == 0x47, b[1] == 0x49, b[2] == 0x46 { return "image/gif" }
        if b.count >= 12, b[0] == 0x52, b[1] == 0x49, b[2] == 0x46, b[3] == 0x46,
           b[8] == 0x57, b[9] == 0x45, b[10] == 0x42, b[11] == 0x50 { return "image/webp" }
        if b.count >= 12, b[4] == 0x66, b[5] == 0x74, b[6] == 0x79, b[7] == 0x70 {
            let brand = String(decoding: b[8..<12], as: UTF8.self)
            if brand.hasPrefix("avi") { return "image/avif" }
            if brand.hasPrefix("hei") || brand.hasPrefix("mif") { return "image/heic" }
        }
        let head = String(decoding: data.prefix(512), as: UTF8.self).lowercased()
        if head.contains("<svg") { return "image/svg+xml" }
        return nil
    }

    // MARK: - Protocole interne pour WebKit

    /// La vue de lecture n'utilise pas le cache réseau de l'app : ses images sont
    /// réécrites en flux-image://…, un protocole servi par CachedImageSchemeHandler.
    static let scheme = "flux-image"

    static func proxyURL(for url: URL) -> String {
        let encoded = url.absoluteString.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? ""
        return "\(scheme)://image/?u=\(encoded)"
    }

    static func originalURL(fromProxy url: URL) -> URL? {
        URLComponents(url: url, resolvingAgainstBaseURL: false)?
            .queryItems?.first { $0.name == "u" }?.value
            .flatMap { URL(string: $0) }
    }
}

/// Sert les images flux-image:// à la vue de lecture : depuis le cache disque,
/// sinon depuis le réseau (et les met en cache au passage).
@MainActor
final class CachedImageSchemeHandler: NSObject, WKURLSchemeHandler {
    /// Tâches encore actives (WebKit peut en annuler ; répondre à une tâche annulée plante)
    private var active: Set<ObjectIdentifier> = []

    func webView(_ webView: WKWebView, start task: any WKURLSchemeTask) {
        guard let url = task.request.url, let original = ImageCache.originalURL(fromProxy: url) else {
            task.didFailWithError(URLError(.badURL))
            return
        }
        let id = ObjectIdentifier(task as AnyObject)
        active.insert(id)
        Task {
            let data = await ImageCache.data(for: original)
            guard active.remove(id) != nil else { return }
            if let data, let mime = ImageCache.mimeType(for: data) {
                task.didReceive(URLResponse(url: url, mimeType: mime, expectedContentLength: data.count, textEncodingName: nil))
                task.didReceive(data)
                task.didFinish()
            } else {
                task.didFailWithError(URLError(.resourceUnavailable))
            }
        }
    }

    func webView(_ webView: WKWebView, stop task: any WKURLSchemeTask) {
        active.remove(ObjectIdentifier(task as AnyObject))
    }
}

/// Miniatures (liste et vignettes) : passent par le cache disque,
/// avec un cache mémoire pour le défilement.
@MainActor
enum ThumbnailLoader {
    private static let memory: NSCache<NSURL, NSImage> = {
        let cache = NSCache<NSURL, NSImage>()
        cache.countLimit = 400
        return cache
    }()

    static func image(for url: URL) async -> NSImage? {
        if let image = memory.object(forKey: url as NSURL) { return image }
        guard let data = await ImageCache.data(for: url), let image = NSImage(data: data) else { return nil }
        memory.setObject(image, forKey: url as NSURL)
        return image
    }
}

/// Équivalent d'AsyncImage qui passe par le cache disque (disponible hors ligne)
struct CachedImage<Placeholder: View>: View {
    let url: URL?
    @ViewBuilder var placeholder: () -> Placeholder

    @State private var image: NSImage?

    var body: some View {
        Group {
            if let image {
                Image(nsImage: image)
                    .resizable()
                    .scaledToFill()
                    .transition(.opacity)
            } else {
                placeholder()
            }
        }
        .task(id: url) {
            image = nil
            guard let url else { return }
            let loaded = await ThumbnailLoader.image(for: url)
            guard !Task.isCancelled else { return }
            withAnimation(.easeIn(duration: 0.2)) { image = loaded }
        }
    }
}
