/** Lecteurs vidéo autorisés dans un article : chargés seulement au clic. */
export interface Embed {
  /** Adresse du lecteur intégré. */
  src: string;
  /** Page de la vidéo (ouverte dans le navigateur si on ne charge pas le lecteur). */
  watch: string;
  provider: "YouTube" | "Vimeo" | "Dailymotion";
}

const ID = /^[\w-]{5,32}$/;

/** Reconnaît l'adresse d'un lecteur YouTube, Vimeo ou Dailymotion ; renvoie `null` pour tout le reste. */
export function parseEmbed(raw: string): Embed | null {
  let url: URL;
  try {
    url = new URL(raw.trim().startsWith("//") ? `https:${raw.trim()}` : raw.trim());
  } catch {
    return null;
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") return null;
  const host = url.hostname.replace(/^www\./, "").toLowerCase();
  const parts = url.pathname.split("/").filter(Boolean);

  if ((host === "youtube.com" || host === "youtube-nocookie.com") && parts[0] === "embed" && ID.test(parts[1] ?? "")) {
    return {
      src: `https://www.youtube-nocookie.com/embed/${parts[1]}`,
      watch: `https://www.youtube.com/watch?v=${parts[1]}`,
      provider: "YouTube",
    };
  }
  if (host === "player.vimeo.com" && parts[0] === "video" && /^\d{3,12}$/.test(parts[1] ?? "")) {
    const hash = url.searchParams.get("h");
    const suffix = hash && /^\w{4,32}$/.test(hash) ? `?h=${hash}` : "";
    return {
      src: `https://player.vimeo.com/video/${parts[1]}${suffix}`,
      watch: `https://vimeo.com/${parts[1]}`,
      provider: "Vimeo",
    };
  }
  if (host === "dailymotion.com" && parts[0] === "embed" && parts[1] === "video" && ID.test(parts[2] ?? "")) {
    return {
      src: `https://www.dailymotion.com/embed/video/${parts[2]}`,
      watch: `https://www.dailymotion.com/video/${parts[2]}`,
      provider: "Dailymotion",
    };
  }
  return null;
}

export interface VideoLabels {
  /** « Lire la vidéo {provider} » */
  play: (provider: string) => string;
  /** « Le lecteur de {provider} ne se charge qu’au clic. » */
  note: (provider: string) => string;
}

/**
 * Remplace chaque cadre vidéo reconnu par un emplacement « cliquer pour lire » (rien n'est chargé
 * chez le fournisseur avant le clic) ; les autres cadres sont supprimés.
 */
export function placeholderizeVideos(html: string, labels: VideoLabels): string {
  if (!/<iframe/i.test(html)) return html;
  const doc = new DOMParser().parseFromString(html, "text/html");
  doc.querySelectorAll("iframe").forEach((frame) => {
    const embed = parseEmbed(frame.getAttribute("src") ?? "");
    if (!embed) {
      frame.remove();
      return;
    }
    const box = doc.createElement("div");
    box.className = "flux-video";
    box.setAttribute("data-embed", embed.src);
    const link = doc.createElement("a");
    link.className = "flux-video-play";
    link.setAttribute("href", embed.watch);
    const title = doc.createElement("span");
    title.className = "flux-video-title";
    title.textContent = `▶ ${labels.play(embed.provider)}`;
    const note = doc.createElement("span");
    note.className = "flux-video-note";
    note.textContent = labels.note(embed.provider);
    link.append(title, note);
    box.append(link);
    frame.replaceWith(box);
  });
  return doc.body.innerHTML;
}

/** Crée le lecteur intégré (le clic de l'utilisateur est la seule chose qui déclenche son chargement). */
export function createPlayer(embed: Embed): HTMLIFrameElement {
  const frame = document.createElement("iframe");
  frame.src = embed.src;
  frame.title = embed.provider;
  frame.allow = "fullscreen; picture-in-picture; encrypted-media";
  frame.allowFullscreen = true;
  frame.referrerPolicy = "strict-origin-when-cross-origin";
  frame.setAttribute("sandbox", "allow-scripts allow-same-origin allow-presentation allow-popups");
  return frame;
}
