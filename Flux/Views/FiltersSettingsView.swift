import SwiftData
import SwiftUI

/// Onglet Réglages › Filtres : règles d'exclusion par mot-clé dans le titre
struct FiltersSettingsView: View {
    @Environment(FeedStore.self) private var store
    @Query(sort: \FilterRule.createdAt) private var rules: [FilterRule]
    @Query private var feeds: [Feed]

    @State private var editing: FilterRule?
    @State private var showNew = false
    /// Recalcul des compteurs après une modification
    @State private var refreshToken = 0

    var body: some View {
        Form {
            Section {
                if rules.isEmpty {
                    Text("Aucune règle. Ajoute un mot-clé pour masquer les articles dont le titre le contient.")
                        .foregroundStyle(.secondary)
                }
                ForEach(rules) { rule in
                    row(rule)
                }
            } header: {
                HStack {
                    Text("Règles")
                    Spacer()
                    Button {
                        showNew = true
                    } label: {
                        Label("Ajouter une règle", systemImage: "plus")
                    }
                    .buttonStyle(.borderless)
                }
            } footer: {
                Text("Les règles portent sur le titre, sans tenir compte des majuscules ni des accents. Elles s'appliquent aux articles existants et à ceux qui arrivent. Les favoris ne sont jamais filtrés. Supprimer une règle fait réapparaître les articles qu'elle masquait.")
                    .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
        // Hauteur selon le nombre de règles (une ligne ≈ 52 pt)
        .settingsTab(idealHeight: 175 + CGFloat(max(rules.count, 1) - 1) * 52 + (rules.isEmpty ? 0 : 8))
        .sheet(isPresented: $showNew) {
            FilterRuleSheet(rule: nil, feeds: sortedFeeds) { keyword, wholeWord, action, feedURLs in
                store.addFilter(keyword: keyword, wholeWord: wholeWord, action: action, feedURLs: feedURLs)
                refreshToken += 1
            }
        }
        .sheet(item: $editing) { rule in
            FilterRuleSheet(rule: rule, feeds: sortedFeeds) { keyword, wholeWord, action, feedURLs in
                store.updateFilter(rule, keyword: keyword, wholeWord: wholeWord, action: action, feedURLs: feedURLs)
                refreshToken += 1
            }
        }
    }

    private var sortedFeeds: [Feed] {
        feeds.sorted { $0.displayTitle.localizedStandardCompare($1.displayTitle) == .orderedAscending }
    }

    /// « Tous les flux », le nom du ou des deux flux, ou « N flux »
    private func scopeLabel(_ rule: FilterRule) -> String {
        guard !rule.feedURLs.isEmpty else { return String(localized: "Tous les flux") }
        let names = rule.feedURLs
            .map { url in feeds.first { $0.url == url }?.displayTitle ?? URL(string: url)?.host() ?? url }
            .sorted { $0.localizedStandardCompare($1) == .orderedAscending }
        return names.count <= 2 ? names.joined(separator: ", ") : String(localized: "\(names.count) flux")
    }

    private func row(_ rule: FilterRule) -> some View {
        let count = store.matchCount(of: rule)
        let scope = scopeLabel(rule)
        var details = [scope, rule.filterAction.label]
        if rule.wholeWord { details.insert(String(localized: "Mot entier"), at: 0) }

        return HStack(spacing: 10) {
            Image(systemName: rule.filterAction == .hide ? "eye.slash" : "checkmark.circle")
                .foregroundStyle(.secondary)
                .frame(width: 18)
            VStack(alignment: .leading, spacing: 2) {
                Text(rule.keyword).font(.body.weight(.semibold))
                Text(details.joined(separator: " · "))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Text(count == 0 ? String(localized: "Aucun article") : String(localized: "\(count) articles"))
                .font(.callout)
                .foregroundStyle(.secondary)
                .monospacedDigit()
                .id(refreshToken)
            Button {
                editing = rule
            } label: {
                Image(systemName: "pencil")
            }
            .buttonStyle(.borderless)
            .help("Modifier la règle")
            Button(role: .destructive) {
                store.deleteFilter(rule)
                refreshToken += 1
            } label: {
                Image(systemName: "trash")
            }
            .buttonStyle(.borderless)
            .help("Supprimer la règle")
        }
        .contentShape(Rectangle())
        .onTapGesture(count: 2) { editing = rule }
    }
}

/// Création ou modification d'une règle
struct FilterRuleSheet: View {
    let rule: FilterRule?
    let feeds: [Feed]
    /// Flux présélectionné pour une nouvelle règle (création depuis le menu d'un flux)
    var initialFeedURL: String? = nil
    /// Mot-clé, mot entier, action, flux concernés (vide = tous)
    let onSave: (String, Bool, FilterAction, [String]) -> Void

    @Environment(\.dismiss) private var dismiss
    @Environment(FeedStore.self) private var store

    @State private var keyword = ""
    @State private var wholeWord = true
    @State private var action: FilterAction = .hide
    /// true = tous les flux, false = seulement ceux cochés
    @State private var allFeeds = true
    @State private var selectedFeeds: Set<String> = []
    @State private var feedSearch = ""

    /// Flux concernés tels qu'enregistrés (vide = tous)
    private var scope: [String] { allFeeds ? [] : selectedFeeds.sorted() }

    private var canSave: Bool {
        !keyword.trimmingCharacters(in: .whitespaces).isEmpty && (allFeeds || !selectedFeeds.isEmpty)
    }

    private var visibleFeeds: [Feed] {
        let query = feedSearch.trimmingCharacters(in: .whitespaces)
        guard !query.isEmpty else { return feeds }
        let folded = CompiledFilter.fold(query)
        return feeds.filter { CompiledFilter.fold($0.displayTitle).contains(folded) }
    }

    /// Aperçu : nombre d'articles actuels dont le titre correspond
    private var previewCount: Int {
        let trimmed = keyword.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty, allFeeds || !selectedFeeds.isEmpty else { return 0 }
        return store.previewMatchCount(
            CompiledFilter(keyword: trimmed, wholeWord: wholeWord, action: action, feedURLs: scope))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(rule == nil ? String(localized: "Nouvelle règle") : String(localized: "Modifier la règle"))
                .font(.title2.bold())

            Form {
                TextField("Mot-clé", text: $keyword, prompt: Text("ex. : Bitcoin"))
                Toggle("Mot entier uniquement", isOn: $wholeWord)
                Picker("Flux concernés", selection: $allFeeds) {
                    Text("Tous les flux").tag(true)
                    Text("Certains flux").tag(false)
                }
                .pickerStyle(.radioGroup)
                Picker("Action", selection: $action) {
                    ForEach(FilterAction.allCases) { a in
                        Text(a.label).tag(a)
                    }
                }
                .pickerStyle(.radioGroup)
            }
            .formStyle(.columns)

            if !allFeeds {
                feedChooser
            }

            Text(previewCount == 0
                 ? String(localized: "Aucun article actuel ne correspond.")
                 : String(localized: "\(previewCount) articles actuels correspondent."))
                .font(.callout)
                .foregroundStyle(.secondary)

            HStack {
                Spacer()
                Button("Annuler", role: .cancel) { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Button(rule == nil ? String(localized: "Ajouter") : String(localized: "Enregistrer")) {
                    onSave(keyword, wholeWord, action, scope)
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
                .disabled(!canSave)
            }
        }
        .padding(20)
        .frame(width: 440)
        .onAppear {
            guard let rule else {
                if let initialFeedURL {
                    allFeeds = false
                    selectedFeeds = [initialFeedURL]
                }
                return
            }
            keyword = rule.keyword
            wholeWord = rule.wholeWord
            action = rule.filterAction
            allFeeds = rule.feedURLs.isEmpty
            selectedFeeds = Set(rule.feedURLs)
        }
    }

    /// Liste des flux à cocher (avec recherche quand il y en a beaucoup)
    private var feedChooser: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                if feeds.count > 8 {
                    TextField("Rechercher un flux", text: $feedSearch)
                        .textFieldStyle(.roundedBorder)
                } else {
                    Spacer()
                }
                Text(selectedFeeds.isEmpty
                     ? String(localized: "Aucun flux coché")
                     : String(localized: "\(selectedFeeds.count) flux cochés"))
                    .font(.callout)
                    .foregroundStyle(selectedFeeds.isEmpty ? AnyShapeStyle(.red) : AnyShapeStyle(.secondary))
                    .monospacedDigit()
                    .fixedSize()
            }

            ScrollView {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(visibleFeeds) { feed in
                        Toggle(isOn: Binding(
                            get: { selectedFeeds.contains(feed.url) },
                            set: { on in
                                if on { selectedFeeds.insert(feed.url) } else { selectedFeeds.remove(feed.url) }
                            }
                        )) {
                            HStack(spacing: 6) {
                                Text(feed.displayTitle).lineLimit(1)
                                if let folder = feed.folder {
                                    Text(folder.name)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                            }
                        }
                        .toggleStyle(.checkbox)
                    }
                    if visibleFeeds.isEmpty {
                        Text("Aucun flux ne correspond.")
                            .foregroundStyle(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(8)
            }
            .frame(height: min(CGFloat(max(feeds.count, 1)) * 22 + 16, 200))
            .background(.background, in: RoundedRectangle(cornerRadius: 6))
            .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(.separator))
        }
    }
}
