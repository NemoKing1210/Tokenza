import type { Lang, LangSetting } from "./i18n";

export type WindowKind = "Session5h" | "Daily" | "Weekly" | "Monthly" | "Credits";

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
  /** When the paid period runs out or renews; absent for providers that do not report it. */
  subscription_ends_at?: string;
  fetched_at: string;
}

export type ProviderStatus = "pending" | "ok" | "error" | "not_found";

export interface ErrorInfo {
  kind: "NotLoggedIn" | "TokenExpired" | "NoSubscription" | "RateLimited" | "SchemaChanged" | "Offline";
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
  /** Credential fields that have a stored value; the values never reach the UI. */
  secrets: string[];
}

export interface ProviderPref {
  id: string;
  enabled: boolean;
  /** Own interval for this provider; null follows the global one. */
  interval_secs: number | null;
  /** Whether this provider's limits count toward the tray icon when the tray follows the chosen ones. */
  show_in_tray: boolean;
  /** Whether this provider also gets a tray icon of its own, next to the main one. */
  own_tray_icon: boolean;
  /** Whether this provider may send threshold toasts. */
  notify: boolean;
  /** Labels of limit windows hidden everywhere, e.g. "Week · Opus". */
  hidden_windows: string[];
  /** Own display name; null uses the built-in one. */
  alias: string | null;
}

export type AuthScheme = "bearer" | "header" | "cookie";

/** One limit window of a user-defined provider: where to find its numbers in the JSON response. */
export interface CustomWindowDef {
  label: string;
  kind: Exclude<WindowKind, "Credits">;
  used_path: string;
  limit_path: string | null;
  remaining: boolean;
  resets_at_path: string | null;
}

/** A provider the user defined. Its token is kept in the OS credential store, not here. */
export interface CustomDef {
  /** Empty for a provider that does not exist yet; the backend assigns it. */
  id: string;
  name: string;
  url: string;
  auth: { scheme: AuthScheme; name: string };
  windows: CustomWindowDef[];
}

export type TrayStyle = "ring" | "bars" | "pie" | "number";
export type TraySource = "all" | "selected" | "single";
export type TrayKind = "session" | "daily" | "weekly" | "monthly";

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
  hotkey_enabled: boolean;
  /** Accelerator such as "Ctrl+Alt+T". */
  hotkey: string;
  theme: "system" | "light" | "dark";
  lang: LangSetting;
  tray_style: TrayStyle;
  /** Whose limits the tray icon follows: every provider, the ones flagged `show_in_tray`, or just `tray_provider`. */
  tray_source: TraySource;
  tray_provider: string;
  /** Kinds of limit windows the tray icon looks at; never empty. */
  tray_kinds: TrayKind[];
  view_mode: "standard" | "compact";
  providers: ProviderPref[];
  custom_providers: CustomDef[];
}

export interface AppStateDto {
  providers: ProviderState[];
  settings: Settings;
  /** The language "auto" resolves to on this system, decided by the backend. */
  resolved_lang: Lang;
}
