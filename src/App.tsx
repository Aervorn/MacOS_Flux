import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { setLang, type Lang } from "./i18n";
import { applyTheme, initialTheme, type ThemeMode } from "./theme";
import styles from "./App.module.css";

interface AppInfo {
  version: string;
  coreVersion: string;
}

interface ImportReport {
  folders: number;
  feeds: number;
  articles: number;
  favoriteTags: number;
  articleTags: number;
  filterRules: number;
  settingsImported: boolean;
  language: string | null;
  warnings: string[];
}

const THEMES: { mode: ThemeMode; labelKey: string }[] = [
  { mode: "light", labelKey: "settings.appearanceLight" },
  { mode: "dark", labelKey: "settings.appearanceDark" },
  { mode: "auto", labelKey: "settings.appearanceAuto" },
];

export default function App() {
  const { t, i18n } = useTranslation();
  const [theme, setTheme] = useState<ThemeMode>(initialTheme);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [importError, setImportError] = useState<string | null>(null);

  useEffect(() => {
    invoke<AppInfo>("app_info")
      .then(setInfo)
      .catch(() => setInfo(null)); // hors Tauri (navigateur seul), on ignore
  }, []);

  const chooseTheme = (mode: ThemeMode) => {
    setTheme(mode);
    applyTheme(mode);
  };

  const printPage = async () => {
    setError(null);
    try {
      await invoke("print_page");
    } catch (e) {
      setError(t("dev.printError", { message: String(e) }));
    }
  };

  const importData = async () => {
    setReport(null);
    setImportError(null);
    try {
      setReport(await invoke<ImportReport>("dev_import_swift_data"));
    } catch (e) {
      setImportError(t("dev.importError", { message: String(e) }));
    }
  };

  return (
    <div className={styles.app}>
      <aside className={styles.sidebar}>
        <div className={styles.brand}>{t("app.name")}</div>
        <div className={styles.itemActive}>{t("sidebar.allArticles")}</div>
        <div className={styles.item}>{t("sidebar.unread")}</div>
        <div className={styles.item}>{t("sidebar.favorites")}</div>
        <div className={styles.sectionTitle}>{t("sidebar.feeds")}</div>

        <div className={styles.footer}>
          <span className={styles.label}>{t("settings.appearance")}</span>
          <div className={styles.segmented}>
            {THEMES.map(({ mode, labelKey }) => (
              <button
                key={mode}
                className={theme === mode ? styles.segmentActive : styles.segment}
                onClick={() => chooseTheme(mode)}
              >
                {t(labelKey)}
              </button>
            ))}
          </div>
          <span className={styles.label}>{t("settings.language")}</span>
          <div className={styles.segmented}>
            {(["fr", "en"] as Lang[]).map((lang) => (
              <button
                key={lang}
                className={i18n.language === lang ? styles.segmentActive : styles.segment}
                onClick={() => setLang(lang)}
              >
                {lang.toUpperCase()}
              </button>
            ))}
          </div>
        </div>
      </aside>

      <main className={styles.main}>
        <div className={styles.empty}>
          <div className={styles.placeholder} />
          <h1 className={styles.emptyTitle}>{t("list.emptyTitle")}</h1>
          <p>{t("list.emptyBody")}</p>
          {info && <p className={styles.label}>{t("dev.version", { version: info.version, core: info.coreVersion })}</p>}
          <button className={styles.devButton} onClick={printPage}>
            {t("dev.print")}
          </button>
          {error && <div className={styles.error}>{error}</div>}
          <button className={styles.devButton} onClick={importData}>
            {t("dev.import")}
          </button>
          {importError && <div className={styles.error}>{importError}</div>}
          {report && (
            <p className={styles.label}>
              {t("dev.importDone", {
                articles: report.articles,
                feeds: report.feeds,
                folders: report.folders,
                tags: report.favoriteTags,
                rules: report.filterRules,
              })}
              {report.settingsImported && ` ${t("dev.importSettings")}`}
              {report.warnings.length > 0 &&
                ` ${t("dev.importWarnings", { count: report.warnings.length, list: report.warnings.join(" ; ") })}`}
            </p>
          )}
        </div>
      </main>
    </div>
  );
}
