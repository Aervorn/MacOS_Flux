import AppKit
import SwiftUI
import UniformTypeIdentifiers
import WebKit

/// Impression et export PDF d'un article.
///
/// On ne passe pas par la vue de lecture : elle suit le mode sombre (texte clair sur fond
/// transparent, illisible sur papier) et sa largeur dépend de la fenêtre. L'article est
/// rendu dans une WKWebView hors écran, en clair et mis en page pour l'impression, puis
/// imprimé par NSPrintOperation (vraies pages A4/Lettre, pas une seule longue page).
@MainActor
enum ArticleExport {
    static func printArticle(_ article: Article, body: String) {
        Task {
            let renderer = PrintRenderer()
            await renderer.load(printHTML(article, body: body), baseURL: article.link.flatMap { URL(string: $0) })
            renderer.run(title: article.title, saveTo: nil)
        }
    }

    static func exportPDF(_ article: Article, body: String) {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.pdf]
        panel.canCreateDirectories = true
        panel.nameFieldStringValue = fileName(for: article.title) + ".pdf"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        Task {
            let renderer = PrintRenderer()
            await renderer.load(printHTML(article, body: body), baseURL: article.link.flatMap { URL(string: $0) })
            renderer.run(title: article.title, saveTo: url)
        }
    }

    private static func printHTML(_ article: Article, body: String) -> String {
        // Hors écran, les images en chargement différé ne seraient jamais chargées
        let body = body
            .replacingOccurrences(of: "loading=\"lazy\"", with: "")
            .replacingOccurrences(of: "loading='lazy'", with: "")
        return ReaderTemplate.render(article, body: body, width: "full", fontSize: 14, forPrint: true)
    }

    /// Titre de l'article utilisable comme nom de fichier
    private static func fileName(for title: String) -> String {
        let cleaned = title
            .components(separatedBy: CharacterSet(charactersIn: "/:\\?%*|\"<>").union(.newlines))
            .joined(separator: "-")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let short = String(cleaned.prefix(100))
        return short.isEmpty ? "Article" : short
    }
}

/// WebView hors écran qui charge l'article puis lance l'impression.
/// Se garde en vie (`active`) jusqu'à la fin de l'impression.
@MainActor
private final class PrintRenderer: NSObject, WKNavigationDelegate {
    private static var active: Set<PrintRenderer> = []

    private let webView: WKWebView
    private let imageHandler: CachedImageSchemeHandler
    private var continuation: CheckedContinuation<Void, Never>?

    override init() {
        let handler = CachedImageSchemeHandler()
        let config = WKWebViewConfiguration()
        config.defaultWebpagePreferences.allowsContentJavaScript = false
        config.setURLSchemeHandler(handler, forURLScheme: ImageCache.scheme)
        imageHandler = handler
        webView = WKWebView(frame: NSRect(x: 0, y: 0, width: 640, height: 900), configuration: config)
        super.init()
        webView.navigationDelegate = self
        PrintRenderer.active.insert(self)
    }

    /// Charge le HTML et attend la fin du chargement (images comprises), 20 s au plus
    func load(_ html: String, baseURL: URL?) async {
        await withCheckedContinuation { (c: CheckedContinuation<Void, Never>) in
            continuation = c
            webView.loadHTMLString(html, baseURL: baseURL)
            Task { @MainActor [weak self] in
                try? await Task.sleep(for: .seconds(20))
                self?.finishLoading()
            }
        }
    }

    private func finishLoading() {
        continuation?.resume()
        continuation = nil
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { finishLoading() }
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { finishLoading() }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        finishLoading()
    }

    /// `saveTo` : enregistre directement en PDF ; nil : affiche la fenêtre d'impression
    func run(title: String, saveTo url: URL?) {
        let info = (NSPrintInfo.shared.copy() as? NSPrintInfo) ?? NSPrintInfo()
        info.horizontalPagination = .fit
        info.verticalPagination = .automatic
        info.isHorizontallyCentered = false
        info.isVerticallyCentered = false
        info.topMargin = 40
        info.bottomMargin = 40
        info.leftMargin = 44
        info.rightMargin = 44
        if let url {
            info.jobDisposition = .save
            info.dictionary().setObject(url, forKey: NSPrintInfo.AttributeKey.jobSavingURL.rawValue as NSString)
        }

        let operation = webView.printOperation(with: info)
        operation.jobTitle = title
        operation.showsPrintPanel = url == nil
        operation.showsProgressPanel = false
        // Sans cadre, WebKit imprime des pages blanches
        operation.view?.frame = webView.bounds

        if let window = NSApp.keyWindow ?? NSApp.mainWindow {
            operation.runModal(for: window, delegate: self,
                               didRun: #selector(printOperationDidRun(_:success:contextInfo:)), contextInfo: nil)
        } else {
            operation.run()
            PrintRenderer.active.remove(self)
        }
    }

    @objc private func printOperationDidRun(_ operation: NSPrintOperation, success: Bool,
                                            contextInfo: UnsafeMutableRawPointer?) {
        PrintRenderer.active.remove(self)
    }
}

// MARK: - Menu Fichier

/// Actions sur l'article affiché, publiées par ArticleDetailView pour le menu Fichier
struct ArticleExportActions {
    let print: () -> Void
    let exportPDF: () -> Void
}

private struct ArticleExportKey: FocusedValueKey {
    typealias Value = ArticleExportActions
}

extension FocusedValues {
    var articleExport: ArticleExportActions? {
        get { self[ArticleExportKey.self] }
        set { self[ArticleExportKey.self] = newValue }
    }
}

/// « Exporter en PDF… » et « Imprimer… » (remplace l'élément Imprimer standard)
struct ArticleExportCommands: View {
    @FocusedValue(\.articleExport) private var actions

    var body: some View {
        Button("Exporter en PDF…") { actions?.exportPDF() }
            .keyboardShortcut("e", modifiers: [.command, .option])
            .disabled(actions == nil)
        Button("Imprimer…") { actions?.print() }
            .keyboardShortcut("p")
            .disabled(actions == nil)
    }
}
