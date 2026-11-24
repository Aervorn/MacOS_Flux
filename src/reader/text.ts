/** Texte brut d'un fragment HTML (sans exécuter de script ni charger d'image). */
export function plainText(html: string): string {
  const doc = new DOMParser().parseFromString(html, "text/html");
  doc.querySelectorAll("script, style, noscript").forEach((n) => n.remove());
  return (doc.body.textContent ?? "").replace(/\s+/g, " ").trim();
}

const EXCERPT_MARKERS = [
  "lire la suite",
  "read more",
  "continue reading",
  "appeared first on",
  "est apparu en premier sur",
  "réservé aux abonnés",
  "seuls les abonnés",
];

/**
 * Le flux ne contient-il qu'un extrait ? (même heuristique que l'app Swift)
 * - texte très court (moins de 700 caractères), ou
 * - texte court (moins de 4000) qui se termine par « … » / « [...] » ou contient un marqueur.
 */
export function looksLikeExcerpt(html: string | null | undefined): boolean {
  if (!html || !html.trim()) return false;
  const text = plainText(html);
  if (text.length < 700) return true;
  if (text.length >= 4000) return false;
  const tail = text.slice(-40).trim();
  if (/(…|\.\.\.|\[…\]|\[\.\.\.\])$/.test(tail)) return true;
  const lower = text.toLowerCase();
  return EXCERPT_MARKERS.some((m) => lower.includes(m));
}

export function wordCount(text: string): number {
  return text.split(/\s+/).filter(Boolean).length;
}

/** Texte de recherche : titre, résumé, corps (30 000 premiers caractères). */
export function buildSearchText(title: string, summary: string, bodyText: string): string {
  return `${title}\n${summary}\n${bodyText}`.slice(0, 30000);
}
