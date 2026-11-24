import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Code2 } from "lucide-react";
import { api } from "../api";
import icon from "../assets/app-icon.png";
import styles from "./AboutDialog.module.css";

const REPOSITORY = "https://github.com/Aervorn/MacOS_Flux";
const READABILITY = "https://github.com/mozilla/readability";

/** Fenêtre « À propos », reprise de celle de l'app Swift. */
export default function AboutDialog({ version, onClose }: { version: string | null; onClose: () => void }) {
  const { t } = useTranslation();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const link = (url: string, label: string, className?: string) => (
    <a
      href={url}
      className={className}
      onClick={(e) => {
        e.preventDefault();
        api.openUrl(url).catch(() => undefined);
      }}
    >
      {label}
    </a>
  );

  return (
    <div className={styles.backdrop} onMouseDown={onClose}>
      <div className={styles.dialog} onMouseDown={(e) => e.stopPropagation()} role="dialog" aria-label={t("about.title")}>
        <img src={icon} alt="" className={styles.icon} draggable={false} />
        <h2>Flux</h2>
        {version && <p className={styles.version}>{t("about.version", { version })}</p>}
        <p className={styles.tagline}>{t("about.tagline")}</p>
        <hr />
        <p className={styles.small}>{t("about.credits")}</p>
        <p className={styles.small}>
          {t("about.extraction")} {link(READABILITY, "Readability.js")} {t("about.license")}
        </p>
        <p className={styles.source}>
          <Code2 size={14} />
          {link(REPOSITORY, t("about.source"))}
        </p>
      </div>
    </div>
  );
}
