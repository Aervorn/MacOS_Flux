import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import dialog from "./AddFeedDialog.module.css";
import styles from "./FolderStyleDialog.module.css";
import { COLOR_NAMES, EMOJIS, SYMBOL_NAMES, folderColor, folderGlyph, folderIcon } from "../icons";

interface Props {
  name: string;
  symbol: string | null;
  colorName: string | null;
  onSubmit: (symbol: string | null, color: string | null) => void;
  onClose: () => void;
}

/** Choix de l'icône (symbole ou emoji) et de la couleur d'un dossier. */
export default function FolderStyleDialog({ name, symbol: initialSymbol, colorName: initialColor, onSubmit, onClose }: Props) {
  const { t } = useTranslation();
  const [symbol, setSymbol] = useState<string | null>(initialSymbol);
  const [color, setColor] = useState<string | null>(initialColor);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className={dialog.backdrop} onMouseDown={onClose}>
      <div className={`${dialog.dialog} ${styles.dialog}`} onMouseDown={(e) => e.stopPropagation()}>
        <h2 className={dialog.title}>{t("dialogs.folderStyleTitle")}</h2>
        <div className={styles.preview}>
          {folderGlyph(symbol, color, 22)}
          <span>{name}</span>
        </div>

        <div className={styles.scroll}>
          <h3 className={styles.heading}>{t("dialogs.folderColor")}</h3>
          <div className={styles.colors}>
            <button
              type="button"
              className={`${styles.swatch} ${color === null ? styles.selected : ""}`}
              style={{ background: folderColor(null) }}
              title={t("dialogs.folderDefaultColor")}
              aria-label={t("dialogs.folderDefaultColor")}
              onClick={() => setColor(null)}
            />
            {COLOR_NAMES.map((c) => (
              <button
                key={c}
                type="button"
                className={`${styles.swatch} ${color === c ? styles.selected : ""}`}
                style={{ background: folderColor(c) }}
                title={c}
                aria-label={c}
                onClick={() => setColor(c)}
              />
            ))}
          </div>

          <h3 className={styles.heading}>{t("dialogs.folderIcon")}</h3>
          <div className={styles.grid}>
            {SYMBOL_NAMES.map((s) => {
              const Icon = folderIcon(s);
              return (
                <button
                  key={s}
                  type="button"
                  className={`${styles.cell} ${symbol === s ? styles.selected : ""}`}
                  onClick={() => setSymbol(s)}
                  aria-label={s}
                >
                  <Icon size={18} color={folderColor(color)} />
                </button>
              );
            })}
          </div>

          <h3 className={styles.heading}>{t("dialogs.folderEmoji")}</h3>
          <div className={styles.grid}>
            {EMOJIS.map((e) => (
              <button
                key={e}
                type="button"
                className={`${styles.cell} ${styles.emoji} ${symbol === e ? styles.selected : ""}`}
                onClick={() => setSymbol(e)}
              >
                {e}
              </button>
            ))}
          </div>
        </div>

        <div className={dialog.actions}>
          <button type="button" className={dialog.button} onClick={onClose}>
            {t("addFeed.cancel")}
          </button>
          <button type="button" className={dialog.primary} autoFocus onClick={() => onSubmit(symbol, color)}>
            {t("dialogs.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
