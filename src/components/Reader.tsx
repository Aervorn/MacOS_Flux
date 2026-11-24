import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowLeft, Check, Circle, CircleCheck, ExternalLink, FileDown, FileText, Minus, Plus, Printer, Rss, Star, Tag } from "lucide-react";
import { api } from "../api";
import type { ArticleDetail, Prefs, SidebarTag } from "../api";
import { errorMessage } from "../errors";
import { formatPublished } from "../format";
import { extractArticle } from "../reader/extract";
import { cachedImageSrc, restoreOnError, rewriteImages } from "../reader/images";
import { bodyHtml, heroAlreadyInBody, sanitizeArticle } from "../reader/sanitize";
import { createPlayer, parseEmbed } from "../reader/video";
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
  /** Tags existants (pour le menu des tags). */
  tags: SidebarTag[];
  /** Appelé après une action qui change les listes ou les compteurs (lu, favori, tags). */
  onChanged: () => void;
  /** Affiche le bouton de retour (grille : le lecteur recouvre la liste). */
  onClose?: () => void;
}

type Status = "idle" | "loading" | "error";

export default function Reader({ articleKey, prefs, onPrefs, onOpenArticle, tags, onChanged, onClose }: Props) {
  const { t, i18n } = useTranslation();
  const [detail, setDetail] = useState<ArticleDetail | null>(null);
  const [showFull, setShowFull] = useState(true);
  const [status, setStatus] = useState<Status>("idle");
  const [error, setError] = useState<string | null>(null);
  const [heroSrc, setHeroSrc] = useState<string | null>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [tagMenu, setTagMenu] = useState(false);
  const [pdf, setPdf] = useState<{ busy: boolean; message: string | null; failed: boolean }>({ busy: false, message: null, failed: false });
  const [newTag, setNewTag] = useState("");
  const onChangedRef = useRef(onChanged);
  onChangedRef.current = onChanged;
  const manualReadKey = useRef<string | null>(null); // lu/non lu choisi à la main : plus de marquage automatique
  const markDelayRef = useRef(prefs.markReadDelay);
  markDelayRef.current = prefs.markReadDelay;

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
        setError(await failureMessage(e));
      }
    }
    return () => {
      cancelled = true;
    };
  }, [articleKey, t]);

  // Échec du téléchargement : sans connexion, on le dit simplement plutôt que d'afficher l'erreur réseau.
  const failureMessage = async (e: unknown) =>
    (await api.isOnline().catch(() => true)) ? errorMessage(e, t) : t("detail.offlineFull");

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
      setError(await failureMessage(e));
    }
  }, [detail, t]);

  // Marquage automatique comme lu (réglage : jamais si < 0, tout de suite si 0, sinon après N secondes).
  // Seul l'article affiché compte : en changer avant la fin du délai annule le marquage.
  const loadedKey = detail?.key ?? null;
  useEffect(() => {
    if (!loadedKey) return;
    const delay = markDelayRef.current;
    if (delay < 0 || detail?.isRead) return;
    const id = setTimeout(() => {
      if (manualReadKey.current === loadedKey) return;
      api
        .setRead([loadedKey], true)
        .then(() => {
          setDetail((d) => (d && d.key === loadedKey ? { ...d, isRead: true } : d));
          onChangedRef.current();
        })
        .catch(() => undefined);
    }, delay * 1000);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loadedKey]);

  // Historique : un article affiché pendant 3 secondes compte comme ouvert.
  useEffect(() => {
    if (!loadedKey) return;
    const id = setTimeout(() => api.recordOpened(loadedKey).catch(() => undefined), 3000);
    return () => clearTimeout(id);
  }, [loadedKey]);

  // Temps de lecture : compté tant que la fenêtre est au premier plan.
  useEffect(() => {
    let since: number | null = document.hasFocus() ? Date.now() : null;
    const finish = () => {
      if (since === null) return;
      const seconds = (Date.now() - since) / 1000;
      since = null;
      if (seconds >= 1) api.addReadingSeconds(articleKey, seconds).catch(() => undefined);
    };
    const onFocus = () => {
      since = Date.now();
    };
    window.addEventListener("blur", finish);
    window.addEventListener("focus", onFocus);
    return () => {
      finish();
      window.removeEventListener("blur", finish);
      window.removeEventListener("focus", onFocus);
    };
  }, [articleKey]);

  const toggleRead = async () => {
    if (!detail) return;
    const read = !detail.isRead;
    manualReadKey.current = detail.key;
    await api.setRead([detail.key], read);
    setDetail((d) => (d ? { ...d, isRead: read } : d));
    onChanged();
  };

  const toggleStar = async () => {
    if (!detail) return;
    const starred = !detail.isStarred;
    await api.setStarred([detail.key], starred);
    setDetail((d) => (d ? { ...d, isStarred: starred } : d));
    onChanged();
  };

  // Un tag range un favori : l'ajouter à un article qui n'en est pas un en fait un favori.
  const toggleTag = async (uid: string) => {
    if (!detail) return;
    const on = !detail.tags.includes(uid);
    await api.setTag(uid, on, [detail.key]);
    setDetail((d) =>
      d ? { ...d, isStarred: d.isStarred || on, tags: on ? [...d.tags, uid] : d.tags.filter((x) => x !== uid) } : d,
    );
    onChanged();
  };

  const createTag = async () => {
    const created = await api.createTag(newTag);
    setNewTag("");
    if (created) {
      await api.setTag(created.uid, true, [articleKey]);
      setDetail((d) => (d ? { ...d, isStarred: true, tags: [...d.tags, created.uid] } : d));
      onChanged();
    }
  };

  const rawBody = useMemo(
    () =>
      detail
        ? bodyHtml({ fullHtml: detail.fullHtml, contentHtml: detail.contentHtml, summary: detail.summary, showFull })
        : "",
    [detail, showFull],
  );
  const safeBody = useMemo(
    () =>
      sanitizeArticle(rawBody, {
        play: (provider) => t("detail.playVideo", { provider }),
        note: (provider) => t("detail.videoNote", { provider }),
      }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [rawBody, i18n.language],
  );

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

  const exportPdf = () => {
    if (!detail || pdf.busy) return;
    setPdf({ busy: true, message: null, failed: false });
    api
      .exportPdf(detail.title)
      .then((path) => {
        const name = path ? path.split("/").pop() ?? path : null;
        setPdf({ busy: false, message: name ? t("detail.pdfSaved", { name }) : null, failed: false });
      })
      .catch((err) => setPdf({ busy: false, message: t("detail.pdfFailed", { message: String(err) }), failed: true }));
  };

  const onBodyClick = (e: React.MouseEvent) => {
    // Vidéo : le lecteur n'est chargé qu'au clic. L'adresse est revérifiée ici, jamais prise telle quelle.
    const box = (e.target as HTMLElement).closest<HTMLElement>(".flux-video");
    if (box) {
      e.preventDefault();
      const embed = parseEmbed(box.dataset.embed ?? "");
      if (embed) {
        box.replaceChildren(createPlayer(embed));
        box.classList.add("flux-video-playing");
      }
      return;
    }
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
        <button
          className={styles.iconButton}
          onClick={toggleRead}
          title={detail.isRead ? t("detail.markUnread") : t("detail.markRead")}
          aria-label={detail.isRead ? t("detail.markUnread") : t("detail.markRead")}
        >
          {detail.isRead ? <CircleCheck size={16} /> : <Circle size={16} />}
        </button>
        <button
          className={`${styles.iconButton} ${detail.isStarred ? styles.starOn : ""}`}
          onClick={toggleStar}
          title={detail.isStarred ? t("detail.unfavorite") : t("detail.favorite")}
          aria-label={detail.isStarred ? t("detail.unfavorite") : t("detail.favorite")}
        >
          <Star size={16} fill={detail.isStarred ? "currentColor" : "none"} />
        </button>
        <div className={styles.menuWrap}>
          <button
            className={styles.iconButton}
            onClick={() => setTagMenu((v) => !v)}
            title={t("detail.tags")}
            aria-label={t("detail.tags")}
          >
            <Tag size={16} />
          </button>
          {tagMenu && (
            <>
              <div className={styles.backdrop} onClick={() => setTagMenu(false)} />
              <div className={styles.menu}>
                {tags.length === 0 && <p className={styles.menuEmpty}>{t("detail.noTags")}</p>}
                {tags.map((tag) => (
                  <button key={tag.uid} className={styles.menuItem} onClick={() => toggleTag(tag.uid)}>
                    <span className={styles.check}>{detail.tags.includes(tag.uid) && <Check size={14} />}</span>
                    {tag.name}
                  </button>
                ))}
                <form
                  className={styles.menuForm}
                  onSubmit={(e) => {
                    e.preventDefault();
                    void createTag();
                  }}
                >
                  <input
                    value={newTag}
                    onChange={(e) => setNewTag(e.target.value)}
                    placeholder={t("detail.newTagName")}
                    spellCheck={false}
                  />
                  <button type="submit" className={styles.pill} disabled={!newTag.trim()}>
                    {t("detail.create")}
                  </button>
                </form>
              </div>
            </>
          )}
        </div>
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
        <button className={styles.iconButton} onClick={exportPdf} disabled={pdf.busy} title={t("detail.exportPdf")} aria-label={t("detail.exportPdf")}>
          <FileDown size={16} />
        </button>
        {detail.link && (
          <button className={styles.iconButton} onClick={() => api.openUrl(detail.link!).catch(() => undefined)} title={t("detail.openBrowser")} aria-label={t("detail.openBrowser")}>
            <ExternalLink size={16} />
          </button>
        )}
      </div>

      <div className={styles.scroll} ref={scrollRef} data-reader-scroll="">
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

          {pdf.message && (
            <p className={pdf.failed ? styles.noticeError : styles.notice} data-noprint>
              {pdf.message}
            </p>
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
