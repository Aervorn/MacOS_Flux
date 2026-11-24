import { useTranslation } from "react-i18next";
import { CheckCheck, Eye, EyeOff, LayoutGrid, List, Menu, PanelTop, Search } from "lucide-react";
import type { ArticleLayout } from "../api";
import styles from "./Toolbar.module.css";

interface Props {
  title: string;
  search: string;
  onSearch: (s: string) => void;
  layout: ArticleLayout;
  onLayout: (l: ArticleLayout) => void;
  hideRead: boolean;
  canHideRead: boolean;
  onToggleHideRead: () => void;
  onMarkAllRead: () => void;
  /** Ouvre le menu de l'app (réglages, OPML, à propos). */
  onMenu: (e: React.MouseEvent<HTMLButtonElement>) => void;
}

export default function Toolbar({ title, search, onSearch, layout, onLayout, hideRead, canHideRead, onToggleHideRead, onMarkAllRead, onMenu }: Props) {
  const { t } = useTranslation();

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

      <label className={styles.search}>
        <Search size={14} />
        <input
          value={search}
          onChange={(e) => onSearch(e.target.value)}
          placeholder={t("toolbar.search")}
          spellCheck={false}
        />
      </label>

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
