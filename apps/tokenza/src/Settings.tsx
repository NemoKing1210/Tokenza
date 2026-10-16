import { useState, type CSSProperties, type ReactNode } from "react";
import { FluentProvider, Select, Switch, webDarkTheme, webLightTheme } from "@fluentui/react-components";
import { selectLang, useStore } from "./store";
import { LANGS, t, type Key, type LangSetting } from "./i18n";
import type { Lang } from "./format";
import type { Settings } from "./types";
import { Row, Segmented } from "./controls";
import UpdatePane from "./UpdatePane";
import ProvidersPane from "./ProvidersPane";
import TitleBar from "./TitleBar";

type Section = "providers" | "update" | "notify" | "appearance" | "general" | "about";

const NAV: [Section, Key][] = [
  ["providers", "navProviders"],
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

export default function SettingsView() {
  const data = useStore((s) => s.data);
  const { saveSettings, quit } = useStore.getState();
  const [section, setSection] = useState<Section>("providers");
  if (!data) return null;
  const { settings: s } = data;
  const lang = selectLang(data);
  const isDark = s.theme === "dark" || (s.theme === "system" && matchMedia("(prefers-color-scheme: dark)").matches);

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
        <Row title={t(lang, "trayStyle")} hint={t(lang, "trayStyleHint")}>
          <Segmented
            label={t(lang, "trayStyle")}
            value={s.tray_style}
            onChange={(v) => saveSettings({ tray_style: v })}
            options={[
              { value: "ring", label: t(lang, "ring") },
              { value: "bars", label: t(lang, "bars") },
            ]}
          />
        </Row>
      </>
    ),
    general: (
      <Row title={t(lang, "autostart")}>
        <Switch checked={s.autostart} onChange={(_, d) => saveSettings({ autostart: d.checked })} aria-label={t(lang, "autostart")} />
      </Row>
    ),
    about: (
      <>
        <p className="lead">{t(lang, "aboutText")}</p>
        <button className="btn" onClick={quit}>
          {t(lang, "quit")}
        </button>
      </>
    ),
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
