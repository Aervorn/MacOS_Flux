import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import {
  Apple, Archive, Atom, Bike, Book, Bookmark, Briefcase, Building2, Camera, Car, Cloud, Cpu,
  Code, CreditCard, Drama, Film, Flag, Flame, FlaskConical, Folder, Gamepad2, GraduationCap, Globe,
  Hammer, Headphones, Heart, House, Image, Inbox, Laptop, Leaf, Lightbulb, Lock, Map, Megaphone,
  MessagesSquare, Monitor, Music, Network, Newspaper, Palette, PawPrint, Pencil, Plane, Puzzle, Radio,
  Shield, ShoppingCart, Smartphone, Sparkles, Star, Sun, Tag, Terminal, Trophy, TrendingUp, Tv, Users,
  Utensils, Library, Wifi, Wrench, Zap, Footprints,
} from "lucide-react";

/** Les dossiers de l'app Swift utilisent des noms de symboles SF Symbols : on les traduit en icônes Lucide. */
const SYMBOLS: Record<string, LucideIcon> = {
  folder: Folder,
  tray: Inbox,
  archivebox: Archive,
  newspaper: Newspaper,
  book: Book,
  "books.vertical": Library,
  bookmark: Bookmark,
  tag: Tag,
  star: Star,
  heart: Heart,
  flag: Flag,
  bolt: Zap,
  flame: Flame,
  sparkles: Sparkles,
  lightbulb: Lightbulb,
  megaphone: Megaphone,
  "globe.europe.africa": Globe,
  map: Map,
  house: House,
  "building.2": Building2,
  briefcase: Briefcase,
  graduationcap: GraduationCap,
  "person.2": Users,
  "bubble.left.and.bubble.right": MessagesSquare,
  laptopcomputer: Laptop,
  desktopcomputer: Monitor,
  iphone: Smartphone,
  applelogo: Apple,
  cpu: Cpu,
  terminal: Terminal,
  "chevron.left.forwardslash.chevron.right": Code,
  network: Network,
  wifi: Wifi,
  "antenna.radiowaves.left.and.right": Radio,
  lock: Lock,
  shield: Shield,
  "wrench.and.screwdriver": Wrench,
  hammer: Hammer,
  atom: Atom,
  flask: FlaskConical,
  gamecontroller: Gamepad2,
  puzzlepiece: Puzzle,
  film: Film,
  tv: Tv,
  "music.note": Music,
  headphones: Headphones,
  camera: Camera,
  photo: Image,
  paintpalette: Palette,
  pencil: Pencil,
  theatermasks: Drama,
  leaf: Leaf,
  pawprint: PawPrint,
  "sun.max": Sun,
  "cloud.sun": Cloud,
  cart: ShoppingCart,
  creditcard: CreditCard,
  "chart.line.uptrend.xyaxis": TrendingUp,
  car: Car,
  airplane: Plane,
  bicycle: Bike,
  "figure.run": Footprints,
  sportscourt: Trophy,
  "fork.knife": Utensils,
};

export function folderIcon(symbol: string | null): LucideIcon {
  return (symbol && SYMBOLS[symbol]) || Folder;
}

/** Couleurs de `FolderColor` (app Swift), proches des teintes système d'Apple. */
const COLORS: Record<string, string> = {
  red: "#ff453a",
  orange: "#ff9f0a",
  yellow: "#ffd60a",
  green: "#30d158",
  mint: "#63e6e2",
  teal: "#40c8e0",
  cyan: "#64d2ff",
  blue: "#0a84ff",
  indigo: "#5e5ce6",
  purple: "#bf5af2",
  pink: "#ff375f",
  brown: "#ac8e68",
  gray: "#98989d",
};

/** Couleur d'un dossier (couleur d'accent si aucune). */
export function folderColor(colorName: string | null): string {
  return (colorName && COLORS[colorName]) || "var(--accent)";
}

/** Noms d'icônes proposés dans le sélecteur de dossier (les symboles de l'app Swift). */
export const SYMBOL_NAMES = Object.keys(SYMBOLS);
/** Noms de couleurs de la palette. */
export const COLOR_NAMES = Object.keys(COLORS);
/** Quelques emojis proposés pour un dossier. */
export const EMOJIS = [
  "📁", "📰", "📚", "📖", "🔖", "⭐", "❤️", "🔥", "💡", "📢", "🌍", "🏠", "🏢", "💼", "🎓", "👥",
  "💬", "💻", "🖥️", "📱", "🍎", "🤖", "🧠", "🔒", "🛡️", "🔧", "🛠️", "🔬", "🚀", "🎮", "🧩", "🎬",
  "📺", "🎵", "🎧", "📷", "🎨", "✏️", "🌿", "🐾", "☀️", "🛒", "💳", "📈", "🚗", "✈️", "🚲", "🏃",
  "⚽", "🍴", "☕", "🏡",
];

/** Vrai si le symbole enregistré est un emoji (et non un nom de symbole de l'app Swift). */
export function isEmoji(symbol: string | null): boolean {
  return !!symbol && !(symbol in SYMBOLS) && /\p{Extended_Pictographic}/u.test(symbol);
}

/** Icône d'un dossier : un emoji tel quel, sinon l'icône Lucide teintée de la couleur du dossier. */
export function folderGlyph(symbol: string | null, colorName: string | null, size = 16): ReactNode {
  if (isEmoji(symbol)) {
    return (
      <span style={{ width: size, height: size, display: "inline-flex", alignItems: "center", justifyContent: "center", fontSize: size - 2, lineHeight: 1 }}>
        {symbol}
      </span>
    );
  }
  const Icon = folderIcon(symbol);
  return <Icon size={size} color={folderColor(colorName)} />;
}
