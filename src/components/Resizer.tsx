import { useState } from "react";
import styles from "./Resizer.module.css";

interface Props {
  /** Largeur actuelle du panneau, en pixels. */
  value: number;
  min: number;
  max: number;
  /** Position de la poignée (CSS : left). */
  left: string;
  /** Appelé en continu pendant le glissement (aperçu), sans enregistrer. */
  onPreview: (v: number) => void;
  /** Appelé une fois au relâchement, avec la valeur finale. */
  onCommit: (v: number) => void;
}

/** Poignée verticale pour redimensionner un panneau à la souris. Double-clic : largeur par défaut. */
export default function Resizer({ value, min, max, left, onPreview, onCommit }: Props) {
  const [active, setActive] = useState(false);
  const clamp = (v: number) => Math.max(min, Math.min(max, Math.round(v)));

  const start = (e: React.PointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    const startX = e.clientX;
    const startValue = value;
    let last = value;
    setActive(true);
    document.body.style.cursor = "col-resize";
    const move = (ev: PointerEvent) => {
      last = clamp(startValue + ev.clientX - startX);
      onPreview(last);
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      document.body.style.cursor = "";
      setActive(false);
      onCommit(last);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  return <div className={`${styles.handle} ${active ? styles.active : ""}`} style={{ left }} onPointerDown={start} role="separator" aria-orientation="vertical" data-noprint />;
}
