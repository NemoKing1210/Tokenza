import { describe, expect, it } from "vitest";
import { demoState } from "./demo";
import { countsInTray, numberCells, trayLevels, trayTone } from "./tray";
import type { Settings } from "./types";

const demo = () => demoState();
const withSettings = (patch: Partial<Settings>) => {
  const d = demo();
  return { providers: d.providers, settings: { ...d.settings, ...patch } };
};

describe("trayLevels", () => {
  it("takes the worst limit over every provider by default", () => {
    const { providers, settings } = withSettings({});
    expect(trayLevels(settings, providers)).toEqual({ worst: 96, session: 78, weekly: 96 });
  });

  it("follows a single provider only", () => {
    const { providers, settings } = withSettings({ tray_source: "single", tray_provider: "claude" });
    expect(trayLevels(settings, providers)).toEqual({ worst: 55, session: 33, weekly: 55 });
  });

  it("follows the chosen providers through their flag", () => {
    const d = demo();
    const prefs = d.settings.providers.map((p) => (p.id === "cursor" ? { ...p, show_in_tray: false } : p));
    const l = trayLevels({ ...d.settings, tray_source: "selected", providers: prefs }, d.providers);
    expect(l.worst).toBe(78); // Cursor's 96% is out; Codex's 5 h window is next.
  });

  it("looks only at the chosen kinds of windows", () => {
    const { providers, settings } = withSettings({ tray_kinds: ["session"] });
    expect(trayLevels(settings, providers)).toEqual({ worst: 78, session: 78, weekly: null });
  });

  it("skips hidden windows and disabled providers, and reports no data as null", () => {
    const d = demo();
    const prefs = d.settings.providers.map((p) => (p.id === "cursor" ? { ...p, hidden_windows: ["Included usage"] } : p));
    expect(trayLevels({ ...d.settings, providers: prefs }, d.providers).worst).toBe(78);
    const off = d.providers.map((p) => ({ ...p, enabled: false }));
    expect(trayLevels(d.settings, off)).toEqual({ worst: null, session: null, weekly: null });
  });
});

describe("tray helpers", () => {
  it("countsInTray mirrors the backend rules", () => {
    const d = demo();
    expect(countsInTray({ ...d.settings, tray_source: "all" }, "claude")).toBe(true);
    expect(countsInTray({ ...d.settings, tray_source: "single", tray_provider: "codex" }, "claude")).toBe(false);
    expect(countsInTray({ ...d.settings, tray_source: "single", tray_provider: "codex" }, "codex")).toBe(true);
  });

  it("uses the same colour bands as the flyout", () => {
    expect(trayTone(50)).toBe("#3A96DD");
    expect(trayTone(70)).toBe("#F5A623");
    expect(trayTone(91)).toBe("#E84A3F");
  });

  it("spells numbers within the 32 px canvas", () => {
    for (const pct of [0, 7, 62, 100]) {
      for (const c of numberCells(pct)) {
        expect(c.x).toBeGreaterThanOrEqual(0);
        expect(c.y).toBeGreaterThanOrEqual(0);
        expect(c.x + c.size).toBeLessThanOrEqual(32);
        expect(c.y + c.size).toBeLessThanOrEqual(32);
      }
    }
    expect(numberCells(62)).not.toEqual(numberCells(63));
  });
});
