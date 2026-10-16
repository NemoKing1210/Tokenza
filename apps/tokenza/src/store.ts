import { create } from "zustand";
import type { AppStateDto, Settings } from "./types";
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
