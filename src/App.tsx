import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { api, sameSelection } from "./api";
import type { AppInfo, ArticleRow, Prefs, Selection, Sidebar as SidebarData } from "./api";
import { hasSavedLang, setLang } from "./i18n";
import { errorMessage } from "./errors";
import { applyTheme, appearanceToMode, modeToAppearance } from "./theme";
import { applyLook } from "./fonts";
import ArticleList from "./components/ArticleList";
import Resizer from "./components/Resizer";
import Sidebar from "./components/Sidebar";
import AddFeedDialog from "./components/AddFeedDialog";
import SettingsDialog, { type Section } from "./components/SettingsDialog";
import ContextMenu, { type MenuItem } from "./components/ContextMenu";
import { BarChart3, Compass, Download, Info, RefreshCw, Settings, SlidersHorizontal, Upload } from "lucide-react";
import { ConfirmDialog, InfoDialog, PromptDialog } from "./components/Dialogs";
import { open as openFile, save as saveFile } from "@tauri-apps/plugin-dialog";
import { articleMenu, articlesMenu, feedMenu, folderMenu, tagMenu, type Ask, type MenuContext } from "./menus";
import Reader from "./components/Reader";
import Toolbar from "./components/Toolbar";
import Onboarding from "./components/Onboarding";
import StatsDialog from "./components/StatsDialog";
import AboutDialog from "./components/AboutDialog";
import { prepareOffline } from "./offline";
import styles from "./App.module.css";

export default function App() {
  const { t } = useTranslation();
  const [prefs, setPrefs] = useState<Prefs | null>(null);
  const [sidebar, setSidebar] = useState<SidebarData | null>(null);
  const [selection, setSelection] = useState<Selection>({ kind: "all" });
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [articles, setArticles] = useState<ArticleRow[]>([]);
  // L'article ouvert dans le lecteur, les articles sélectionnés (⌘-clic, ⇧-clic…) et le curseur du clavier.
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [selectedKeys, setSelectedKeys] = useState<string[]>([]);
  const [cursorKey, setCursorKey] = useState<string | null>(null);
  const anchorRef = useRef<string | null>(null);
  const [reloadTick, setReloadTick] = useState(0);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [tour, setTour] = useState(false);
  const [showStats, setShowStats] = useState(false);
  const [showAbout, setShowAbout] = useState(false);
  const [update, setUpdate] = useState<string | null>(null);
  const [swiftImport, setSwiftImport] = useState<{ available: boolean; dismissed: boolean }>({ available: false, dismissed: false });
  const [refreshing, setRefreshing] = useState(false);
  const [showAddFeed, setShowAddFeed] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const [asking, setAsking] = useState<Ask | null>(null);
  const [settings, setSettings] = useState<Section | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

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
    api.checkUpdate().then((u) => setUpdate(u?.version ?? null)).catch(() => undefined); // silencieux : pas de réseau, ou hors Tauri
    api.swiftImportStatus().then(setSwiftImport).catch(() => undefined);
    api.archivePending().catch(() => undefined); // favoris arrivés par la synchro ou l'import
    loadSettings().catch((e) => setNotice(String(e)));
    loadSidebar().catch((e) => setNotice(String(e)));
  }, [loadSettings, loadSidebar]);

  useEffect(() => {
    const id = setTimeout(() => setDebouncedSearch(search), 200);
    return () => clearTimeout(id);
  }, [search]);

  // Polices et densité : à l'ouverture, puis à chaque changement dans les réglages.
  useEffect(() => {
    if (prefs) applyLook(prefs);
  }, [prefs?.uiFont, prefs?.articleFont, prefs?.sidebarDensity]); // eslint-disable-line react-hooks/exhaustive-deps

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
      .catch((e) => setNotice(String(e)));
    return () => {
      cancelled = true;
    };
  }, [ready, selection, debouncedSearch, hideRead, hideTagged, reloadTick]);

  // Après une action (lu, favori, tags…) : compteurs et liste à jour.
  const refresh = useCallback(() => {
    loadSidebar().catch((e) => setNotice(String(e)));
    setReloadTick((n) => n + 1);
  }, [loadSidebar]);

  const markAllRead = async () => {
    try {
      await api.markAllRead(selection, prefs?.hideTaggedFavorites ?? false);
      refresh();
    } catch (e) {
      setNotice(String(e));
    }
  };

  // Actualisation de tous les flux, puis compteurs et liste à jour.
  // Connexion : on vérifie vraiment (le navigateur intégré ne prévient pas toujours d'une coupure).
  const [online, setOnline] = useState(true);
  const checkOnline = useCallback(async () => {
    const ok = await api.isOnline().catch(() => navigator.onLine);
    setOnline(ok);
    return ok;
  }, []);

  const refreshFeeds = useCallback(
    async (manual = false) => {
      if (!(await checkOnline())) {
        if (manual) setNotice(t("offline.refresh"));
        return; // hors ligne : on garde ce qui est déjà là
      }
      setRefreshing(true);
      try {
        const report = await api.refreshAll();
        if (report.added > 0 || report.failed > 0) refresh();
      } catch (e) {
        setNotice(errorMessage(e, t));
      } finally {
        setRefreshing(false);
      }
      // Lecture hors ligne (si activée) et taille du cache d'images : sans bloquer l'interface
      void prepareOffline();
    },
    [refresh, t, checkOnline],
  );

  // Connexion perdue ou retrouvée : indicateur « Hors ligne », et actualisation au retour.
  const wasOnline = useRef(true);
  useEffect(() => {
    const check = () =>
      void checkOnline().then((ok) => {
        if (ok && !wasOnline.current) void refreshFeedsRef.current();
        wasOnline.current = ok;
      });
    check();
    const id = setInterval(check, 15_000);
    window.addEventListener("online", check);
    window.addEventListener("offline", check);
    window.addEventListener("focus", check);
    return () => {
      clearInterval(id);
      window.removeEventListener("online", check);
      window.removeEventListener("offline", check);
      window.removeEventListener("focus", check);
    };
  }, [checkOnline]);
  const refreshFeedsRef = useRef(refreshFeeds);
  refreshFeedsRef.current = refreshFeeds;

  // Au lancement (une fois les réglages chargés), puis à intervalle régulier.
  const refreshMinutes = prefs?.refreshMinutes ?? null;
  useEffect(() => {
    if (refreshMinutes === null) return;
    void refreshFeeds();
    const id = setInterval(() => void refreshFeeds(), Math.max(refreshMinutes, 5) * 60_000);
    return () => clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshMinutes]);

  // Synchro iCloud : un autre Mac a changé des choses (la liste se recharge ; les nouveaux flux
  // sont actualisés), et on resynchronise au retour dans l'app.
  const syncEnabled = prefs?.syncEnabled ?? false;
  useEffect(() => {
    let off: (() => void) | undefined;
    listen<number>("sync-changed", (e) => {
      refresh();
      if (e.payload > 0) void refreshFeeds();
    })
      .then((un) => (off = un))
      .catch(() => undefined);
    const onFocus = () => {
      if (syncEnabled) api.syncNow().catch(() => undefined);
    };
    window.addEventListener("focus", onFocus);
    return () => {
      off?.();
      window.removeEventListener("focus", onFocus);
    };
  }, [refresh, refreshFeeds, syncEnabled]);

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
    fail: (e) => setNotice(errorMessage(e, t)),
    hideTaggedFavorites: prefs?.hideTaggedFavorites ?? false,
  });

  const openMenu = (e: React.MouseEvent, items: MenuItem[]) => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY, items });
  };

  const openArticleMenu = async (e: React.MouseEvent, article: ArticleRow) => {
    e.preventDefault();
    const { clientX: x, clientY: y } = e;
    // Plusieurs articles sélectionnés : le menu agit sur tous ; sinon sur l'article cliqué.
    if (selectedKeys.length > 1 && selectedKeys.includes(article.key)) {
      setMenu({ x, y, items: articlesMenu(menuContext(), selectedKeys) });
      return;
    }
    if (!selectedKeys.includes(article.key)) {
      setSelectedKeys([article.key]);
      setCursorKey(article.key);
      anchorRef.current = article.key;
    }
    const detail = await api.article(article.key).catch(() => null);
    setMenu({ x, y, items: articleMenu(menuContext(), article, detail) });
  };

  // --- Sélection et clavier -------------------------------------------------------------------

  const openArticle = useCallback((key: string | null) => {
    setSelectedKey(key);
    setSelectedKeys(key ? [key] : []);
    setCursorKey(key);
    anchorRef.current = key;
  }, []);

  // Un changement de vue repart sans sélection.
  useEffect(() => {
    setSelectedKeys([]);
    setCursorKey(null);
    anchorRef.current = null;
  }, [selection]);

  const rangeKeys = (from: string, to: string): string[] => {
    const i = articles.findIndex((a) => a.key === from);
    const j = articles.findIndex((a) => a.key === to);
    if (i < 0 || j < 0) return [to];
    return articles.slice(Math.min(i, j), Math.max(i, j) + 1).map((a) => a.key);
  };

  const fullGrid = prefs?.articleLayout === "gridFull";

  const clickArticle = (article: ArticleRow, e: React.MouseEvent) => {
    const key = article.key;
    if (e.shiftKey && anchorRef.current) {
      setSelectedKeys(rangeKeys(anchorRef.current, key));
      setCursorKey(key);
      if (!fullGrid) setSelectedKey(key);
    } else if (e.metaKey || e.ctrlKey) {
      const on = !selectedKeys.includes(key);
      const next = on ? [...selectedKeys, key] : selectedKeys.filter((k) => k !== key);
      setSelectedKeys(next);
      setCursorKey(key);
      anchorRef.current = key;
      if (!fullGrid) setSelectedKey(on ? key : selectedKey === key ? (next[next.length - 1] ?? null) : selectedKey);
    } else {
      openArticle(key);
    }
  };

  // Déplace le curseur (j / k, flèches) ; avec `extend`, étend la sélection.
  const moveCursor = (delta: number, extend: boolean) => {
    if (articles.length === 0) return;
    const current = cursorKey ?? selectedKey;
    const index = current ? articles.findIndex((a) => a.key === current) : -1;
    const target = index < 0 ? (delta > 0 ? 0 : articles.length - 1) : Math.min(articles.length - 1, Math.max(0, index + delta));
    const key = articles[target].key;
    document.querySelector(`[data-article-key="${CSS.escape(key)}"]`)?.scrollIntoView({ block: "nearest" });
    if (extend) {
      const anchor = anchorRef.current ?? current ?? key;
      anchorRef.current = anchor;
      setSelectedKeys(rangeKeys(anchor, key));
      setCursorKey(key);
      if (!fullGrid) setSelectedKey(key);
    } else if (fullGrid && !selectedKey) {
      setSelectedKeys([key]);
      setCursorKey(key);
      anchorRef.current = key;
    } else {
      openArticle(key);
    }
  };

  // Articles visés par une action au clavier : la sélection, sinon l'article ouvert.
  const targetRows = (): ArticleRow[] => {
    const keys = selectedKeys.length > 0 ? selectedKeys : selectedKey ? [selectedKey] : [];
    return articles.filter((a) => keys.includes(a.key));
  };

  const toggleReadKeys = () => {
    const rows = targetRows();
    if (rows.length === 0) return;
    const allRead = rows.every((a) => a.isRead);
    api.setRead(rows.map((a) => a.key), !allRead).then(refresh).catch((e) => setNotice(errorMessage(e, t)));
  };

  const toggleStarKeys = () => {
    const rows = targetRows();
    if (rows.length === 0) return;
    const allStarred = rows.every((a) => a.isStarred);
    api.setStarred(rows.map((a) => a.key), !allStarred).then(refresh).catch((e) => setNotice(errorMessage(e, t)));
  };

  const openInBrowser = () => {
    const link = targetRows()[0]?.link;
    if (link) api.openUrl(link).catch((e) => setNotice(errorMessage(e, t)));
  };

  const spaceScroll = (up: boolean) => {
    const el = document.querySelector<HTMLElement>("[data-reader-scroll]");
    if (!el) {
      if (!up) moveCursor(1, false);
      return;
    }
    const atBottom = el.scrollTop + el.clientHeight >= el.scrollHeight - 4;
    if (!up && atBottom) moveCursor(1, false);
    else el.scrollBy({ top: (up ? -1 : 1) * el.clientHeight * 0.9, behavior: "smooth" });
  };

  const modalOpen =
    tour || showStats || showAbout || settings !== null || asking !== null || notice !== null || showAddFeed || update !== null || menu !== null ||
    (swiftImport.available && !swiftImport.dismissed);

  // Raccourcis (comme Reeder). Les touches seules sont ignorées pendant la saisie dans un champ.
  const onKeyRef = useRef<(e: KeyboardEvent) => void>(() => undefined);
  onKeyRef.current = (e: KeyboardEvent) => {
    if (!prefs || modalOpen || e.isComposing) return;
    const typing = !!(e.target as HTMLElement | null)?.closest?.("input, textarea, select, [contenteditable='true']");
    const key = e.key.toLowerCase();
    const cmd = e.metaKey || e.ctrlKey;

    if (cmd) {
      // Lettre réelle (e.key) et non position de la touche : sur un clavier AZERTY, ⌘Q est la touche « A » du QWERTY
      if (key === "a" && !typing) {
        e.preventDefault();
        setSelectedKeys(articles.map((a) => a.key));
      } else if (e.code === "KeyN") {
        e.preventDefault();
        if (e.shiftKey) createFolder();
        else setShowAddFeed(true);
      } else if (e.code === "KeyR" && !e.shiftKey) {
        e.preventDefault();
        void refreshFeeds(true);
      } else if (e.code === "KeyK" && e.shiftKey) {
        e.preventDefault();
        void markAllRead();
      } else if (e.code === "KeyS" && e.altKey) {
        e.preventDefault();
        setShowStats(true);
      } else if (e.key === ",") {
        e.preventDefault();
        setSettings("appearance");
      } else if ((e.code === "KeyP" && !e.altKey) || (e.code === "KeyE" && e.altKey)) {
        e.preventDefault();
        if (selectedKey) api.printPage().catch(() => undefined);
      }
      return;
    }
    if (typing || e.altKey) return;

    const grid = prefs.articleLayout !== "list";
    const stop = () => e.preventDefault();
    switch (e.key) {
      case "ArrowDown":
        if (!grid) { stop(); moveCursor(1, e.shiftKey); }
        return;
      case "ArrowUp":
        if (!grid) { stop(); moveCursor(-1, e.shiftKey); }
        return;
      case "ArrowRight":
        if (grid) { stop(); moveCursor(1, e.shiftKey); }
        return;
      case "ArrowLeft":
        if (grid) { stop(); moveCursor(-1, e.shiftKey); }
        return;
      case "Enter":
        if (fullGrid && cursorKey && !selectedKey) { stop(); openArticle(cursorKey); }
        return;
      case " ":
        stop();
        (document.activeElement as HTMLElement | null)?.blur?.();
        spaceScroll(e.shiftKey);
        return;
    }
    if (e.shiftKey) return;
    switch (key) {
      case "j": moveCursor(1, false); break;
      case "k": moveCursor(-1, false); break;
      case "m": toggleReadKeys(); break;
      case "s": toggleStarKeys(); break;
      case "b": openInBrowser(); break;
    }
  };
  // ⌘A vient du menu de macOS : texte d'un champ en cours de saisie, sinon tous les articles de la liste.
  const selectAllRef = useRef<() => void>(() => undefined);
  selectAllRef.current = () => {
    const field = document.activeElement as HTMLElement | null;
    if (field?.closest?.("input, textarea")) document.execCommand("selectAll");
    else if (!modalOpen) setSelectedKeys(articles.map((a) => a.key));
  };
  useEffect(() => {
    let off: (() => void) | undefined;
    listen("select-all", () => selectAllRef.current())
      .then((un) => (off = un))
      .catch(() => undefined);
    return () => off?.();
  }, []);
  useEffect(() => {
    const handler = (e: KeyboardEvent) => onKeyRef.current(e);
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const createFolder = () =>
    setAsking({
      kind: "prompt",
      title: t("dialogs.newFolder"),
      placeholder: t("dialogs.folderName"),
      confirmLabel: t("dialogs.create"),
      onSubmit: (name) => {
        api.createFolder(name).then(refresh).catch((err) => setNotice(errorMessage(err, t)));
      },
    });

  // Largeurs des panneaux : aperçu pendant le glissement, enregistrées au relâchement.
  const [dragSidebar, setDragSidebar] = useState<number | null>(null);
  const [dragList, setDragList] = useState<number | null>(null);

  const updatePrefs = (patch: Partial<Prefs>) => {
    if (!prefs) return;
    const next = { ...prefs, ...patch };
    setPrefs(next);
    api.savePrefs(next).catch((e) => setNotice(String(e)));
  };

  // Doublons : -1 = désactivé, sinon sensibilité 0 (faible), 1 (normale) ou 2 (forte). Les groupes sont recalculés.
  const setDuplicates = async (level: number) => {
    if (!prefs) return;
    const next = {
      ...prefs,
      groupDuplicates: level >= 0,
      duplicateSensitivity: level >= 0 ? level : prefs.duplicateSensitivity,
    };
    setPrefs(next);
    try {
      await api.savePrefs(next);
      await api.regroupDuplicates();
      refresh();
    } catch (e) {
      setNotice(errorMessage(e, t));
    }
  };

  const importOpml = async () => {
    try {
      const path = await openFile({
        multiple: false,
        directory: false,
        filters: [{ name: "OPML", extensions: ["opml", "xml"] }],
      });
      if (typeof path !== "string") return;
      const result = await api.importOpml(path);
      setNotice(
        result.added === 0 && result.skipped === 0
          ? t("opml.none")
          : t("opml.imported", { added: result.added, skipped: result.skipped }),
      );
      await loadSidebar();
      if (result.added > 0) void refreshFeeds();
    } catch (e) {
      setNotice(errorMessage(e, t));
    }
  };

  const exportOpml = async () => {
    try {
      const path = await saveFile({ defaultPath: "Flux.opml", filters: [{ name: "OPML", extensions: ["opml"] }] });
      if (!path) return;
      await api.exportOpml(path);
      setNotice(t("opml.exported"));
    } catch (e) {
      setNotice(errorMessage(e, t));
    }
  };

  const openAppMenu = (e: React.MouseEvent<HTMLButtonElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    setMenu({
      x: rect.right - 230,
      y: rect.bottom + 6,
      items: [
        { label: t("settings.open"), icon: <Settings size={16} />, onClick: () => setSettings("appearance") },
        { label: t("settings.filters"), icon: <SlidersHorizontal size={16} />, onClick: () => setSettings("filters") },
        { label: t("stats.menu"), icon: <BarChart3 size={16} />, onClick: () => setShowStats(true) },
        "separator",
        ...(swiftImport.available
          ? [{ label: t("swiftImport.menu"), icon: <Download size={16} />, onClick: () => void importSwiftData() }]
          : []),
        { label: t("settings.importOpml"), icon: <Download size={16} />, onClick: () => void importOpml() },
        { label: t("settings.exportOpml"), icon: <Upload size={16} />, onClick: () => void exportOpml() },
        "separator",
        { label: t("onboarding.menu"), icon: <Compass size={16} />, onClick: () => setTour(true) },
        { label: t("update.check"), icon: <RefreshCw size={16} />, onClick: () => void checkUpdateNow() },
        {
          label: t("settings.about"),
          icon: <Info size={16} />,
          onClick: () => setShowAbout(true),
        },
      ],
    });
  };

  const select = (s: Selection) => {
    if (sameSelection(s, selection)) return;
    setSelection(s);
    setSelectedKey(null);
  };

  const importSwiftData = async () => {
    setSwiftImport({ available: false, dismissed: true });
    try {
      const report = await api.importSwift();
      await loadSettings();
      await loadSidebar();
      setSelectedKey(null);
      setReloadTick((n) => n + 1);
      let text = t("swiftImport.done", {
        articles: report.articles,
        feeds: report.feeds,
        folders: report.folders,
        tags: report.favoriteTags,
        rules: report.filterRules,
      });
      if (report.warnings.length > 0) text += " " + t("swiftImport.warnings", { count: report.warnings.length, list: report.warnings.join(" ; ") });
      setNotice(text);
    } catch (e) {
      setNotice(t("swiftImport.error", { message: String(e) }));
    }
  };

  const checkUpdateNow = async () => {
    try {
      const u = await api.checkUpdate();
      if (u) setUpdate(u.version);
      else setNotice(t("update.upToDate", { version: info?.version ?? "" }));
    } catch (e) {
      setNotice(t("update.error", { message: String(e) }));
    }
  };

  const installUpdate = async () => {
    setUpdate(null);
    setNotice(t("update.installing"));
    try {
      await api.installUpdate(); // relance l'app en cas de succès
    } catch (e) {
      setNotice(t("update.error", { message: String(e) }));
    }
  };

  // Première ouverture : la découverte de Flux, si on n'a encore aucun flux (sinon, rien à découvrir).
  const feedCount = sidebar ? sidebar.feeds.length + sidebar.folders.reduce((n, f) => n + f.feeds.length, 0) : null;
  const importOffered = swiftImport.available && !swiftImport.dismissed;
  useEffect(() => {
    if (!prefs || feedCount === null || prefs.onboardingSeen || importOffered) return;
    if (feedCount === 0) setTour(true);
    updatePrefs({ onboardingSeen: true });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [prefs?.onboardingSeen, feedCount, importOffered]);

  const dismissSwiftImport = () => {
    setSwiftImport((s) => ({ ...s, dismissed: true }));
    api.swiftImportDismiss().catch(() => undefined);
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
  const sidebarWidth = dragSidebar ?? prefs.sidebarWidth;
  const listWidth = dragList ?? prefs.articleListWidth;

  const list = (
    <div className={styles.scroll}>
      <ArticleList
        articles={articles}
        layout={prefs.articleLayout}
        selectedKeys={selectedKeys}
        showSummary={prefs.showSummary}
        onSelect={clickArticle}
        onMenu={(e, article) => void openArticleMenu(e, article)}
      />
    </div>
  );

  return (
    <div className={styles.app} style={{ ["--sidebar-width" as string]: `${sidebarWidth}px` }}>
      <Resizer
        value={sidebarWidth}
        min={180}
        max={420}
        left={`calc(var(--gap) + ${sidebarWidth}px + var(--gap) / 2)`}
        onPreview={setDragSidebar}
        onCommit={(v) => {
          updatePrefs({ sidebarWidth: v });
          setDragSidebar(null);
        }}
      />
      <Sidebar
        data={sidebar}
        selection={selection}
        onSelect={select}
        favoritesExpanded={prefs.favoritesExpanded}
        onToggleFavorites={() => updatePrefs({ favoritesExpanded: !prefs.favoritesExpanded })}
        refreshing={refreshing}
        onRefresh={() => void refreshFeeds(true)}
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
          onMenu={openAppMenu}
          offline={!online}
        />

        {prefs.articleLayout !== "gridFull" ? (
          <div className={styles.split}>
            <div className={styles.listColumn} data-noprint style={{ width: listWidth }}>
              {list}
            </div>
            <Resizer
              value={listWidth}
              min={280}
              max={700}
              left={`${listWidth}px`}
              onPreview={setDragList}
              onCommit={(v) => {
                updatePrefs({ articleListWidth: v });
                setDragList(null);
              }}
            />
            <section className={styles.detail}>
              {selectedKey ? (
                <Reader
                  articleKey={selectedKey}
                  prefs={prefs}
                  onPrefs={updatePrefs}
                  onOpenArticle={openArticle}
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
            onOpenArticle={openArticle}
            tags={sidebar?.tags ?? []}
            onChanged={refresh}
            onClose={() => setSelectedKey(null)}
          />
        ) : (
          list
        )}
      </main>
      {showStats && <StatsDialog onClose={() => setShowStats(false)} />}
      {showAbout && <AboutDialog version={info?.version ?? null} onClose={() => setShowAbout(false)} />}
      {tour && (
        <Onboarding
          onClose={() => setTour(false)}
          onOpenSync={() => {
            setTour(false);
            setSettings("sync");
          }}
        />
      )}
      {update && (
        <ConfirmDialog
          title={t("update.title")}
          message={t("update.message", { version: update })}
          confirmLabel={t("update.install")}
          onConfirm={() => void installUpdate()}
          onClose={() => setUpdate(null)}
        />
      )}
      {swiftImport.available && !swiftImport.dismissed && (
        <ConfirmDialog
          title={t("swiftImport.title")}
          message={t("swiftImport.message")}
          confirmLabel={t("swiftImport.confirm")}
          onConfirm={() => void importSwiftData()}
          onClose={dismissSwiftImport}
        />
      )}
      {notice && <InfoDialog title="Flux" message={notice} onClose={() => setNotice(null)} />}
      {settings && (
        <SettingsDialog
          section={settings}
          onSection={setSettings}
          prefs={prefs}
          onPrefs={updatePrefs}
          themeMode={currentMode}
          onTheme={(mode) => {
            applyTheme(mode);
            updatePrefs({ appearance: modeToAppearance(mode) });
          }}
          onDuplicates={(level) => void setDuplicates(level)}
          feeds={[...(sidebar?.feeds ?? []), ...(sidebar?.folders.flatMap((f) => f.feeds) ?? [])].sort((a, b) =>
            a.title.localeCompare(b.title),
          )}
          onChanged={refresh}
          onClose={() => setSettings(null)}
        />
      )}
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
