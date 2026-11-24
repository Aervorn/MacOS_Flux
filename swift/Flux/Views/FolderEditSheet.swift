import SwiftUI

/// Personnalisation d'un dossier : nom, couleur et icône (SF Symbols).
struct FolderEditSheet: View {
    let folder: Folder

    @Environment(\.dismiss) private var dismiss
    @Environment(FeedStore.self) private var store

    @State private var name: String
    @State private var symbol: String
    @State private var colorName: String?

    init(folder: Folder) {
        self.folder = folder
        _name = State(initialValue: folder.name)
        _symbol = State(initialValue: folder.symbolName)
        _colorName = State(initialValue: folder.colorName)
    }

    private var tint: Color {
        colorName.flatMap(FolderColor.init(rawValue:))?.color ?? .accentColor
    }

    static let symbols: [String] = [
        "folder", "tray", "archivebox", "newspaper", "book", "books.vertical", "bookmark", "tag",
        "star", "heart", "flag", "bolt", "flame", "sparkles", "lightbulb", "megaphone",
        "globe.europe.africa", "map", "house", "building.2", "briefcase", "graduationcap", "person.2", "bubble.left.and.bubble.right",
        "laptopcomputer", "desktopcomputer", "iphone", "applelogo", "cpu", "terminal", "chevron.left.forwardslash.chevron.right", "network",
        "wifi", "antenna.radiowaves.left.and.right", "lock", "shield", "wrench.and.screwdriver", "hammer", "atom", "flask",
        "gamecontroller", "puzzlepiece", "film", "tv", "music.note", "headphones", "camera", "photo",
        "paintpalette", "pencil", "theatermasks", "leaf", "pawprint", "sun.max", "cloud.sun", "cart",
        "creditcard", "chart.line.uptrend.xyaxis", "car", "airplane", "bicycle", "figure.run", "sportscourt", "fork.knife",
    ]

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            // Aperçu + nom
            HStack(spacing: 14) {
                Image(systemName: symbol)
                    .font(.system(size: 26, weight: .medium))
                    .foregroundStyle(tint)
                    .frame(width: 52, height: 52)
                    .background(tint.opacity(0.15), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                TextField("Nom du dossier", text: $name)
                    .textFieldStyle(.roundedBorder)
                    .font(.title3)
            }

            // Couleur
            VStack(alignment: .leading, spacing: 8) {
                Text("Couleur").font(.headline)
                HStack(spacing: 4) {
                    colorSwatch(nil, color: .accentColor)
                    ForEach(FolderColor.allCases) { c in
                        colorSwatch(c.rawValue, color: c.color)
                    }
                }
            }

            // Icône
            VStack(alignment: .leading, spacing: 8) {
                Text("Icône").font(.headline)
                LazyVGrid(columns: Array(repeating: GridItem(.fixed(36), spacing: 6), count: 8), spacing: 6) {
                    ForEach(Self.symbols, id: \.self) { s in
                        Button {
                            symbol = s
                        } label: {
                            Image(systemName: s)
                                .font(.system(size: 15))
                                .frame(width: 36, height: 32)
                                .foregroundStyle(symbol == s ? .white : .primary)
                                .background {
                                    RoundedRectangle(cornerRadius: 7, style: .continuous)
                                        .fill(symbol == s ? AnyShapeStyle(tint) : AnyShapeStyle(HierarchicalShapeStyle.quaternary.opacity(0.6)))
                                }
                                .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .help(s)
                    }
                }
            }

            HStack {
                Spacer()
                Button("Annuler", role: .cancel) { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Button("Enregistrer") {
                    store.update(folder, name: name, symbol: symbol, colorName: colorName)
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
                .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(20)
        .frame(width: 400)
    }

    /// Pastille de couleur ; `nil` = couleur d'accent du système
    private func colorSwatch(_ value: String?, color: Color) -> some View {
        let selected = colorName == value
        return Button {
            colorName = value
        } label: {
            Circle()
                .fill(color)
                .frame(width: 16, height: 16)
                .padding(3)
                .overlay {
                    Circle().strokeBorder(selected ? Color.primary.opacity(0.6) : .clear, lineWidth: 2)
                }
        }
        .buttonStyle(.plain)
        .help(value.flatMap(FolderColor.init(rawValue:))?.label ?? String(localized: "Couleur d'accent du système"))
    }
}
