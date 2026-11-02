import AppKit
import SwiftData
import SwiftUI

struct ArticleDetailView: View {
    let article: Article
    /// Ouvre un autre article (les autres sources de la même actualité)
    var onSelectArticle: ((String) -> Void)? = nil

    @State private var showFull = false
    @Environment(FeedStore.self) private var store
    @AppStorage(Prefs.readerWidth) private var readerWidth = "narrow"
    @AppStorage(Prefs.readerFontSize) private var readerFontSize = 17
    @State private var loading = false
    @State private var errorMessage: String?
    /// « Nouveau tag… » depuis la barre d'outils
    @State private var tagTargets: [Article]?
    @Query(sort: \FavoriteTag.name) private var tags: [FavoriteTag]

    private var articleURL: URL? { article.link.flatMap { URL(string: $0) } }

    private var bodyHTML: String {
        if showFull, let full = article.fullHTML { return full }
        if let content = article.contentHTML, !content.isEmpty { return content }
        return "<p>\(HTMLUtils.escape(article.summary))</p>"
    }

    var body: some View {
        VStack(spacing: 0) {
            let similar = store.similarArticles(to: article)
            if !similar.isEmpty {
                SimilarArticlesBar(articles: similar) { onSelectArticle?($0) }
                Divider()
            }
            reader
        }
    }

    private var reader: some View {
        ArticleWebView(html: ReaderTemplate.render(article, body: bodyHTML, width: readerWidth, fontSize: readerFontSize),
                       baseURL: articleURL,
                       contentID: article.key + (showFull ? "|full" : "|feed"))
            .overlay(alignment: .top) {
                if loading {
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text("Récupération de l'article complet…")
                    }
                    .font(.callout)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 8)
                    .background(.regularMaterial, in: Capsule())
                    .padding(.top, 12)
                    .transition(.opacity)
                }
            }
            .overlay(alignment: .bottom) {
                if let errorMessage {
                    Label(errorMessage, systemImage: "exclamationmark.triangle")
                        .font(.callout)
                        .padding(.horizontal, 14)
                        .padding(.vertical, 8)
                        .background(.regularMaterial, in: Capsule())
                        .padding(.bottom, 16)
                }
            }
            .animation(.easeInOut(duration: 0.2), value: loading)
            .toolbar {
                ToolbarItem {
                    Menu {
                        Button {
                            readerFontSize = min(readerFontSize + 1, Prefs.readerFontSizes.upperBound)
                        } label: {
                            Label("Agrandir le texte", systemImage: "textformat.size.larger")
                        }
                        Button {
                            readerFontSize = max(readerFontSize - 1, Prefs.readerFontSizes.lowerBound)
                        } label: {
                            Label("Réduire le texte", systemImage: "textformat.size.smaller")
                        }
                        Button("Taille par défaut") { readerFontSize = 17 }
                            .disabled(readerFontSize == 17)
                        Divider()
                        Picker("Largeur du texte", selection: $readerWidth) {
                            Label("Étroite", systemImage: "rectangle.portrait").tag("narrow")
                            Label("Large", systemImage: "rectangle").tag("wide")
                            Label("Toute la largeur", systemImage: "arrow.left.and.right").tag("full")
                        }
                        .pickerStyle(.inline)
                    } label: {
                        Label("Texte", systemImage: "textformat.size")
                    }
                    .help("Taille et largeur du texte (⌘+ ou ⌘= / ⌘−)")
                }
                ToolbarItemGroup {
                    Button {
                        ArticleExport.exportPDF(article, body: bodyHTML)
                    } label: {
                        Label("Exporter en PDF", systemImage: "arrow.down.doc")
                    }
                    .help("Exporter l'article en PDF (⌥⌘E)")

                    Button {
                        toggleFull()
                    } label: {
                        Label(showFull ? String(localized: "Version du flux") : String(localized: "Article complet"),
                              systemImage: showFull ? "doc.plaintext" : "doc.richtext")
                    }
                    .help(showFull ? String(localized: "Afficher le contenu fourni par le flux") : String(localized: "Récupérer l'article complet depuis le site"))
                    .disabled(articleURL == nil || loading)

                    Button {
                        store.setStarred(!article.isStarred, [article])
                    } label: {
                        Label("Favori", systemImage: article.isStarred ? "star.fill" : "star")
                    }
                    .help(article.isStarred ? String(localized: "Retirer des favoris") : String(localized: "Ajouter aux favoris"))

                    Menu {
                        TagMenuContent(tags: tags, articles: [article], store: store) { tagTargets = [article] }
                    } label: {
                        Label("Tags", systemImage: article.tagUIDs.isEmpty ? "tag" : "tag.fill")
                    }
                    .help("Tags du favori")

                    if let url = articleURL {
                        ShareLink(item: url)
                        Button {
                            NSWorkspace.shared.open(url)
                        } label: {
                            Label("Ouvrir dans le navigateur", systemImage: "safari")
                        }
                        .help("Ouvrir dans le navigateur")
                    }
                }
            }
            .task { await prepare() }
            .newTagAlert(for: $tagTargets)
            // Menu Fichier › Imprimer… / Exporter en PDF… (contenu affiché : extrait ou article complet)
            .focusedSceneValue(\.articleExport, ArticleExportActions(
                print: { ArticleExport.printArticle(article, body: bodyHTML) },
                exportPDF: { ArticleExport.exportPDF(article, body: bodyHTML) }))
    }

    /// Si le flux ne fournit qu'un extrait, on va chercher l'article complet.
    private func prepare() async {
        if article.fullHTML != nil {
            showFull = true
            return
        }
        // Le flux ne donne qu'un extrait (court, ou tronqué par « … » / « Lire la suite ») : on va chercher l'article
        if HTMLUtils.looksLikeExcerpt(article.contentHTML), articleURL != nil {
            await loadFull()
        }
    }

    private func toggleFull() {
        if showFull {
            showFull = false
        } else if article.fullHTML != nil {
            showFull = true
        } else {
            Task { await loadFull() }
        }
    }

    private func loadFull() async {
        guard let url = articleURL else { return }
        loading = true
        errorMessage = nil
        defer { loading = false }
        do {
            let content = try await FullTextLoader.load(url)
            guard !Task.isCancelled else { return }
            if let content {
                article.fullHTML = content
                article.updateSearchText()
                if article.imageURL == nil {
                    article.imageURL = HTMLUtils.firstImage(in: content)
                }
                showFull = true
            } else {
                errorMessage = String(localized: "Impossible d'extraire le contenu de cette page.")
            }
        } catch {
            guard !Task.isCancelled else { return }
            errorMessage = String(localized: "Échec du téléchargement : \(error.localizedDescription)")
        }
    }
}

/// « Aussi publié par » : les autres sources de la même actualité
struct SimilarArticlesBar: View {
    let articles: [Article]
    let onSelect: (String) -> Void

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "square.stack")
                .foregroundStyle(.secondary)
            Text("Aussi publié par")
                .foregroundStyle(.secondary)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(articles) { article in
                        Button {
                            onSelect(article.key)
                        } label: {
                            Text(article.sourceName)
                                .lineLimit(1)
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .help(article.title)
                    }
                }
            }
        }
        .font(.callout)
        .padding(.horizontal, 14)
        .padding(.vertical, 7)
    }
}

