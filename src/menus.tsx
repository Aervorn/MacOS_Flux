import type { TFunction } from "i18next";
import { CheckCheck, Circle, CircleCheck, ExternalLink, FolderInput, Link, Pencil, Plus, Star, Tag, Trash2 } from "lucide-react";
import { api } from "./api";
import type { ArticleDetail, ArticleRow, Selection, SidebarFeed, SidebarFolder, SidebarTag, Sidebar } from "./api";
import type { MenuItem } from "./components/ContextMenu";

/** Fenêtres que les menus peuvent ouvrir. */
export type Ask =
  | { kind: "prompt"; title: string; initial?: string; placeholder?: string; confirmLabel: string; allowEmpty?: boolean; onSubmit: (v: string) => void }
  | { kind: "confirm"; title: string; message: string; confirmLabel: string; onConfirm: () => void };

export interface MenuContext {
  t: TFunction;
  sidebar: Sidebar | null;
  selection: Selection;
  /** Recharge compteurs et liste. */
  refresh: () => void;
  select: (s: Selection) => void;
  ask: (a: Ask) => void;
  fail: (e: unknown) => void;
  hideTaggedFavorites: boolean;
}

const copy = (text: string) => navigator.clipboard?.writeText(text).catch(() => undefined);

/** Exécute une action, puis recharge ; une erreur est affichée. */
function run(ctx: MenuContext, action: () => Promise<unknown>) {
  action().then(ctx.refresh).catch(ctx.fail);
}

function folderOf(sidebar: Sidebar | null, feedUrl: string): string | null {
  return sidebar?.folders.find((f) => f.feeds.some((x) => x.url === feedUrl))?.uid ?? null;
}

export function feedMenu(ctx: MenuContext, feed: SidebarFeed): MenuItem[] {
  const { t } = ctx;
  const current = folderOf(ctx.sidebar, feed.url);
  const move = (uid: string | null) => run(ctx, () => api.moveFeed(feed.url, uid));
  return [
    {
      label: t("menu.rename"),
      icon: <Pencil size={16} />,
      onClick: () =>
        ctx.ask({
          kind: "prompt",
          title: t("dialogs.renameFeed"),
          initial: feed.title,
          confirmLabel: t("dialogs.save"),
          allowEmpty: true,
          onSubmit: (name) => run(ctx, () => api.renameFeed(feed.url, name)),
        }),
    },
    {
      label: t("menu.markAllRead"),
      icon: <CheckCheck size={16} />,
      onClick: () => run(ctx, () => api.markAllRead({ kind: "feed", url: feed.url }, ctx.hideTaggedFavorites)),
    },
    {
      label: t("menu.moveTo"),
      icon: <FolderInput size={16} />,
      submenu: [
        { label: t("menu.noFolder"), checked: current === null, onClick: () => move(null) },
        ...(ctx.sidebar?.folders.length ? (["separator"] as MenuItem[]) : []),
        ...(ctx.sidebar?.folders ?? []).map((f) => ({
          label: f.name,
          checked: current === f.uid,
          onClick: () => move(f.uid),
        })),
      ],
    },
    "separator",
    {
      label: t("menu.openWebsite"),
      icon: <ExternalLink size={16} />,
      disabled: !feed.siteUrl,
      onClick: () => feed.siteUrl && api.openUrl(feed.siteUrl).catch(ctx.fail),
    },
    { label: t("menu.copyFeedUrl"), icon: <Link size={16} />, onClick: () => copy(feed.url) },
    "separator",
    {
      label: t("menu.deleteFeed"),
      icon: <Trash2 size={16} />,
      danger: true,
      onClick: () =>
        ctx.ask({
          kind: "confirm",
          title: t("dialogs.deleteFeedTitle", { name: feed.title }),
          message: t("dialogs.deleteFeedBody"),
          confirmLabel: t("dialogs.delete"),
          onConfirm: () => {
            if (ctx.selection.kind === "feed" && ctx.selection.url === feed.url) ctx.select({ kind: "all" });
            run(ctx, () => api.deleteFeed(feed.url));
          },
        }),
    },
  ];
}

export function folderMenu(ctx: MenuContext, folder: SidebarFolder): MenuItem[] {
  const { t } = ctx;
  return [
    {
      label: t("menu.rename"),
      icon: <Pencil size={16} />,
      onClick: () =>
        ctx.ask({
          kind: "prompt",
          title: t("dialogs.renameFolder"),
          initial: folder.name,
          confirmLabel: t("dialogs.save"),
          onSubmit: (name) => run(ctx, () => api.renameFolder(folder.uid, name)),
        }),
    },
    {
      label: t("menu.markAllRead"),
      icon: <CheckCheck size={16} />,
      onClick: () => run(ctx, () => api.markAllRead({ kind: "folder", uid: folder.uid }, ctx.hideTaggedFavorites)),
    },
    "separator",
    {
      label: t("menu.deleteFolder"),
      icon: <Trash2 size={16} />,
      danger: true,
      onClick: () =>
        ctx.ask({
          kind: "confirm",
          title: t("dialogs.deleteFolderTitle", { name: folder.name }),
          message: t("dialogs.deleteFolderBody"),
          confirmLabel: t("dialogs.delete"),
          onConfirm: () => {
            if (ctx.selection.kind === "folder" && ctx.selection.uid === folder.uid) ctx.select({ kind: "all" });
            run(ctx, () => api.deleteFolder(folder.uid));
          },
        }),
    },
  ];
}

export function tagMenu(ctx: MenuContext, tag: SidebarTag): MenuItem[] {
  const { t } = ctx;
  return [
    {
      label: t("menu.rename"),
      icon: <Pencil size={16} />,
      onClick: () =>
        ctx.ask({
          kind: "prompt",
          title: t("dialogs.renameTag"),
          initial: tag.name,
          confirmLabel: t("dialogs.save"),
          onSubmit: (name) => run(ctx, () => api.renameTag(tag.uid, name)),
        }),
    },
    "separator",
    {
      label: t("menu.deleteTag"),
      icon: <Trash2 size={16} />,
      danger: true,
      onClick: () =>
        ctx.ask({
          kind: "confirm",
          title: t("dialogs.deleteTagTitle", { name: tag.name }),
          message: t("dialogs.deleteTagBody"),
          confirmLabel: t("dialogs.delete"),
          onConfirm: () => {
            if (ctx.selection.kind === "tag" && ctx.selection.uid === tag.uid) ctx.select({ kind: "starred" });
            run(ctx, () => api.deleteTag(tag.uid));
          },
        }),
    },
  ];
}

/** Menu d'un article ; `detail` apporte ses tags (la liste ne les contient pas). */
export function articleMenu(ctx: MenuContext, article: ArticleRow, detail: ArticleDetail | null): MenuItem[] {
  const { t } = ctx;
  const tagUids = detail?.tags ?? [];
  return [
    {
      label: article.isRead ? t("menu.markUnread") : t("menu.markRead"),
      icon: article.isRead ? <Circle size={16} /> : <CircleCheck size={16} />,
      onClick: () => run(ctx, () => api.setRead([article.key], !article.isRead)),
    },
    {
      label: article.isStarred ? t("menu.unfavorite") : t("menu.favorite"),
      icon: <Star size={16} />,
      onClick: () => run(ctx, () => api.setStarred([article.key], !article.isStarred)),
    },
    {
      label: t("menu.tags"),
      icon: <Tag size={16} />,
      submenu: [
        ...(ctx.sidebar?.tags ?? []).map((tag) => ({
          label: tag.name,
          checked: tagUids.includes(tag.uid),
          onClick: () => run(ctx, () => api.setTag(tag.uid, !tagUids.includes(tag.uid), [article.key])),
        })),
        ...(ctx.sidebar?.tags.length ? (["separator"] as MenuItem[]) : []),
        {
          label: t("menu.newTag"),
          icon: <Plus size={16} />,
          onClick: () =>
            ctx.ask({
              kind: "prompt",
              title: t("dialogs.newTag"),
              placeholder: t("dialogs.tagName"),
              confirmLabel: t("dialogs.create"),
              onSubmit: (name) =>
                run(ctx, async () => {
                  const created = await api.createTag(name);
                  if (created) await api.setTag(created.uid, true, [article.key]);
                }),
            }),
        },
      ],
    },
    "separator",
    {
      label: t("menu.openBrowser"),
      icon: <ExternalLink size={16} />,
      disabled: !article.link,
      onClick: () => article.link && api.openUrl(article.link).catch(ctx.fail),
    },
    {
      label: t("menu.copyLink"),
      icon: <Link size={16} />,
      disabled: !article.link,
      onClick: () => article.link && copy(article.link),
    },
  ];
}
