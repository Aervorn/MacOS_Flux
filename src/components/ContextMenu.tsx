import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronRight } from "lucide-react";
import styles from "./ContextMenu.module.css";

export type MenuItem =
  | "separator"
  | {
      label: string;
      icon?: React.ReactNode;
      onClick?: () => void;
      danger?: boolean;
      checked?: boolean;
      disabled?: boolean;
      submenu?: MenuItem[];
    };

interface Props {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
}

const MARGIN = 8;

function MenuList({
  items,
  onClose,
  style,
  flip,
}: {
  items: MenuItem[];
  onClose: () => void;
  style?: React.CSSProperties;
  flip?: boolean;
}) {
  const [open, setOpen] = useState<number | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const [flipped, setFlipped] = useState(false);
  const [shiftY, setShiftY] = useState(0);

  // Un sous-menu qui dépasserait à droite de la fenêtre s'ouvre à gauche, et remonte
  // s'il dépasserait en bas (si la liste est plus haute que la fenêtre, elle défile).
  useLayoutEffect(() => {
    if (!flip || !ref.current) return;
    const rect = ref.current.getBoundingClientRect();
    setFlipped(rect.right > window.innerWidth - MARGIN);
    const overflow = rect.bottom - (window.innerHeight - MARGIN);
    if (overflow > 0) {
      const room = rect.top - MARGIN; // de combien on peut remonter
      setShiftY(-Math.min(overflow, Math.max(room, 0)));
    }
  }, [flip]);

  return (
    <div
      ref={ref}
      className={`${styles.menu} ${flip ? styles.submenu : ""} ${flipped ? styles.flipped : ""}`}
      style={{ ...style, ...(shiftY ? { transform: `translateY(${shiftY}px)` } : {}), ...(flip ? { maxHeight: window.innerHeight - 2 * MARGIN, overflowY: "auto" } : {}) }}
      role="menu"
    >
      {items.map((item, i) => {
        if (item === "separator") return <div key={i} className={styles.separator} role="separator" />;
        const hasSub = !!item.submenu;
        return (
          <div key={i} className={styles.itemWrap} onMouseEnter={() => setOpen(hasSub ? i : null)}>
            <button
              role="menuitem"
              className={`${styles.item} ${item.danger ? styles.danger : ""}`}
              disabled={item.disabled}
              onClick={() => {
                if (hasSub) return;
                onClose();
                item.onClick?.();
              }}
            >
              <span className={styles.icon}>{item.checked ? <Check size={16} /> : item.icon}</span>
              <span className={styles.label}>{item.label}</span>
              {hasSub && <ChevronRight size={14} className={styles.chevron} />}
            </button>
            {hasSub && open === i && <MenuList items={item.submenu!} onClose={onClose} flip />}
          </div>
        );
      })}
    </div>
  );
}

/** Menu contextuel maison (clic droit) : se ferme au clic ailleurs, sur Échap ou si la fenêtre perd le focus. */
export default function ContextMenu({ x, y, items, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: x, top: y });

  // Le menu reste entièrement dans la fenêtre.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    setPos({
      left: Math.max(MARGIN, Math.min(x, window.innerWidth - rect.width - MARGIN)),
      top: Math.max(MARGIN, Math.min(y, window.innerHeight - rect.height - MARGIN)),
    });
  }, [x, y, items]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", onClose);
    window.addEventListener("resize", onClose);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", onClose);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);

  return createPortal(
    <div
      className={styles.backdrop}
      onMouseDown={onClose}
      onContextMenu={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      <div ref={ref} className={styles.root} style={pos} onMouseDown={(e) => e.stopPropagation()}>
        <MenuList items={items} onClose={onClose} />
      </div>
    </div>,
    document.body,
  );
}
