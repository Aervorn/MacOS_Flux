import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { CheckCheck, Eye, EyeOff, LayoutGrid, List, Menu, PanelTop, Search, SlidersHorizontal, WifiOff } from "lucide-react";
import type { ArticleLayout } from "../api";
import styles from "./Toolbar.module.css";

interface Props {
  title: string;
  search: string;
  onSearch: (s: string) => void;
  /** Où chercher : dans le titre, dans le corps de l'article, ou dans les deux. */
  searchInTitle: boolean;
  searchInBody: boolean;
  onSearchScope: (inTitle: boolean, inBody: boolean) => void;
  layout: ArticleLayout;
  onLayout: (l: ArticleLayout) => void;
  hideRead: boolean;
  canHideRead: boolean;
  onToggleHideRead: () => void;
  onMarkAllRead: () => void;
  /** Ouvre le menu de l'app (réglages, OPML, à propos). */
  onMenu: (e: React.MouseEvent<HTMLButtonElement>) => void;
  /** Pas de connexion : on affiche une pastille « Hors ligne ». */
  offline?: boolean;
}

export default function Toolbar({ title, search, onSearch, searchInTitle, searchInBody, onSearchScope, layout, onLayout, hideRead, canHideRead, onToggleHideRead, onMarkAllRead, onMenu, offline }: Props) {
  const { t } = useTranslation();
  const [scopeOpen, setScopeOpen] = useState(false);
  const scopeRef = useRef<HTMLDivElement>(null);

  // Le menu des options de recherche se ferme au clic ailleurs ou avec Échap.
  useEffect(() => {
    if (!scopeOpen) return;
    const onDown = (e: MouseEvent) => {
      if (!scopeRef.current?.contains(e.target as Node)) setScopeOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setScopeOpen(false);
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [scopeOpen]);

  const scopeChanged = !(searchInTitle && searchInBody);

  const layoutButton = (value: ArticleLayout, icon: React.ReactNode, label: string) => (
    <button
      className={`${styles.iconButton} ${layout === value ? styles.active : ""}`}
      onClick={() => onLayout(value)}
      title={label}
      aria-label={label}
    >
      {icon}
    </button>
  );

  return (
    <header className={styles.toolbar} data-noprint data-tauri-drag-region>
      <h1 className={styles.title} data-tauri-drag-region>{title}</h1>
      {offline && (
        <span className={styles.offline} title={t("toolbar.offlineHelp")}>
          <WifiOff size={13} />
          {t("toolbar.offline")}
        </span>
      )}

      <label className={styles.search}>
        <Search size={14} />
        <input
          value={search}
          onChange={(e) => onSearch(e.target.value)}
          placeholder={t("toolbar.search")}
          spellCheck={false}
        />
      </label>

      <div className={styles.scope} ref={scopeRef}>
        <button
          className={`${styles.iconButton} ${scopeChanged || scopeOpen ? styles.active : ""}`}
          onClick={() => setScopeOpen((o) => !o)}
          title={t("toolbar.searchScope")}
          aria-label={t("toolbar.searchScope")}
          aria-expanded={scopeOpen}
        >
          <SlidersHorizontal size={16} />
        </button>
        {scopeOpen && (
          <div className={styles.scopeMenu} role="group" aria-label={t("toolbar.searchScope")}>
            <span className={styles.scopeHeading}>{t("toolbar.searchIn")}</span>
            {/* Une case au moins reste cochée : on ne peut pas décocher la dernière. */}
            <label className={styles.scopeOption}>
              <input
                type="checkbox"
                checked={searchInTitle}
                disabled={searchInTitle && !searchInBody}
                onChange={(e) => onSearchScope(e.target.checked, searchInBody)}
              />
              {t("toolbar.searchTitle")}
            </label>
            <label className={styles.scopeOption}>
              <input
                type="checkbox"
                checked={searchInBody}
                disabled={searchInBody && !searchInTitle}
                onChange={(e) => onSearchScope(searchInTitle, e.target.checked)}
              />
              {t("toolbar.searchBody")}
            </label>
          </div>
        )}
      </div>

      <button
        className={styles.iconButton}
        onClick={onMarkAllRead}
        title={t("toolbar.markAllRead")}
        aria-label={t("toolbar.markAllRead")}
      >
        <CheckCheck size={16} />
      </button>

      {canHideRead && (
        <button
          className={`${styles.iconButton} ${hideRead ? styles.active : ""}`}
          onClick={onToggleHideRead}
          title={hideRead ? t("toolbar.showRead") : t("toolbar.hideRead")}
          aria-label={hideRead ? t("toolbar.showRead") : t("toolbar.hideRead")}
        >
          {hideRead ? <EyeOff size={16} /> : <Eye size={16} />}
        </button>
      )}

      <div className={styles.segmented}>
        {layoutButton("list", <List size={16} />, t("toolbar.layoutList"))}
        {layoutButton("grid", <LayoutGrid size={16} />, t("toolbar.layoutGrid"))}
        {layoutButton("gridFull", <PanelTop size={16} />, t("toolbar.layoutGridFull"))}
      </div>

      <button className={styles.iconButton} onClick={onMenu} title={t("settings.menu")} aria-label={t("settings.menu")}>
        <Menu size={16} />
      </button>
    </header>
  );
}
