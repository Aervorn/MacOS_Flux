import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AlertTriangle, ChevronDown, ChevronRight, CircleDot, FolderPlus, History, Inbox, Plus, RefreshCw, Star, Tag } from "lucide-react";
import type { Selection, Sidebar as SidebarData, SidebarFeed, SidebarFolder, SidebarSmartFolder, SidebarTag } from "../api";
import { sameSelection } from "../api";
import Favicon from "./Favicon";
import { folderGlyph } from "../icons";
import { feedErrorDetail } from "../menus";
import styles from "./Sidebar.module.css";

interface Props {
  data: SidebarData | null;
  selection: Selection;
  onSelect: (s: Selection) => void;
  favoritesExpanded: boolean;
  onToggleFavorites: () => void;
  footer?: React.ReactNode;
  refreshing: boolean;
  onRefresh: () => void;
  onAddFeed: () => void;
  onAddFolder: () => void;
  onFeedMenu: (e: React.MouseEvent, feed: SidebarFeed) => void;
  onFolderMenu: (e: React.MouseEvent, folder: SidebarFolder) => void;
  onTagMenu: (e: React.MouseEvent, tag: SidebarTag) => void;
  onNewSmartFolder: () => void;
  onSmartFolderMenu: (e: React.MouseEvent, folder: SidebarSmartFolder) => void;
  onToggleFolder: (uid: string, open: boolean) => void;
  /** Un flux a été déposé sur un dossier (null : à la racine). */
  onMoveFeed: (url: string, folderUid: string | null) => void;
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
  onNewSmartFolder,
  onSmartFolderMenu,
  onToggleFolder,
  onMoveFeed,
}: Props) {
  const { t } = useTranslation();
  // L'état ouvert/fermé des dossiers n'est pas encore enregistré : on part de celui de l'app Swift.
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});

  // Glisser-déposer d'un flux vers un dossier, géré à la souris (le glisser-déposer HTML de la
  // WebView de macOS ne laisse pas déposer ici). Les cibles portent l'attribut `data-drop`
  // (uid du dossier, ou ROOT pour la racine).
  const ROOT = "__root";
  const [ghost, setGhost] = useState<{ title: string; x: number; y: number } | null>(null);
  const [dropKey, setDropKey] = useState<string | null>(null);
  const press = useRef<{ url: string; title: string; x: number; y: number } | null>(null);
  const moved = useRef(false);
  const hover = useRef<{ key: string | null; timer: number | null }>({ key: null, timer: null });
  // État le plus récent, lu par les écouteurs de la souris.
  const latest = useRef({ data, collapsed, onMoveFeed, onToggleFolder });
  latest.current = { data, collapsed, onMoveFeed, onToggleFolder };

  const targetAt = (x: number, y: number): string | null =>
    (document.elementFromPoint(x, y) as HTMLElement | null)?.closest<HTMLElement>("[data-drop]")?.dataset.drop ?? null;

  const clearHover = () => {
    if (hover.current.timer !== null) window.clearTimeout(hover.current.timer);
    hover.current = { key: null, timer: null };
  };

  const startFeedDrag = (e: React.MouseEvent, feed: SidebarFeed) => {
    if (e.button !== 0) return;
    press.current = { url: feed.url, title: feed.title, x: e.clientX, y: e.clientY };
    moved.current = false;

    const onMove = (ev: MouseEvent) => {
      const p = press.current;
      if (!p) return;
      if (!moved.current && Math.hypot(ev.clientX - p.x, ev.clientY - p.y) < 6) return;
      moved.current = true;
      document.body.style.cursor = "grabbing";
      setGhost({ title: p.title, x: ev.clientX, y: ev.clientY });
      const key = targetAt(ev.clientX, ev.clientY);
      setDropKey(key);
      if (key !== hover.current.key) {
        clearHover();
        hover.current.key = key;
        // Un dossier fermé s'ouvre quand on s'y attarde.
        const folder = key && key !== ROOT ? latest.current.data?.folders.find((f) => f.uid === key) : undefined;
        if (folder && !(folder.uid in latest.current.collapsed ? !latest.current.collapsed[folder.uid] : folder.isExpanded)) {
          hover.current.timer = window.setTimeout(() => {
            setCollapsed((c) => ({ ...c, [folder.uid]: false }));
            latest.current.onToggleFolder(folder.uid, true);
          }, 600);
        }
      }
    };
    const end = (ev: MouseEvent | null) => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      window.removeEventListener("keydown", onKey);
      document.body.style.cursor = "";
      const p = press.current;
      const dragged = moved.current;
      press.current = null;
      clearHover();
      setGhost(null);
      setDropKey(null);
      if (dragged) {
        // Le « clic » qui suit le relâchement ne doit pas sélectionner le flux.
        window.setTimeout(() => (moved.current = false), 0);
      }
      if (!p || !dragged || !ev) return;
      const key = targetAt(ev.clientX, ev.clientY);
      if (!key) return;
      const folderUid = key === ROOT ? null : key;
      const current = latest.current.data?.folders.find((f) => f.feeds.some((x) => x.url === p.url))?.uid ?? null;
      if (current !== folderUid) latest.current.onMoveFeed(p.url, folderUid);
    };
    const onUp = (ev: MouseEvent) => end(ev);
    const onKey = (ev: KeyboardEvent) => ev.key === "Escape" && end(null);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    window.addEventListener("keydown", onKey);
  };

  const row = (
    sel: Selection,
    icon: React.ReactNode,
    label: string,
    count = 0,
    indent = false,
    onMenu?: (e: React.MouseEvent) => void,
    feed?: SidebarFeed,
  ) => (
    <button
      key={JSON.stringify(sel)}
      className={`${styles.row} ${sameSelection(selection, sel) ? styles.active : ""} ${indent ? styles.indent : ""}`}
      onClick={() => !moved.current && onSelect(sel)}
      onContextMenu={onMenu}
      onMouseDown={feed ? (e) => startFeedDrag(e, feed) : undefined}
    >
      <span className={styles.icon}>{icon}</span>
      <span className={styles.label}>{label}</span>
      {feed?.error && (
        <span className={styles.errorBadge} title={`${t("sidebar.feedError")}\n${feedErrorDetail(t, feed.error, feed.lastRefreshed)}`}>
          <AlertTriangle size={14} />
        </span>
      )}
      <Count n={count} />
    </button>
  );

  const feedRow = (f: SidebarFeed, indent: boolean) =>
    row({ kind: "feed", url: f.url }, <Favicon siteUrl={f.siteUrl} />, f.title, f.unread, indent, (e) => onFeedMenu(e, f), f);

  const isOpen = (uid: string, expanded: boolean) => (uid in collapsed ? !collapsed[uid] : expanded);

  return (
    <aside className={styles.sidebar} data-noprint>
      {/* Place des boutons rouge/jaune/vert de la fenêtre, et zone pour la déplacer */}
      <div className={styles.titlebar} data-tauri-drag-region />
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

        {/* Dossiers intelligents : au même niveau que « Flux » */}
        <div className={styles.section}>
          <span className={styles.sectionTitle}>{t("sidebar.smartFolders")}</span>
          <button
            className={styles.sectionButton}
            onClick={onNewSmartFolder}
            title={t("sidebar.addSmartFolder")}
            aria-label={t("sidebar.addSmartFolder")}
          >
            <Plus size={14} />
          </button>
        </div>
        {data && data.smartFolders.length === 0 && <p className={styles.hint}>{t("sidebar.noSmartFolders")}</p>}
        {data?.smartFolders.map((folder) =>
          row(
            { kind: "smart", uid: folder.uid },
            folderGlyph(folder.symbol ?? "sparkles", folder.colorName),
            folder.name,
            folder.unread,
            false,
            (e) => onSmartFolderMenu(e, folder),
          ),
        )}

        <div className={`${styles.section} ${dropKey === ROOT ? styles.dropTarget : ""}`} data-drop={ROOT}>
          {t("sidebar.feeds")}
        </div>
        {data && data.folders.length === 0 && data.feeds.length === 0 && <p className={styles.hint}>{t("sidebar.noFeeds")}</p>}

        {data?.folders.map((folder) => {
          const open = isOpen(folder.uid, folder.isExpanded);
          return (
            <div
              key={folder.uid}
              className={`${styles.block} ${dropKey === folder.uid ? styles.dropTarget : ""}`}
              data-drop={folder.uid}
            >
              <div className={styles.group}>
                {row(
                  { kind: "folder", uid: folder.uid },
                  folderGlyph(folder.symbol, folder.colorName),
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

        {data && data.feeds.length > 0 && (
          <div className={`${styles.block} ${dropKey === ROOT ? styles.dropTarget : ""}`} data-drop={ROOT}>
            {data.feeds.map((f) => feedRow(f, false))}
          </div>
        )}
      </nav>

      {footer && <div className={styles.footer}>{footer}</div>}
      {ghost && (
        <div className={styles.ghost} style={{ left: ghost.x + 12, top: ghost.y + 8 }}>
          {ghost.title}
        </div>
      )}
    </aside>
  );
}
