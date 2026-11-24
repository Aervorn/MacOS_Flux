/** Date d'un article : relative pour les dernières 24 h, sinon jour et mois (avec l'année si ancienne). */
export function formatPublished(seconds: number, lang: string, now: number = Date.now()): string {
  const date = new Date(seconds * 1000);
  const diffMs = now - date.getTime();
  const hour = 3_600_000;
  if (diffMs >= 0 && diffMs < 24 * hour) {
    const rtf = new Intl.RelativeTimeFormat(lang, { numeric: "auto" });
    if (diffMs < hour) return rtf.format(-Math.max(1, Math.round(diffMs / 60_000)), "minute");
    return rtf.format(-Math.round(diffMs / hour), "hour");
  }
  const sameYear = date.getFullYear() === new Date(now).getFullYear();
  return date.toLocaleDateString(lang, sameYear ? { day: "numeric", month: "short" } : { day: "numeric", month: "short", year: "numeric" });
}
