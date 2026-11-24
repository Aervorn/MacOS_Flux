import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import styles from "./AddFeedDialog.module.css";

function useEscape(onClose: () => void) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
}

interface PromptProps {
  title: string;
  initial?: string;
  placeholder?: string;
  confirmLabel: string;
  /** Autorise une valeur vide (ex. rétablir le nom d'origine d'un flux). */
  allowEmpty?: boolean;
  onSubmit: (value: string) => void;
  onClose: () => void;
}

/** Petite fenêtre avec un champ de texte (renommer, nouveau dossier, nouveau tag…). */
export function PromptDialog({ title, initial = "", placeholder, confirmLabel, allowEmpty, onSubmit, onClose }: PromptProps) {
  const { t } = useTranslation();
  const [value, setValue] = useState(initial);
  const input = useRef<HTMLInputElement>(null);
  useEscape(onClose);
  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);

  const valid = allowEmpty || value.trim().length > 0;
  return (
    <div className={styles.backdrop} onMouseDown={onClose}>
      <form
        className={styles.dialog}
        onMouseDown={(e) => e.stopPropagation()}
        onSubmit={(e) => {
          e.preventDefault();
          if (valid) onSubmit(value);
        }}
      >
        <h2 className={styles.title}>{title}</h2>
        <input
          ref={input}
          className={styles.field}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder={placeholder}
          spellCheck={false}
        />
        <div className={styles.actions}>
          <button type="button" className={styles.button} onClick={onClose}>
            {t("addFeed.cancel")}
          </button>
          <button type="submit" className={styles.primary} disabled={!valid}>
            {confirmLabel}
          </button>
        </div>
      </form>
    </div>
  );
}

interface ConfirmProps {
  title: string;
  message: string;
  confirmLabel: string;
  onConfirm: () => void;
  onClose: () => void;
}

export function ConfirmDialog({ title, message, confirmLabel, onConfirm, onClose }: ConfirmProps) {
  const { t } = useTranslation();
  useEscape(onClose);
  return (
    <div className={styles.backdrop} onMouseDown={onClose}>
      <div className={styles.dialog} onMouseDown={(e) => e.stopPropagation()}>
        <h2 className={styles.title}>{title}</h2>
        <p className={styles.help}>{message}</p>
        <div className={styles.actions}>
          <button type="button" className={styles.button} onClick={onClose}>
            {t("addFeed.cancel")}
          </button>
          <button
            type="button"
            className={styles.primary}
            autoFocus
            onClick={() => {
              onConfirm();
              onClose();
            }}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

interface InfoProps {
  title: string;
  message: string;
  onClose: () => void;
}

/** Message d'information avec un seul bouton. */
export function InfoDialog({ title, message, onClose }: InfoProps) {
  const { t } = useTranslation();
  useEscape(onClose);
  return (
    <div className={styles.backdrop} onMouseDown={onClose}>
      <div className={styles.dialog} onMouseDown={(e) => e.stopPropagation()}>
        <h2 className={styles.title}>{title}</h2>
        <p className={styles.help}>{message}</p>
        <div className={styles.actions}>
          <button type="button" className={styles.primary} autoFocus onClick={onClose}>
            {t("filters.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
