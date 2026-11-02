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
