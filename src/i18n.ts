import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import fr from "./locales/fr.json";
import en from "./locales/en.json";

export type Lang = "fr" | "en";
const STORAGE_KEY = "flux.lang";

function initialLang(): Lang {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === "fr" || saved === "en") return saved;
  } catch {
    /* stockage indisponible : on retombe sur la langue du système */
  }
  return navigator.language.toLowerCase().startsWith("fr") ? "fr" : "en";
}

void i18n.use(initReactI18next).init({
  resources: { fr: { translation: fr }, en: { translation: en } },
  lng: initialLang(),
  fallbackLng: "fr",
  interpolation: { escapeValue: false },
});

/** Vrai si l'utilisateur a déjà choisi une langue dans cette app. */
export function hasSavedLang(): boolean {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    return saved === "fr" || saved === "en";
  } catch {
    return false;
  }
}

export function setLang(lang: Lang) {
  void i18n.changeLanguage(lang);
  document.documentElement.lang = lang;
  try {
    localStorage.setItem(STORAGE_KEY, lang);
  } catch {
    /* sans importance */
  }
}

document.documentElement.lang = i18n.language;
export default i18n;
