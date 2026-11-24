import { Readability } from "@mozilla/readability";
import { buildSearchText, plainText, wordCount } from "./text";

export interface Extracted {
  html: string;
  searchText: string;
  wordCount: number;
  /** Première image non « pixel espion », à utiliser si l'article n'en a pas. */
  imageUrl: string | null;
}

const TRACKING = /(pixel|tracking|tracker|beacon|spacer|1x1|analytics|doubleclick)/i;

export function firstUsableImage(html: string): string | null {
  const doc = new DOMParser().parseFromString(html, "text/html");
  for (const img of Array.from(doc.querySelectorAll("img"))) {
    const src = img.getAttribute("src");
    if (!src || !/^https?:\/\//i.test(src) || TRACKING.test(src)) continue;
    const w = Number(img.getAttribute("width") ?? "");
    const h = Number(img.getAttribute("height") ?? "");
    if ((w > 0 && w <= 2) || (h > 0 && h <= 2)) continue;
    return src;
  }
  return null;
}

/** Extrait l'article d'une page entière avec Readability. */
export function extractArticle(pageHtml: string, pageUrl: string, title: string, summary: string): Extracted | null {
  const doc = new DOMParser().parseFromString(pageHtml, "text/html");
  const base = doc.createElement("base");
  base.href = pageUrl;
  doc.head.prepend(base);

  const result = new Readability(doc).parse();
  const content = result?.content?.trim();
  if (!content) return null;

  const text = plainText(content);
  if (!text) return null;
  return {
    html: content,
    searchText: buildSearchText(title, summary, text),
    wordCount: wordCount(text),
    imageUrl: firstUsableImage(content),
  };
}
