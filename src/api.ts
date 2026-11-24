import { invoke } from "@tauri-apps/api/core";

export type Selection =
  | { kind: "all" }
  | { kind: "unread" }
  | { kind: "starred" }
  | { kind: "history" }
  | { kind: "folder"; uid: string }
  | { kind: "tag"; uid: string }
  | { kind: "feed"; url: string };

export type ArticleLayout = "list" | "grid" | "gridFull";
export type Appearance = "system" | "light" | "dark";

/** Miroir de `flux_core::prefs::Prefs` (clés en camelCase). */
export interface Prefs {
  refreshMinutes: number;
  retentionDays: number;
  markReadDelay: number;
  imageCacheLimitMb: number;
  readerWidth: "narrow" | "wide" | "full";
  showSummary: boolean;
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
  articleLayout: ArticleLayout;
  articleListWidth: number;
  favoritesExpanded: boolean;
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

export interface Sidebar {
  unread: number;
  folders: SidebarFolder[];
  feeds: SidebarFeed[];
  tags: SidebarTag[];
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
}

export interface ListQuery {
  selection: Selection;
  search: string;
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

export interface AppInfo {
  version: string;
  coreVersion: string;
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  settings: () => invoke<Settings>("get_settings"),
  savePrefs: (prefs: Prefs) => invoke<void>("save_prefs", { prefs }),
  sidebar: () => invoke<Sidebar>("get_sidebar"),
  articles: (query: ListQuery) => invoke<ArticleRow[]>("list_articles", { query }),
  printPage: () => invoke<void>("print_page"),
  devImport: () => invoke<ImportReport>("dev_import_swift_data"),
};

/** Miniature servie par le protocole flux-image (cache de l'app Swift, lecture seule). */
export const imageSrc = (thumb: string) => `flux-image://localhost/${thumb}`;

export const sameSelection = (a: Selection, b: Selection) => JSON.stringify(a) === JSON.stringify(b);
