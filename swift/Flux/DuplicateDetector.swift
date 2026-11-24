import Foundation
import NaturalLanguage

/// Repère les articles qui parlent de la même actualité dans plusieurs sources.
///
/// Deux articles de flux différents, publiés à moins de 48 h d'écart, sont regroupés :
/// - s'ils pointent vers le même lien (republication) ;
/// - ou s'ils partagent au moins deux mots importants dans le titre et que
///   leurs titres sont assez proches, mot à mot (indice de Jaccard) ou par le sens
///   (plongements de phrases de NaturalLanguage, calculés sur le Mac).
/// Le résultat n'est jamais parfait : la sensibilité règle le compromis.
@MainActor
final class DuplicateDetector {
    enum Sensitivity: Int, CaseIterable, Identifiable {
        case low = 0, normal = 1, high = 2
        var id: Int { rawValue }

        var label: String {
            switch self {
            case .low: String(localized: "Faible (moins de regroupements, presque pas d'erreurs)")
            case .normal: String(localized: "Normale")
            case .high: String(localized: "Forte (plus de regroupements, quelques erreurs)")
            }
        }

        /// Recouvrement minimal des mots des titres, pondérés par leur rareté
        var wordThreshold: Double { [0.5, 0.4, 0.32][rawValue] }
        /// Similarité cosinus minimale entre les sens des titres
        var meaningThreshold: Double { [0.88, 0.83, 0.78][rawValue] }
    }

    static let window: TimeInterval = 48 * 3600
    /// À augmenter quand l'algorithme change : les groupes sont alors recalculés
    static let version = 2

    struct Signature {
        let words: Set<String>
        let link: String?
        let title: String
        let language: NLLanguage?
    }

    private let sensitivity: Sensitivity
    private var vectors: [String: [Double]] = [:]
    /// Nombre de titres contenant chaque mot (sur la période comparée)
    private var documentFrequency: [String: Int] = [:]
    private var documentCount = 1
    private var embeddings: [NLLanguage: NLEmbedding] = [:]

    init(sensitivity: Sensitivity) {
        self.sensitivity = sensitivity
    }

    // MARK: - Signature

    func signature(title: String, link: String?) -> Signature {
        let recognizer = NLLanguageRecognizer()
        recognizer.processString(title)
        return Signature(
            words: Self.significantWords(title),
            link: link.flatMap(Self.normalizedLink),
            title: title,
            language: recognizer.dominantLanguage
        )
    }

    /// Mots du titre sans majuscules, accents, mots courants ni pluriels simples
    static func significantWords(_ text: String) -> Set<String> {
        let folded = text.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil)
        var words = Set<String>()
        for raw in folded.split(whereSeparator: { !$0.isLetter && !$0.isNumber }) {
            var word = String(raw)
            // On garde les nombres (« iPhone 18 », « 2026 ») même courts
            let isNumber = word.allSatisfy(\.isNumber)
            guard isNumber || word.count >= 3, !stopWords.contains(word) else { continue }
            if !isNumber, word.count > 4, word.hasSuffix("s") || word.hasSuffix("x") { word.removeLast() }
            words.insert(word)
        }
        return words
    }

    /// Lien sans protocole, www, paramètres ni barre finale
    static func normalizedLink(_ link: String) -> String? {
        guard var components = URLComponents(string: link), var host = components.host?.lowercased() else { return nil }
        if host.hasPrefix("www.") { host.removeFirst(4) }
        components.query = nil
        components.fragment = nil
        var path = components.path
        while path.hasSuffix("/") { path.removeLast() }
        return host + path
    }

    // MARK: - Comparaison

    /// Mesure la fréquence des mots sur les titres de la période : un mot présent dans
    /// beaucoup de titres (« iPhone », « Apple » une semaine de keynote) ne prouve rien
    func prepare(_ signatures: [Signature]) {
        documentCount = max(signatures.count, 1)
        documentFrequency = [:]
        for signature in signatures {
            for word in signature.words { documentFrequency[word, default: 0] += 1 }
        }
    }

    /// Mot trop courant sur la période pour distinguer une actualité d'une autre
    private func isCommon(_ word: String) -> Bool {
        (documentFrequency[word] ?? 0) > max(4, documentCount / 50)
    }

    /// Poids d'un mot : d'autant plus fort qu'il est rare
    private func weight(_ word: String) -> Double {
        log(Double(documentCount + 1) / Double((documentFrequency[word] ?? 0) + 1)) + 0.05
    }

    /// Score de similarité (nil = pas un doublon)
    func score(_ a: Signature, _ b: Signature) -> Double? {
        if let la = a.link, la == b.link { return 3 }

        let shared = a.words.intersection(b.words)
        // Au moins deux mots partagés qui ne sont pas des mots du moment
        guard shared.filter({ !isCommon($0) }).count >= 2 else { return nil }
        let union = a.words.union(b.words)
        let overlap = shared.reduce(0) { $0 + weight($1) } / max(union.reduce(0) { $0 + weight($1) }, 0.0001)
        if overlap >= sensitivity.wordThreshold { return 1 + overlap }

        // Titres formulés différemment : on compare aussi leur sens, s'ils se recoupent déjà un peu
        guard overlap >= 0.25, let language = a.language, language == b.language,
              let va = vector(for: a), let vb = vector(for: b) else { return nil }
        let similarity = Self.cosine(va, vb)
        return similarity >= sensitivity.meaningThreshold ? similarity : nil
    }

    private func vector(for signature: Signature) -> [Double]? {
        if let cached = vectors[signature.title] { return cached }
        guard let language = signature.language else { return nil }
        if embeddings[language] == nil {
            guard let embedding = NLEmbedding.sentenceEmbedding(for: language) else { return nil }
            embeddings[language] = embedding
        }
        let vector = embeddings[language]?.vector(for: signature.title.lowercased())
        vectors[signature.title] = vector
        return vector
    }

    private static func cosine(_ a: [Double], _ b: [Double]) -> Double {
        guard a.count == b.count, !a.isEmpty else { return 0 }
        var dot = 0.0, na = 0.0, nb = 0.0
        for i in a.indices {
            dot += a[i] * b[i]
            na += a[i] * a[i]
            nb += b[i] * b[i]
        }
        guard na > 0, nb > 0 else { return 0 }
        return dot / (na.squareRoot() * nb.squareRoot())
    }

    // MARK: - Mots courants (français et anglais)

    private static let stopWords: Set<String> = [
        // français
        "les", "des", "une", "est", "son", "sa", "ses", "aux", "par", "pour", "dans", "sur", "avec", "sans",
        "qui", "que", "quoi", "dont", "mais", "plus", "moins", "tres", "tout", "tous", "toute", "toutes",
        "cette", "ces", "cet", "leur", "leurs", "nous", "vous", "ils", "elles", "elle", "lui", "eux", "pas",
        "peu", "deja", "encore", "aussi", "comme", "entre", "vers", "chez", "apres", "avant", "depuis",
        "pendant", "selon", "contre", "sous", "fait", "faire", "font", "sont", "ont", "avoir", "etre", "ete",
        "sera", "seront", "peut", "peuvent", "va", "vont", "voici", "voila", "enfin", "bien", "quand",
        "comment", "pourquoi", "ou", "lors", "notre", "votre", "nos", "vos", "mon", "ton", "mes", "tes",
        "ceux", "celle", "celles", "celui", "ici", "la", "le", "un", "et", "de", "du", "au", "en", "ne", "se",
        "ce", "il", "on", "nouveau", "nouvelle", "nouveaux", "nouvelles", "officiel", "officiellement",
        "savoir", "faut",
        // anglais
        "the", "and", "for", "with", "from", "that", "this", "these", "those", "are", "was", "were", "has",
        "have", "had", "will", "can", "its", "it's", "your", "you", "our", "their", "they", "but", "not",
        "now", "new", "more", "than", "about", "into", "over", "after", "before", "just", "what", "how",
        "why", "when", "who", "here", "there", "all", "out", "get", "gets", "got", "finally", "officially",
        "announces", "announced", "launches", "launched", "says", "said",
    ]
}
