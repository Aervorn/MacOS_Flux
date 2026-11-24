import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { EyeOff, CircleCheck, Pencil, Plus, Trash2 } from "lucide-react";
import { api } from "../api";
import type { FilterInput, FilterRule, SidebarFeed } from "../api";
import { errorMessage } from "../errors";
import base from "./AddFeedDialog.module.css";
import styles from "./FiltersDialog.module.css";

interface Props {
  feeds: SidebarFeed[];
  /** Appelé après chaque modification (compteurs et liste à recharger). */
  onChanged: () => void;
  onClose: () => void;
}

const EMPTY: FilterInput = { keyword: "", wholeWord: true, action: "hide", feedUrls: [] };

/** Gestion des règles de filtres : liste, puis formulaire d'ajout ou de modification. */
export default function FiltersDialog({ feeds, onChanged, onClose }: Props) {
  const { t } = useTranslation();
  const [rules, setRules] = useState<FilterRule[]>([]);
  const [editing, setEditing] = useState<{ uid: string | null; form: FilterInput } | null>(null);
  const [preview, setPreview] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    api.filters().then(setRules).catch((e) => setError(errorMessage(e, t)));
  }, [t]);
  useEffect(load, [load]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (editing) setEditing(null);
      else onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [editing, onClose]);

  // Aperçu : combien d'articles la règle toucherait.
  const form = editing?.form;
  useEffect(() => {
    if (!form || !form.keyword.trim()) {
      setPreview(null);
      return;
    }
    let cancelled = false;
    const id = setTimeout(() => {
      api
        .previewFilter(form.keyword, form.wholeWord, form.feedUrls)
        .then((n) => !cancelled && setPreview(n))
        .catch(() => undefined);
    }, 250);
    return () => {
      cancelled = true;
      clearTimeout(id);
    };
  }, [form]);

  const feedName = (url: string) => feeds.find((f) => f.url === url)?.title ?? url;
  const scope = (rule: FilterRule) => {
    if (rule.feedUrls.length === 0) return t("filters.allFeeds");
    if (rule.feedUrls.length <= 2) return rule.feedUrls.map(feedName).join(", ");
    return t("filters.someFeeds", { count: rule.feedUrls.length });
  };

  const run = async (job: () => Promise<void>) => {
    setError(null);
    try {
      await job();
      load();
      onChanged();
    } catch (e) {
      setError(errorMessage(e, t));
    }
  };

  const save = () =>
    editing &&
    run(async () => {
      const { uid, form } = editing;
      if (uid) await api.updateFilter(uid, form);
      else await api.addFilter(form);
      setEditing(null);
    });

  const toggleFeed = (url: string) =>
    setEditing((cur) => {
      if (!cur) return cur;
      const set = new Set(cur.form.feedUrls);
      if (!set.delete(url)) set.add(url);
      return { ...cur, form: { ...cur.form, feedUrls: [...set] } };
    });

  const patch = (change: Partial<FilterInput>) =>
    setEditing((cur) => (cur ? { ...cur, form: { ...cur.form, ...change } } : cur));

  return (
    <div className={base.backdrop} onMouseDown={onClose}>
      <div className={`${base.dialog} ${styles.dialog}`} onMouseDown={(e) => e.stopPropagation()}>
        <h2 className={base.title}>{t("filters.title")}</h2>

        {!editing && (
          <>
            <div className={styles.list}>
              {rules.length === 0 && <p className={base.help}>{t("filters.empty")}</p>}
              {rules.map((rule) => (
                <div key={rule.uid} className={styles.rule}>
                  {rule.action === "hide" ? <EyeOff size={16} /> : <CircleCheck size={16} />}
                  <div className={styles.ruleText}>
                    <div className={styles.keyword}>{rule.keyword}</div>
                    <div className={styles.details}>
                      {[
                        rule.wholeWord ? t("filters.wholeWord") : null,
                        scope(rule),
                        t(rule.action === "hide" ? "filters.hide" : "filters.markRead"),
                        t("filters.matches", { count: rule.matchCount }),
                      ]
                        .filter(Boolean)
                        .join(" · ")}
                    </div>
                  </div>
                  <button
                    className={styles.icon}
                    title={t("filters.edit")}
                    onClick={() =>
                      setEditing({
                        uid: rule.uid,
                        form: {
                          keyword: rule.keyword,
                          wholeWord: rule.wholeWord,
                          action: rule.action,
                          feedUrls: rule.feedUrls,
                        },
                      })
                    }
                  >
                    <Pencil size={15} />
                  </button>
                  <button
                    className={styles.icon}
                    title={t("filters.delete")}
                    onClick={() => run(() => api.deleteFilter(rule.uid))}
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              ))}
            </div>
            <p className={base.help}>{t("filters.footer")}</p>
            {error && <p className={base.error}>{error}</p>}
            <div className={base.actions}>
              <button className={base.button} onClick={() => setEditing({ uid: null, form: EMPTY })}>
                <Plus size={14} /> {t("filters.add")}
              </button>
              <button className={base.primary} onClick={onClose}>
                {t("filters.close")}
              </button>
            </div>
          </>
        )}

        {editing && form && (
          <form
            className={base.dialog}
            style={{ width: "auto", padding: 0, border: "none", background: "none" }}
            onSubmit={(e) => {
              e.preventDefault();
              if (form.keyword.trim()) void save();
            }}
          >
            <input
              className={base.field}
              autoFocus
              value={form.keyword}
              onChange={(e) => patch({ keyword: e.target.value })}
              placeholder={t("filters.keyword")}
              spellCheck={false}
            />
            <label className={styles.check}>
              <input type="checkbox" checked={form.wholeWord} onChange={(e) => patch({ wholeWord: e.target.checked })} />
              {t("filters.wholeWord")}
            </label>
            <div className={base.row}>
              <span>{t("filters.action")}</span>
              <select value={form.action} onChange={(e) => patch({ action: e.target.value as FilterInput["action"] })}>
                <option value="hide">{t("filters.hide")}</option>
                <option value="markRead">{t("filters.markRead")}</option>
              </select>
            </div>
            <span className={base.help}>
              {t("filters.feeds")} : {form.feedUrls.length === 0 ? t("filters.allFeeds") : t("filters.someFeeds", { count: form.feedUrls.length })}
            </span>
            <div className={styles.feeds}>
              {feeds.map((feed) => (
                <label key={feed.url} className={styles.check}>
                  <input type="checkbox" checked={form.feedUrls.includes(feed.url)} onChange={() => toggleFeed(feed.url)} />
                  {feed.title}
                </label>
              ))}
            </div>
            {preview !== null && <p className={styles.preview}>{t("filters.preview", { count: preview })}</p>}
            {error && <p className={base.error}>{error}</p>}
            <div className={base.actions}>
              <button type="button" className={base.button} onClick={() => setEditing(null)}>
                {t("filters.back")}
              </button>
              <button type="submit" className={base.primary} disabled={!form.keyword.trim()}>
                {editing.uid ? t("filters.save") : t("filters.create")}
              </button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}
