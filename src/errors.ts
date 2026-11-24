import type { TFunction } from "i18next";

/**
 * Texte d'une erreur renvoyée par le cœur Rust. Les erreurs de flux arrivent sous forme de code
 * (`feed:not_a_feed`, `feed:http:404`…) pour pouvoir être traduites ; le reste est affiché tel quel.
 */
export function errorMessage(error: unknown, t: TFunction): string {
  const raw = error instanceof Error ? error.message : String(error);
  if (raw === "feed:invalid_url") return t("errors.invalidUrl");
  if (raw === "feed:not_a_feed") return t("errors.notAFeed");
  if (raw === "feed:already_exists") return t("errors.alreadyExists");
  const http = /^feed:http:(\d+)$/.exec(raw);
  if (http) return t("errors.http", { code: http[1] });
  return raw;
}
