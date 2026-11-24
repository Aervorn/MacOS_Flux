import { useTranslation } from "react-i18next";
import { Layers, Star } from "lucide-react";
import type { ArticleLayout, ArticleRow } from "../api";
import { formatPublished } from "../format";
import Thumb from "./Thumb";
import styles from "./ArticleList.module.css";

interface Props {
  articles: ArticleRow[];
  layout: ArticleLayout;
  selectedKey: string | null;
  showSummary: boolean;
  onSelect: (key: string) => void;
  onMenu: (e: React.MouseEvent, article: ArticleRow) => void;
}

export default function ArticleList({ articles, layout, selectedKey, showSummary, onSelect, onMenu }: Props) {
  const { t, i18n } = useTranslation();

  if (articles.length === 0) {
    return <p className={styles.empty}>{t("list.noResults")}</p>;
  }

  const grid = layout !== "list";

  if (grid) {
    // Vignettes : miniature + titre seulement (comme l'app Swift), colonnes selon la largeur.
    return (
      <div className={styles.grid}>
        {articles.map((a) => (
          <button
            key={a.key}
            className={`${styles.card} ${a.key === selectedKey ? styles.selected : ""} ${a.isRead ? styles.read : ""}`}
            onClick={() => onSelect(a.key)}
            onContextMenu={(e) => onMenu(e, a)}
            title={a.source}
          >
            <div className={styles.cardMedia}>
              <Thumb thumb={a.thumb} className={styles.cardThumb} />
              {a.otherSources > 0 && (
                <span className={styles.cardBadge} title={t("list.otherSources", { count: a.otherSources })}>
                  <Layers size={11} />+{a.otherSources}
                </span>
              )}
              {a.isStarred && <Star size={12} fill="currentColor" className={styles.cardStar} />}
            </div>
            <div className={styles.cardTitleRow}>
              {!a.isRead && <span className={styles.dot} />}
              <span className={styles.cardTitle}>{a.title}</span>
            </div>
          </button>
        ))}
      </div>
    );
  }

  return (
    <div className={styles.list}>
      {articles.map((a) => {
        const selected = a.key === selectedKey;
        return (
          <button
            key={a.key}
            className={`${styles.item} ${selected ? styles.selected : ""} ${a.isRead ? styles.read : ""}`}
            onClick={() => onSelect(a.key)}
            onContextMenu={(e) => onMenu(e, a)}
          >
            <Thumb thumb={a.thumb} className={styles.itemThumb} />
            <div className={styles.body}>
              <div className={styles.meta}>
                <span className={styles.source}>{a.source}</span>
                <span>{formatPublished(a.published, i18n.language)}</span>
                {a.readingMinutes > 0 && <span>{t("list.readingMinutes", { count: a.readingMinutes })}</span>}
                {a.otherSources > 0 && (
                  <span className={styles.badge} title={t("list.otherSources", { count: a.otherSources })}>
                    <Layers size={11} />+{a.otherSources}
                  </span>
                )}
                {a.isStarred && <Star size={12} className={styles.star} />}
              </div>
              <div className={styles.title}>{a.title}</div>
              {showSummary && a.summary && <div className={styles.summary}>{a.summary}</div>}
            </div>
          </button>
        );
      })}
    </div>
  );
}
