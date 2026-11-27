// Résumés d'articles avec le modèle d'Apple Intelligence (framework FoundationModels), calculés
// sur le Mac. Compilé en bibliothèque statique par build.rs et appelé depuis Rust (src/ai.rs).
//
// FoundationModels n'existe qu'à partir de macOS 26, sur Mac Apple Silicon : le framework est lié
// « faiblement », et Flux continue de s'ouvrir sur macOS 14 et 15, où la fonction est simplement
// indisponible. Sans le SDK de macOS 26 (Xcode trop ancien), ce fichier compile sans l'IA.

import Foundation
#if canImport(FoundationModels) && !FLUX_NO_AI
import FoundationModels
#endif

/// Résultat transmis d'une tâche asynchrone au fil qui attend.
private final class Box: @unchecked Sendable {
    var value = "err:unknown"
}

/// 0 disponible, 1 système ou Mac sans le framework, 2 Mac non compatible,
/// 3 Apple Intelligence désactivée, 4 modèle pas encore téléchargé, 5 autre raison.
@_cdecl("flux_ai_status")
public func fluxAIStatus() -> Int32 {
#if canImport(FoundationModels) && !FLUX_NO_AI
    if #available(macOS 26.0, *) {
        switch SystemLanguageModel.default.availability {
        case .available:
            return 0
        case .unavailable(let reason):
            // Comparaison par le nom : la liste des raisons peut s'allonger d'une version à l'autre
            let name = String(describing: reason)
            if name.contains("deviceNotEligible") { return 2 }
            if name.contains("NotEnabled") { return 3 }
            if name.contains("NotReady") { return 4 }
            return 5
        @unknown default:
            return 5
        }
    }
#endif
    return 1
}

/// Résume `text` dans la langue `lang` ("fr" ou "en"). Bloque jusqu'au résultat : à appeler hors
/// du fil principal. Renvoie "ok:<résumé>" ou "err:<message>", à libérer avec `flux_ai_free`.
@_cdecl("flux_ai_summarize")
public func fluxAISummarize(_ text: UnsafePointer<CChar>, _ lang: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>? {
    let article = String(cString: text)
    let french = String(cString: lang).hasPrefix("fr")
    let box = Box()
#if canImport(FoundationModels) && !FLUX_NO_AI
    if #available(macOS 26.0, *) {
        let instructions = french
            ? "Tu résumes des articles de presse, en français. Réponds uniquement par 3 à 5 puces courtes, chacune sur une ligne commençant par « - », sans introduction ni conclusion. Reste fidèle au texte : n'ajoute aucune information ni aucun avis."
            : "You summarize news articles, in English. Reply only with 3 to 5 short bullet points, each on its own line starting with \"- \", with no introduction or conclusion. Stay faithful to the text: add no information and no opinion."
        let prompt = (french ? "Article à résumer :\n\n" : "Article to summarize:\n\n") + article
        let done = DispatchSemaphore(value: 0)
        Task.detached {
            do {
                // Garde-fous « transformation de contenu » : prévus par Apple pour résumer un texte,
                // même quand il parle de sujets sensibles (faits divers, santé…). Avec les garde-fous
                // par défaut, beaucoup d'articles d'actualité sont refusés.
                let model = SystemLanguageModel(useCase: .general, guardrails: .permissiveContentTransformations)
                let session = LanguageModelSession(model: model, instructions: instructions)
                let response = try await session.respond(to: prompt)
                box.value = "ok:" + response.content
            } catch {
                box.value = "err:" + String(describing: error)
            }
            done.signal()
        }
        done.wait()
    } else {
        box.value = "err:unsupported"
    }
#else
    box.value = "err:unsupported"
#endif
    return strdup(box.value)
}

@_cdecl("flux_ai_free")
public func fluxAIFree(_ pointer: UnsafeMutablePointer<CChar>?) {
    free(pointer)
}
