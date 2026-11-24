import { imageSrc } from "../api";

export async function sha256Hex(text: string): Promise<string> {
  const bytes = new TextEncoder().encode(text);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, "0")).join("");
}

/** Adresse de l'image dans le cache local (même nommage que l'app Swift). */
export async function cachedImageSrc(url: string): Promise<string> {
  return imageSrc(await sha256Hex(url));
}

/**
 * Remplace les images http(s) d'un conteneur par leur copie en cache, avec repli sur l'adresse
 * d'origine si la copie n'existe pas (voir `restoreOnError`).
 */
export async function rewriteImages(root: HTMLElement): Promise<void> {
  const imgs = Array.from(root.querySelectorAll("img"));
  await Promise.all(
    imgs.map(async (img) => {
      const src = img.getAttribute("src");
      if (!src || !/^https?:\/\//i.test(src)) return;
      img.dataset.orig = src;
      img.src = await cachedImageSrc(src);
    }),
  );
}

/** À brancher sur l'événement « error » (phase de capture) : revient à l'image d'origine. */
export function restoreOnError(target: EventTarget): void {
  if (!(target instanceof HTMLImageElement)) return;
  const orig = target.dataset.orig;
  if (orig && target.src !== orig) {
    target.src = orig;
    delete target.dataset.orig;
  }
}
