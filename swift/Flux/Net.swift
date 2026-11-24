import Foundation

enum FeedError: LocalizedError {
    case invalidURL
    case notAFeed
    case alreadyExists
    case http(Int)

    var errorDescription: String? {
        switch self {
        case .invalidURL: String(localized: "Adresse invalide.")
        case .notAFeed: String(localized: "Aucun flux RSS ou Atom trouvé à cette adresse.")
        case .alreadyExists: String(localized: "Ce flux est déjà dans ta liste.")
        case .http(let code): String(localized: "Le serveur a répondu avec l'erreur \(code).")
        }
    }
}

enum Net {
    /// User-Agent de Safari : certains sites (Cloudflare…) bloquent les clients inconnus.
    private static let userAgent =
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15"

    static let session: URLSession = {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 20
        config.httpAdditionalHeaders = [
            "User-Agent": userAgent,
            "Accept": "application/rss+xml, application/atom+xml, application/feed+json, application/xml, text/xml, application/json;q=0.9, text/html;q=0.9, */*;q=0.8",
            "Accept-Language": "fr-FR,fr;q=0.9,en;q=0.8",
        ]
        return URLSession(configuration: config)
    }()

    /// Renvoie les données et l'URL finale (après redirections).
    static func get(_ url: URL) async throws -> (Data, URL) {
        let (data, response) = try await session.data(from: url)
        if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
            throw FeedError.http(http.statusCode)
        }
        return (data, response.url ?? url)
    }

    static func decodeHTML(_ data: Data) -> String {
        String(data: data, encoding: .utf8) ?? String(data: data, encoding: .isoLatin1) ?? ""
    }
}
