import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import { errorMessage } from "../errors";
import base from "./AddFeedDialog.module.css";
import filters from "./FiltersPanel.module.css";
import styles from "./SmartFolderDialog.module.css";

interface Props {
  /** null : nouveau dossier intelligent. */
  uid: string | null;
  /** Appelé après l'enregistrement, avec l'uid du dossier. */
  onSaved: (uid: string, created: boolean) => void;
  onClose: () => void;
}

/** Les mots-clés se saisissent un par ligne (la virgule sépare aussi). */
const parseKeywords = (text: string) =>
  text
    .split(/[\n,]/)
    .map((k) => k.trim())
    .filter(Boolean);

/** Création ou modification d'un dossier intelligent : nom, mots-clés, mot entier. */
export default function SmartFolderDialog({ uid, onSaved, onClose }: Props) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [text, setText] = useState("");
  const [wholeWord, setWholeWord] = useState(true);
  const [loaded, setLoaded] = useState(uid === null);
  const [preview, setPreview] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const nameField = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (uid === null) return;
    api
      .smartFolders()
      .then((all) => {
        const folder = all.find((f) => f.uid === uid);
        if (!folder) {
          onClose();
          return;
        }
        setName(folder.name);
        setText(folder.keywords.join("\n"));
        setWholeWord(folder.wholeWord);
        setLoaded(true);
      })
      .catch((e) => setError(errorMessage(e, t)));
  }, [uid]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (loaded) nameField.current?.focus();
  }, [loaded]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const keywords = parseKeywords(text);
  const valid = name.trim().length > 0 && keywords.length > 0;

  // Aperçu : combien d'articles actuels iraient dans le dossier.
  useEffect(() => {
    if (keywords.length === 0) {
      setPreview(null);
      return;
    }
    let cancelled = false;
    const id = setTimeout(() => {
      api
        .previewSmartFolder(keywords, wholeWord)
        .then((n) => !cancelled && setPreview(n))
        .catch(() => undefined);
    }, 250);
    return () => {
      cancelled = true;
      clearTimeout(id);
    };
  }, [text, wholeWord]); // eslint-disable-line react-hooks/exhaustive-deps

  const save = async () => {
    if (!valid || saving) return;
    setSaving(true);
    setError(null);
    try {
      if (uid) {
        await api.updateSmartFolder(uid, name, keywords, wholeWord);
        onSaved(uid, false);
      } else {
        onSaved(await api.createSmartFolder(name, keywords, wholeWord), true);
      }
    } catch (e) {
      setError(errorMessage(e, t));
      setSaving(false);
    }
  };

  return (
    <div className={base.backdrop} onMouseDown={onClose}>
      <form
        className={base.dialog}
        onMouseDown={(e) => e.stopPropagation()}
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <h2 className={base.title}>{uid ? t("smart.editTitle") : t("smart.newTitle")}</h2>
        <input
          ref={nameField}
          className={base.field}
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder={t("smart.name")}
          spellCheck={false}
          disabled={!loaded}
        />
        <textarea
          className={`${base.field} ${styles.keywords}`}
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder={t("smart.keywordsPlaceholder")}
          spellCheck={false}
          rows={5}
          disabled={!loaded}
        />
        <p className={base.help}>{t("smart.keywordsHelp")}</p>
        <label className={filters.check}>
          <input type="checkbox" checked={wholeWord} onChange={(e) => setWholeWord(e.target.checked)} />
          {t("filters.wholeWord")}
        </label>
        {preview !== null && <p className={filters.preview}>{t("smart.preview", { count: preview })}</p>}
        {error && <p className={base.error}>{error}</p>}
        <div className={base.actions}>
          <button type="button" className={base.button} onClick={onClose}>
            {t("addFeed.cancel")}
          </button>
          <button type="submit" className={base.primary} disabled={!valid || saving}>
            {uid ? t("filters.save") : t("filters.create")}
          </button>
        </div>
      </form>
    </div>
  );
}
