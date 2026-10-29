import { useEffect, useState } from "react";
import { inTauri } from "./store";
import { t, type Lang } from "./i18n";

/** The app mark: a ring with an accent arc, matching the tray icon and the app icon. */
function Mark() {
  return (
    <svg className="tb-mark" width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden>
      <circle cx="8" cy="8" r="5.6" stroke="currentColor" strokeOpacity="0.28" strokeWidth="2.2" />
      <path d="M8 2.4a5.6 5.6 0 0 1 5.3 3.8" stroke="var(--accent)" strokeWidth="2.2" strokeLinecap="round" />
    </svg>
  );
}

const Glyph = {
  minimize: <path d="M2.5 6h7" />,
  maximize: <rect x="2.5" y="2.5" width="7" height="7" rx="1" />,
  restore: (
    <>
      <rect x="2.5" y="4" width="5.5" height="5.5" rx="1" />
      <path d="M4.5 4V3.5a1 1 0 0 1 1-1H9a1 1 0 0 1 1 1V7a1 1 0 0 1-1 1H8.5" />
    </>
  ),
  close: <path d="M2.8 2.8l6.4 6.4M9.2 2.8L2.8 9.2" />,
};

function Control({ kind, label, onClick }: { kind: keyof typeof Glyph; label: string; onClick: () => void }) {
  return (
    <button className={`tb-btn tb-${kind}`} onClick={onClick} aria-label={label} title={label} tabIndex={-1}>
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.1" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
        {Glyph[kind]}
      </svg>
    </button>
  );
}

async function win() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow();
}

/** Replaces the native Windows title bar: drag area, minimize / maximize / close. */
export default function TitleBar({ lang }: { lang: Lang }) {
  const [maximized, setMaximized] = useState(false);
  const [version, setVersion] = useState("");

  useEffect(() => {
    if (!inTauri) return;
    void import("@tauri-apps/api/core")
      .then(({ invoke }) => invoke<{ version: string }>("about_info"))
      .then((i) => setVersion(i.version))
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (!inTauri) return;
    let off: (() => void) | undefined;
    let dead = false;
    void (async () => {
      const w = await win();
      const sync = async () => setMaximized(await w.isMaximized());
      await sync();
      const un = await w.onResized(sync);
      if (dead) un();
      else off = un;
    })();
    return () => {
      dead = true;
      off?.();
    };
  }, []);

  const run = (fn: (w: Awaited<ReturnType<typeof win>>) => Promise<unknown>) => () => {
    if (inTauri) void win().then(fn);
  };

  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="tb-brand">
        <Mark />
        <span className="tb-name">Tokenza</span>
        {version && <span className="tb-version">v{version}</span>}
      </div>
      <div className="tb-controls">
        <Control kind="minimize" label={t(lang, "winMinimize")} onClick={run((w) => w.minimize())} />
        <Control
          kind={maximized ? "restore" : "maximize"}
          label={t(lang, maximized ? "winRestore" : "winMaximize")}
          onClick={run((w) => w.toggleMaximize())}
        />
        {/* Closing only hides the window: the app keeps running in the tray. */}
        <Control kind="close" label={t(lang, "winClose")} onClick={run((w) => w.close())} />
      </div>
    </header>
  );
}
