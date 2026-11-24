import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import { api, type ReadingStats, type SourceCount } from "../api";
import styles from "./StatsDialog.module.css";

const PERIODS = [7, 30, 90, 365] as const;

function duration(seconds: number): string {
  const minutes = Math.round(seconds / 60);
  if (minutes < 1) return seconds >= 1 ? "< 1 min" : "0 min";
  if (minutes < 60) return `${minutes} min`;
  return `${Math.floor(minutes / 60)} h ${String(minutes % 60).padStart(2, "0")}`;
}

function SourceList({ title, rows, empty }: { title: string; rows: SourceCount[]; empty: string }) {
  const max = Math.max(1, ...rows.map((r) => r.count));
  return (
    <section className={styles.sources}>
      <h3>{title}</h3>
      {rows.length === 0 && <p className={styles.muted}>{empty}</p>}
      {rows.map((r) => (
        <div key={r.name} className={styles.source}>
          <span className={styles.sourceName}>{r.name}</span>
          <span className={styles.sourceBar}>
            <span style={{ width: `${(r.count / max) * 100}%` }} />
          </span>
          <span className={styles.sourceCount}>{r.count}</span>
        </div>
      ))}
    </section>
  );
}

export default function StatsDialog({ onClose }: { onClose: () => void }) {
  const { t, i18n } = useTranslation();
  const [period, setPeriod] = useState<(typeof PERIODS)[number]>(30);
  const [stats, setStats] = useState<ReadingStats | null>(null);

  useEffect(() => {
    let cancelled = false;
    api
      .readingStats(period)
      .then((s) => !cancelled && setStats(s))
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [period]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const fmt = (d: Date) => d.toLocaleDateString(i18n.language, { day: "numeric", month: "short" });

  // Un bâton par jour, ou par semaine (7 jours, aujourd'hui en dernier) sur 12 mois.
  const bars = useMemo(() => {
    if (!stats) return [];
    const step = period === 365 ? 7 : 1;
    const daily = stats.daily;
    const out: { label: string; count: number }[] = [];
    for (let end = daily.length; end > 0; end -= step) {
      const start = Math.max(0, end - step);
      const count = daily.slice(start, end).reduce((a, b) => a + b, 0);
      const date = new Date();
      date.setDate(date.getDate() - (daily.length - 1 - start));
      out.unshift({ label: t(step === 1 ? "stats.day" : "stats.week", { date: fmt(date), count }), count });
    }
    return out;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stats, period, i18n.language]);
  const maxBar = Math.max(1, ...bars.map((b) => b.count));

  return (
    <div className={styles.backdrop} onMouseDown={onClose}>
      <div className={styles.dialog} onMouseDown={(e) => e.stopPropagation()} role="dialog" aria-label={t("stats.title")}>
        <header className={styles.header}>
          <h2>{t("stats.title")}</h2>
          <button className={styles.close} onClick={onClose} aria-label={t("stats.close")}>
            <X size={16} />
          </button>
        </header>
        <div className={styles.periods}>
          {PERIODS.map((p) => (
            <button key={p} className={p === period ? styles.active : undefined} onClick={() => setPeriod(p)}>
              {t(`stats.period${p}`)}
            </button>
          ))}
        </div>
        {stats && (
          <>
            <div className={styles.tiles}>
              <div>
                <strong>{stats.readCount}</strong>
                <span>{t("stats.read")}</span>
                <small>{t("stats.received", { count: stats.receivedCount })}</small>
              </div>
              <div>
                <strong>{duration(stats.readingSeconds)}</strong>
                <span>{t("stats.time")}</span>
              </div>
              <div>
                <strong>{duration(stats.estimatedSeconds)}</strong>
                <span>{t("stats.estimated")}</span>
              </div>
              <div>
                <strong>{stats.perDay.toLocaleString(i18n.language, { maximumFractionDigits: 1 })}</strong>
                <span>{t("stats.perDay")}</span>
              </div>
            </div>
            <section>
              <h3>{t("stats.chart")}</h3>
              <div className={styles.chart}>
                {bars.map((b, i) => (
                  <span key={i} className={styles.bar} title={b.label}>
                    <span style={{ height: `${(b.count / maxBar) * 100}%`, minHeight: b.count > 0 ? 2 : 0 }} />
                  </span>
                ))}
              </div>
            </section>
            <div className={styles.columns}>
              <SourceList title={t("stats.mostRead")} rows={stats.mostRead} empty={t("stats.empty")} />
              <SourceList title={t("stats.mostActive")} rows={stats.mostActive} empty={t("stats.empty")} />
            </div>
            <p className={styles.footer}>
              {stats.trackingSince
                ? t("stats.footer", {
                    date: new Date(stats.trackingSince * 1000).toLocaleDateString(i18n.language, { dateStyle: "long" }),
                  })
                : t("stats.footerNone")}
            </p>
          </>
        )}
      </div>
    </div>
  );
}
