import { describe, expect, it } from "vitest";
import { formatAgo, formatDuration, formatReset, toneFor, windowLabel } from "./format";

describe("toneFor", () => {
  it("uses the documented bands", () => {
    expect(toneFor(42)).toBe("accent");
    expect(toneFor(70)).toBe("warning");
    expect(toneFor(90)).toBe("warning");
    expect(toneFor(90.5)).toBe("critical");
  });
});

describe("formatDuration", () => {
  it("keeps the two largest units", () => {
    expect(formatDuration((72 * 60 + 5) * 1000, "ru")).toBe("1 ч 12 мин");
    expect(formatDuration((46 * 3600 + 10) * 1000, "ru")).toBe("1 д 22 ч");
    expect(formatDuration(5 * 60 * 1000, "en")).toBe("5 min");
    expect(formatDuration(30 * 1000, "ru")).toBe("30 с");
    expect(formatDuration(90 * 1000, "en")).toBe("1 min 30 s");
    expect(formatDuration((12 * 60 + 40) * 1000, "en")).toBe("12 min");
  });
});

describe("formatReset", () => {
  const now = Date.parse("2026-10-16T12:00:00Z");
  it("counts down in the chosen language", () => {
    expect(formatReset("2026-10-16T13:12:00Z", now, "ru")).toBe("сброс через 1 ч 12 мин");
    expect(formatReset("2026-10-16T13:12:00Z", now, "en", true)).toBe("cycle resets in 1 h 12 min");
  });
  it("handles missing and past dates", () => {
    expect(formatReset(null, now, "ru")).toBe("");
    expect(formatReset("2026-10-16T11:00:00Z", now, "en")).toBe("resets now");
  });
});

describe("formatAgo", () => {
  const now = Date.parse("2026-10-16T12:00:30Z");
  it("formats age", () => {
    expect(formatAgo("2026-10-16T12:00:00Z", now, "ru")).toBe("обновлено 30 с назад");
    expect(formatAgo("2026-10-16T12:00:28Z", now, "en")).toBe("updated just now");
  });
});

describe("windowLabel", () => {
  it("translates by kind and keeps the model suffix", () => {
    expect(windowLabel({ kind: "Weekly", label: "Week · Opus", used_pct: 1, resets_at: null }, "ru")).toBe("Неделя · Opus");
    expect(windowLabel({ kind: "Session5h", label: "5 h", used_pct: 1, resets_at: null }, "en")).toBe("5 h");
  });
});
