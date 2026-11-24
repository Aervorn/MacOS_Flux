import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { api, type OfflineInfo, type Prefs } from "../api";
import { countPending, onOfflineProgress, prepareOffline, type OfflineProgress } from "../offline";
import styles from "./SettingsDialog.module.css";

interface Props {
  prefs: Prefs;
  onPrefs: (patch: Partial<Prefs>) => void;
  toggle: (label: string, checked: boolean, onChange: (v: boolean) => void) => ReactNode;
  select: (value: number, options: { value: number; label: string }[], onPick: (v: number) => void) => ReactNode;
}

const LIMITS_MB = [250, 500, 1024, 2048, 5120];

function size(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} Ko`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(bytes < 100 * 1024 ** 2 ? 1 : 0)} Mo`;
  return `${(bytes / 1024 ** 3).toFixed(1)} Go`;
}

/** Lecture hors ligne et taille du cache d'images. */
export default function OfflineSettings({ prefs, onPrefs, toggle, select }: Props) {
  const { t } = useTranslation();
  const [info, setInfo] = useState<OfflineInfo | null>(null);
  const [progress, setProgress] = useState<OfflineProgress>(null);
  const [pending, setPending] = useState<number | null>(null);
  useEffect(() => onOfflineProgress(setProgress), []);
  // Articles qui attendent encore leur texte complet (recompté à chaque fin de préparation)
  useEffect(() => {
    if (progress === null && prefs.offlineEnabled) void countPending().then(setPending);
  }, [progress, prefs.offlineEnabled]);

  const load = useCallback(() => api.offlineStatus().then(setInfo).catch(() => undefined), []);
  useEffect(() => {
    void load();
  }, [load]);

  // Pendant une préparation, on suit la taille du cache.
  useEffect(() => {
    if (!info?.working) return;
    const id = setInterval(() => void load(), 2000);
    return () => clearInterval(id);
  }, [info?.working, load]);

  // Après un changement de réglage, on laisse le temps de l'enregistrer puis on applique.
  const apply = () =>
    setTimeout(() => {
      prepareOffline()
        .catch(() => undefined)
        .finally(() => void load());
      void load();
    }, 300);

  const limits = LIMITS_MB.includes(prefs.imageCacheLimitMb) ? LIMITS_MB : [...LIMITS_MB, prefs.imageCacheLimitMb].sort((a, b) => a - b);
  const label = (mb: number) => (mb >= 1024 ? t("settings.cacheGb", { count: mb / 1024 }) : t("settings.cacheMb", { count: mb }));

  return (
    <>
      <div className={styles.row}>
        {toggle(t("settings.offlineEnabled"), prefs.offlineEnabled, (offlineEnabled) => {
          onPrefs({ offlineEnabled });
          if (offlineEnabled) apply();
        })}
        <p className={styles.help}>{t("settings.offlineHelp")}</p>
        {prefs.offlineEnabled && (
          <p className={styles.help}>
            {progress
              ? t("settings.offlineArticles", { done: progress.done, total: progress.total })
              : info?.working
                ? t("settings.cachePreparing")
                : pending
                  ? t("settings.offlinePending", { count: pending })
                  : t("settings.offlineReady")}
            {!progress && !info?.working && (
              <>
                {" "}
                <button onClick={apply}>{t("settings.offlineNow")}</button>
              </>
            )}
          </p>
        )}
      </div>
      <div className={styles.row}>
        <span className={styles.label}>{t("settings.cacheLimit")}</span>
        {select(prefs.imageCacheLimitMb, limits.map((mb) => ({ value: mb, label: label(mb) })), (imageCacheLimitMb) => {
          onPrefs({ imageCacheLimitMb });
          apply();
        })}
        <p className={styles.help}>{t("settings.cacheLimitHelp")}</p>
        {info && (
          <p className={styles.help}>
            {t("settings.cacheSize", { size: size(info.cacheBytes) })}{" "}
            <button
              onClick={() =>
                api
                  .clearImageCache()
                  .catch(() => undefined)
                  .finally(() => void load())
              }
            >
              {t("settings.cacheClear")}
            </button>
          </p>
        )}
      </div>
    </>
  );
}
