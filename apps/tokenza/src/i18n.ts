import { en, type Dict } from "./locales/en";
import { ru } from "./locales/ru";
import { es } from "./locales/es";
import { zh } from "./locales/zh";
import { pt } from "./locales/pt";
import { de } from "./locales/de";
import { fr } from "./locales/fr";

export type Lang = "en" | "ru" | "es" | "zh" | "pt" | "de" | "fr";
/** What the user picks: a fixed language or "auto" (follow the system). */
export type LangSetting = "auto" | Lang;
export type Key = keyof Dict;

/** Native names, shown as-is in the language picker. */
export const LANGS: { code: Lang; name: string }[] = [
  { code: "en", name: "English" },
  { code: "ru", name: "Русский" },
  { code: "es", name: "Español" },
  { code: "zh", name: "中文（简体）" },
  { code: "pt", name: "Português (Brasil)" },
  { code: "de", name: "Deutsch" },
  { code: "fr", name: "Français" },
];

const dict: Record<Lang, Dict> = { en, ru, es, zh, pt, de, fr };

export const FALLBACK_LANG: Lang = "en";

export function t(lang: Lang, key: Key, vars?: Record<string, string>): string {
  let s: string = (dict[lang] ?? dict[FALLBACK_LANG])[key];
  for (const [k, v] of Object.entries(vars ?? {})) s = s.replaceAll(`{${k}}`, v);
  return s;
}

/**
 * First supported language among the system's preferred ones, else English.
 * Matches on the primary subtag, so "pt-PT" and "zh-Hant-TW" map to Portuguese and Chinese.
 */
export function detectLang(preferred: readonly string[] | undefined): Lang {
  for (const tag of preferred ?? []) {
    const primary = tag.toLowerCase().split(/[-_]/)[0];
    const hit = LANGS.find((l) => l.code === primary);
    if (hit) return hit.code;
  }
  return FALLBACK_LANG;
}

export function resolveLang(setting: LangSetting, systemResolved: Lang): Lang {
  return setting === "auto" ? systemResolved : setting;
}
