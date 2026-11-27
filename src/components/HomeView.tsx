import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { AlertTriangle, ChevronRight, Flame, Inbox, Sparkles } from "lucide-react";
import { api, type ArticleRow, type ListQuery, type Selection, type Sidebar } from "../api";
import ArticleList from "./ArticleList";
import styles from "./HomeView.module.css";

/** Sujets repris par plusieurs sources : sur cette période, et au plus ce nombre. */
const TOPIC_WINDOW = 48 * 3600;
const MAX_TOPICS = 6;
/** Non-lus récents : sur cette période, et au plus ce nombre. */
const RECENT_WINDOW = 24 * 3600;
const MAX_RECENT = 8;
/** Articles montrés par dossier intelligent. */
const PER_SMART_FOLDER = 3;

const query = (selection: Selection, hideRead: boolean): ListQuery => ({
  selection,
  search: "",
  searchTitle: true,
  searchBody: true,
  hideRead,
  hideTaggedFavorites: false,
  sticky: [],
});

interface Data {
  topics: ArticleRow[];
  recent: ArticleRow[];
  smart: { uid: string; name: string; unread: number; articles: ArticleRow[] }[];
  today: number | null;
}

interface Props {
  sidebar: Sidebar | null;
  /** Change à chaque actualisation ou action : la page se recharge. */
  reloadTick: number;
  onOpen: (key: string) => void;
  onSelect: (s: Selection) => void;
  onMenu: (e: React.MouseEvent, article: ArticleRow) => void;
}

/** Accueil : un briefing de ce qui s'est passé, sans rien à configurer. */
export default function HomeView({ sidebar, reloadTick, onOpen, onSelect, onMenu }: Props) {
  const { t, i18n } = useTranslation();
  const [data, setData] = useState<Data | null>(null);
  const smartFolders = sidebar?.smartFolders ?? [];
  const smartKey = smartFolders.map((f) => `${f.uid}:${f.unread}`).join(",");

  useEffect(() => {
    let cancelled = false;
    const now = Date.now() / 1000;
    (async () => {
      const all = await api.articles(query({ kind: "all" }, false));
      const topics = all
        .filter((a) => a.otherSources > 0 && a.published >= now - TOPIC_WINDOW)
        .sort((a, b) => b.otherSources - a.otherSources || b.published - a.published)
        .slice(0, MAX_TOPICS);
      const inTopics = new Set(topics.map((a) => a.key));
      const recent = all
        .filter((a) => !a.isRead && a.published >= now - RECENT_WINDOW && !inTopics.has(a.key))
        .slice(0, MAX_RECENT);
      const smart = await Promise.all(
        smartFolders
          .filter((f) => f.unread > 0)
          .map(async (f) => ({
            uid: f.uid,
            name: f.name,
            unread: f.unread,
            articles: (await api.articles(query({ kind: "smart", uid: f.uid }, true))).slice(0, PER_SMART_FOLDER),
          })),
      );
      const today = await api
        .readingStats(1)
        .then((s) => s.receivedCount)
        .catch(() => null);
      if (!cancelled) setData({ topics, recent, smart, today });
    })().catch(() => {
      if (!cancelled) setData({ topics: [], recent: [], smart: [], today: null });
    });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reloadTick, smartKey]);

  const feeds = [...(sidebar?.feeds ?? []), ...(sidebar?.folders.flatMap((f) => f.feeds) ?? [])];
  const errors = feeds.filter((f) => f.error).length;
  const date = new Date().toLocaleDateString(i18n.language, { weekday: "long", day: "numeric", month: "long" });
  const open = (a: ArticleRow) => onOpen(a.key);

  const section = (icon: React.ReactNode, title: string, help: string | null, body: React.ReactNode) => (
    <section className={styles.section}>
      <header className={styles.sectionHeader}>
        {icon}
        <h2>{title}</h2>
        {help && <span className={styles.help}>{help}</span>}
      </header>
      {body}
    </section>
  );

  return (
    <div className={styles.home}>
      <div className={styles.hero}>
        <h1 className={styles.date}>{date}</h1>
        <p className={styles.facts}>
          {data?.today != null && <span>{t("home.today", { count: data.today })}</span>}
          <button className={styles.factLink} onClick={() => onSelect({ kind: "unread" })}>
            {t("home.unread", { count: sidebar?.unread ?? 0 })}
          </button>
          {errors > 0 && (
            <span className={styles.errors}>
              <AlertTriangle size={13} />
              {t("home.errors", { count: errors })}
            </span>
          )}
        </p>
      </div>

      {data === null ? null : (
        <>
          {section(
            <Flame size={16} />,
            t("home.topics"),
            t("home.topicsHelp"),
            data.topics.length > 0 ? (
              <ArticleList articles={data.topics} layout="grid" selectedKeys={[]} summaryLines={0} onSelect={open} onMenu={onMenu} />
            ) : (
              <p className={styles.empty}>{t("home.noTopics")}</p>
            ),
          )}

          {data.smart.length > 0 &&
            section(
              <Sparkles size={16} />,
              t("home.smartFolders"),
              null,
              <div className={styles.smartGrid}>
                {data.smart.map((f) => (
                  <div key={f.uid} className={styles.smartCard}>
                    <button className={styles.smartTitle} onClick={() => onSelect({ kind: "smart", uid: f.uid })}>
                      <span>{f.name}</span>
                      <span className={styles.count}>{f.unread}</span>
                      <ChevronRight size={14} />
                    </button>
                    <ArticleList articles={f.articles} layout="list" selectedKeys={[]} summaryLines={0} onSelect={open} onMenu={onMenu} />
                  </div>
                ))}
              </div>,
            )}

          {section(
            <Inbox size={16} />,
            t("home.recent"),
            null,
            data.recent.length > 0 ? (
              <ArticleList articles={data.recent} layout="list" selectedKeys={[]} summaryLines={1} onSelect={open} onMenu={onMenu} />
            ) : (
              <p className={styles.empty}>{t("home.noRecent")}</p>
            ),
          )}
        </>
      )}
    </div>
  );
}
