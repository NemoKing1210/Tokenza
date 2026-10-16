import { describe, expect, it } from "vitest";
import { LANGS, detectLang, resolveLang, t, type Key } from "./i18n";
import { en } from "./locales/en";
import { ru } from "./locales/ru";
import { es } from "./locales/es";
import { zh } from "./locales/zh";
import { pt } from "./locales/pt";
import { de } from "./locales/de";
import { fr } from "./locales/fr";
import { formatDuration, formatReset } from "./format";

const locales = { en, ru, es, zh, pt, de, fr } as Record<string, Record<string, string>>;
const placeholders = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(",");

describe("detectLang", () => {
  it("takes the first supported preferred language", () => {
    expect(detectLang(["ru-RU", "en-US"])).toBe("ru");
    expect(detectLang(["ja-JP", "de-DE", "en-US"])).toBe("de");
    expect(detectLang(["pt-PT"])).toBe("pt");
    expect(detectLang(["zh-Hant-TW"])).toBe("zh");
    expect(detectLang(["es_419"])).toBe("es");
    expect(detectLang(["FR-ca"])).toBe("fr");
  });

  it("falls back to English", () => {
    expect(detectLang(["ja-JP", "ko-KR"])).toBe("en");
    expect(detectLang([])).toBe("en");
    expect(detectLang(undefined)).toBe("en");
  });

  it("an explicit choice beats the system language", () => {
    expect(resolveLang("fr", "ru")).toBe("fr");
    expect(resolveLang("auto", "ru")).toBe("ru");
  });
});

describe("locales", () => {
  it("offers seven languages", () => {
    expect(LANGS.map((l) => l.code).sort()).toEqual(["de", "en", "es", "fr", "pt", "ru", "zh"]);
  });

  for (const [code, dict] of Object.entries(locales)) {
    it(`${code}: same keys and placeholders as English, nothing empty`, () => {
      expect(Object.keys(dict).sort()).toEqual(Object.keys(en).sort());
      for (const key of Object.keys(en)) {
        expect(dict[key], `${code}.${key}`).toBeTruthy();
        expect(placeholders(dict[key]!), `${code}.${key}`).toBe(placeholders((en as Record<string, string>)[key]!));
      }
    });
  }

  it("translations are not left in English by accident", () => {
    const sameAsEnglish = (code: string) =>
      Object.keys(en).filter((k) => locales[code]![k] === (en as Record<string, string>)[k]);
    // Short technical strings (units, "5 h", "min", provider-neutral words) may legitimately match.
    for (const code of ["ru", "zh"]) expect(sameAsEnglish(code).length, code).toBeLessThan(5);
    for (const code of ["es", "pt", "de", "fr"]) expect(sameAsEnglish(code).length, code).toBeLessThan(20);
  });

  it("t() substitutes variables in every language", () => {
    for (const { code } of LANGS) {
      expect(t(code, "nowEvery", { x: "5 min" })).toContain("5 min");
      expect(t(code, "hits429", { n: "3" })).toContain("3");
    }
  });
});

describe("localized formatting", () => {
  const now = Date.parse("2026-10-16T12:00:00Z");
  it("uses each language's units and phrasing", () => {
    expect(formatDuration((72 * 60 + 5) * 1000, "zh")).toBe("1 小时 12 分钟");
    expect(formatDuration((46 * 3600 + 10) * 1000, "de")).toBe("1 Tg. 22 Std.");
    expect(formatReset("2026-10-16T13:12:00Z", now, "es")).toBe("se reinicia en 1 h 12 min");
    expect(formatReset("2026-10-16T13:12:00Z", now, "fr", true)).toBe("le cycle est réinitialisé dans 1 h 12 min");
    expect(formatReset("2026-10-16T13:12:00Z", now, "zh")).toBe("1 小时 12 分钟后重置");
  });
});

// Keeps the Key type exercised so a renamed key fails type-checking here too.
const _k: Key = "refresh";
void _k;
