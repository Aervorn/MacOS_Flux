import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import type { SidebarFolder } from "../api";
import { errorMessage } from "../errors";
import styles from "./AddFeedDialog.module.css";

interface Props {
  folders: SidebarFolder[];
  initialFolderUid: string | null;
  onClose: () => void;
  /** Appelé quand le flux est ajouté (adresse du flux créé). */
  onAdded: (feedUrl: string) => void;
}

export default function AddFeedDialog({ folders, initialFolderUid, onClose, onAdded }: Props) {
  const { t } = useTranslation();
  const [address, setAddress] = useState("");
  const [folderUid, setFolderUid] = useState<string>(initialFolderUid ?? "");
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => input.current?.focus(), []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && !working && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, working]);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!address.trim() || working) return;
    setWorking(true);
    setError(null);
    try {
      const added = await api.addFeed(address, folderUid || null);
      onAdded(added.url);
    } catch (err) {
      setError(errorMessage(err, t));
      setWorking(false);
    }
  };

  return (
    <div className={styles.backdrop} onMouseDown={() => !working && onClose()}>
      <form className={styles.dialog} onMouseDown={(e) => e.stopPropagation()} onSubmit={submit}>
        <h2 className={styles.title}>{t("addFeed.title")}</h2>
        <p className={styles.help}>{t("addFeed.help")}</p>
        <input
          ref={input}
          className={styles.field}
          value={address}
          onChange={(e) => setAddress(e.target.value)}
          placeholder="https://exemple.fr/feed.xml"
          spellCheck={false}
          autoCapitalize="off"
          disabled={working}
        />
        {folders.length > 0 && (
          <label className={styles.row}>
            <span>{t("addFeed.folder")}</span>
            <select className={styles.field} value={folderUid} onChange={(e) => setFolderUid(e.target.value)} disabled={working}>
              <option value="">{t("addFeed.noFolder")}</option>
              {folders.map((f) => (
                <option key={f.uid} value={f.uid}>
                  {f.name}
                </option>
              ))}
            </select>
          </label>
        )}
        {error && <p className={styles.error}>{error}</p>}
        <div className={styles.actions}>
          <button type="button" className={styles.button} onClick={onClose} disabled={working}>
            {t("addFeed.cancel")}
          </button>
          <button type="submit" className={styles.primary} disabled={!address.trim() || working}>
            {working ? t("addFeed.adding") : t("addFeed.add")}
          </button>
        </div>
      </form>
    </div>
  );
}
