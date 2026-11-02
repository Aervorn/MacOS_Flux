import Foundation

/// Règles d'exclusion prêtes à être testées (mot-clé normalisé, expression compilée)
struct CompiledFilter {
    let uid: String
    /// Flux concernés (vide = tous les flux)
    let feedURLs: Set<String>
    let action: FilterAction
    private let needle: String
    private let regex: NSRegularExpression?

    init(_ rule: FilterRule) {
        self.init(uid: rule.uid, keyword: rule.keyword, wholeWord: rule.wholeWord,
                  action: rule.filterAction, feedURLs: rule.feedURLs)
    }

    init(uid: String = "", keyword: String, wholeWord: Bool, action: FilterAction, feedURLs: [String]) {
        self.uid = uid
        self.feedURLs = Set(feedURLs)
        self.action = action
        needle = CompiledFilter.fold(keyword.trimmingCharacters(in: .whitespacesAndNewlines))
        if wholeWord, !needle.isEmpty {
            // Mot entier : pas de lettre ni de chiffre juste avant ou juste après
            let pattern = "(?<![\\p{L}\\p{N}])" + NSRegularExpression.escapedPattern(for: needle) + "(?![\\p{L}\\p{N}])"
            regex = try? NSRegularExpression(pattern: pattern)
        } else {
            regex = nil
        }
    }

    /// Minuscules et sans accents : « Élection » = « election »
    static func fold(_ s: String) -> String {
        s.folding(options: [.caseInsensitive, .diacriticInsensitive, .widthInsensitive], locale: nil)
    }

    func matches(title: String, feedURL articleFeed: String) -> Bool {
        guard !needle.isEmpty else { return false }
        if !feedURLs.isEmpty, !feedURLs.contains(articleFeed) { return false }
        let folded = CompiledFilter.fold(title)
        if let regex {
            return regex.firstMatch(in: folded, range: NSRange(folded.startIndex..., in: folded)) != nil
        }
        return folded.contains(needle)
    }
}
