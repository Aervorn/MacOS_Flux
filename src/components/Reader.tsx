import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowLeft, ExternalLink, FileText, Minus, Plus, Printer, Rss } from "lucide-react";
import { api } from "../api";
import type { ArticleDetail, Prefs } from "../api";
import { formatPublished } from "../format";
import { extractArticle } from "../reader/extract";
import { cachedImageSrc, restoreOnError, rewriteImages } from "../reader/images";
import { bodyHtml, heroAlreadyInBody, sanitizeArticle } from "../reader/sanitize";
import { looksLikeExcerpt } from "../reader/text";
import styles from "./Reader.module.css";

const MIN_FONT = 13;
const MAX_FONT = 28;
const DEFAULT_FONT = 17;
const WIDTHS: Prefs["readerWidth"][] = ["narrow", "wide", "full"];

interface Props {
  articleKey: string;
  prefs: Prefs;
  onPrefs: (patch: Partial<Prefs>) => void;
  onOpenArticle: (key: string) => void;
  /** Affiche le bouton de retour (grille : le lecteur recouvre la liste). */
  onClose?: () => void;
}

type Status = "idle" | "loading" | "error";

export default function Reader({ articleKey, prefs, onPrefs, onOpenArticle, onClose }: Props) {
  const { t, i18n } = useTranslation();
  const [detail, setDetail] = useState<ArticleDetail | null>(null);
  const [showFull, setShowFull] = useState(true);
  const [status, setStatus] = useState<Status>("idle");
  const [error, setError] = useState<string | null>(null);
  const [heroSrc, setHeroSrc] = useState<string | null>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Chargement de l'article, et récupération automatique du texte complet si le flux n'a qu'un extrait.
  useEffect(() => {
    let cancelled = false;
    setDetail(null);
    setStatus("idle");
    setError(null);
    setShowFull(true);
    scrollRef.current?.scrollTo({ top: 0 });

    (async () => {
      const loaded = await api.article(articleKey);
      if (cancelled || !loaded) return;
      setDetail(loaded);
      if (!loaded.fullHtml && loaded.link && looksLikeExcerpt(loaded.contentHtml || loaded.summary)) {
        await fetchFull(loaded, () => cancelled);
      }
    })().catch((e) => !cancelled && (setStatus("error"), setError(String(e))));

    async function fetchFull(article: ArticleDetail, isCancelled: () => boolean) {
      if (!article.link) return;
      setStatus("loading");
      setError(null);
      try {
        const page = await api.fetchPage(article.link);
        const extracted = extractArticle(page.html, page.finalUrl, article.title, article.summary);
        if (!extracted) throw new Error(t("detail.extractFailed"));
        await api.saveFullHtml({
          key: article.key,
          html: extracted.html,
          searchText: extracted.searchText,
          wordCount: extracted.wordCount,
          imageUrl: extracted.imageUrl,
        });
        if (isCancelled()) return;
        setDetail((d) =>
          d && d.key === article.key
            ? { ...d, fullHtml: extracted.html, imageUrl: d.imageUrl ?? extracted.imageUrl }
            : d,
        );
        setStatus("idle");
      } catch (e) {
        if (isCancelled()) return;
        setStatus("error");
        setError(String(e instanceof Error ? e.message : e));
      }
    }
    return () => {
      cancelled = true;
    };
  }, [articleKey, t]);

  const loadFullNow = useCallback(async () => {
    if (!detail?.link) return;
    const article = detail;
    setStatus("loading");
    setError(null);
    try {
      const page = await api.fetchPage(article.link!);
      const extracted = extractArticle(page.html, page.finalUrl, article.title, article.summary);
      if (!extracted) throw new Error(t("detail.extractFailed"));
      await api.saveFullHtml({
        key: article.key,
        html: extracted.html,
        searchText: extracted.searchText,
        wordCount: extracted.wordCount,
        imageUrl: extracted.imageUrl,
      });
      setDetail((d) =>
        d && d.key === article.key ? { ...d, fullHtml: extracted.html, imageUrl: d.imageUrl ?? extracted.imageUrl } : d,
      );
      setShowFull(true);
      setStatus("idle");
    } catch (e) {
      setStatus("error");
      setError(String(e instanceof Error ? e.message : e));
    }
  }, [detail, t]);

  const rawBody = useMemo(
    () =>
      detail
        ? bodyHtml({ fullHtml: detail.fullHtml, contentHtml: detail.contentHtml, summary: detail.summary, showFull })
        : "",
    [detail, showFull],
  );
  const safeBody = useMemo(() => sanitizeArticle(rawBody), [rawBody]);

  // Images du corps : copie en cache locale si elle existe.
  useEffect(() => {
    if (bodyRef.current) void rewriteImages(bodyRef.current);
  }, [safeBody]);

  const showHero = !!detail?.imageUrl && !heroAlreadyInBody(detail.imageUrl, rawBody);
  const heroUrl = showHero ? detail!.imageUrl! : null;
  useEffect(() => {
    setHeroSrc(null);
    if (!heroUrl) return;
    let cancelled = false;
    cachedImageSrc(heroUrl).then((src) => !cancelled && setHeroSrc(src));
    return () => {
      cancelled = true;
    };
  }, [heroUrl]);

  const setFont = useCallback(
    (size: number) => onPrefs({ readerFontSize: Math.min(MAX_FONT, Math.max(MIN_FONT, size)) }),
    [onPrefs],
  );

  // ⌘+ / ⌘= / ⌘− / ⌘0 (⌘= pour les claviers AZERTY), Échap pour revenir à la grille.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey && (e.key === "+" || e.key === "=")) {
        e.preventDefault();
        setFont(prefs.readerFontSize + 1);
      } else if (e.metaKey && (e.key === "-" || e.key === "_")) {
        e.preventDefault();
        setFont(prefs.readerFontSize - 1);
      } else if (e.metaKey && e.key === "0") {
        e.preventDefault();
        setFont(DEFAULT_FONT);
      } else if (e.key === "Escape" && onClose) {
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [prefs.readerFontSize, setFont, onClose]);

  const onBodyClick = (e: React.MouseEvent) => {
    const anchor = (e.target as HTMLElement).closest("a");
    if (!anchor) return;
    const href = anchor.getAttribute("href") ?? "";
    if (href.startsWith("#")) return; // ancre interne : on reste dans l'article
    e.preventDefault();
    if (href) api.openUrl(href).catch(() => undefined);
  };

  if (!detail) return <div className={styles.empty} />;

  const date = formatPublished(detail.published, i18n.language);
  const canToggle = !!detail.fullHtml;

  return (
    <div className={styles.reader}>
      <div className={styles.toolbar} data-noprint>
        {onClose && (
          <button className={styles.iconButton} onClick={onClose} title={t("detail.back")} aria-label={t("detail.back")}>
            <ArrowLeft size={16} />
          </button>
        )}
        <span className={styles.spacer} />
        {canToggle && (
          <button
            className={styles.pill}
            onClick={() => setShowFull((v) => !v)}
            title={showFull ? t("detail.showFeed") : t("detail.showFull")}
          >
            {showFull ? <Rss size={14} /> : <FileText size={14} />}
            {showFull ? t("detail.showFeed") : t("detail.showFull")}
          </button>
        )}
        {!detail.fullHtml && detail.link && (
          <button className={styles.pill} onClick={loadFullNow} disabled={status === "loading"}>
            <FileText size={14} />
            {t("detail.loadFull")}
          </button>
        )}
        <div className={styles.group}>
          <button className={styles.iconButton} onClick={() => setFont(prefs.readerFontSize - 1)} title={t("detail.smaller")} aria-label={t("detail.smaller")}>
            <Minus size={14} />
          </button>
          <button className={styles.iconButton} onClick={() => setFont(prefs.readerFontSize + 1)} title={t("detail.larger")} aria-label={t("detail.larger")}>
            <Plus size={14} />
          </button>
        </div>
        <div className={styles.group}>
          {WIDTHS.map((w) => (
            <button
              key={w}
              className={prefs.readerWidth === w ? styles.pillActive : styles.pill}
              onClick={() => onPrefs({ readerWidth: w })}
            >
              {t(`detail.width_${w}`)}
            </button>
          ))}
        </div>
        <button className={styles.iconButton} onClick={() => api.printPage().catch(() => undefined)} title={t("detail.print")} aria-label={t("detail.print")}>
          <Printer size={16} />
        </button>
        {detail.link && (
          <button className={styles.iconButton} onClick={() => api.openUrl(detail.link!).catch(() => undefined)} title={t("detail.openBrowser")} aria-label={t("detail.openBrowser")}>
            <ExternalLink size={16} />
          </button>
        )}
      </div>

      <div className={styles.scroll} ref={scrollRef}>
        <article
          className={styles.article}
          data-width={prefs.readerWidth}
          style={{ "--reader-font": `${prefs.readerFontSize}px` } as React.CSSProperties}
        >
          <p className={styles.meta}>
            {[detail.source, detail.author, date].filter(Boolean).join(" · ")}
          </p>
          <h1 className={styles.title}>{detail.title}</h1>

          {detail.similar.length > 0 && (
            <div className={styles.similar} data-noprint>
              <span>{t("detail.alsoPublished")}</span>
              {detail.similar.map((s) => (
                <button key={s.key} className={styles.pill} title={s.title} onClick={() => onOpenArticle(s.key)}>
                  {s.source}
                </button>
              ))}
            </div>
          )}

          {status === "loading" && <p className={styles.notice} data-noprint>{t("detail.loading")}</p>}
          {status === "error" && (
            <p className={styles.noticeError} data-noprint>
              {t("detail.loadError", { message: error ?? "" })}
            </p>
          )}

          {heroSrc && (
            <img
              className={styles.hero}
              src={heroSrc}
              alt=""
              onError={(e) => {
                if (heroUrl && e.currentTarget.src !== heroUrl) e.currentTarget.src = heroUrl;
              }}
            />
          )}

          <div
            ref={bodyRef}
            className={styles.content}
            onClick={onBodyClick}
            onErrorCapture={(e) => restoreOnError(e.target)}
            dangerouslySetInnerHTML={{ __html: safeBody }}
          />

          {detail.link && <p className={styles.source}>{t("detail.sourceLine", { link: detail.link })}</p>}
        </article>
      </div>
    </div>
  );
}
