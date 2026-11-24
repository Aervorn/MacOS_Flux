import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, type BackupInfo, type Prefs } from "../api";
import { formatPublished } from "../format";
import Select from "./Select";
import styles from "./SettingsDialog.module.css";

interface Props {
  prefs: Prefs;
  onPrefs: (patch: Partial<Prefs>) => void;
}

const KEEP = [3, 7, 14, 30];

const megabytes = (bytes: number) => `${Math.max(0.1, bytes / 1_048_576).toFixed(1)} Mo`;

/** Réglages > Sauvegarde : une archive par jour dans iCloud Drive, et restauration. */
export default function BackupSection({ prefs, onPrefs }: Props) {
  const { t, i18n } = useTranslation();
  const [info, setInfo] = useState<BackupInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(() => api.backupStatus().then(setInfo).catch(() => undefined), []);
  useEffect(() => {
    void load();
    const id = setInterval(() => void load(), 3000);
    return () => clearInterval(id);
  }, [load]);

  const errorText = (e: string | null) =>
    e === "icloud_unavailable" ? t("sync.icloudUnavailable") : e === "icloud_permission" ? t("sync.permission") : e;
  const date = (secs: number) => formatPublished(secs, i18n.language);

  const backupNow = async () => {
    setBusy(true);
    setNotice(null);
    try {
      await api.backupNow();
    } catch {
      /* l'erreur s'affiche dans l'état */
    } finally {
      setBusy(false);
      void load();
    }
  };

  const restore = async (path: string, name: string) => {
    const ok = await ask(t("backup.restoreConfirm", { name }), {
      title: t("backup.restoreTitle"),
      okLabel: t("backup.restore"),
      cancelLabel: t("addFeed.cancel"),
      kind: "warning",
    });
    if (!ok) return;
    setBusy(true);
    setNotice(null);
    try {
      const relaunching = await api.restoreBackup(path);
      if (!relaunching) {
        // En développement l'app ne se relance pas seule : on propose de quitter tout de suite.
        const quit = await ask(t("backup.relaunchManually"), {
          title: t("backup.restoreTitle"),
          okLabel: t("backup.quitNow"),
          cancelLabel: t("backup.later"),
          kind: "info",
        });
        if (quit) await api.quitApp();
        else setNotice(t("backup.relaunchManually"));
      }
    } catch (e) {
      setNotice(String(e));
    } finally {
      setBusy(false);
      void load();
    }
  };

  const error = errorText(info?.error ?? null);
  const working = busy || info?.working;

  return (
    <>
      <div className={styles.row}>
        <label className={styles.toggle}>
          <input type="checkbox" checked={prefs.backupEnabled} onChange={(e) => onPrefs({ backupEnabled: e.target.checked })} />
          {t("backup.enable")}
        </label>
        <p className={styles.help}>{t("backup.help")}</p>
      </div>

      <div className={styles.row}>
        <span className={styles.label}>{t("backup.keep")}</span>
        <Select
          value={prefs.backupKeep}
          onChange={(backupKeep) => onPrefs({ backupKeep })}
          options={(KEEP.includes(prefs.backupKeep) ? KEEP : [...KEEP, prefs.backupKeep].sort((a, b) => a - b)).map((n) => ({
            value: n,
            label: t("backup.keepCount", { count: n }),
          }))}
        />
      </div>

      <div className={styles.row}>
        <label className={styles.toggle}>
          <input type="checkbox" checked={prefs.backupIncludeArchive} onChange={(e) => onPrefs({ backupIncludeArchive: e.target.checked })} />
          {t("backup.includeArchive")}
        </label>
        <p className={styles.help}>{t("backup.includeArchiveHelp")}</p>
      </div>

      <div className={styles.row}>
        <span className={styles.label}>{t("sync.status")}</span>
        <p className={styles.help}>
          {working ? t("backup.working") : info?.lastBackup ? t("backup.last", { date: date(info.lastBackup) }) : t("backup.never")}
        </p>
        {error && <p className={styles.error}>{error}</p>}
        {notice && <p className={styles.help}>{notice}</p>}
        <div>
          <button className={styles.action} onClick={() => void backupNow()} disabled={working}>
            {t("backup.now")}
          </button>
        </div>
      </div>

      <div className={styles.row}>
        <span className={styles.label}>{t("backup.available")}</span>
        {info && info.backups.length === 0 && <p className={styles.help}>{t("backup.none")}</p>}
        <ul className={styles.backupList}>
          {info?.backups.map((b) => (
            <li key={b.path}>
              <span className={styles.backupName}>
                {b.name.replace(/^Flux /, "").replace(/\.zip$/, "").replace(/\./, ":")}
                <span className={styles.help}>
                  {" "}
                  — {b.device}
                  {b.downloaded ? ` · ${megabytes(b.size)}` : ` · ${t("backup.notDownloaded")}`}
                </span>
              </span>
              <button className={styles.action} onClick={() => void restore(b.path, b.name)} disabled={working}>
                {t("backup.restore")}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </>
  );
}
