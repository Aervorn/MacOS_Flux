import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { BookOpen, Cloud, Palette, Rss, SlidersHorizontal, Terminal, X } from "lucide-react";
import type { AppInfo, ImportReport, Prefs, SidebarFeed } from "../api";
import { setLang, type Lang } from "../i18n";
import type { ThemeMode } from "../theme";
import { ARTICLE_FONTS, UI_FONTS } from "../fonts";
import FiltersPanel from "./FiltersPanel";
import SyncSection from "./SyncSection";
import Select from "./Select";
import styles from "./SettingsDialog.module.css";

export type Section = "appearance" | "reading" | "feeds" | "filters" | "sync" | "dev";

const SECTIONS: { id: Section; icon: React.ReactNode; labelKey: string }[] = [
  { id: "appearance", icon: <Palette size={16} />, labelKey: "settings.sectionAppearance" },
  { id: "reading", icon: <BookOpen size={16} />, labelKey: "settings.sectionReading" },
  { id: "feeds", icon: <Rss size={16} />, labelKey: "settings.sectionFeeds" },
  { id: "filters", icon: <SlidersHorizontal size={16} />, labelKey: "settings.sectionFilters" },
  { id: "sync", icon: <Cloud size={16} />, labelKey: "settings.sectionSync" },
  { id: "dev", icon: <Terminal size={16} />, labelKey: "settings.sectionDev" },
];

const THEMES: { mode: ThemeMode; labelKey: string }[] = [
  { mode: "light", labelKey: "settings.appearanceLight" },
  { mode: "dark", labelKey: "settings.appearanceDark" },
  { mode: "auto", labelKey: "settings.appearanceAuto" },
];

const MIN_FONT = 13;
const MAX_FONT = 28;

/** Valeurs proposées, plus la valeur actuelle si elle n'y figure pas (réglage importé). */
const withCurrent = (values: number[], current: number) => (values.includes(current) ? values : [...values, current].sort((a, b) => a - b));

interface DevTools {
  info: AppInfo | null;
  report: ImportReport | null;
  message: string | null;
  onPrint: () => void;
  onImportSwift: () => void;
}

interface Props {
  section: Section;
  onSection: (s: Section) => void;
  prefs: Prefs;
  onPrefs: (patch: Partial<Prefs>) => void;
  themeMode: ThemeMode;
  onTheme: (m: ThemeMode) => void;
  onDuplicates: (level: number) => void;
  feeds: SidebarFeed[];
  /** Recharge compteurs et liste (après un changement de filtres). */
  onChanged: () => void;
  dev: DevTools;
  onClose: () => void;
}

export default function SettingsDialog({ section, onSection, prefs, onPrefs, themeMode, onTheme, onDuplicates, feeds, onChanged, dev, onClose }: Props) {
  const { t, i18n } = useTranslation();
  const [filtersEditing, setFiltersEditing] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Dans le formulaire des filtres, Échap ferme d'abord le formulaire.
      if (e.key === "Escape" && !(section === "filters" && filtersEditing)) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, section, filtersEditing]);

  const segmented = <T extends string | number>(value: T, options: { value: T; label: string }[], onPick: (v: T) => void) => (
    <div className={styles.segmented}>
      {options.map((o) => (
        <button key={String(o.value)} className={value === o.value ? styles.segmentActive : styles.segment} onClick={() => onPick(o.value)}>
          {o.label}
        </button>
      ))}
    </div>
  );

  const select = (value: number, options: { value: number; label: string }[], onPick: (v: number) => void) => (
    <Select value={value} options={options} onChange={onPick} />
  );

  const row = (label: string, control: React.ReactNode, help?: string) => (
    <div className={styles.row}>
      <span className={styles.label}>{label}</span>
      {control}
      {help && <p className={styles.help}>{help}</p>}
    </div>
  );

  const toggle = (label: string, checked: boolean, onChange: (v: boolean) => void) => (
    <label className={styles.toggle}>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );

  const dupLevel = prefs.groupDuplicates ? prefs.duplicateSensitivity : -1;
  const days = (n: number) => t("settings.days", { count: n });

  const body = (() => {
    switch (section) {
      case "appearance":
        return (
          <>
            {row(t("settings.appearance"), segmented(themeMode, THEMES.map((x) => ({ value: x.mode, label: t(x.labelKey) })), onTheme))}
            {row(
              t("settings.language"),
              segmented(i18n.language as Lang, (["fr", "en"] as Lang[]).map((l) => ({ value: l, label: l === "fr" ? "Français" : "English" })), setLang),
            )}
            {row(
              t("settings.uiFont"),
              segmented<Prefs["uiFont"]>(
                prefs.uiFont,
                UI_FONTS.map((f) => ({ value: f.id, label: f.id === "system" ? t("settings.fontSystem") : f.name })),
                (uiFont) => onPrefs({ uiFont }),
              ),
            )}
            {row(
              t("settings.sidebarDensity"),
              segmented<Prefs["sidebarDensity"]>(
                prefs.sidebarDensity,
                [
                  { value: "compact", label: t("settings.densityCompact") },
                  { value: "normal", label: t("settings.densityNormal") },
                  { value: "spacious", label: t("settings.densitySpacious") },
                ],
                (sidebarDensity) => onPrefs({ sidebarDensity }),
              ),
            )}
          </>
        );
      case "reading":
        return (
          <>
            {row(
              t("settings.articleFont"),
              <div className={styles.fontGrid}>
                {ARTICLE_FONTS.map((f) => (
                  <button
                    key={f.id}
                    className={prefs.articleFont === f.id ? styles.fontCardActive : styles.fontCard}
                    onClick={() => onPrefs({ articleFont: f.id })}
                  >
                    <span className={styles.fontSample} style={{ fontFamily: f.title }}>
                      Aa
                    </span>
                    <span className={styles.fontText}>
                      <span className={styles.fontName}>{f.id === "classic" ? t("settings.fontClassicName") : f.name}</span>
                      <span className={styles.fontDesc}>{t(f.descKey)}</span>
                    </span>
                  </button>
                ))}
              </div>,
            )}
            {row(
              t("settings.fontSize"),
              <div className={styles.stepper}>
                <button onClick={() => onPrefs({ readerFontSize: Math.max(MIN_FONT, prefs.readerFontSize - 1) })} disabled={prefs.readerFontSize <= MIN_FONT}>
                  −
                </button>
                <span>{prefs.readerFontSize} px</span>
                <button onClick={() => onPrefs({ readerFontSize: Math.min(MAX_FONT, prefs.readerFontSize + 1) })} disabled={prefs.readerFontSize >= MAX_FONT}>
                  +
                </button>
              </div>,
            )}
            {row(
              t("settings.lineWidth"),
              segmented<Prefs["readerWidth"]>(
                prefs.readerWidth,
                [
                  { value: "narrow", label: t("settings.widthNarrow") },
                  { value: "wide", label: t("settings.widthWide") },
                  { value: "full", label: t("settings.widthFull") },
                ],
                (readerWidth) => onPrefs({ readerWidth }),
              ),
            )}
            {toggle(t("settings.showSummary"), prefs.showSummary, (showSummary) => onPrefs({ showSummary }))}
          </>
        );
      case "feeds":
        return (
          <>
            {row(
              t("settings.refreshEvery"),
              select(
                prefs.refreshMinutes,
                withCurrent([15, 30, 60, 120, 360], prefs.refreshMinutes).map((v) => ({
                  value: v,
                  label: v < 60 ? t("settings.minutes", { count: v }) : t("settings.hours", { count: v / 60 }),
                })),
                (refreshMinutes) => onPrefs({ refreshMinutes }),
              ),
            )}
            {row(
              t("settings.markReadDelay"),
              select(
                prefs.markReadDelay,
                withCurrent([0, 5, 10, 30, -1], prefs.markReadDelay).map((v) => ({
                  value: v,
                  label: v === 0 ? t("settings.immediately") : v < 0 ? t("settings.never") : t("settings.afterSeconds", { count: v }),
                })),
                (markReadDelay) => onPrefs({ markReadDelay }),
              ),
            )}
            {row(
              t("settings.duplicates"),
              segmented(
                dupLevel,
                [
                  { value: -1, label: t("settings.dupOff") },
                  { value: 0, label: t("settings.dupLow") },
                  { value: 1, label: t("settings.dupNormal") },
                  { value: 2, label: t("settings.dupHigh") },
                ],
                onDuplicates,
              ),
              t("settings.duplicatesHelp"),
            )}
            <div className={styles.row}>
              {toggle(t("settings.autoMarkRead"), prefs.autoMarkReadEnabled, (autoMarkReadEnabled) => onPrefs({ autoMarkReadEnabled }))}
              {select(
                prefs.autoMarkReadDays,
                withCurrent([1, 2, 3, 7, 14, 30], prefs.autoMarkReadDays).map((v) => ({ value: v, label: days(v) })),
                (autoMarkReadDays) => onPrefs({ autoMarkReadDays }),
              )}
              <p className={styles.help}>{t("settings.autoMarkReadHelp")}</p>
            </div>
            {row(
              t("settings.retention"),
              select(
                prefs.retentionDays,
                withCurrent([14, 30, 60, 90, 180, 365, 0], prefs.retentionDays)
                  .sort((a, b) => (a === 0 ? 1 : b === 0 ? -1 : a - b))
                  .map((v) => ({ value: v, label: v === 0 ? t("settings.forever") : days(v) })),
                (retentionDays) => onPrefs({ retentionDays }),
              ),
              t("settings.retentionHelp"),
            )}
          </>
        );
      case "filters":
        return <FiltersPanel feeds={feeds} onChanged={onChanged} onEditingChange={setFiltersEditing} />;
      case "sync":
        return <SyncSection prefs={prefs} onPrefs={onPrefs} />;
      case "dev":
        return (
          <>
            {dev.info && <p className={styles.help}>{t("dev.version", { version: dev.info.version, core: dev.info.coreVersion })}</p>}
            <div>
              <button className={styles.button} onClick={dev.onPrint}>
                {t("dev.print")}
              </button>
            </div>
            <div>
              <button className={styles.button} onClick={dev.onImportSwift}>
                {t("dev.import")}
              </button>
            </div>
            {dev.report && (
              <p className={styles.help}>
                {t("dev.importDone", {
                  articles: dev.report.articles,
                  feeds: dev.report.feeds,
                  folders: dev.report.folders,
                  tags: dev.report.favoriteTags,
                  rules: dev.report.filterRules,
                })}
                {dev.report.settingsImported && ` ${t("dev.importSettings")}`}
                {dev.report.warnings.length > 0 &&
                  ` ${t("dev.importWarnings", { count: dev.report.warnings.length, list: dev.report.warnings.join(" ; ") })}`}
              </p>
            )}
            {dev.message && <p className={styles.error}>{dev.message}</p>}
          </>
        );
    }
  })();

  const current = SECTIONS.find((s) => s.id === section)!;
  return (
    <div className={styles.backdrop} onMouseDown={onClose}>
      <div className={styles.dialog} onMouseDown={(e) => e.stopPropagation()}>
        <nav className={styles.nav}>
          <h2 className={styles.navTitle}>{t("settings.title")}</h2>
          {SECTIONS.map((s) => (
            <button key={s.id} className={s.id === section ? styles.navItemActive : styles.navItem} onClick={() => onSection(s.id)}>
              {s.icon}
              {t(s.labelKey)}
            </button>
          ))}
        </nav>
        <div className={styles.content}>
          <div className={styles.header}>
            <h2>{t(current.labelKey)}</h2>
            <button className={styles.close} onClick={onClose} aria-label={t("filters.close")} title={t("filters.close")}>
              <X size={16} />
            </button>
          </div>
          <div className={styles.body}>{body}</div>
        </div>
      </div>
    </div>
  );
}
