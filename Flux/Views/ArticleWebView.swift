import AppKit
import SwiftUI
import WebKit

/// Affiche le HTML de l'article. JavaScript est désactivé ; un clic sur un lien
/// l'ouvre dans le navigateur par défaut.
struct ArticleWebView: NSViewRepresentable {
    let html: String
    let baseURL: URL?
    /// Même contenu (seule la mise en forme change) : on garde la position de lecture
    var contentID: String = ""
    @Environment(KeyboardRouter.self) private var keyboard

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.defaultWebpagePreferences.allowsContentJavaScript = false
        // Images servies depuis le cache disque (lecture hors ligne)
        config.setURLSchemeHandler(context.coordinator.imageHandler, forURLScheme: ImageCache.scheme)
        let webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = context.coordinator
        webView.setValue(false, forKey: "drawsBackground")
        keyboard.readerWebView = webView   // pour faire défiler l'article à la touche espace
        return webView
    }

    func updateNSView(_ webView: WKWebView, context: Context) {
        let coordinator = context.coordinator
        guard coordinator.lastHTML != html else { return }
        let restyleOnly = coordinator.lastHTML != nil && coordinator.lastContentID == contentID
        coordinator.lastHTML = html
        coordinator.lastContentID = contentID
        guard restyleOnly else {
            webView.loadHTMLString(html, baseURL: baseURL)
            return
        }
        // Taille ou largeur du texte modifiée : on recharge en restant au même endroit (en proportion)
        let html = html, baseURL = baseURL
        Task { @MainActor in
            let fraction = try? await webView.callAsyncJavaScript(
                "const el = document.scrollingElement || document.documentElement; return el.scrollHeight > 0 ? el.scrollTop / el.scrollHeight : 0;",
                arguments: [:], in: nil, contentWorld: .defaultClient) as? Double
            coordinator.restoreFraction = fraction
            webView.loadHTMLString(html, baseURL: baseURL)
        }
    }

    @MainActor
    final class Coordinator: NSObject, WKNavigationDelegate {
        var lastHTML: String?
        var lastContentID: String?
        var restoreFraction: Double?
        let imageHandler = CachedImageSchemeHandler()

        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            guard let fraction = restoreFraction, fraction > 0 else { return }
            restoreFraction = nil
            webView.callAsyncJavaScript(
                "const el = document.scrollingElement || document.documentElement; el.scrollTop = f * el.scrollHeight;",
                arguments: ["f": fraction], in: nil, in: .defaultClient, completionHandler: nil)
        }

        func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction) async -> WKNavigationActionPolicy {
            guard navigationAction.navigationType == .linkActivated,
                  let url = navigationAction.request.url else { return .allow }
            // Ancres internes (notes de bas de page…) : on reste dans la vue
            if url.fragment != nil, let current = webView.url,
               url.absoluteString.components(separatedBy: "#").first == current.absoluteString.components(separatedBy: "#").first {
                return .allow
            }
            NSWorkspace.shared.open(url)
            return .cancel
        }
    }
}

enum ReaderTemplate {
    /// `width` : "narrow" (680 pt, confortable à lire), "wide" (960 pt) ou "full" (toute la largeur)
    /// `fontSize` : taille du texte en points (le titre et les légendes suivent en proportion)
    /// `forPrint` : mise en page claire pour le papier, avec l'adresse de l'article à la fin
    static func render(_ article: Article, body: String, width: String = "narrow", fontSize: Int = 17,
                       forPrint: Bool = false) -> String {
        let maxWidth = switch width {
        case "full": "none"
        case "wide": "960px"
        default: "680px"
        }
        var meta = [Optional(article.sourceName), article.author]
            .compactMap { $0 }
            .filter { !$0.isEmpty }
            .map(HTMLUtils.escape)
        meta.append(article.published.formatted(Date.FormatStyle(date: .long, time: .shortened, locale: .app)))

        var hero = ""
        if let image = article.imageURL, let imageURL = URL(string: image) {
            let fileName = imageURL.lastPathComponent
            let alreadyInBody = fileName.count > 4 && body.contains(fileName)
            if !alreadyInBody {
                hero = "<img class=\"hero\" src=\"\(HTMLUtils.escape(ImageCache.proxyURL(for: imageURL)))\" alt=\"\">"
            }
        }
        // Toutes les images passent par le cache disque (disponibles hors ligne)
        let body = HTMLUtils.rewriteImagesForCache(body, base: article.link.flatMap { URL(string: $0) })
        var source = ""
        if forPrint, let link = article.link, !link.isEmpty {
            source = "<div class=\"source\">\(HTMLUtils.escape(String(localized: "Source :"))) \(HTMLUtils.escape(link))</div>"
        }

        return """
        <!doctype html>
        <html lang="\(Locale.app.language.languageCode?.identifier ?? "en")">
        <head>
        <meta charset="utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <style>\(css) article { max-width: \(maxWidth); } html { font-size: \(fontSize)px; }\(forPrint ? printCSS : "")</style>
        </head>
        <body>
        <article>
          <div class="meta">\(meta.joined(separator: " · "))</div>
          <h1 class="title">\(HTMLUtils.escape(article.title))</h1>
          \(hero)
          <div class="content">\(body)</div>
          \(source)
        </article>
        </body>
        </html>
        """
    }

    /// Impression : toujours en clair (même en mode sombre), sans marges propres à l'écran,
    /// sans couper les images ni les blocs en deux
    private static let printCSS = """
    :root { color-scheme: light; --text: #1d1d1f; --muted: #6e6e73; --link: #0a66d8; --rule: rgba(0,0,0,.12); --code: rgba(0,0,0,.05); }
    body { padding: 0; background: #fff; }
    article { max-width: none; }
    .hero { max-height: 320px; }
    img, figure, pre, blockquote, table { break-inside: avoid; }
    h1, h2, h3 { break-after: avoid; }
    pre { white-space: pre-wrap; overflow: visible; }
    .source { margin-top: 2.4em; padding-top: .8em; border-top: 1px solid var(--rule); color: var(--muted); font-size: .76rem; word-break: break-all; }
    """

    private static let css = """
    :root { color-scheme: light dark; --text: #1d1d1f; --muted: #6e6e73; --link: #0a66d8; --rule: rgba(0,0,0,.1); --code: rgba(0,0,0,.05); }
    @media (prefers-color-scheme: dark) {
      :root { --text: #e8e8ed; --muted: #98989f; --link: #5eaaff; --rule: rgba(255,255,255,.14); --code: rgba(255,255,255,.07); }
    }
    html { -webkit-text-size-adjust: 100%; }
    body {
      margin: 0; padding: 44px 48px 96px;
      font: 1rem/1.68 -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;
      color: var(--text); background: transparent;
      -webkit-font-smoothing: antialiased;
    }
    article { max-width: 680px; margin: 0 auto; }
    .meta { color: var(--muted); font-size: .71rem; font-weight: 600; letter-spacing: .06em; text-transform: uppercase; }
    h1.title { font: 700 1.88rem/1.18 "New York", ui-serif, Georgia, serif; letter-spacing: -.01em; margin: 10px 0 22px; }
    .hero { display: block; width: 100%; max-height: 440px; object-fit: cover; border-radius: 14px; margin: 0 0 28px; }
    .content h1, .content h2, .content h3 { line-height: 1.3; margin: 1.6em 0 .5em; }
    .content h1 { font-size: 1.5em; } .content h2 { font-size: 1.3em; } .content h3 { font-size: 1.1em; }
    .content p { margin: 0 0 1.1em; }
    a { color: var(--link); text-decoration: none; }
    a:hover { text-decoration: underline; }
    img, video, iframe, svg { max-width: 100%; height: auto; border-radius: 10px; }
    figure { margin: 1.8em 0; }
    figcaption { color: var(--muted); font-size: .76rem; line-height: 1.45; margin-top: 8px; text-align: center; }
    blockquote { margin: 1.4em 0; padding: 2px 0 2px 18px; border-left: 3px solid var(--rule); color: var(--muted); font-style: italic; }
    hr { border: 0; border-top: 1px solid var(--rule); margin: 2em 0; }
    pre, code { font: .82rem/1.5 ui-monospace, "SF Mono", Menlo, monospace; }
    code { background: var(--code); padding: .1em .35em; border-radius: 5px; }
    pre { background: var(--code); padding: 14px 16px; border-radius: 10px; overflow-x: auto; }
    pre code { background: none; padding: 0; }
    table { border-collapse: collapse; width: 100%; font-size: .88rem; }
    td, th { border-bottom: 1px solid var(--rule); padding: 6px 8px; text-align: left; }
    ul, ol { padding-left: 1.4em; }
    """
}
