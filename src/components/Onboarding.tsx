import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowLeft, ArrowRight, BookOpen, Cloud, Filter, Plus, Radio, Star, type LucideIcon } from "lucide-react";
import styles from "./Onboarding.module.css";

interface Page {
  id: string;
  icon: LucideIcon;
  color: string;
  /** Nombre d'astuces sous le texte (clés `onboarding.<id>.tip1`…). */
  tips: number;
}

const PAGES: Page[] = [
  { id: "welcome", icon: Radio, color: "#ff9f43", tips: 0 },
  { id: "add", icon: Plus, color: "#64a1f7", tips: 3 },
  { id: "read", icon: BookOpen, color: "#8b7cf6", tips: 3 },
  { id: "sort", icon: Filter, color: "#2dd4bf", tips: 2 },
  { id: "keep", icon: Star, color: "#facc15", tips: 2 },
  { id: "icloud", icon: Cloud, color: "#22d3ee", tips: 0 },
];

interface Props {
  onClose: () => void;
  /** Ouvre les réglages de synchronisation (dernière page). */
  onOpenSync: () => void;
}

/** Découverte de Flux, au premier lancement (et dans le menu de l'app). */
export default function Onboarding({ onClose, onOpenSync }: Props) {
  const { t } = useTranslation();
  const [index, setIndex] = useState(0);
  const page = PAGES[index];
  const last = index === PAGES.length - 1;
  const Icon = page.icon;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowRight") setIndex((i) => Math.min(PAGES.length - 1, i + 1));
      else if (e.key === "ArrowLeft") setIndex((i) => Math.max(0, i - 1));
      else if (e.key === "Escape") onClose();
      else return;
      e.stopPropagation();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  return (
    <div className={styles.backdrop}>
      <div className={styles.dialog} role="dialog" aria-label={t(`onboarding.${page.id}.title`)}>
        <div className={styles.top}>
          {!last && (
            <button className={styles.skip} onClick={onClose}>
              {t("onboarding.skip")}
            </button>
          )}
        </div>
        <div className={styles.page} key={page.id}>
          <div className={styles.icon} style={{ background: `${page.color}26`, color: page.color }}>
            <Icon size={34} />
          </div>
          <h2 className={styles.title}>{t(`onboarding.${page.id}.title`)}</h2>
          <p className={styles.text}>{t(`onboarding.${page.id}.text`)}</p>
          {page.tips > 0 && (
            <ul className={styles.tips}>
              {Array.from({ length: page.tips }, (_, n) => (
                <li key={n}>{t(`onboarding.${page.id}.tip${n + 1}`)}</li>
              ))}
            </ul>
          )}
          {last && (
            <button className={styles.button} onClick={onOpenSync}>
              {t("onboarding.openSettings")}
            </button>
          )}
        </div>
        <div className={styles.dots}>
          {PAGES.map((p, n) => (
            <button
              key={p.id}
              className={n === index ? styles.dotOn : styles.dot}
              onClick={() => setIndex(n)}
              aria-label={t(`onboarding.${p.id}.title`)}
            />
          ))}
        </div>
        <div className={styles.footer}>
          <button className={styles.button} onClick={() => setIndex(index - 1)} style={{ visibility: index === 0 ? "hidden" : "visible" }}>
            <ArrowLeft size={14} /> {t("onboarding.previous")}
          </button>
          {last ? (
            <button className={styles.primary} onClick={onClose}>
              {t("onboarding.start")}
            </button>
          ) : (
            <button className={styles.primary} onClick={() => setIndex(index + 1)}>
              {t("onboarding.next")} <ArrowRight size={14} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
