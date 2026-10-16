import type { Lang, LangSetting } from "./i18n";

export type WindowKind = "Session5h" | "Weekly" | "Monthly" | "Credits";

export interface UsageWindow {
  kind: WindowKind;
  label: string;
  used_pct: number | null;
  resets_at: string | null;
  detail?: string;
}

export interface UsageSnapshot {
  provider: string;
  account: string;
  windows: UsageWindow[];
  fetched_at: string;
}

export type ProviderStatus = "pending" | "ok" | "error" | "not_found";

export interface ErrorInfo {
  kind: "NotLoggedIn" | "TokenExpired" | "RateLimited" | "SchemaChanged" | "Offline";
  message: string;
}

export type TuneState = "fixed" | "steady" | "boosted" | "relaxed" | "rate_limited";

export interface PollInfo {
  interval_secs: number;
  base_secs: number;
  min_secs: number;
  floor_secs: number | null;
  state: TuneState;
  rate_limit_hits: number;
  next_refresh_at: string | null;
}

export interface ProviderState {
  id: string;
  enabled: boolean;
  status: ProviderStatus;
  snapshot: UsageSnapshot | null;
  error: ErrorInfo | null;
  last_attempt: string | null;
  poll: PollInfo | null;
}

export interface ProviderPref {
  id: string;
  enabled: boolean;
  /** Own interval for this provider; null follows the global one. */
  interval_secs: number | null;
  /** Whether this provider's limits count toward the tray icon. */
  show_in_tray: boolean;
  /** Whether this provider may send threshold toasts. */
  notify: boolean;
  /** Labels of limit windows hidden everywhere, e.g. "Week · Opus". */
  hidden_windows: string[];
  /** Own display name; null uses the built-in one. */
  alias: string | null;
}

export interface Settings {
  update_mode: "smart" | "fixed";
  interval_secs: number;
  min_interval_secs: number;
  max_interval_secs: number;
  boost_near_limit: boolean;
  slow_when_idle: boolean;
  warn_pct: number;
  crit_pct: number;
  notify_reset: boolean;
  autostart: boolean;
  theme: "system" | "light" | "dark";
  lang: LangSetting;
  tray_style: "ring" | "bars";
  view_mode: "standard" | "compact";
  providers: ProviderPref[];
}

export interface AppStateDto {
  providers: ProviderState[];
  settings: Settings;
  /** The language "auto" resolves to on this system, decided by the backend. */
  resolved_lang: Lang;
}
