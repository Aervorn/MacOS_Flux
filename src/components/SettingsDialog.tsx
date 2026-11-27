import { useEffect, useState } from "react";
import { api, type AiStatus } from "../api";
import { useTranslation } from "react-i18next";
import { Archive, Bell, BookOpen, Cloud, Palette, Rss, SlidersHorizontal, X } from "lucide-react";
import type { Prefs, SidebarFeed, SidebarFolder, SidebarSmartFolder } from "../api";
import { setLang, type Lang } from "../i18n";
import type { ThemeChoice } from "../theme";
import AppearanceThemes from "./AppearanceThemes";
import NotificationSettings from "./NotificationSettings";
import { ARTICLE_FONTS, UI_FONTS } from "../fonts";
import FiltersPanel from "./FiltersPanel";
import BackupSection from "./BackupSection";
import SyncSection from "./SyncSection";
import ArchiveSettings from "./ArchiveSettings";
import OfflineSettings from "./OfflineSettings";
import Select from "./Select";
import styles from "./SettingsDialog.module.css";

export type Section = "appearance" | "reading" | "feeds" | "notifications" | "filters" | "sync" | "backup";

const SECTIONS: { id: Section; icon: React.ReactNode; labelKey: string }[] = [
  { id: "appearance", icon: <Palette size={16} />, labelKey: "settings.sectionAppearance" },
  { id: "reading", icon: <BookOpen size={16} />, labelKey: "settings.sectionReading" },
  { id: "feeds", icon: <Rss size={16} />, labelKey: "settings.sectionFeeds" },
  { id: "notifications", icon: <Bell size={16} />, labelKey: "settings.sectionNotifications" },
  { id: "filters", icon: <SlidersHorizontal size={16} />, labelKey: "settings.sectionFilters" },
  { id: "sync", icon: <Cloud size={16} />, labelKey: "settings.sectionSync" },
  { id: "backup", icon: <Archive size={16} />, labelKey: "settings.sectionBackup" },
];

const MIN_FONT = 13;
const MAX_FONT = 28;

/** Valeurs proposées, plus la valeur actuelle si elle n'y figure pas (réglage importé). */
const withCurrent = (values: number[], current: number) => (values.includes(current) ? values : [...values, current].sort((a, b) => a - b));

interface Props {
  section: Section;
  onSection: (s: Section) => void;
  prefs: Prefs;
  onPrefs: (patch: Partial<Prefs>) => void;
  onTheme: (patch: Partial<ThemeChoice>) => void;
  onDuplicates: (level: number) => void;
  feeds: SidebarFeed[];
  folders: SidebarFolder[];
  smartFolders: SidebarSmartFolder[];
  /** Recharge compteurs et liste (après un changement de filtres). */
  onChanged: () => void;
  onClose: () => void;
}

export default function SettingsDialog({ section, onSection, prefs, onPrefs, onTheme, onDuplicates, feeds, folders, smartFolders, onChanged, onClose }: Props) {
  const { t, i18n } = useTranslation();
  const [filtersEditing, setFiltersEditing] = useState(false);
  const [ai, setAi] = useState<AiStatus>("unsupported");
  useEffect(() => {
    api.aiStatus().then(setAi).catch(() => undefined);
  }, []);

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
            <AppearanceThemes prefs={prefs} onTheme={onTheme} />
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
            {row(
              t("settings.summaryLines"),
              <div className={styles.stepper}>
                <button onClick={() => onPrefs({ summaryLines: Math.max(0, prefs.summaryLines - 1) })} disabled={prefs.summaryLines <= 0}>
                  −
                </button>
                <span>{prefs.summaryLines === 0 ? t("settings.summaryNone") : t("settings.summaryLinesCount", { count: prefs.summaryLines })}</span>
                <button onClick={() => onPrefs({ summaryLines: Math.min(4, prefs.summaryLines + 1) })} disabled={prefs.summaryLines >= 4}>
                  +
                </button>
              </div>,
            )}
            {ai !== "unsupported" && ai !== "deviceNotEligible" && (
              <div className={styles.row}>
                {toggle(t("ai.autoSummarize"), prefs.autoSummarize, (autoSummarize) => onPrefs({ autoSummarize }))}
                <p className={styles.help}>{t("ai.autoSummarizeHelp")}</p>
              </div>
            )}
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
            <OfflineSettings prefs={prefs} onPrefs={onPrefs} toggle={toggle} select={select} />
            <ArchiveSettings prefs={prefs} onPrefs={onPrefs} toggle={toggle} />
          </>
        );
      case "notifications":
        return (
          <NotificationSettings
            prefs={prefs}
            onPrefs={onPrefs}
            feeds={feeds}
            folders={folders}
            smartFolders={smartFolders}
            toggle={toggle}
            segmented={segmented}
          />
        );
      case "filters":
        return <FiltersPanel feeds={feeds} onChanged={onChanged} onEditingChange={setFiltersEditing} />;
      case "sync":
        return <SyncSection prefs={prefs} onPrefs={onPrefs} onChanged={onChanged} />;
      case "backup":
        return <BackupSection prefs={prefs} onPrefs={onPrefs} />;
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
