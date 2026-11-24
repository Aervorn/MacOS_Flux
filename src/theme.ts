import type { Appearance } from "./api";

export type ThemeMode = "light" | "dark" | "auto";
const STORAGE_KEY = "flux.theme";

/** Le thème est appliqué via l'attribut data-theme ; le CSS gère "auto" avec prefers-color-scheme. */
export function applyTheme(mode: ThemeMode) {
  document.documentElement.dataset.theme = mode;
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    /* sans importance */
  }
}

/** Dernier thème connu, pour le premier affichage avant la lecture des réglages. */
export function initialTheme(): ThemeMode {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === "light" || saved === "dark" || saved === "auto") return saved;
  } catch {
    /* ignoré */
  }
  return "auto";
}

/** Les réglages disent « system » là où le CSS dit « auto ». */
export const appearanceToMode = (a: Appearance): ThemeMode => (a === "system" ? "auto" : a);
export const modeToAppearance = (m: ThemeMode): Appearance => (m === "auto" ? "system" : m);
