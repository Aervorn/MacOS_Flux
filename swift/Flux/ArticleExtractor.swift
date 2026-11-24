import Foundation
import WebKit

struct ExtractedArticle: Decodable {
    var title: String?
    var content: String?
    var byline: String?
    var excerpt: String?
}

/// Extrait le contenu principal d'une page avec Readability.js (le moteur du
/// mode Lecture de Firefox), exécuté dans un WKWebView invisible.
/// Le HTML de la page est parsé avec DOMParser : ses scripts ne s'exécutent pas
/// et ses ressources (images, pubs…) ne sont pas téléchargées.
@MainActor
final class ArticleExtractor: NSObject, WKNavigationDelegate {
    static let shared = ArticleExtractor()

    private var webView: WKWebView?
    private var setupTask: Task<Bool, Never>?
    private var loadContinuation: CheckedContinuation<Void, Never>?

    func extract(html: String, url: URL) async throws -> ExtractedArticle? {
        guard let webView = await readyWebView() else { return nil }
        let js = """
        const doc = new DOMParser().parseFromString(html, "text/html");
        let base = doc.querySelector("base");
        if (!base) {
            base = doc.createElement("base");
            (doc.head || doc.documentElement).prepend(base);
        }
        base.setAttribute("href", url);
        const result = new Readability(doc).parse();
        if (!result || !result.content) { return null; }
        return JSON.stringify({ title: result.title, content: result.content, byline: result.byline, excerpt: result.excerpt });
        """
        let value = try await webView.callAsyncJavaScript(
            js, arguments: ["html": html, "url": url.absoluteString], in: nil, contentWorld: .page)
        guard let json = value as? String, let data = json.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(ExtractedArticle.self, from: data)
    }

    private func readyWebView() async -> WKWebView? {
        if setupTask == nil {
            setupTask = Task { await self.setUp() }
        }
        return await setupTask!.value ? webView : nil
    }

    private func setUp() async -> Bool {
        guard let scriptURL = Bundle.main.url(forResource: "Readability", withExtension: "js"),
              let source = try? String(contentsOf: scriptURL, encoding: .utf8) else {
            print("Readability.js introuvable dans le bundle")
            return false
        }
        let webView = WKWebView(frame: NSRect(x: 0, y: 0, width: 800, height: 600))
        webView.navigationDelegate = self
        self.webView = webView

        await withCheckedContinuation { continuation in
            loadContinuation = continuation
            webView.loadHTMLString("<!doctype html><html><head></head><body></body></html>", baseURL: nil)
        }
        do {
            _ = try await webView.evaluateJavaScript(source + "\n;true")
            return true
        } catch {
            print("Échec du chargement de Readability.js : \(error)")
            return false
        }
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        loadContinuation?.resume()
        loadContinuation = nil
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        loadContinuation?.resume()
        loadContinuation = nil
    }
}

enum FullTextLoader {
    /// Télécharge la page de l'article et renvoie le HTML nettoyé.
    @MainActor
    static func load(_ url: URL) async throws -> String? {
        let (data, finalURL) = try await Net.get(url)
        let html = Net.decodeHTML(data)
        return try await ArticleExtractor.shared.extract(html: html, url: finalURL)?.content
    }
}
