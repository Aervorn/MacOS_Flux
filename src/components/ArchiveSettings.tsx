import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { api, type ArchiveInfo, type Prefs } from "../api";
import styles from "./SettingsDialog.module.css";

interface Props {
  prefs: Prefs;
  onPrefs: (patch: Partial<Prefs>) => void;
  toggle: (label: string, checked: boolean, onChange: (v: boolean) => void) => ReactNode;
}

function size(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} Ko`;
  return `${(bytes / (1024 * 1024)).toFixed(bytes < 100 * 1024 * 1024 ? 1 : 0)} Mo`;
}

/** Réglage « Archiver les images des favoris », avec l'état de l'archive. */
export default function ArchiveSettings({ prefs, onPrefs, toggle }: Props) {
  const { t } = useTranslation();
  const [info, setInfo] = useState<ArchiveInfo | null>(null);
  const [working, setWorking] = useState(false);

  const load = useCallback(() => api.archiveStatus().then(setInfo).catch(() => undefined), []);
  useEffect(() => {
    void load();
  }, [load]);

  const archiveNow = () => {
    setWorking(true);
    api
      .archivePending()
      .catch(() => undefined)
      .finally(() => {
        setWorking(false);
        void load();
      });
  };

  // Pendant un archivage lancé ailleurs (étoile, démarrage), on rafraîchit l'état.
  useEffect(() => {
    if (!info?.working) return;
    const id = setInterval(() => void load(), 1500);
    return () => clearInterval(id);
  }, [info?.working, load]);

  const busy = working || info?.working;
  return (
    <div className={styles.row}>
      {toggle(t("settings.archiveFavorites"), prefs.archiveFavorites, (archiveFavorites) => {
        onPrefs({ archiveFavorites });
        if (archiveFavorites) setTimeout(archiveNow, 300); // laisse le temps d'enregistrer le réglage
      })}
      <p className={styles.help}>{t("settings.archiveHelp")}</p>
      {info && (
        <p className={styles.help}>
          {busy
            ? t("settings.archiving")
            : info.pending > 0 && prefs.archiveFavorites
              ? t("settings.archivePending", { count: info.pending })
              : t("settings.archiveDone", { count: info.archived, size: size(info.bytes) })}
          {!busy && prefs.archiveFavorites && info.pending > 0 && (
            <>
              {" "}
              <button onClick={archiveNow}>{t("settings.archiveRetry")}</button>
            </>
          )}
        </p>
      )}
    </div>
  );
}
