import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, ChevronRight, CircleDot, History, Inbox, Rss, Star, Tag } from "lucide-react";
import type { Selection, Sidebar as SidebarData, SidebarFeed } from "../api";
import { sameSelection } from "../api";
import { folderColor, folderIcon } from "../icons";
import styles from "./Sidebar.module.css";

interface Props {
  data: SidebarData | null;
  selection: Selection;
  onSelect: (s: Selection) => void;
  favoritesExpanded: boolean;
  onToggleFavorites: () => void;
  footer: React.ReactNode;
}

function Count({ n }: { n: number }) {
  return n > 0 ? <span className={styles.count}>{n}</span> : null;
}

export default function Sidebar({ data, selection, onSelect, favoritesExpanded, onToggleFavorites, footer }: Props) {
  const { t } = useTranslation();
  // L'état ouvert/fermé des dossiers n'est pas encore enregistré : on part de celui de l'app Swift.
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});

  const row = (sel: Selection, icon: React.ReactNode, label: string, count = 0, indent = false) => (
    <button
      key={JSON.stringify(sel)}
      className={`${styles.row} ${sameSelection(selection, sel) ? styles.active : ""} ${indent ? styles.indent : ""}`}
      onClick={() => onSelect(sel)}
    >
      <span className={styles.icon}>{icon}</span>
      <span className={styles.label}>{label}</span>
      <Count n={count} />
    </button>
  );

  const feedRow = (f: SidebarFeed, indent: boolean) =>
    row({ kind: "feed", url: f.url }, <Rss size={16} />, f.title, f.unread, indent);

  const isOpen = (uid: string, expanded: boolean) => (uid in collapsed ? !collapsed[uid] : expanded);

  return (
    <aside className={styles.sidebar}>
      <div className={styles.brand}>{t("app.name")}</div>

      <nav className={styles.nav}>
        {row({ kind: "all" }, <Inbox size={16} />, t("sidebar.allArticles"))}
        {row({ kind: "unread" }, <CircleDot size={16} />, t("sidebar.unread"), data?.unread ?? 0)}

        <div className={styles.group}>
          {row({ kind: "starred" }, <Star size={16} />, t("sidebar.favorites"))}
          {data && data.tags.length > 0 && (
            <button className={styles.chevron} onClick={onToggleFavorites} aria-label={t("sidebar.favorites")}>
              {favoritesExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
            </button>
          )}
        </div>
        {favoritesExpanded &&
          data?.tags.map((tag) => row({ kind: "tag", uid: tag.uid }, <Tag size={15} />, tag.name, 0, true))}

        {row({ kind: "history" }, <History size={16} />, t("sidebar.history"))}

        <div className={styles.section}>{t("sidebar.feeds")}</div>
        {data && data.folders.length === 0 && data.feeds.length === 0 && (
          <p className={styles.hint}>{t("sidebar.noFeeds")}</p>
        )}

        {data?.folders.map((folder) => {
          const Icon = folderIcon(folder.symbol);
          const open = isOpen(folder.uid, folder.isExpanded);
          return (
            <div key={folder.uid}>
              <div className={styles.group}>
                {row(
                  { kind: "folder", uid: folder.uid },
                  <Icon size={16} color={folderColor(folder.colorName)} />,
                  folder.name,
                  folder.unread,
                )}
                <button
                  className={styles.chevron}
                  onClick={() => setCollapsed((c) => ({ ...c, [folder.uid]: open }))}
                  aria-label={folder.name}
                >
                  {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                </button>
              </div>
              {open && folder.feeds.map((f) => feedRow(f, true))}
            </div>
          );
        })}

        {data?.feeds.map((f) => feedRow(f, false))}
      </nav>

      <div className={styles.footer}>{footer}</div>
    </aside>
  );
}
