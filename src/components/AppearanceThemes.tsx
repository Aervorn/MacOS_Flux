import { useTranslation } from "react-i18next";
import type { Appearance, Prefs, ThemeId } from "../api";
import { ACCENTS, DARK_THEMES, LIGHT_THEMES, resolveTheme, schemeOf, type ThemeChoice } from "../theme";
import styles from "./AppearanceThemes.module.css";

interface Props {
  prefs: Prefs;
  onTheme: (patch: Partial<ThemeChoice>) => void;
}

/** Réglages › Apparence : thème, thèmes de jour et de nuit en automatique, couleur d'accent. */
export default function AppearanceThemes({ prefs, onTheme }: Props) {
  const { t } = useTranslation();
  const name = (id: ThemeId) => t(`settings.themes.${id}`);

  /** Aperçu miniature, peint avec les couleurs du thème lui-même. */
  const preview = (id: ThemeId) => (
    <span className={styles.preview} data-swatch={id}>
      <span className={styles.previewSide} />
      <span className={styles.previewPanel}>
        <span className={styles.previewTitle} />
        <span className={styles.previewLine} />
        <span className={styles.previewAccent} />
      </span>
    </span>
  );

  const card = (key: string, label: string, active: boolean, onPick: () => void, art: React.ReactNode) => (
    <button key={key} type="button" className={active ? styles.cardActive : styles.card} aria-pressed={active} onClick={onPick}>
      {art}
      <span>{label}</span>
    </button>
  );

  const group = (label: string, cards: React.ReactNode, help?: string) => (
    <div className={styles.group}>
      <span className={styles.label}>{label}</span>
      <div className={styles.grid}>{cards}</div>
      {help && <p className={styles.help}>{help}</p>}
    </div>
  );

  const themeCard = (id: ThemeId, active: boolean, onPick: () => void) => card(id, name(id), active, onPick, preview(id));
  const pickAppearance = (appearance: Appearance) => onTheme({ appearance });
  const auto = prefs.appearance === "system";
  // Les pastilles d'accent prennent la teinte adaptée au thème affiché en ce moment
  const shown = resolveTheme(prefs, window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? true);

  return (
    <>
      {group(
        t("settings.theme"),
        <>
          {card(
            "system",
            t("settings.appearanceAuto"),
            auto,
            () => pickAppearance("system"),
            <span className={styles.split}>
              {preview(prefs.lightTheme)}
              {preview(prefs.darkTheme)}
            </span>,
          )}
          {[...LIGHT_THEMES, ...DARK_THEMES].map((id) => themeCard(id, prefs.appearance === id, () => pickAppearance(id)))}
        </>,
      )}
      {auto && (
        <>
          {group(
            t("settings.dayTheme"),
            LIGHT_THEMES.map((id) => themeCard(id, prefs.lightTheme === id, () => onTheme({ lightTheme: id }))),
          )}
          {group(
            t("settings.nightTheme"),
            DARK_THEMES.map((id) => themeCard(id, prefs.darkTheme === id, () => onTheme({ darkTheme: id }))),
            t("settings.autoThemeHelp"),
          )}
        </>
      )}
      <div className={styles.group}>
        <span className={styles.label}>{t("settings.accent")}</span>
        <div className={styles.accents}>
          {ACCENTS.map((accent) => {
            const label = t(`settings.accents.${accent}`);
            return (
              <button
                key={accent}
                type="button"
                className={prefs.accent === accent ? styles.accentActive : styles.accent}
                aria-pressed={prefs.accent === accent}
                aria-label={label}
                title={label}
                onClick={() => onTheme({ accent })}
              >
                {/* « Celle du thème » : le point reprend l'accent propre au thème affiché */}
                <span
                  className={styles.dot}
                  data-swatch={accent === "theme" ? shown : undefined}
                  data-accent-swatch={accent === "theme" ? undefined : accent}
                  data-scheme={schemeOf(shown)}
                />
                {accent === "theme" && <span>{label}</span>}
              </button>
            );
          })}
        </div>
      </div>
    </>
  );
}
