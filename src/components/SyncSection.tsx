import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, type Prefs, type SyncStatus } from "../api";
import { formatPublished } from "../format";
import styles from "./SettingsDialog.module.css";

interface Props {
  prefs: Prefs;
  onPrefs: (patch: Partial<Prefs>) => void;
  /** Recharge la barre latérale et la liste (la synchro a pu changer des choses). */
  onChanged: () => void;
}

/** Réglages > Synchronisation : activer la synchro iCloud Drive et voir où elle en est. */
export default function SyncSection({ prefs, onPrefs, onChanged }: Props) {
  const { t, i18n } = useTranslation();
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  const load = useCallback(() => api.syncStatus().then(setStatus).catch(() => undefined), []);
  useEffect(() => {
    void load();
    const id = setInterval(() => void load(), 2000);
    return () => clearInterval(id);
  }, [load]);

  const toggle = async (enabled: boolean) => {
    onPrefs({ syncEnabled: enabled });
    setBusy(true);
    setFailure(null);
    try {
      await api.syncApply(enabled);
      onChanged();
    } catch (e) {
      setFailure(String(e));
    } finally {
      setBusy(false);
      void load();
    }
  };

  const syncNow = async () => {
    setBusy(true);
    setFailure(null);
    try {
      await api.syncNow();
      onChanged();
    } catch (e) {
      setFailure(String(e));
    } finally {
      setBusy(false);
      void load();
    }
  };

  const raw = status?.error ?? failure;
  const error = raw ? (raw === "icloud_unavailable" ? t("sync.icloudUnavailable") : raw === "icloud_permission" ? t("sync.permission") : raw) : null;
  const date = (secs: number) => formatPublished(secs, i18n.language);

  return (
    <>
      <div className={styles.row}>
        <label className={styles.toggle}>
          <input type="checkbox" checked={prefs.syncEnabled} disabled={busy} onChange={(e) => void toggle(e.target.checked)} />
          {t("sync.enable")}
        </label>
        <p className={styles.help}>{t("sync.help")}</p>
      </div>

      {prefs.syncEnabled && (
        <div className={styles.row}>
          <span className={styles.label}>{t("sync.status")}</span>
          <p className={styles.help}>
            {status?.syncing || busy
              ? t("sync.syncing")
              : status?.lastSync
                ? t("sync.lastSync", { date: date(status.lastSync) })
                : t("sync.never")}
          </p>
          {error && <p className={styles.error}>{error}</p>}
          {status && status.others.length > 0 && (
            <ul className={styles.deviceList}>
              {status.others.map((o) => (
                <li key={o.name + o.updated}>
                  {o.name} <span className={styles.help}>— {t("sync.updated", { date: date(o.updated) })}</span>
                </li>
              ))}
            </ul>
          )}
          {status && status.others.length === 0 && !error && <p className={styles.help}>{t("sync.noOther")}</p>}
          {status && status.incompatible.length > 0 && (
            <p className={styles.error}>{t("sync.incompatible", { names: status.incompatible.map((o) => o.name).join(", ") })}</p>
          )}
          <div>
            <button className={styles.action} onClick={() => void syncNow()} disabled={busy}>
              {t("sync.now")}
            </button>
          </div>
        </div>
      )}
    </>
  );
}
