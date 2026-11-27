import type { AccentChoice, Appearance, DarkTheme, LightTheme, Prefs, ThemeId } from "./api";

export const LIGHT_THEMES: LightTheme[] = ["light", "sepia", "dawn"];
export const DARK_THEMES: DarkTheme[] = ["dark", "pureBlack", "nord", "forest"];
export const ACCENTS: AccentChoice[] = ["theme", "blue", "violet", "pink", "red", "orange", "green"];

/** Les réglages qui déterminent les couleurs de l'app. */
export type ThemeChoice = Pick<Prefs, "appearance" | "lightTheme" | "darkTheme" | "accent">;

const STORAGE_KEY = "flux.themeChoice";
const FALLBACK: ThemeChoice = { appearance: "system", lightTheme: "light", darkTheme: "dark", accent: "theme" };

const isLight = (id: string): id is LightTheme => (LIGHT_THEMES as string[]).includes(id);
const isDark = (id: string): id is DarkTheme => (DARK_THEMES as string[]).includes(id);

export const schemeOf = (id: ThemeId): "light" | "dark" => (isLight(id) ? "light" : "dark");

/** Thème à afficher : celui choisi, ou en automatique celui du jour ou de la nuit. */
export function resolveTheme(choice: ThemeChoice, systemDark: boolean): ThemeId {
  const { appearance } = choice;
  if (isLight(appearance) || isDark(appearance)) return appearance;
  // Mode automatique (ou valeur inconnue, par exemple venue d'une version plus récente)
  if (systemDark) return isDark(choice.darkTheme) ? choice.darkTheme : "dark";
  return isLight(choice.lightTheme) ? choice.lightTheme : "light";
}

const systemQuery = () => window.matchMedia?.("(prefers-color-scheme: dark)");

let current: ThemeChoice = FALLBACK;
let watching = false;

function paint() {
  const id = resolveTheme(current, systemQuery()?.matches ?? true);
  const root = document.documentElement;
  root.dataset.theme = id;
  root.dataset.scheme = schemeOf(id);
  if (current.accent !== "theme" && ACCENTS.includes(current.accent)) root.dataset.accent = current.accent;
  else delete root.dataset.accent;
}

/** Applique le thème et suit ensuite les changements d'apparence de macOS. */
export function applyTheme(choice: ThemeChoice) {
  const { appearance, lightTheme, darkTheme, accent } = choice;
  current = { appearance, lightTheme, darkTheme, accent };
  if (!watching) {
    watching = true;
    systemQuery()?.addEventListener("change", paint);
  }
  paint();
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(current));
  } catch {
    /* sans importance */
  }
}

/** Dernier choix connu, pour le premier affichage avant la lecture des réglages. */
export function initialTheme(): ThemeChoice {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved) return { ...FALLBACK, ...(JSON.parse(saved) as Partial<ThemeChoice>) };
    // Ancienne clé (Flux 0.8.5 et avant) : « light », « dark » ou « auto »
    const old = localStorage.getItem("flux.theme");
    if (old === "light" || old === "dark") return { ...FALLBACK, appearance: old as Appearance };
  } catch {
    /* ignoré */
  }
  return FALLBACK;
}
