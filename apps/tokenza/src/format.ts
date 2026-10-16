import { t, type Lang } from "./i18n";
import type { CustomDef, UsageWindow } from "./types";

export type { Lang };
export type Tone = "accent" | "warning" | "critical";

/** < 70 accent, 70–90 amber, > 90 red (same bands as the tray icon). */
export function toneFor(pct: number): Tone {
  if (pct > 90) return "critical";
  if (pct >= 70) return "warning";
  return "accent";
}

/** E.g. "1 d 22 h", "1 h 12 min", "5 min": the two largest non-zero units, localized. */
export function formatDuration(ms: number, lang: Lang): string {
  const u = { d: t(lang, "unitD"), h: t(lang, "unitH"), m: t(lang, "unitM"), s: t(lang, "unitS") };
  const total = Math.max(0, Math.round(ms / 1000));
  const d = Math.floor(total / 86400);
  const h = Math.floor((total % 86400) / 3600);
  const m = Math.floor((total % 3600) / 60);
  if (d > 0) return h > 0 ? `${d} ${u.d} ${h} ${u.h}` : `${d} ${u.d}`;
  if (h > 0) return m > 0 ? `${h} ${u.h} ${m} ${u.m}` : `${h} ${u.h}`;
  // Under ten minutes the seconds matter (a 90 s interval is not "1 min").
  if (m > 0) return m < 10 && total % 60 > 0 ? `${m} ${u.m} ${total % 60} ${u.s}` : `${m} ${u.m}`;
  return `${total} ${u.s}`;
}

export function formatReset(resetsAt: string | null, now: number, lang: Lang, cycle = false): string {
  if (!resetsAt) return "";
  const ms = new Date(resetsAt).getTime() - now;
  if (ms <= 0) return t(lang, cycle ? "cycleResetsNow" : "resetsNow");
  return t(lang, cycle ? "cycleResetsIn" : "resetsIn", { x: formatDuration(ms, lang) });
}

/** "subscription until 1 Nov 2026", or "subscription ended …" once the date has passed. */
export function formatSubscriptionEnd(endsAt: string, now: number, lang: Lang): string {
  const end = new Date(endsAt);
  const date = end.toLocaleDateString(lang, { day: "numeric", month: "short", year: "numeric" });
  return t(lang, end.getTime() > now ? "subscriptionUntil" : "subscriptionEnded", { x: date });
}

export function windowLabel(w: UsageWindow, lang: Lang, short = false): string {
  const suffix = w.label.includes(" · ") ? ` · ${w.label.split(" · ").slice(1).join(" · ")}` : "";
  switch (w.kind) {
    case "Session5h":
      return t(lang, "winSession");
    case "Daily":
      return t(lang, "winDaily");
    case "Weekly":
      return t(lang, "winWeekly") + suffix;
    case "Monthly":
      return t(lang, short ? "winMonthlyShort" : "winMonthly");
    case "Credits":
      return t(lang, "winCredits");
  }
}

export const PROVIDER_NAMES: Record<string, string> = {
  claude: "Claude Code",
  codex: "Codex",
  cursor: "Cursor",
  opencode: "OpenCode",
  commandcode: "Command Code",
  devin: "Devin",
  antigravity: "Antigravity",
};

/** Built-in name, or the name of a user-defined provider. */
export const providerName = (id: string, custom: CustomDef[] = []): string =>
  PROVIDER_NAMES[id] ?? custom.find((d) => d.id === id)?.name ?? id;

export function planLabel(account: string): string {
  if (!account) return "";
  return account.charAt(0).toUpperCase() + account.slice(1);
}
