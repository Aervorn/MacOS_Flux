import AppKit
import SwiftData
import SwiftUI

/// Contenu d'un menu « Tags » : un tag est coché quand tous les articles le portent.
/// Tout est passé en paramètre (pas de @Query ni d'environnement) : le contenu d'un
/// menu contextuel n'hérite pas toujours de l'environnement de la vue.
struct TagMenuContent: View {
    let tags: [FavoriteTag]
    let articles: [Article]
    let store: FeedStore
    /// « Nouveau tag… » : la vue parente affiche la fenêtre de saisie (`newTagAlert`)
    let onNewTag: () -> Void

    var body: some View {
        ForEach(tags) { tag in
            let all = !articles.isEmpty && articles.allSatisfy { $0.tagUIDs.contains(tag.uid) }
            Toggle(tag.name, isOn: Binding(
                get: { all },
                set: { store.setTag(tag.uid, $0, articles) }
            ))
        }
        if !tags.isEmpty { Divider() }
        Button("Nouveau tag…") { onNewTag() }
    }
}

/// Fenêtre « Nouveau tag » : crée le tag et l'ajoute aux articles donnés (aucun : création seule)
private struct NewTagAlert: ViewModifier {
    @Binding var articles: [Article]?
    @Environment(FeedStore.self) private var store
    @State private var name = ""

    func body(content: Content) -> some View {
        content.alert("Nouveau tag", isPresented: Binding(
            get: { articles != nil },
            set: { if !$0 { articles = nil } }
        )) {
            TextField("Nom du tag", text: $name)
            Button("Créer") {
                let targets = articles ?? []
                if let tag = store.createTag(named: name), !targets.isEmpty {
                    store.setTag(tag.uid, true, targets)
                }
                name = ""
            }
            .keyboardShortcut(.defaultAction)
            Button("Annuler", role: .cancel) { name = "" }
        } message: {
            if let articles, !articles.isEmpty {
                Text("Le tag est ajouté à l'article, qui rejoint les favoris s'il n'y était pas.")
            }
        }
    }
}

extension View {
    /// `articles` non nil : affiche la fenêtre « Nouveau tag » pour ces articles
    func newTagAlert(for articles: Binding<[Article]?>) -> some View {
        modifier(NewTagAlert(articles: articles))
    }
}

// MARK: - Tags affichés sous un favori

/// Tags d'un article sous forme de pastilles, sur plusieurs lignes si besoin.
/// La largeur de chaque pastille et le retour à la ligne sont calculés d'avance
/// (`height(_:maxWidth:)`), pour que la liste connaisse la hauteur exacte de chaque ligne.
struct TagChips: View {
    let names: [String]
    let maxWidth: CGFloat

    static let fontSize: CGFloat = 11
    static let chipHeight: CGFloat = 18
    static let spacing: CGFloat = 4
    private static let horizontalPadding: CGFloat = 7
    private static let font = NSFont.systemFont(ofSize: fontSize, weight: .medium)

    var body: some View {
        TagFlowLayout(maxWidth: maxWidth) {
            ForEach(Array(names.enumerated()), id: \.offset) { _, name in
                Text(verbatim: name)
                    .font(.system(size: Self.fontSize, weight: .medium))
                    .lineLimit(1)
                    .truncationMode(.tail)
                    .padding(.horizontal, Self.horizontalPadding)
                    .frame(width: Self.width(of: name, maxWidth: maxWidth), height: Self.chipHeight)
                    .foregroundStyle(.tint)
                    .background(Capsule().fill(Color.accentColor.opacity(0.14)))
            }
        }
    }

    /// Largeur d'une pastille (texte mesuré + marges), sans dépasser la ligne
    static func width(of name: String, maxWidth: CGFloat) -> CGFloat {
        let text = (name as NSString).size(withAttributes: [.font: font]).width
        return min(ceil(text) + 2 * horizontalPadding + 2, maxWidth)
    }

    /// Position de chaque pastille (même calcul pour la mise en page et pour la hauteur)
    static func arrange(_ widths: [CGFloat], maxWidth: CGFloat) -> (origins: [CGPoint], lines: Int) {
        var origins: [CGPoint] = []
        var x: CGFloat = 0
        var line = 0
        for width in widths {
            if x > 0 && x + width > maxWidth {
                line += 1
                x = 0
            }
            origins.append(CGPoint(x: x, y: CGFloat(line) * (chipHeight + spacing)))
            x += width + spacing
        }
        return (origins, widths.isEmpty ? 0 : line + 1)
    }

    /// Hauteur occupée par les tags (0 sans tag)
    static func height(_ names: [String], maxWidth: CGFloat) -> CGFloat {
        let lines = arrange(names.map { width(of: $0, maxWidth: maxWidth) }, maxWidth: maxWidth).lines
        return lines == 0 ? 0 : CGFloat(lines) * chipHeight + CGFloat(lines - 1) * spacing
    }
}

/// Place les pastilles de gauche à droite et passe à la ligne quand `maxWidth` est atteint
private struct TagFlowLayout: Layout {
    let maxWidth: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let widths = subviews.map { $0.sizeThatFits(.unspecified).width }
        let (origins, lines) = TagChips.arrange(widths, maxWidth: maxWidth)
        let height = lines == 0 ? 0 : CGFloat(lines) * TagChips.chipHeight + CGFloat(lines - 1) * TagChips.spacing
        // Largeur réellement occupée (la ligne la plus longue)
        let width = zip(origins, widths).map { $0.x + $1 }.max() ?? 0
        return CGSize(width: width, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let sizes = subviews.map { $0.sizeThatFits(.unspecified) }
        let origins = TagChips.arrange(sizes.map(\.width), maxWidth: maxWidth).origins
        for (index, subview) in subviews.enumerated() {
            subview.place(at: CGPoint(x: bounds.minX + origins[index].x, y: bounds.minY + origins[index].y),
                          anchor: .topLeading, proposal: ProposedViewSize(sizes[index]))
        }
    }
}
