// Vérifie que fr.json et en.json ont exactement les mêmes clés.
import { readFileSync } from "node:fs";

const load = (lang) => JSON.parse(readFileSync(new URL(`../src/locales/${lang}.json`, import.meta.url), "utf8"));
const keys = (obj, prefix = "") =>
  Object.entries(obj).flatMap(([k, v]) =>
    typeof v === "object" && v !== null ? keys(v, `${prefix}${k}.`) : [`${prefix}${k}`],
  );

const fr = new Set(keys(load("fr")));
const en = new Set(keys(load("en")));
const missingEn = [...fr].filter((k) => !en.has(k));
const missingFr = [...en].filter((k) => !fr.has(k));

if (missingEn.length || missingFr.length) {
  if (missingEn.length) console.error("Clés absentes de en.json :", missingEn.join(", "));
  if (missingFr.length) console.error("Clés absentes de fr.json :", missingFr.join(", "));
  process.exit(1);
}
console.log(`i18n OK : ${fr.size} clés dans chaque langue.`);
