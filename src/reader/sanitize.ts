import DOMPurify from "dompurify";

/**
 * Nettoie le HTML d'un article avant de l'afficher dans la fenêtre de l'app.
 * Le contenu vient d'Internet : aucun script, style, formulaire ni cadre n'est conservé.
 */
export function sanitizeArticle(html: string): string {
  return DOMPurify.sanitize(html, {
    USE_PROFILES: { html: true },
    FORBID_TAGS: ["style", "link", "form", "input", "button", "select", "textarea", "iframe", "object", "embed", "base", "meta", "svg", "math"],
    FORBID_ATTR: ["style", "srcset", "sizes", "loading", "ping"],
    ALLOWED_URI_REGEXP: /^(?:https?:|mailto:|#|data:image\/(?:png|jpe?g|gif|webp);)/i,
  });
}

export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c] as string);
}

/** Corps affiché : texte complet si demandé et disponible, sinon contenu du flux, sinon résumé. */
export function bodyHtml(opts: { fullHtml: string | null; contentHtml: string | null; summary: string; showFull: boolean }): string {
  if (opts.showFull && opts.fullHtml) return opts.fullHtml;
  if (opts.contentHtml && opts.contentHtml.trim()) return opts.contentHtml;
  return `<p>${escapeHtml(opts.summary)}</p>`;
}

/** Nom de fichier d'une adresse d'image (sans paramètres). */
export function imageFileName(url: string): string {
  const path = url.split(/[?#]/)[0] ?? "";
  return (path.split("/").pop() ?? "").toLowerCase();
}

/** L'image principale est-elle déjà dans le corps ? Si oui, on ne l'affiche pas deux fois. */
export function heroAlreadyInBody(imageUrl: string, bodyHtml: string): boolean {
  const name = imageFileName(imageUrl);
  return name.length > 3 && bodyHtml.toLowerCase().includes(name);
}
