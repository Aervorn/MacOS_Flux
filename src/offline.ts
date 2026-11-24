import { api } from "./api";
import { extractArticle } from "./reader/extract";
import { looksLikeExcerpt } from "./reader/text";

/** Articles traités / à traiter pendant une préparation (null = rien en cours). */
export type OfflineProgress = { done: number; total: number } | null;

const listeners = new Set<(p: OfflineProgress) => void>();
let current: OfflineProgress = null;
let running = false;
/** Articles dont l'extraction a échoué pendant cette session (paywall, page vide) : on n'insiste pas. */
const failed = new Set<string>();

const CONCURRENCY = 4;

function publish(p: OfflineProgress) {
  current = p;
  listeners.forEach((l) => l(p));
}

export function onOfflineProgress(listener: (p: OfflineProgress) => void): () => void {
  listeners.add(listener);
  listener(current);
  return () => listeners.delete(listener);
}

/** Nombre d'articles dont le texte complet reste à récupérer. */
export async function countPending(): Promise<number> {
  const pending = await api.offlinePendingTexts().catch(() => []);
  return pending.filter((a) => !failed.has(a.key) && looksLikeExcerpt(a.contentHtml || a.summary)).length;
}

/**
 * Prépare la lecture hors ligne (si l'option est active) : récupère le texte complet des articles
 * non lus et des favoris dont le flux ne donne qu'un extrait, puis télécharge leurs images et ramène
 * le cache sous sa limite. L'extraction se fait ici, dans l'interface, car c'est là que tourne le
 * code qui sait extraire un article d'une page.
 */
export async function prepareOffline(): Promise<void> {
  if (running) return;
  running = true;
  try {
    const pending = (await api.offlinePendingTexts().catch(() => []))
      .filter((a) => !failed.has(a.key) && looksLikeExcerpt(a.contentHtml || a.summary));
    let done = 0;
    let stop = false;
    publish(pending.length > 0 ? { done, total: pending.length } : null);

    const queue = [...pending];
    const worker = async () => {
      while (!stop) {
        const article = queue.shift();
        if (!article) return;
        try {
          const page = await api.fetchPage(article.link);
          const extracted = extractArticle(page.html, page.finalUrl, article.title, article.summary);
          if (!extracted) throw new Error("extraction");
          await api.saveFullHtml({
            key: article.key,
            html: extracted.html,
            searchText: extracted.searchText,
            wordCount: extracted.wordCount,
            imageUrl: extracted.imageUrl,
          });
        } catch {
          failed.add(article.key);
          // Plus de réseau : inutile de continuer (et ces articles seront retentés plus tard)
          if (!(await api.isOnline().catch(() => true))) {
            stop = true;
            failed.delete(article.key);
          }
        }
        done += 1;
        publish({ done, total: pending.length });
      }
    };
    await Promise.all(Array.from({ length: CONCURRENCY }, worker));
    publish(null);
    if (!stop) await api.offlineMaintain().catch(() => undefined); // images, puis taille du cache
  } finally {
    publish(null);
    running = false;
  }
}
