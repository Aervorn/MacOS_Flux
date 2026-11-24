import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, sameSelection } from "./api";
import type { AppInfo, ArticleRow, ImportReport, Prefs, Selection, Sidebar as SidebarData } from "./api";
import { hasSavedLang, setLang, type Lang } from "./i18n";
import { errorMessage } from "./errors";
import { applyTheme, appearanceToMode, modeToAppearance, type ThemeMode } from "./theme";
import ArticleList from "./components/ArticleList";
import Sidebar from "./components/Sidebar";
import AddFeedDialog from "./components/AddFeedDialog";
import ContextMenu, { type MenuItem } from "./components/ContextMenu";
import { ConfirmDialog, PromptDialog } from "./components/Dialogs";
import { articleMenu, feedMenu, folderMenu, tagMenu, type Ask, type MenuContext } from "./menus";
import Reader from "./components/Reader";
import Toolbar from "./components/Toolbar";
import styles from "./App.module.css";

const THEMES: { mode: ThemeMode; labelKey: string }[] = [
  { mode: "light", labelKey: "settings.appearanceLight" },
  { mode: "dark", labelKey: "settings.appearanceDark" },
  { mode: "auto", labelKey: "settings.appearanceAuto" },
];

export default function App() {
  const { t, i18n } = useTranslation();
  const [prefs, setPrefs] = useState<Prefs | null>(null);
  const [sidebar, setSidebar] = useState<SidebarData | null>(null);
  const [selection, setSelection] = useState<Selection>({ kind: "all" });
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [articles, setArticles] = useState<ArticleRow[]>([]);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [reloadTick, setReloadTick] = useState(0);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [showAddFeed, setShowAddFeed] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const [asking, setAsking] = useState<Ask | null>(null);

  // La clé sélectionnée sert à garder l'article visible à la prochaine requête, sans la relancer.
  const selectedKeyRef = useRef<string | null>(null);
  selectedKeyRef.current = selectedKey;

  const loadSettings = useCallback(async () => {
    const { prefs: loaded, language } = await api.settings();
    setPrefs(loaded);
    applyTheme(appearanceToMode(loaded.appearance));
    if ((language === "fr" || language === "en") && !hasSavedLang()) setLang(language);
  }, []);

  const loadSidebar = useCallback(async () => setSidebar(await api.sidebar()), []);

  useEffect(() => {
    api.appInfo().then(setInfo).catch(() => setInfo(null)); // hors Tauri (navigateur seul), on ignore
    loadSettings().catch((e) => setMessage(String(e)));
    loadSidebar().catch((e) => setMessage(String(e)));
  }, [loadSettings, loadSidebar]);

  useEffect(() => {
    const id = setTimeout(() => setDebouncedSearch(search), 200);
    return () => clearTimeout(id);
  }, [search]);

  const canHideRead = selection.kind === "all" || selection.kind === "folder" || selection.kind === "feed";
  const hideRead = (prefs?.hideRead ?? false) && canHideRead;
  const hideTagged = prefs?.hideTaggedFavorites ?? false;
  const ready = prefs !== null;

  useEffect(() => {
    if (!ready) return;
    let cancelled = false;
    api
      .articles({
        selection,
        search: debouncedSearch,
        hideRead,
        hideTaggedFavorites: hideTagged,
        sticky: selectedKeyRef.current ? [selectedKeyRef.current] : [],
      })
      .then((rows) => {
        if (!cancelled) setArticles(rows);
      })
      .catch((e) => setMessage(String(e)));
    return () => {
      cancelled = true;
    };
  }, [ready, selection, debouncedSearch, hideRead, hideTagged, reloadTick]);

  // Après une action (lu, favori, tags…) : compteurs et liste à jour.
  const refresh = useCallback(() => {
    loadSidebar().catch((e) => setMessage(String(e)));
    setReloadTick((n) => n + 1);
  }, [loadSidebar]);

  const markAllRead = async () => {
    try {
      await api.markAllRead(selection, prefs?.hideTaggedFavorites ?? false);
      refresh();
    } catch (e) {
      setMessage(String(e));
    }
  };

  // Actualisation de tous les flux, puis compteurs et liste à jour.
  const refreshFeeds = useCallback(async () => {
    setRefreshing(true);
    try {
      const report = await api.refreshAll();
      if (report.added > 0 || report.failed > 0) refresh();
    } catch (e) {
      setMessage(errorMessage(e, t));
    } finally {
      setRefreshing(false);
    }
  }, [refresh, t]);

  // Au lancement (une fois les réglages chargés), puis à intervalle régulier.
  const refreshMinutes = prefs?.refreshMinutes ?? null;
  useEffect(() => {
    if (refreshMinutes === null) return;
    void refreshFeeds();
    const id = setInterval(() => void refreshFeeds(), Math.max(refreshMinutes, 5) * 60_000);
    return () => clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshMinutes]);

  const menuContext = (): MenuContext => ({
    t,
    sidebar,
    selection,
    refresh,
    select: (s) => {
      setSelection(s);
      setSelectedKey(null);
    },
    ask: setAsking,
    fail: (e) => setMessage(errorMessage(e, t)),
    hideTaggedFavorites: prefs?.hideTaggedFavorites ?? false,
  });

  const openMenu = (e: React.MouseEvent, items: MenuItem[]) => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY, items });
  };

  const openArticleMenu = async (e: React.MouseEvent, article: ArticleRow) => {
    e.preventDefault();
    const { clientX: x, clientY: y } = e;
    const detail = await api.article(article.key).catch(() => null);
    setMenu({ x, y, items: articleMenu(menuContext(), article, detail) });
  };

  const createFolder = () =>
    setAsking({
      kind: "prompt",
      title: t("dialogs.newFolder"),
      placeholder: t("dialogs.folderName"),
      confirmLabel: t("dialogs.create"),
      onSubmit: (name) => {
        api.createFolder(name).then(refresh).catch((err) => setMessage(errorMessage(err, t)));
      },
    });

  const updatePrefs = (patch: Partial<Prefs>) => {
    if (!prefs) return;
    const next = { ...prefs, ...patch };
    setPrefs(next);
    api.savePrefs(next).catch((e) => setMessage(String(e)));
  };

  const select = (s: Selection) => {
    if (sameSelection(s, selection)) return;
    setSelection(s);
    setSelectedKey(null);
  };

  const importData = async () => {
    setReport(null);
    setMessage(null);
    try {
      setReport(await api.devImport());
      await loadSettings();
      await loadSidebar();
      setSelectedKey(null);
      setReloadTick((n) => n + 1);
    } catch (e) {
      setMessage(t("dev.importError", { message: String(e) }));
    }
  };

  const printPage = async () => {
    setMessage(null);
    try {
      await api.printPage();
    } catch (e) {
      setMessage(t("dev.printError", { message: String(e) }));
    }
  };

  if (!prefs) return null;

  const title = (() => {
    switch (selection.kind) {
      case "all":
        return t("sidebar.allArticles");
      case "unread":
        return t("sidebar.unread");
      case "starred":
        return t("sidebar.favorites");
      case "history":
        return t("sidebar.history");
      case "folder":
        return sidebar?.folders.find((f) => f.uid === selection.uid)?.name ?? "";
      case "tag":
        return sidebar?.tags.find((tag) => tag.uid === selection.uid)?.name ?? "";
      case "feed": {
        const all = [...(sidebar?.feeds ?? []), ...(sidebar?.folders.flatMap((f) => f.feeds) ?? [])];
        return all.find((f) => f.url === selection.url)?.title ?? "";
      }
    }
  })();

  const currentMode = appearanceToMode(prefs.appearance);

  const footer = (
    <div className={styles.footer}>
      <span className={styles.label}>{t("settings.appearance")}</span>
      <div className={styles.segmented}>
        {THEMES.map(({ mode, labelKey }) => (
          <button
            key={mode}
            className={currentMode === mode ? styles.segmentActive : styles.segment}
            onClick={() => {
              applyTheme(mode);
              updatePrefs({ appearance: modeToAppearance(mode) });
            }}
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

      <details className={styles.dev}>
        <summary>Dev</summary>
        {info && <p className={styles.label}>{t("dev.version", { version: info.version, core: info.coreVersion })}</p>}
        <button className={styles.devButton} onClick={printPage}>
          {t("dev.print")}
        </button>
        <button className={styles.devButton} onClick={importData}>
          {t("dev.import")}
        </button>
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
        {message && <p className={styles.error}>{message}</p>}
      </details>
    </div>
  );

  const list = (
    <div className={styles.scroll}>
      <ArticleList
        articles={articles}
        layout={prefs.articleLayout}
        selectedKey={selectedKey}
        showSummary={prefs.showSummary}
        onSelect={setSelectedKey}
        onMenu={(e, article) => void openArticleMenu(e, article)}
      />
    </div>
  );

  return (
    <div className={styles.app}>
      <Sidebar
        data={sidebar}
        selection={selection}
        onSelect={select}
        favoritesExpanded={prefs.favoritesExpanded}
        onToggleFavorites={() => updatePrefs({ favoritesExpanded: !prefs.favoritesExpanded })}
        footer={footer}
        refreshing={refreshing}
        onRefresh={() => void refreshFeeds()}
        onAddFeed={() => setShowAddFeed(true)}
        onAddFolder={createFolder}
        onFeedMenu={(e, feed) => openMenu(e, feedMenu(menuContext(), feed))}
        onFolderMenu={(e, folder) => openMenu(e, folderMenu(menuContext(), folder))}
        onTagMenu={(e, tag) => openMenu(e, tagMenu(menuContext(), tag))}
        onToggleFolder={(uid, open) => {
          api.setFolderExpanded(uid, open).catch(() => undefined);
        }}
      />

      <main className={styles.main}>
        <Toolbar
          title={title}
          search={search}
          onSearch={setSearch}
          layout={prefs.articleLayout}
          onLayout={(articleLayout) => updatePrefs({ articleLayout })}
          hideRead={hideRead}
          canHideRead={canHideRead}
          onToggleHideRead={() => updatePrefs({ hideRead: !prefs.hideRead })}
          onMarkAllRead={markAllRead}
        />

        {prefs.articleLayout === "list" ? (
          <div className={styles.split}>
            <div className={styles.listColumn} data-noprint style={{ width: prefs.articleListWidth }}>
              {list}
            </div>
            <section className={styles.detail}>
              {selectedKey ? (
                <Reader
                  articleKey={selectedKey}
                  prefs={prefs}
                  onPrefs={updatePrefs}
                  onOpenArticle={setSelectedKey}
                  tags={sidebar?.tags ?? []}
                  onChanged={refresh}
                />
              ) : (
                <p className={styles.placeholder}>{t("detail.select")}</p>
              )}
            </section>
          </div>
        ) : selectedKey ? (
          <Reader
            articleKey={selectedKey}
            prefs={prefs}
            onPrefs={updatePrefs}
            onOpenArticle={setSelectedKey}
            tags={sidebar?.tags ?? []}
            onChanged={refresh}
            onClose={() => setSelectedKey(null)}
          />
        ) : (
          list
        )}
      </main>
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
      {asking?.kind === "prompt" && (
        <PromptDialog
          title={asking.title}
          initial={asking.initial}
          placeholder={asking.placeholder}
          confirmLabel={asking.confirmLabel}
          allowEmpty={asking.allowEmpty}
          onSubmit={(value) => {
            setAsking(null);
            asking.onSubmit(value);
          }}
          onClose={() => setAsking(null)}
        />
      )}
      {asking?.kind === "confirm" && (
        <ConfirmDialog
          title={asking.title}
          message={asking.message}
          confirmLabel={asking.confirmLabel}
          onConfirm={asking.onConfirm}
          onClose={() => setAsking(null)}
        />
      )}
      {showAddFeed && (
        <AddFeedDialog
          folders={sidebar?.folders ?? []}
          initialFolderUid={selection.kind === "folder" ? selection.uid : null}
          onClose={() => setShowAddFeed(false)}
          onAdded={(feedUrl) => {
            setShowAddFeed(false);
            refresh();
            select({ kind: "feed", url: feedUrl });
          }}
        />
      )}
    </div>
  );
}
