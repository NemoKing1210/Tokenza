import { useEffect, useState } from "react";

/**
 * The current time, re-read every `intervalMs` while `active` and the page is visible. The tray app
 * keeps its windows mounted while they are hidden, so a clock that kept ticking would re-render them
 * for as long as the app runs. Becoming active or visible again reads the time at once.
 */
export function useNow(intervalMs: number, active = true): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    let id: ReturnType<typeof setInterval> | undefined;
    const stop = () => {
      clearInterval(id);
      id = undefined;
    };
    const sync = () => {
      stop();
      if (document.visibilityState === "hidden") return;
      setNow(Date.now());
      id = setInterval(() => setNow(Date.now()), intervalMs);
    };
    sync();
    document.addEventListener("visibilitychange", sync);
    return () => {
      stop();
      document.removeEventListener("visibilitychange", sync);
    };
  }, [intervalMs, active]);
  return now;
}
