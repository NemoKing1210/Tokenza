import { useState, useSyncExternalStore, type CSSProperties, type KeyboardEvent, type ReactNode } from "react";
import { FluentProvider, Select, Switch, webDarkTheme, webLightTheme } from "@fluentui/react-components";
import { selectLang, useStore } from "./store";
import { LANGS, t, type Key, type LangSetting } from "./i18n";
import type { Lang } from "./format";
import type { Settings } from "./types";
import { Row, Segmented } from "./controls";
import AboutPane from "./AboutPane";
import UpdatePane from "./UpdatePane";
import ProvidersPane from "./ProvidersPane";
import TrayPane from "./TrayPane";
import TitleBar from "./TitleBar";

type Section = "providers" | "tray" | "update" | "notify" | "appearance" | "general" | "about";

const NAV: [Section, Key][] = [
  ["providers", "navProviders"],
  ["tray", "navTray"],
  ["update", "navUpdate"],
  ["notify", "navNotify"],
  ["appearance", "navAppearance"],
  ["general", "navGeneral"],
  ["about", "navAbout"],
];

function TestNotification({ lang }: { lang: Lang }) {
  const testNotification = useStore((s) => s.testNotification);
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const run = async () => {
    setBusy(true);
    setResult(null);
    const err = await testNotification();
    setResult(err ? { ok: false, text: t(lang, "testNotifyFail", { x: err }) } : { ok: true, text: t(lang, "testNotifySent") });
    setBusy(false);
  };
  return (
    <>
      <Row title={t(lang, "testNotify")} hint={t(lang, "testNotifyHint")}>
        <button className="btn" onClick={run} disabled={busy}>
          {t(lang, "testNotifyBtn")}
        </button>
      </Row>
      {result && (
        <p className={result.ok ? "lead" : "lead text-critical"} role="status">
          {result.text}
        </p>
      )}
    </>
  );
}

const darkQuery = matchMedia("(prefers-color-scheme: dark)");
const onSchemeChange = (cb: () => void) => {
  darkQuery.addEventListener("change", cb);
  return () => darkQuery.removeEventListener("change", cb);
};

/** The system's dark mode, kept current while the window is open (the CSS follows it on its own). */
const useSystemDark = () => useSyncExternalStore(onSchemeChange, () => darkQuery.matches);

const DEFAULT_HOTKEY = "Ctrl+Alt+T";
const MODIFIER_KEYS = ["Control", "Alt", "Shift", "Meta", "AltGraph"];

/** The accelerator name the backend parses for a physical key, or null for keys it does not offer. */
function acceleratorKey(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1\d|2[0-4])$/.test(code)) return code;
  return ["Space", "Tab", "Enter", "Backspace", "Delete", "Insert", "Home", "End", "PageUp", "PageDown", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(code) ? code : null;
}

function HotkeyRow({ lang, s }: { lang: Lang; s: Settings }) {
  const setHotkey = useStore((st) => st.setHotkey);
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<Key | null>(null);

  const onKeyDown = async (e: KeyboardEvent<HTMLButtonElement>) => {
    if (!recording) return;
    e.preventDefault();
    if (e.key === "Escape") {
      setRecording(false);
      setError(null);
      return;
    }
    if (MODIFIER_KEYS.includes(e.key)) return; // wait for the key that completes the combination
    const key = acceleratorKey(e.code);
    if (!key || !(e.ctrlKey || e.altKey || e.metaKey)) {
      setError("hotkeyInvalid");
      return;
    }
    const accel = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super", key].filter(Boolean).join("+");
    const err = await setHotkey(accel);
    setError(err === "in_use" ? "hotkeyInUse" : err ? "hotkeyInvalid" : null);
    if (!err) setRecording(false);
  };
  const stop = () => {
    setRecording(false);
    setError(null);
  };
  const reset = async () => {
    const err = await setHotkey(DEFAULT_HOTKEY);
    setError(err === "in_use" ? "hotkeyInUse" : err ? "hotkeyInvalid" : null);
  };

  return (
    <>
      <Row title={t(lang, "hotkeyKeys")}>
        <div className="hotkey">
          {s.hotkey !== DEFAULT_HOTKEY && !recording && (
            <button className="btn btn-sm" onClick={reset}>
              {t(lang, "hotkeyReset")}
            </button>
          )}
          <button
            className={recording ? "btn btn-accent hotkey-btn" : "btn hotkey-btn"}
            disabled={!s.hotkey_enabled}
            aria-label={t(lang, "hotkeyKeys")}
            onClick={() => {
              setRecording(true);
              setError(null);
            }}
            onKeyDown={onKeyDown}
            onBlur={stop}
          >
            {recording
              ? t(lang, "hotkeyPrompt")
              : s.hotkey.split("+").map((k) => (
                  <kbd key={k} className="kbd">
                    {k === "Super" ? "Win" : k}
                  </kbd>
                ))}
          </button>
        </div>
      </Row>
      {error && (
        <p className="lead text-critical" role="status">
          {t(lang, error)}
        </p>
      )}
    </>
  );
}

export default function SettingsView() {
  const data = useStore((s) => s.data);
  const { saveSettings, quit } = useStore.getState();
  const [section, setSection] = useState<Section>("providers");
  const systemDark = useSystemDark();
  if (!data) return null;
  const { settings: s } = data;
  const lang = selectLang(data);
  const isDark = s.theme === "dark" || (s.theme === "system" && systemDark);

  const pct = (key: "warn_pct" | "crit_pct", options: number[]) => (
    <Select
      value={String(s[key])}
      onChange={(_, d) => saveSettings({ [key]: Number(d.value) } as Partial<Settings>)}
      aria-label={t(lang, key === "warn_pct" ? "warn" : "crit")}
    >
      {options.map((v) => (
        <option key={v} value={v}>
          {v}%
        </option>
      ))}
    </Select>
  );

  const panes: Record<Section, ReactNode> = {
    providers: <ProvidersPane />,
    tray: <TrayPane />,
    update: <UpdatePane />,
    notify: (
      <>
        <Row title={t(lang, "warn")} hint={t(lang, "warnHint")}>
          {pct("warn_pct", [50, 60, 70, 75, 80, 85, 90])}
        </Row>
        <Row title={t(lang, "crit")}>{pct("crit_pct", [90, 95, 98, 100])}</Row>
        <Row title={t(lang, "notifyReset")} hint={t(lang, "notifyResetHint")}>
          <Switch checked={s.notify_reset} onChange={(_, d) => saveSettings({ notify_reset: d.checked })} aria-label={t(lang, "notifyReset")} />
        </Row>
        <TestNotification lang={lang} />
      </>
    ),
    appearance: (
      <>
        <Row title={t(lang, "theme")} hint={t(lang, "themeHint")}>
          <Segmented
            label={t(lang, "theme")}
            value={s.theme}
            onChange={(v) => saveSettings({ theme: v })}
            options={[
              { value: "system", label: t(lang, "system") },
              { value: "light", label: t(lang, "light") },
              { value: "dark", label: t(lang, "dark") },
            ]}
          />
        </Row>
        <Row title={t(lang, "language")}>
          <Select
            value={s.lang}
            onChange={(_, d) => saveSettings({ lang: d.value as LangSetting })}
            aria-label={t(lang, "language")}
          >
            <option value="auto">{t(lang, "langAuto")}</option>
            {LANGS.map((l) => (
              <option key={l.code} value={l.code}>
                {l.name}
              </option>
            ))}
          </Select>
        </Row>
        <Row title={t(lang, "viewMode")} hint={t(lang, "viewModeHint")}>
          <Segmented
            label={t(lang, "viewMode")}
            value={s.view_mode}
            onChange={(v) => saveSettings({ view_mode: v })}
            options={[
              { value: "standard", label: t(lang, "viewStandard") },
              { value: "compact", label: t(lang, "viewCompact") },
            ]}
          />
        </Row>
      </>
    ),
    general: (
      <>
        <Row title={t(lang, "autostart")}>
          <Switch checked={s.autostart} onChange={(_, d) => saveSettings({ autostart: d.checked })} aria-label={t(lang, "autostart")} />
        </Row>
        <Row title={t(lang, "hotkey")} hint={t(lang, "hotkeyHint")}>
          <Switch checked={s.hotkey_enabled} onChange={(_, d) => saveSettings({ hotkey_enabled: d.checked })} aria-label={t(lang, "hotkey")} />
        </Row>
        <HotkeyRow lang={lang} s={s} />
      </>
    ),
    about: <AboutPane lang={lang} quit={quit} />,
  };

  return (
    <FluentProvider theme={isDark ? webDarkTheme : webLightTheme} style={{ background: "transparent" }}>
      <div className="window">
      <TitleBar lang={lang} title={t(lang, "settings")} />
      <div className="settings">
        <nav className="nav" aria-label="Sections">
          {NAV.map(([id, key]) => (
            <button key={id} className={id === section ? "nav-item nav-on" : "nav-item"} aria-current={id === section ? "page" : undefined} onClick={() => setSection(id)}>
              {t(lang, key)}
            </button>
          ))}
        </nav>
        <main className="pane">
          <div className="pane-body" key={section}>
            <h1>{t(lang, NAV.find(([id]) => id === section)![1])}</h1>
            {panes[section]}
          </div>
        </main>
      </div>
      </div>
    </FluentProvider>
  );
}
