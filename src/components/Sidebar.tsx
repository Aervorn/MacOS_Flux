import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, ChevronRight, CircleDot, FolderPlus, History, Inbox, Plus, RefreshCw, Rss, Star, Tag } from "lucide-react";
import type { Selection, Sidebar as SidebarData, SidebarFeed, SidebarFolder, SidebarTag } from "../api";
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
  refreshing: boolean;
  onRefresh: () => void;
  onAddFeed: () => void;
  onAddFolder: () => void;
  onFeedMenu: (e: React.MouseEvent, feed: SidebarFeed) => void;
  onFolderMenu: (e: React.MouseEvent, folder: SidebarFolder) => void;
  onTagMenu: (e: React.MouseEvent, tag: SidebarTag) => void;
  onToggleFolder: (uid: string, open: boolean) => void;
}

function Count({ n }: { n: number }) {
  return n > 0 ? <span className={styles.count}>{n}</span> : null;
}

export default function Sidebar({ data, selection, onSelect, favoritesExpanded, onToggleFavorites, footer, refreshing,
  onRefresh,
  onAddFeed,
  onAddFolder,
  onFeedMenu,
  onFolderMenu,
  onTagMenu,
  onToggleFolder,
}: Props) {
  const { t } = useTranslation();
  // L'état ouvert/fermé des dossiers n'est pas encore enregistré : on part de celui de l'app Swift.
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});

  const row = (
    sel: Selection,
    icon: React.ReactNode,
    label: string,
    count = 0,
    indent = false,
    onMenu?: (e: React.MouseEvent) => void,
  ) => (
    <button
      key={JSON.stringify(sel)}
      className={`${styles.row} ${sameSelection(selection, sel) ? styles.active : ""} ${indent ? styles.indent : ""}`}
      onClick={() => onSelect(sel)}
      onContextMenu={onMenu}
    >
      <span className={styles.icon}>{icon}</span>
      <span className={styles.label}>{label}</span>
      <Count n={count} />
    </button>
  );

  const feedRow = (f: SidebarFeed, indent: boolean) =>
    row({ kind: "feed", url: f.url }, <Rss size={16} />, f.title, f.unread, indent, (e) => onFeedMenu(e, f));

  const isOpen = (uid: string, expanded: boolean) => (uid in collapsed ? !collapsed[uid] : expanded);

  return (
    <aside className={styles.sidebar} data-noprint>
      <div className={styles.brand}>
        <span className={styles.brandName}>{t("app.name")}</span>
        <button className={styles.headerButton} onClick={onAddFolder} title={t("sidebar.addFolder")} aria-label={t("sidebar.addFolder")}>
          <FolderPlus size={16} />
        </button>
        <button className={styles.headerButton} onClick={onAddFeed} title={t("sidebar.addFeed")} aria-label={t("sidebar.addFeed")}>
          <Plus size={16} />
        </button>
        <button
          className={`${styles.headerButton} ${refreshing ? styles.spinning : ""}`}
          onClick={onRefresh}
          disabled={refreshing}
          title={t("sidebar.refresh")}
          aria-label={t("sidebar.refresh")}
        >
          <RefreshCw size={16} />
        </button>
      </div>

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
          data?.tags.map((tag) => row({ kind: "tag", uid: tag.uid }, <Tag size={15} />, tag.name, 0, true, (e) => onTagMenu(e, tag)))}

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
                  false,
                  (e) => onFolderMenu(e, folder),
                )}
                <button
                  className={styles.chevron}
                  onClick={() => {
                    setCollapsed((c) => ({ ...c, [folder.uid]: open }));
                    onToggleFolder(folder.uid, !open);
                  }}
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
