import { useRef, useState } from "react";
import { ChevronsUpDown } from "lucide-react";
import ContextMenu from "./ContextMenu";
import styles from "./Select.module.css";

interface Props<T extends string | number> {
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  disabled?: boolean;
  className?: string;
}

/** Liste déroulante aux couleurs de l'app (la liste native de macOS ne suit pas notre style). */
export default function Select<T extends string | number>({ value, options, onChange, disabled, className }: Props<T>) {
  const button = useRef<HTMLButtonElement>(null);
  const [anchor, setAnchor] = useState<{ x: number; y: number; width: number } | null>(null);
  const current = options.find((o) => o.value === value);

  const open = () => {
    const rect = button.current?.getBoundingClientRect();
    if (rect) setAnchor({ x: rect.left, y: rect.bottom + 4, width: rect.width });
  };

  return (
    <>
      <button type="button" ref={button} className={`${styles.select} ${className ?? ""}`} onClick={open} disabled={disabled}>
        <span className={styles.value}>{current?.label ?? ""}</span>
        <ChevronsUpDown size={14} />
      </button>
      {anchor && (
        <ContextMenu
          x={anchor.x}
          y={anchor.y}
          minWidth={anchor.width}
          items={options.map((o) => ({ label: o.label, checked: o.value === value, onClick: () => onChange(o.value) }))}
          onClose={() => setAnchor(null)}
        />
      )}
    </>
  );
}
