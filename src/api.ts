import { invoke } from "@tauri-apps/api/core";
// Import croisé sans risque : `offline` n'utilise `api` qu'à l'intérieur de ses fonctions.
import { prepareOffline } from "./offline";

/** « home » (l'accueil) n'existe que dans l'interface : elle n'est jamais envoyée au cœur Rust. */
export type Selection =
  | { kind: "home" }
  | { kind: "all" }
  | { kind: "unread" }
  | { kind: "starred" }
  | { kind: "history" }
  | { kind: "folder"; uid: string }
  | { kind: "tag"; uid: string }
  | { kind: "feed"; url: string }
  | { kind: "smart"; uid: string };

/** Autorisation donnée à Flux dans Réglages Système › Notifications. */
export type NotificationPermission = "notDetermined" | "denied" | "authorized" | "unknown";

/** Résumés par Apple Intelligence : disponibles, ou pourquoi pas. */
export type AiStatus = "available" | "unsupported" | "deviceNotEligible" | "notEnabled" | "notReady" | "unavailable";

export type ArticleLayout = "list" | "grid" | "gridFull";
export type LightTheme = "light" | "sepia" | "dawn";
export type DarkTheme = "dark" | "pureBlack" | "nord" | "forest";
export type ThemeId = LightTheme | DarkTheme;
export type Appearance = "system" | ThemeId;
export type AccentChoice = "theme" | "blue" | "violet" | "pink" | "red" | "orange" | "green";

/** Miroir de `flux_core::prefs::Prefs` (clés en camelCase). */
export interface Prefs {
  refreshMinutes: number;
  retentionDays: number;
  markReadDelay: number;
  imageCacheLimitMb: number;
  readerWidth: "narrow" | "wide" | "full";
  /** Lignes du début de l'article sous le titre, en liste (0 à 4). */
  summaryLines: number;
  /** La recherche porte sur le titre, sur le corps de l'article, ou sur les deux. */
  searchInTitle: boolean;
  searchInBody: boolean;
  readerFontSize: number;
  archiveFavorites: boolean;
  groupDuplicates: boolean;
  duplicateSensitivity: number;
  hideRead: boolean;
  hideTaggedFavorites: boolean;
  backupEnabled: boolean;
  backupKeep: number;
  backupIncludeArchive: boolean;
  autoMarkReadEnabled: boolean;
  autoMarkReadDays: number;
  offlineEnabled: boolean;
  appearance: Appearance;
  /** Thèmes utilisés en mode automatique, selon l'apparence de macOS. */
  lightTheme: LightTheme;
  darkTheme: DarkTheme;
  accent: AccentChoice;
  articleLayout: ArticleLayout;
  articleListWidth: number;
  syncEnabled: boolean;
  sidebarWidth: number;
  favoritesExpanded: boolean;
  uiFont: "inter" | "geist" | "system";
  articleFont: "classic" | "geist" | "literata" | "sourceSerif" | "merriweather" | "atkinson" | "systemSerif" | "mono";
  sidebarDensity: "compact" | "normal" | "spacious";
  onboardingSeen: boolean;
  /** Notifications des nouveaux articles (réglage propre à ce Mac). */
  notifyEnabled: boolean;
  /** Tous les flux, ou seulement les flux, dossiers, dossiers intelligents et mots-clés choisis. */
  notifyAll: boolean;
  notifyFeeds: string[];
  notifyFolders: string[];
  notifySmartFolders: string[];
  notifyKeywords: string[];
  /** Fermer la fenêtre laisse Flux dans la barre des menus. */
  menuBar: boolean;
  /** Notifier aussi quand la fenêtre de Flux est au premier plan. */
  notifyInFront: boolean;
  /** Résumer automatiquement chaque article ouvert (Apple Intelligence). */
  autoSummarize: boolean;
}

export interface Settings {
  prefs: Prefs;
  language: string | null;
}

export interface SidebarFeed {
  url: string;
  title: string;
  siteUrl: string | null;
  unread: number;
  /** Message de la dernière actualisation ratée. */
  error: string | null;
  /** Dernière actualisation réussie (secondes). */
  lastRefreshed: number | null;
}

export interface SidebarFolder {
  uid: string;
  name: string;
  symbol: string | null;
  colorName: string | null;
  isExpanded: boolean;
  unread: number;
  feeds: SidebarFeed[];
}

export interface SidebarTag {
  uid: string;
  name: string;
}

export interface SidebarSmartFolder {
  uid: string;
  name: string;
  symbol: string | null;
  colorName: string | null;
  unread: number;
}

/** Dossier intelligent tel que le renvoie `list_smart_folders`. */
export interface SmartFolder {
  uid: string;
  name: string;
  keywords: string[];
  wholeWord: boolean;
  symbol: string | null;
  colorName: string | null;
  articleCount: number;
}

export interface Sidebar {
  unread: number;
  folders: SidebarFolder[];
  feeds: SidebarFeed[];
  tags: SidebarTag[];
  smartFolders: SidebarSmartFolder[];
}

export interface ArticleRow {
  key: string;
  title: string;
  summary: string;
  source: string;
  feedUrl: string;
  published: number;
  openedAt: number | null;
  isRead: boolean;
  isStarred: boolean;
  thumb: string | null;
  author: string | null;
  link: string | null;
  readingMinutes: number;
  otherSources: number;
}

export interface SimilarArticle {
  key: string;
  source: string;
  title: string;
}

/** Miroir de `flux_core::articles::ArticleDetail`. */
export interface ArticleDetail {
  key: string;
  title: string;
  summary: string;
  source: string;
  feedUrl: string;
  link: string | null;
  author: string | null;
  published: number;
  imageUrl: string | null;
  contentHtml: string | null;
  fullHtml: string | null;
  isRead: boolean;
  isStarred: boolean;
  /** uid des tags de l'article */
  tags: string[];
  similar: SimilarArticle[];
}

export interface FilterRule {
  uid: string;
  keyword: string;
  wholeWord: boolean;
  action: "hide" | "markRead";
  /** Vide = tous les flux. */
  feedUrls: string[];
  /** Articles actuellement touchés par la règle. */
  matchCount: number;
}

export interface FilterInput {
  keyword: string;
  wholeWord: boolean;
  action: "hide" | "markRead";
  feedUrls: string[];
}

export interface OpmlResult {
  added: number;
  skipped: number;
}

export interface TagInfo {
  uid: string;
  name: string;
}

export interface FetchedPage {
  html: string;
  finalUrl: string;
}

export interface ListQuery {
  selection: Selection;
  search: string;
  searchTitle: boolean;
  searchBody: boolean;
  hideRead: boolean;
  hideTaggedFavorites: boolean;
  sticky: string[];
}

export interface ImportReport {
  folders: number;
  feeds: number;
  articles: number;
  favoriteTags: number;
  articleTags: number;
  filterRules: number;
  settingsImported: boolean;
  language: string | null;
  warnings: string[];
}

export interface SwiftImportStatus {
  available: boolean;
  dismissed: boolean;
}

export interface SourceCount {
  name: string;
  count: number;
}

export interface ReadingStats {
  readCount: number;
  receivedCount: number;
  readingSeconds: number;
  estimatedSeconds: number;
  perDay: number;
  /** Articles ouverts par jour, le dernier élément étant aujourd'hui */
  daily: number[];
  mostRead: SourceCount[];
  mostActive: SourceCount[];
  /** Secondes UNIX de la première ouverture enregistrée */
  trackingSince: number | null;
}

export interface ArchiveInfo {
  pending: number;
  archived: number;
  bytes: number;
  working: boolean;
}

export interface OfflineInfo {
  cacheBytes: number;
  working: boolean;
}

export interface PendingText {
  key: string;
  link: string;
  title: string;
  summary: string;
  contentHtml: string | null;
}

export interface AddedFeed {
  url: string;
  title: string;
  articles: number;
}

export interface RefreshReport {
  feeds: number;
  added: number;
  failed: number;
  offline: boolean;
}

export interface AppInfo {
  version: string;
  coreVersion: string;
}

/** Miroir de `SyncStatus` (src-tauri). */
export interface SyncStatus {
  syncing: boolean;
  /** Secondes Unix */
  lastSync: number | null;
  /** `icloud_unavailable` ou un message */
  error: string | null;
  others: { name: string; updated: number }[];
  /** Mac dont le fichier de synchro est d'une autre version : il faut le mettre à jour. */
  incompatible: { name: string; updated: number }[];
  icloudAvailable: boolean;
}

/** Miroir de `BackupInfo` (src-tauri). */
export interface BackupFile {
  name: string;
  device: string;
  path: string;
  size: number;
  /** Secondes Unix */
  modified: number;
  downloaded: boolean;
}

export interface BackupInfo {
  working: boolean;
  error: string | null;
  lastBackup: number | null;
  icloudAvailable: boolean;
  backups: BackupFile[];
}

/** Nouveau favori : on récupère son texte complet, puis on archive. */
async function archiveSoon(): Promise<void> {
  await prepareOffline().catch(() => undefined);
  await api.archivePending().catch(() => undefined);
}

export const api = {
  backupStatus: () => invoke<BackupInfo>("backup_status"),
  backupNow: () => invoke<void>("backup_now"),
  quitApp: () => invoke<void>("quit_app"),
  /** Renvoie vrai si l'app se relance toute seule (faux en développement). */
  restoreBackup: (path: string) => invoke<boolean>("restore_backup", { path }),
  syncStatus: () => invoke<SyncStatus>("sync_status"),
  syncNow: () => invoke<void>("sync_now"),
  syncApply: (enabled: boolean) => invoke<void>("sync_apply", { enabled }),
  appInfo: () => invoke<AppInfo>("app_info"),
  settings: () => invoke<Settings>("get_settings"),
  savePrefs: (prefs: Prefs) => invoke<void>("save_prefs", { prefs }),
  sidebar: () => invoke<Sidebar>("get_sidebar"),
  articles: (query: ListQuery) => invoke<ArticleRow[]>("list_articles", { query }),
  article: (key: string) => invoke<ArticleDetail | null>("get_article", { key }),
  fetchPage: (url: string) => invoke<FetchedPage>("fetch_page", { url }),
  saveFullHtml: (args: {
    key: string;
    html: string;
    searchText: string;
    wordCount: number;
    imageUrl: string | null;
  }) => invoke<boolean>("save_full_html", args),
  setRead: (keys: string[], read: boolean) => invoke<void>("set_read", { keys, read }),
  markAllRead: (selection: Selection, hideTaggedFavorites: boolean) =>
    invoke<number>("mark_all_read", { selection, hideTaggedFavorites }),
  setStarred: (keys: string[], starred: boolean) =>
    invoke<void>("set_starred", { keys, starred }).then(() => {
      if (starred) void archiveSoon();
    }),
  createTag: (name: string) => invoke<TagInfo | null>("create_tag", { name }),
  setTag: (uid: string, on: boolean, keys: string[]) =>
    invoke<void>("set_tag", { uid, on, keys }).then(() => {
      if (on) void archiveSoon(); // étiqueter un article en fait un favori
    }),
  recordOpened: (key: string) => invoke<void>("record_opened", { key }),
  archiveStatus: () => invoke<ArchiveInfo>("archive_status"),
  archivePending: () => invoke<number>("archive_pending"),
  isOnline: () => invoke<boolean>("is_online"),
  offlineStatus: () => invoke<OfflineInfo>("offline_status"),
  offlinePendingTexts: () => invoke<PendingText[]>("offline_pending_texts"),
  offlineMaintain: () => invoke<void>("offline_maintain"),
  clearImageCache: () => invoke<void>("clear_image_cache"),
  readingStats: (days: number) =>
    invoke<ReadingStats>("reading_stats", { days, tzOffset: -new Date().getTimezoneOffset() * 60 }),
  addReadingSeconds: (key: string, seconds: number) => invoke<void>("add_reading_seconds", { key, seconds }),
  addFeed: (input: string, folderUid: string | null) => invoke<AddedFeed>("add_feed", { input, folderUid }),
  changeFeedAddress: (url: string, input: string) => invoke<AddedFeed>("change_feed_address", { url, input }),
  refreshAll: () => invoke<RefreshReport>("refresh_all"),
  testNotification: () => invoke<void>("test_notification"),
  notificationPermission: () => invoke<NotificationPermission>("notification_permission"),
  requestNotificationPermission: () => invoke<NotificationPermission>("request_notification_permission"),
  openNotificationSettings: () => invoke<void>("open_notification_settings"),
  aiStatus: () => invoke<AiStatus>("ai_status"),
  summarize: (text: string, lang: string) => invoke<string>("summarize_article", { text, lang }),
  setUiLanguage: (lang: string) => invoke<void>("set_ui_language", { lang }),
  renameFeed: (url: string, name: string) => invoke<void>("rename_feed", { url, name }),
  moveFeed: (url: string, folderUid: string | null) => invoke<void>("move_feed", { url, folderUid }),
  deleteFeed: (url: string) => invoke<void>("delete_feed", { url }),
  createFolder: (name: string) => invoke<string | null>("create_folder", { name }),
  renameFolder: (uid: string, name: string) => invoke<void>("rename_folder", { uid, name }),
  setFolderStyle: (uid: string, symbol: string | null, color: string | null) =>
    invoke<void>("set_folder_style", { uid, symbol, color }),
  deleteFolder: (uid: string) => invoke<void>("delete_folder", { uid }),
  setFolderExpanded: (uid: string, expanded: boolean) => invoke<void>("set_folder_expanded", { uid, expanded }),
  renameTag: (uid: string, name: string) => invoke<void>("rename_tag", { uid, name }),
  deleteTag: (uid: string) => invoke<void>("delete_tag", { uid }),
  filters: () => invoke<FilterRule[]>("list_filters"),
  addFilter: (r: FilterInput) => invoke<void>("add_filter", { ...r }),
  updateFilter: (uid: string, r: FilterInput) => invoke<void>("update_filter", { uid, ...r }),
  deleteFilter: (uid: string) => invoke<void>("delete_filter", { uid }),
  previewFilter: (keyword: string, wholeWord: boolean, feedUrls: string[]) =>
    invoke<number>("preview_filter", { keyword, wholeWord, feedUrls }),
  smartFolders: () => invoke<SmartFolder[]>("list_smart_folders"),
  createSmartFolder: (name: string, keywords: string[], wholeWord: boolean) =>
    invoke<string>("create_smart_folder", { name, keywords, wholeWord }),
  updateSmartFolder: (uid: string, name: string, keywords: string[], wholeWord: boolean) =>
    invoke<void>("update_smart_folder", { uid, name, keywords, wholeWord }),
  deleteSmartFolder: (uid: string) => invoke<void>("delete_smart_folder", { uid }),
  setSmartFolderStyle: (uid: string, symbol: string | null, color: string | null) =>
    invoke<void>("set_smart_folder_style", { uid, symbol, color }),
  previewSmartFolder: (keywords: string[], wholeWord: boolean) =>
    invoke<number>("preview_smart_folder", { keywords, wholeWord }),
  importOpml: (path: string) => invoke<OpmlResult>("import_opml", { path }),
  exportOpml: (path: string) => invoke<void>("export_opml", { path }),
  regroupDuplicates: () => invoke<void>("regroup_duplicates"),
  detachFromCluster: (key: string) => invoke<void>("detach_from_cluster", { key }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  printPage: () => invoke<void>("print_page"),
  /** Enregistre l'article affiché en PDF ; renvoie le chemin, ou null si annulé. */
  exportPdf: (title: string) => invoke<string | null>("export_pdf", { title }),
  checkUpdate: () => invoke<{ version: string } | null>("check_update"),
  installUpdate: () => invoke<void>("install_update"),
  swiftImportStatus: () => invoke<SwiftImportStatus>("swift_import_status"),
  swiftImportDismiss: () => invoke<void>("swift_import_dismiss"),
  importSwift: () => invoke<ImportReport>("import_swift_data"),
};

/** Miniature servie par le protocole flux-image (cache d'images de l'app). */
export const imageSrc = (thumb: string) => `flux-image://localhost/${thumb}`;

export const sameSelection = (a: Selection, b: Selection) => JSON.stringify(a) === JSON.stringify(b);
