import { create } from "zustand";
import type { AppStateDto, CustomDef, ProviderState, Settings } from "./types";
import { demoState } from "./demo";
import { resolveLang, type Lang } from "./i18n";
import { withTransition } from "./motion";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

/** Setting changes that reshape the UI and therefore look better as one morph. */
const VISUAL_KEYS: (keyof Settings)[] = ["theme", "lang", "view_mode", "providers"];

/** The spinner shows for at least this long so a fast refresh still reads as an action. */
const MIN_SPIN_MS = 700;
const MAX_SPIN_MS = 8000;

interface Store {
  data: AppStateDto | null;
  /** True while a manual refresh is in flight. */
  refreshing: boolean;
  /** Flyout lifecycle from the backend: "hidden" before it is shown, "shown" after. */
  flyout: { phase: "hidden" | "shown"; enter: number };
  init: () => Promise<void>;
  refresh: () => void;
  resetTuning: (id: string) => void;
  /** Resolves to an error message, or null when the toast was handed to Windows. */
  testNotification: () => Promise<string | null>;
  /** `instant` skips the morph, for changes the caller already animated itself (drag and drop). */
  saveSettings: (patch: Partial<Settings>, opts?: { instant?: boolean }) => void;
  /** Each credential action resolves to an error code from the backend, or null on success. */
  setSecret: (id: string, field: string, value: string) => Promise<string | null>;
  clearSecret: (id: string, field: string) => Promise<string | null>;
  saveCustom: (def: CustomDef, token: string | null) => Promise<string | null>;
  removeCustom: (id: string) => Promise<string | null>;
  /** Resolves to an error code from the backend ("invalid", "in_use"), or null on success. */
  setHotkey: (accel: string) => Promise<string | null>;
  /** Tries a provider definition against its URL without saving it. */
  testCustom: (def: CustomDef, token: string | null) => Promise<{ windows: { label: string; used_pct: number | null }[] | null; error: string | null }>;
  openSettings: () => void;
  hideFlyout: () => void;
  setFlyoutHeight: (h: number) => void;
  quit: () => void;
}

let started = false;
let spinStarted = 0;
let spinTimer: ReturnType<typeof setTimeout> | undefined;

export const useStore = create<Store>((set, get) => {
  const stopSpin = () => {
    clearTimeout(spinTimer);
    const wait = Math.max(0, MIN_SPIN_MS - (Date.now() - spinStarted));
    spinTimer = setTimeout(() => set({ refreshing: false }), wait);
  };

  return {
    data: null,
    refreshing: false,
    // Outside Tauri (browser preview) there is no window lifecycle, so the flyout is simply shown.
    flyout: { phase: inTauri ? "hidden" : "shown", enter: 1 },

    async init() {
      if (started) return;
      started = true;
      if (!inTauri) {
        // Plain browser (design review, vite dev): show demo data.
        set({ data: demoState() });
        // Stands in for the window being shown again: coming back to the tab replays the entrance.
        document.addEventListener("visibilitychange", () => {
          if (document.visibilityState === "visible") set((s) => ({ flyout: { phase: "shown", enter: s.flyout.enter + 1 } }));
        });
        return;
      }
      const { listen } = await import("@tauri-apps/api/event");
      await listen<AppStateDto>("usage-updated", (e) => {
        set({ data: e.payload });
        if (get().refreshing) stopSpin();
      });
      await listen<Settings>("settings-updated", (e) => {
        const cur = get().data;
        if (cur) set({ data: { ...cur, settings: e.payload } });
      });
      await listen("flyout-shown", () => set((s) => ({ flyout: { phase: "shown", enter: s.flyout.enter + 1 } })));
      await listen("flyout-hidden", () => set((s) => ({ flyout: { ...s.flyout, phase: "hidden" } })));
      // Safety nets: if the show event fired before this listener existed (right after launch), or got lost,
      // the flyout would stay invisible. Ask the window itself, and follow the page visibility too.
      const reveal = () => set((s) => (s.flyout.phase === "hidden" ? { flyout: { phase: "shown", enter: s.flyout.enter + 1 } } : s));
      document.addEventListener("visibilitychange", () => document.visibilityState === "visible" && reveal());
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        if (getCurrentWindow().label === "flyout" && (await getCurrentWindow().isVisible())) reveal();
      } catch {
        // No permission or API: the events and visibilitychange still cover it.
      }
      set({ data: await call<AppStateDto>("get_state") });
    },

    refresh() {
      spinStarted = Date.now();
      set({ refreshing: true });
      clearTimeout(spinTimer);
      spinTimer = setTimeout(() => set({ refreshing: false }), MAX_SPIN_MS);
      if (inTauri) void call("refresh");
      else stopSpin(); // browser preview: nothing to wait for
    },

    resetTuning(id) {
      if (inTauri) void call("reset_tuning", { id });
    },

    async testNotification() {
      if (!inTauri) return null;
      try {
        await call("send_test_notification");
        return null;
      } catch (e) {
        return String(e);
      }
    },

    saveSettings(patch, opts) {
      const cur = get().data;
      if (!cur) return;
      const next = { ...cur.settings, ...patch };
      const apply = () => set({ data: { ...cur, settings: next } }); // optimistic; the backend echoes the normalized value
      if (!opts?.instant && VISUAL_KEYS.some((k) => k in patch)) withTransition(apply);
      else apply();
      if (inTauri) void call("set_settings", { new: next });
    },

    async setSecret(id, field, value) {
      if (!inTauri) return demoSecret(set, get, id, field, value.trim() ? true : null);
      return attempt(() => call("set_provider_secret", { id, field, value }));
    },

    async clearSecret(id, field) {
      if (!inTauri) return demoSecret(set, get, id, field, false);
      return attempt(() => call("clear_provider_secret", { id, field }));
    },

    async saveCustom(def, token) {
      if (!inTauri) return demoSaveCustom(set, get, def, token);
      return attempt(() => call("save_custom_provider", { def, token }));
    },

    async removeCustom(id) {
      if (!inTauri) return demoRemoveCustom(set, get, id);
      return attempt(() => call("remove_custom_provider", { id }));
    },

    async testCustom(def, token) {
      if (!inTauri) {
        // Browser preview has no backend: pretend the windows read a few made-up values.
        return { windows: def.windows.map((w, i) => ({ label: w.label, used_pct: 42 + i * 8 })), error: null };
      }
      try {
        return { windows: await call("test_custom_provider", { def, token }), error: null };
      } catch (e) {
        return { windows: null, error: String(e) };
      }
    },

    async setHotkey(accel) {
      if (!inTauri) {
        const cur = get().data;
        if (cur) set({ data: { ...cur, settings: { ...cur.settings, hotkey: accel } } });
        return null;
      }
      return attempt(() => call("set_hotkey", { accel }));
    },

    openSettings() {
      if (inTauri) void call("open_settings");
    },
    hideFlyout() {
      if (inTauri) void call("hide_flyout");
    },
    setFlyoutHeight(h) {
      if (inTauri) void call("set_flyout_height", { height: h });
    },
    quit() {
      if (inTauri) void call("quit_app");
    },
  };
});

/** The language the UI should use right now: the user's pick, or the system's when set to auto. */
export const selectLang = (d: AppStateDto | null): Lang => (d ? resolveLang(d.settings.lang, d.resolved_lang) : "en");

const NO_CUSTOM: CustomDef[] = [];
/** User-defined providers, for names. A stable empty list keeps selectors from re-rendering. */
export const useCustomProviders = () => useStore((s) => s.data?.settings.custom_providers ?? NO_CUSTOM);

/** Runs a backend call and turns a rejection into its message. */
async function attempt(run: () => Promise<unknown>): Promise<string | null> {
  try {
    await run();
    return null;
  } catch (e) {
    return String(e);
  }
}

// Browser preview has no backend: these mimic it on the demo data so the screens can be tried out.
type StoreApi = { set: (p: Partial<Store>) => void; get: () => Store };

function demoSecret(set: StoreApi["set"], get: StoreApi["get"], id: string, field: string, present: boolean | null): string | null {
  const cur = get().data;
  if (!cur) return null;
  if (present === null) return "empty";
  const providers = cur.providers.map((p) =>
    p.id === id ? { ...p, secrets: present ? [...new Set([...p.secrets, field])] : p.secrets.filter((f) => f !== field) } : p,
  );
  set({ data: { ...cur, providers } });
  return null;
}

function demoSaveCustom(set: StoreApi["set"], get: StoreApi["get"], def: CustomDef, token: string | null): string | null {
  const cur = get().data;
  if (!cur) return null;
  if (!def.id && !token?.trim()) return "empty";
  if (!def.url.startsWith("https://")) return "the URL must start with https://";
  const id = def.id || `custom-${def.name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "provider"}`;
  const saved = { ...def, id };
  const exists = cur.settings.custom_providers.some((d) => d.id === id);
  const custom_providers = exists ? cur.settings.custom_providers.map((d) => (d.id === id ? saved : d)) : [...cur.settings.custom_providers, saved];
  const providers: ProviderState[] = exists
    ? cur.providers
    : [
        ...cur.providers,
        {
          id, enabled: true, status: "ok", error: null, last_attempt: new Date().toISOString(), poll: null, secrets: ["token"],
          snapshot: {
            provider: id, account: "", fetched_at: new Date().toISOString(),
            windows: def.windows.map((w, i) => ({ kind: w.kind, label: w.label, used_pct: 30 + i * 20, resets_at: new Date(Date.now() + 3 * 3600_000).toISOString() })),
          },
        },
      ];
  const prefs = exists
    ? cur.settings.providers
    : [...cur.settings.providers, { id, enabled: true, interval_secs: null, show_in_tray: true, notify: true, hidden_windows: [], alias: null }];
  set({ data: { ...cur, providers, settings: { ...cur.settings, custom_providers, providers: prefs } } });
  return null;
}

function demoRemoveCustom(set: StoreApi["set"], get: StoreApi["get"], id: string): string | null {
  const cur = get().data;
  if (!cur) return null;
  set({
    data: {
      ...cur,
      providers: cur.providers.filter((p) => p.id !== id),
      settings: {
        ...cur.settings,
        custom_providers: cur.settings.custom_providers.filter((d) => d.id !== id),
        providers: cur.settings.providers.filter((p) => p.id !== id),
      },
    },
  });
  return null;
}
