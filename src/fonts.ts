import type { Prefs } from "./api";

const SYSTEM_SANS = '-apple-system, BlinkMacSystemFont, "SF Pro Text", system-ui, sans-serif';

/** Polices de l'interface (identifiant → famille CSS). */
export const UI_FONTS: { id: Prefs["uiFont"]; name: string; stack: string }[] = [
  { id: "inter", name: "Inter", stack: `"Inter Variable", "Inter", ${SYSTEM_SANS}` },
  { id: "geist", name: "Geist", stack: `"Geist Variable", ${SYSTEM_SANS}` },
  { id: "system", name: "SF Pro", stack: SYSTEM_SANS },
];

/** Polices de l'article : `title` pour le titre, `body` pour le texte (`null` : celle de l'interface). */
export const ARTICLE_FONTS: { id: Prefs["articleFont"]; name: string; body: string | null; title: string; descKey: string }[] = [
  { id: "classic", name: "Classique", body: null, title: '"New York", ui-serif, Georgia, serif', descKey: "settings.fontClassic" },
  { id: "geist", name: "Geist", body: '"Geist Variable"', title: '"Geist Variable"', descKey: "settings.fontGeist" },
  { id: "literata", name: "Literata", body: '"Literata Variable", Georgia, serif', title: '"Literata Variable", Georgia, serif', descKey: "settings.fontLiterata" },
  { id: "sourceSerif", name: "Source Serif", body: '"Source Serif 4 Variable", Georgia, serif', title: '"Source Serif 4 Variable", Georgia, serif', descKey: "settings.fontSourceSerif" },
  { id: "merriweather", name: "Merriweather", body: '"Merriweather Variable", Georgia, serif', title: '"Merriweather Variable", Georgia, serif', descKey: "settings.fontMerriweather" },
  { id: "atkinson", name: "Atkinson", body: '"Atkinson Hyperlegible", sans-serif', title: '"Atkinson Hyperlegible", sans-serif', descKey: "settings.fontAtkinson" },
  { id: "systemSerif", name: "Georgia", body: "Georgia, serif", title: "Georgia, serif", descKey: "settings.fontSystemSerif" },
  { id: "mono", name: "Mono", body: '"Geist Mono Variable", ui-monospace, "SF Mono", Menlo, monospace', title: '"Geist Mono Variable", ui-monospace, "SF Mono", Menlo, monospace', descKey: "settings.fontMono" },
];

/** Applique polices et densité à toute l'interface (variables CSS sur <html>). */
export function applyLook(prefs: Pick<Prefs, "uiFont" | "articleFont" | "sidebarDensity">) {
  const root = document.documentElement;
  const ui = UI_FONTS.find((f) => f.id === prefs.uiFont) ?? UI_FONTS[0];
  const article = ARTICLE_FONTS.find((f) => f.id === prefs.articleFont) ?? ARTICLE_FONTS[0];
  root.style.setProperty("--font", ui.stack);
  root.style.setProperty("--font-article", article.body ?? "var(--font)");
  root.style.setProperty("--font-article-title", article.title);
  root.dataset.density = prefs.sidebarDensity;
}
