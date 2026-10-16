import { createContext, useContext, useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";

export const reducedMotion = () =>
  typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

type ViewTransitionDoc = Document & { startViewTransition?: (cb: () => void | Promise<void>) => unknown };

/**
 * Runs a state update inside a View Transition so everything that changes (layout, theme, language,
 * order) morphs instead of jumping. Falls back to a plain update without the API or with reduced motion.
 */
export function withTransition(update: () => void) {
  const doc = document as ViewTransitionDoc;
  if (reducedMotion() || typeof doc.startViewTransition !== "function") {
    update();
    return;
  }
  let applied = false;
  // flushSync makes React commit (and run layout effects) before the browser takes the "after" snapshot.
  doc.startViewTransition(() => {
    if (applied) return;
    applied = true;
    flushSync(update);
  });
  // The browser calls back only after painting a frame; if it never does (throttled or occluded window),
  // the change must still land.
  setTimeout(() => {
    if (applied) return;
    applied = true;
    update();
  }, 400);
}

export const easeOutCubic = (x: number) => 1 - Math.pow(1 - x, 3);

/**
 * `token` changes whenever the flyout is shown again; `entering` is true while its entrance plays.
 * Counters replay from zero only for an entrance, never for a remount caused by a view switch.
 */
export const EnterContext = createContext({ token: 0, entering: false });

/**
 * Tweens a number toward `target`. Starts from 0 on mount and whenever the entrance token changes,
 * then follows later value changes from where it currently is.
 */
export function useCountUp(target: number, duration = 650): number {
  const { token: enter, entering } = useContext(EnterContext);
  const [value, setValue] = useState(() => (reducedMotion() || !entering ? target : 0));
  const current = useRef(value);
  const lastEnter = useRef(enter);

  useEffect(() => {
    if (reducedMotion()) {
      current.current = target;
      setValue(target);
      return;
    }
    // A new entrance replays from zero; a plain value change continues from the shown number.
    const from = lastEnter.current !== enter ? 0 : current.current;
    lastEnter.current = enter;
    const start = performance.now();
    let raf = 0;
    const step = (now: number) => {
      const p = Math.min(1, (now - start) / duration);
      const v = from + (target - from) * easeOutCubic(p);
      current.current = v;
      setValue(v);
      if (p < 1) raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [target, enter, duration]);

  return value;
}
