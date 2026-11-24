import { useEffect, useState } from "react";
import { imageSrc } from "../api";
import styles from "./Thumb.module.css";

/** Miniature d'un article : image du cache, ou fond dégradé s'il n'y en a pas (ou si elle manque). */
export default function Thumb({ thumb, className }: { thumb: string | null; className?: string }) {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [thumb]);

  return (
    <div className={`${styles.thumb} ${className ?? ""}`}>
      {thumb && !failed && (
        <img src={imageSrc(thumb)} alt="" loading="lazy" draggable={false} onError={() => setFailed(true)} />
      )}
    </div>
  );
}
