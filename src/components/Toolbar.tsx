import { useTranslation } from "react-i18next";
import { Eye, EyeOff, LayoutGrid, List, PanelTop, Search } from "lucide-react";
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
}

export default function Toolbar({ title, search, onSearch, layout, onLayout, hideRead, canHideRead, onToggleHideRead }: Props) {
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
    <header className={styles.toolbar}>
      <h1 className={styles.title}>{title}</h1>

      <label className={styles.search}>
        <Search size={14} />
        <input
          value={search}
          onChange={(e) => onSearch(e.target.value)}
          placeholder={t("toolbar.search")}
          spellCheck={false}
        />
      </label>

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
    </header>
  );
}
