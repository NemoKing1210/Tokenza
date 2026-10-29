import { detectLang } from "./i18n";
import type { AppStateDto, UsageSnapshot } from "./types";

const inMs = (ms: number) => new Date(Date.now() + ms).toISOString();
const ago = (ms: number) => new Date(Date.now() - ms).toISOString();
const H = 3600_000;

function snap(provider: string, account: string, windows: UsageSnapshot["windows"], agoMs: number, subscriptionInMs?: number): UsageSnapshot {
  return { provider, account, windows, fetched_at: ago(agoMs), ...(subscriptionInMs !== undefined && { subscription_ends_at: inMs(subscriptionInMs) }) };
}

/** Browser-only sample data that mirrors the Figma mockups. */
export function demoState(): AppStateDto {
  return {
    resolved_lang: detectLang(typeof navigator === "undefined" ? [] : navigator.languages),
    settings: {
      update_mode: "smart",
      interval_secs: 180,
      min_interval_secs: 60,
      max_interval_secs: 1800,
      boost_near_limit: true,
      slow_when_idle: true,
      warn_pct: 80,
      crit_pct: 95,
      notify_reset: true,
      autostart: false,
      hotkey_enabled: true,
      hotkey: "Ctrl+Alt+T",
      theme: "system",
      lang: "auto",
      tray_style: "ring",
      tray_source: "all",
      tray_provider: "claude",
      tray_kinds: ["session", "daily", "weekly", "monthly"],
      custom_providers: [],
      providers: [
        { id: "claude", enabled: true, interval_secs: null, show_in_tray: true, own_tray_icon: false, notify: true, hidden_windows: [], alias: null },
        { id: "codex", enabled: true, interval_secs: null, show_in_tray: true, own_tray_icon: false, notify: true, hidden_windows: [], alias: null },
        { id: "cursor", enabled: true, interval_secs: null, show_in_tray: true, own_tray_icon: false, notify: true, hidden_windows: [], alias: null },
        { id: "opencode", enabled: true, interval_secs: null, show_in_tray: true, own_tray_icon: false, notify: true, hidden_windows: [], alias: null },
        { id: "commandcode", enabled: true, interval_secs: null, show_in_tray: true, own_tray_icon: false, notify: true, hidden_windows: [], alias: null },
        { id: "devin", enabled: true, interval_secs: null, show_in_tray: true, own_tray_icon: false, notify: true, hidden_windows: [], alias: null },
        { id: "antigravity", enabled: true, interval_secs: null, show_in_tray: true, own_tray_icon: false, notify: true, hidden_windows: [], alias: null },
      ],
    },
    providers: [
      {
        id: "claude", enabled: true, status: "ok", error: null, secrets: [], last_attempt: ago(30_000),
        poll: { interval_secs: 360, base_secs: 180, min_secs: 180, floor_secs: 216, state: "rate_limited", rate_limit_hits: 2, next_refresh_at: inMs(150_000) },
        snapshot: snap("claude", "max", [
          { kind: "Session5h", label: "5 h", used_pct: 33, resets_at: inMs(3 * H + 2 * 60_000) },
          { kind: "Weekly", label: "Week", used_pct: 55, resets_at: inMs(46 * H) },
        ], 30_000),
      },
      {
        id: "codex", enabled: true, status: "ok", error: null, secrets: [], last_attempt: ago(30_000),
        poll: { interval_secs: 90, base_secs: 180, min_secs: 180, floor_secs: null, state: "boosted", rate_limit_hits: 0, next_refresh_at: inMs(60_000) },
        snapshot: snap("codex", "plus", [
          { kind: "Session5h", label: "5 h", used_pct: 78, resets_at: inMs(H + 12 * 60_000) },
          { kind: "Weekly", label: "Week", used_pct: 13, resets_at: inMs(21 * 24 * H) },
        ], 30_000, 34 * 24 * H),
      },
      {
        id: "cursor", enabled: true, status: "ok", error: null, secrets: [], last_attempt: ago(120_000),
        poll: { interval_secs: 300, base_secs: 300, min_secs: 300, floor_secs: null, state: "steady", rate_limit_hits: 0, next_refresh_at: inMs(180_000) },
        snapshot: snap("cursor", "pro", [
          { kind: "Monthly", label: "Included usage", used_pct: 96, resets_at: inMs(24 * 24 * H) },
        ], 120_000),
      },
      {
        id: "opencode", enabled: true, status: "ok", error: null, secrets: ["workspace_id", "cookie"], last_attempt: ago(90_000),
        poll: { interval_secs: 300, base_secs: 300, min_secs: 300, floor_secs: null, state: "steady", rate_limit_hits: 0, next_refresh_at: inMs(210_000) },
        snapshot: snap("opencode", "go", [
          { kind: "Session5h", label: "5 h", used_pct: 22, resets_at: inMs(2 * H + 30 * 60_000) },
          { kind: "Weekly", label: "Week", used_pct: 41, resets_at: inMs(4 * 24 * H) },
          { kind: "Monthly", label: "Month", used_pct: 18, resets_at: null },
        ], 90_000),
      },
      {
        id: "commandcode", enabled: true, status: "ok", error: null, secrets: ["api_key"], last_attempt: ago(150_000),
        poll: { interval_secs: 300, base_secs: 300, min_secs: 300, floor_secs: null, state: "steady", rate_limit_hits: 0, next_refresh_at: inMs(150_000) },
        snapshot: snap("commandcode", "pro", [
          { kind: "Session5h", label: "5 h", used_pct: 9, resets_at: inMs(4 * H) },
          { kind: "Weekly", label: "Week", used_pct: 63, resets_at: inMs(3 * 24 * H) },
          { kind: "Monthly", label: "Month", used_pct: 27, resets_at: inMs(15 * 24 * H) },
        ], 150_000, 15 * 24 * H),
      },
      {
        id: "devin", enabled: true, status: "ok", error: null, secrets: [], last_attempt: ago(100_000),
        poll: { interval_secs: 300, base_secs: 300, min_secs: 300, floor_secs: null, state: "steady", rate_limit_hits: 0, next_refresh_at: inMs(200_000) },
        snapshot: snap("devin", "Pro", [
          { kind: "Daily", label: "Day", used_pct: 35, resets_at: inMs(7 * H) },
          { kind: "Weekly", label: "Week", used_pct: 12, resets_at: inMs(5 * 24 * H) },
        ], 100_000),
      },
      {
        id: "antigravity", enabled: true, status: "ok", error: null, secrets: [], last_attempt: ago(110_000),
        poll: { interval_secs: 300, base_secs: 300, min_secs: 300, floor_secs: null, state: "steady", rate_limit_hits: 0, next_refresh_at: inMs(190_000) },
        snapshot: snap("antigravity", "", [
          { kind: "Session5h", label: "5 h · gemini-3-pro", used_pct: 58, resets_at: inMs(2 * H) },
          { kind: "Weekly", label: "Week · gemini-3-pro", used_pct: 21, resets_at: inMs(4 * 24 * H) },
        ], 110_000),
      },
    ],
  };
}
