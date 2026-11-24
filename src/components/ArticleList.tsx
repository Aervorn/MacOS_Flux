import { useTranslation } from "react-i18next";
import { Star } from "lucide-react";
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

  return (
    <div className={grid ? styles.grid : styles.list}>
      {articles.map((a) => {
        const selected = a.key === selectedKey;
        const meta = (
          <div className={styles.meta}>
            <span className={styles.source}>{a.source}</span>
            <span>{formatPublished(a.published, i18n.language)}</span>
            {a.readingMinutes > 0 && <span>{t("list.readingMinutes", { count: a.readingMinutes })}</span>}
            {a.isStarred && <Star size={12} className={styles.star} />}
          </div>
        );
        return (
          <button
            key={a.key}
            className={`${grid ? styles.card : styles.item} ${selected ? styles.selected : ""} ${a.isRead ? styles.read : ""}`}
            onClick={() => onSelect(a.key)}
            onContextMenu={(e) => onMenu(e, a)}
          >
            <Thumb thumb={a.thumb} className={grid ? styles.cardThumb : styles.itemThumb} />
            <div className={styles.body}>
              {meta}
              <div className={styles.title}>{a.title}</div>
              {(grid || showSummary) && a.summary && <div className={styles.summary}>{a.summary}</div>}
            </div>
          </button>
        );
      })}
    </div>
  );
}
