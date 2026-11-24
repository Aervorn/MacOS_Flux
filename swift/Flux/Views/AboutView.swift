import AppKit
import SwiftUI

/// Entrée « À propos de Flux » du menu de l'application
struct AboutMenuItem: View {
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Button("À propos de Flux") { openWindow(id: "about") }
    }
}

struct AboutView: View {
    static let repositoryURL = URL(string: "https://github.com/Aervorn/MacOS_Flux")!
    static let readabilityURL = URL(string: "https://github.com/mozilla/readability")!

    private var version: String {
        let info = Bundle.main.infoDictionary
        let short = info?["CFBundleShortVersionString"] as? String ?? "?"
        let build = info?["CFBundleVersion"] as? String ?? "?"
        return String(localized: "Version \(short) (\(build))")
    }

    var body: some View {
        VStack(spacing: 0) {
            Image(nsImage: NSApp.applicationIconImage)
                .resizable()
                .frame(width: 112, height: 112)
                .padding(.top, 8)

            Text("Flux")
                .font(.system(size: 26, weight: .bold))
                .padding(.top, 6)
            Text(version)
                .font(.callout)
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
                .padding(.top, 2)

            Text("Un lecteur RSS simple pour macOS : tes flux, des miniatures, et l'article complet sans ouvrir le navigateur.")
                .font(.callout)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 14)

            Divider().padding(.vertical, 16)

            VStack(spacing: 6) {
                Text("Créé par Aervorn, avec l'aide de Claude.")
                HStack(spacing: 4) {
                    Text("Extraction des articles :")
                    Link("Readability.js", destination: Self.readabilityURL)
                    Text("(Mozilla, Apache 2.0)")
                }
            }
            .font(.caption)
            .foregroundStyle(.secondary)

            Link(destination: Self.repositoryURL) {
                Label("Code source sur GitHub", systemImage: "chevron.left.forwardslash.chevron.right")
            }
            .font(.callout)
            .padding(.top, 14)
        }
        .padding(.horizontal, 28)
        .padding(.bottom, 24)
        .padding(.top, 12)
        .frame(width: 340)
    }
}
