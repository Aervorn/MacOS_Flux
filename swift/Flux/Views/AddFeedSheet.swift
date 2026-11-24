import SwiftData
import SwiftUI

struct AddFeedSheet: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(FeedStore.self) private var store
    @Query(sort: \Folder.name) private var folders: [Folder]

    @State private var address = ""
    @State private var folderUID: String?
    @State private var working = false
    @State private var errorMessage: String?

    init(initialFolderUID: String? = nil) {
        _folderUID = State(initialValue: initialFolderUID)
    }

    private var canSubmit: Bool {
        !address.trimmingCharacters(in: .whitespaces).isEmpty && !working
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Ajouter un flux")
                .font(.title2.bold())
            Text("Colle l'adresse du flux RSS ou Atom, ou simplement celle du site : le flux sera détecté automatiquement.")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            TextField("https://exemple.fr/feed.xml", text: $address)
                .textFieldStyle(.roundedBorder)
                .onSubmit { if canSubmit { add() } }
                .disabled(working)

            if !folders.isEmpty {
                Picker("Dossier", selection: $folderUID) {
                    Text("Aucun").tag(String?.none)
                    Divider()
                    ForEach(folders) { folder in
                        Text(folder.name).tag(Optional(folder.uid))
                    }
                }
                .disabled(working)
            }

            if let errorMessage {
                Text(errorMessage)
                    .font(.callout)
                    .foregroundStyle(.red)
            }

            HStack {
                Spacer()
                Button("Annuler", role: .cancel) { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Button {
                    add()
                } label: {
                    if working {
                        ProgressView().controlSize(.small).frame(width: 50)
                    } else {
                        Text("Ajouter").frame(width: 50)
                    }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(!canSubmit)
            }
        }
        .padding(20)
        .frame(width: 460)
    }

    private func add() {
        Task {
            working = true
            errorMessage = nil
            do {
                let folder = folders.first { $0.uid == folderUID }
                try await store.addFeed(urlString: address, folder: folder)
                dismiss()
            } catch {
                errorMessage = error.localizedDescription
            }
            working = false
        }
    }
}
