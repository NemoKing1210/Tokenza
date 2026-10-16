import type { ProviderState, Settings, TrayKind, TrayStyle, WindowKind } from "./types";

export const TRAY_STYLES: TrayStyle[] = ["ring", "bars", "pie", "number"];
export const TRAY_KINDS: TrayKind[] = ["session", "daily", "weekly", "monthly"];

export interface TrayLevels {
  /** Highest used percentage over everything the icon follows, or null without data. */
  worst: number | null;
  session: number | null;
  weekly: number | null;
}

/** Shown in the previews until the first real numbers arrive. */
export const SAMPLE_LEVELS: TrayLevels = { worst: 62, session: 62, weekly: 35 };

/** A credit balance has no percentage, so the icon never looks at it. */
const KIND_OF: Record<WindowKind, TrayKind | null> = {
  Session5h: "session",
  Daily: "daily",
  Weekly: "weekly",
  Monthly: "monthly",
  Credits: null,
};

/** Mirrors `Settings::counts_in_tray` in the backend. */
export function countsInTray(s: Settings, id: string): boolean {
  if (s.tray_source === "single") return s.tray_provider === id;
  if (s.tray_source === "selected") return s.providers.find((p) => p.id === id)?.show_in_tray ?? true;
  return true;
}

/** What the icon draws, computed like the backend does (`tray_levels`), so the previews match it. */
export function trayLevels(s: Settings, providers: ProviderState[]): TrayLevels {
  const l: TrayLevels = { worst: null, session: null, weekly: null };
  const max = (a: number | null, b: number) => (a === null ? b : Math.max(a, b));
  for (const p of providers) {
    if (!p.enabled || !p.snapshot || !countsInTray(s, p.id)) continue;
    const hidden = s.providers.find((x) => x.id === p.id)?.hidden_windows ?? [];
    for (const w of p.snapshot.windows) {
      const kind = KIND_OF[w.kind];
      if (w.used_pct === null || hidden.includes(w.label) || (kind && !s.tray_kinds.includes(kind))) continue;
      l.worst = max(l.worst, w.used_pct);
      if (w.kind === "Session5h" || w.kind === "Daily") l.session = max(l.session, w.used_pct);
      else if (w.kind === "Weekly" || w.kind === "Monthly") l.weekly = max(l.weekly, w.used_pct);
    }
  }
  return l;
}

/** The icon's colours: the same bands as the flyout, in the exact shades the backend paints. */
export const TRAY_TONES = { accent: "#3A96DD", warning: "#F5A623", critical: "#E84A3F" };
export const TRAY_TRACK = "rgba(144, 144, 152, 0.53)";

export function trayTone(pct: number): string {
  return pct > 90 ? TRAY_TONES.critical : pct >= 70 ? TRAY_TONES.warning : TRAY_TONES.accent;
}

/** 3x5 pixel digits, one row per entry (bit 2 is the left column); same glyphs as the backend. */
export const DIGITS: number[][] = [
  [0b111, 0b101, 0b101, 0b101, 0b111],
  [0b010, 0b110, 0b010, 0b010, 0b111],
  [0b111, 0b001, 0b111, 0b100, 0b111],
  [0b111, 0b001, 0b111, 0b001, 0b111],
  [0b101, 0b101, 0b111, 0b001, 0b001],
  [0b111, 0b100, 0b111, 0b001, 0b111],
  [0b111, 0b100, 0b111, 0b101, 0b111],
  [0b111, 0b001, 0b001, 0b001, 0b001],
  [0b111, 0b101, 0b111, 0b101, 0b111],
  [0b111, 0b101, 0b111, 0b001, 0b111],
];

/** Squares (x, y, size) that spell the value on the 32 px canvas, centred like the backend does. */
export function numberCells(pct: number): { x: number; y: number; size: number }[] {
  const text = String(Math.round(Math.min(100, Math.max(0, pct))));
  const cell = text.length > 2 ? 3 : 4;
  const gap = cell / 2;
  const width = text.length * 3 * cell + (text.length - 1) * gap;
  const x0 = (32 - width) / 2;
  const y0 = (32 - 5 * cell) / 2;
  const out: { x: number; y: number; size: number }[] = [];
  [...text].forEach((ch, n) => {
    DIGITS[Number(ch)]!.forEach((bits, row) => {
      for (let col = 0; col < 3; col++) {
        if (bits & (0b100 >> col)) out.push({ x: x0 + n * (3 * cell + gap) + col * cell, y: y0 + row * cell, size: cell });
      }
    });
  });
  return out;
}
