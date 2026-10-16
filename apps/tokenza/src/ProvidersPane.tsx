import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import { Switch } from "@fluentui/react-components";
import { selectLang, useStore } from "./store";
import { t, type Lang } from "./i18n";
import { planLabel, providerName, toneFor, windowLabel } from "./format";
import { ConnectSection, CustomActions, CustomForm, secretFieldsFor } from "./Connect";
import { ProviderLogo } from "./Logo";
import { ChevronDown, GripIcon } from "./icons";
import { BASE_OPTIONS, SecondsSelect } from "./UpdatePane";
import type { CustomDef, ProviderPref, ProviderState, Settings, UsageWindow } from "./types";

/** Where each provider's numbers come from: shown for transparency, deliberately not translated. */
const SOURCES: Record<string, string> = {
  claude: "%USERPROFILE%\\.claude\\.credentials.json  →  api.anthropic.com",
  codex: "%USERPROFILE%\\.codex\\auth.json  →  chatgpt.com",
  cursor: "%APPDATA%\\Cursor\\…\\state.vscdb  →  cursor.com",
  opencode: "OPENCODE_GO_WORKSPACE_ID + OPENCODE_GO_AUTH_COOKIE  →  opencode.ai",
  commandcode: "%USERPROFILE%\\.commandcode\\auth.json  →  api.commandcode.ai",
  devin: "%APPDATA%\\Devin\\credentials.toml  →  server.codeium.com",
  antigravity: "%USERPROFILE%\\.gemini\\antigravity-cli\\antigravity-oauth-token  →  cloudcode-pa.googleapis.com",
};

const DEFAULT_ORDER = ["claude", "codex", "cursor", "opencode", "commandcode", "devin", "antigravity"];

const hostOf = (url: string) => {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
};

type Tone = "ok" | "warn" | "bad" | "idle";

/** One short line that says whether the provider works right now. */
function statusOf(st: ProviderState | undefined, enabled: boolean, lang: Lang): { tone: Tone; text: string } {
  if (!enabled) return { tone: "idle", text: t(lang, "off") };
  if (!st || st.status === "pending") return { tone: "idle", text: t(lang, "pending") };
  switch (st.status) {
    case "ok":
      return { tone: "ok", text: t(lang, "stOk") };
    case "not_found":
      return { tone: "idle", text: t(lang, "notFoundTitle") };
    case "error":
      switch (st.error?.kind) {
        case "NotLoggedIn":
          if (secretFieldsFor(st.id).length > 0) return { tone: "warn", text: t(lang, "connectTitle") };
          return { tone: "warn", text: t(lang, "loginTitle") };
        case "TokenExpired":
          return { tone: "warn", text: t(lang, "loginTitle") };
        case "RateLimited":
          return { tone: "warn", text: t(lang, "rateTitle") };
        case "NoSubscription":
          return { tone: "idle", text: t(lang, "noSubTitle") };
        case "SchemaChanged":
          // The cause (an HTTP status or a fixed phrase, never a credential) tells a changed endpoint from a wrong setup.
          return { tone: "bad", text: `${t(lang, "schemaTitle")}: ${(st.error.message ?? "").replace(/^response schema changed:\s*/, "")}` };
        default:
          return { tone: "idle", text: t(lang, "offlineTitle") };
      }
  }
}

function MiniPills({ windows, lang }: { windows: UsageWindow[]; lang: Lang }) {
  const shown = windows.filter((w) => w.used_pct !== null).slice(0, 3);
  if (!shown.length) return null;
  return (
    <div className="prow-pills" aria-hidden>
      {shown.map((w, i) => (
        <span className="mini" key={`${w.label}-${i}`}>
          <i className={`mini-dot tone-${toneFor(w.used_pct!)}`} />
          {windowLabel(w, lang, true)} <b>{Math.round(w.used_pct!)}%</b>
        </span>
      ))}
    </div>
  );
}

function NameField({ fallback, pref, lang, onChange }: { fallback: string; pref: ProviderPref; lang: Lang; onChange: (alias: string | null) => void }) {
  const [draft, setDraft] = useState(pref.alias ?? "");
  // Follows outside changes (reset, other window) without fighting the user's typing.
  useEffect(() => setDraft(pref.alias ?? ""), [pref.alias]);
  const commit = () => onChange(draft.trim() ? draft.trim() : null);
  return (
    <div className="field-wrap">
      <input
        className="field"
        value={draft}
        maxLength={24}
        placeholder={fallback}
        aria-label={t(lang, "provName")}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
          if (e.key === "Escape") setDraft(pref.alias ?? "");
        }}
      />
      {pref.alias && (
        <button className="field-clear" onClick={() => onChange(null)} aria-label={t(lang, "provReset")} title={t(lang, "provReset")}>
          ×
        </button>
      )}
    </div>
  );
}

interface RowProps {
  pref: ProviderPref;
  st: ProviderState | undefined;
  s: Settings;
  custom: CustomDef | undefined;
  onEdit: () => void;
  lang: Lang;
  open: boolean;
  onToggleOpen: () => void;
  update: (patch: Partial<ProviderPref>) => void;
  index: number;
  last: boolean;
  motion: { transform?: string; state: "idle" | "dragging" | "settling" | "shifted" };
  setRef: (el: HTMLDivElement | null) => void;
  onGripDown: (e: ReactPointerEvent<HTMLButtonElement>) => void;
  onGripMove: (e: ReactPointerEvent<HTMLButtonElement>) => void;
  onGripUp: (e: ReactPointerEvent<HTMLButtonElement>) => void;
  onGripKey: (e: KeyboardEvent<HTMLButtonElement>) => void;
}

function ProviderRow(p: RowProps) {
  const { pref, st, lang } = p;
  const defaultName = providerName(pref.id, p.s.custom_providers);
  const name = pref.alias?.trim() || defaultName;
  const status = statusOf(st, pref.enabled, lang);
  const account = st?.snapshot?.account === "claude.ai" ? "" : (st?.snapshot?.account ?? "");
  const plan = planLabel(account);
  const windows = st?.snapshot?.windows ?? [];

  const cls = ["prow", p.open && "is-open", !pref.enabled && "is-off", `is-${p.motion.state}`].filter(Boolean).join(" ");
  const style = { transform: p.motion.transform, viewTransitionName: `prov-${pref.id}` } as CSSProperties;

  return (
    <div className={cls} ref={p.setRef} style={style}>
      <div className="prow-main">
        <button
          className="grip"
          aria-label={`${name}: ${t(lang, "provDrag")}`}
          title={t(lang, "provDrag")}
          onPointerDown={p.onGripDown}
          onPointerMove={p.onGripMove}
          onPointerUp={p.onGripUp}
          onPointerCancel={p.onGripUp}
          onKeyDown={p.onGripKey}
        >
          <GripIcon />
        </button>
        <ProviderLogo id={pref.id} name={name} />
        <div className="prow-text">
          <div className="prow-title">
            <span className="prow-name">{name}</span>
            {plan && <span className="pill">{plan}</span>}
          </div>
          <div className="prow-sub">
            <i className={`dot dot-${status.tone}`} />
            <span>{status.text}</span>
          </div>
        </div>
        {pref.enabled && <MiniPills windows={windows.filter((w) => !pref.hidden_windows.includes(w.label))} lang={lang} />}
        <Switch checked={pref.enabled} onChange={(_, d) => p.update({ enabled: d.checked })} aria-label={name} />
        <button
          className="icon-btn chev"
          onClick={p.onToggleOpen}
          aria-expanded={p.open}
          aria-label={`${name}: ${t(lang, "provDetails")}`}
          title={t(lang, "provDetails")}
        >
          <ChevronDown size={16} />
        </button>
      </div>

      {/* Collapsed content stays out of the tab order and the accessibility tree. */}
      <div className="pdetail" inert={!p.open}>
        <div className="pdetail-inner">
          <div className="pdetail-body">
            <ConnectSection id={pref.id} secrets={st?.secrets ?? []} lang={lang} />

            <div className="dline">
              <div className="dline-text">
                <div className="dline-title">{t(lang, "provName")}</div>
                <div className="dline-hint">{t(lang, "provNameHint")}</div>
              </div>
              <NameField fallback={defaultName} pref={pref} lang={lang} onChange={(alias) => p.update({ alias })} />
            </div>

            <div className="dline">
              <div className="dline-text">
                <div className="dline-title">{t(lang, "provNotify")}</div>
                <div className="dline-hint">{t(lang, "provNotifyHint")}</div>
              </div>
              <Switch checked={pref.notify} onChange={(_, d) => p.update({ notify: d.checked })} aria-label={t(lang, "provNotify")} />
            </div>

            <div className="dline">
              <div className="dline-text">
                <div className="dline-title">{t(lang, "baseInterval")}</div>
                <div className="dline-hint">{t(lang, "baseIntervalHint")}</div>
              </div>
              <SecondsSelect
                value={pref.interval_secs}
                options={BASE_OPTIONS}
                lang={lang}
                onChange={(v) => p.update({ interval_secs: v })}
                label={`${name}: ${t(lang, "baseInterval")}`}
                extra={{ value: "", label: t(lang, "asGlobal") }}
              />
            </div>

            <div className="dgroup">
              <div className="dline-title">{t(lang, "provWindows")}</div>
              <div className="dline-hint">{t(lang, "provWindowsHint")}</div>
              {windows.length === 0 ? (
                <div className="dempty">{t(lang, "provWindowsEmpty")}</div>
              ) : (
                <div className="wlist">
                  {windows.map((w, i) => {
                    const visible = !pref.hidden_windows.includes(w.label);
                    const toggle = (show: boolean) =>
                      p.update({
                        hidden_windows: show ? pref.hidden_windows.filter((h) => h !== w.label) : [...pref.hidden_windows, w.label],
                      });
                    return (
                      <label className="wrow" key={`${w.label}-${i}`}>
                        <span className="wrow-name">{windowLabel(w, lang)}</span>
                        <span className="wrow-pct">{w.used_pct === null ? (w.detail ?? "—") : `${Math.round(w.used_pct)}%`}</span>
                        <Switch checked={visible} onChange={(_, d) => toggle(d.checked)} aria-label={windowLabel(w, lang)} />
                      </label>
                    );
                  })}
                </div>
              )}
            </div>

            <div className="dfoot">
              <div className="dsource">
                <span className="dsource-label">{t(lang, "provSource")}</span>
                <code>{p.custom ? `Credential Manager  →  ${hostOf(p.custom.url)}` : (SOURCES[pref.id] ?? "—")}</code>
              </div>
              {p.custom && <CustomActions def={p.custom} lang={lang} onEdit={p.onEdit} />}
              <button
                className="btn btn-sm"
                onClick={() => p.update({ interval_secs: null, notify: true, hidden_windows: [], alias: null })}
              >
                {t(lang, "provResetOne")}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

interface Drag {
  id: string;
  startIdx: number;
  target: number;
  dy: number;
  settling: boolean;
}

export default function ProvidersPane() {
  const data = useStore((s) => s.data);
  const saveSettings = useStore((s) => s.saveSettings);
  const [openId, setOpenId] = useState<string | null>(null);
  /** The add/edit form: a new provider, or the id of the one being edited. */
  const [form, setForm] = useState<{ edit: string | null } | null>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [instant, setInstant] = useState(false);
  const rows = useRef(new Map<string, HTMLDivElement>());
  const geo = useRef<{ rects: DOMRect[]; startY: number; gap: number } | null>(null);

  if (!data) return null;
  const s = data.settings;
  const lang = selectLang(data);
  const ids = s.providers.map((p) => p.id);
  const enabledCount = s.providers.filter((p) => p.enabled).length;

  const update = (id: string, patch: Partial<ProviderPref>) =>
    saveSettings({ providers: s.providers.map((p) => (p.id === id ? { ...p, ...patch } : p)) }, { instant: true });

  const reorder = (from: number, to: number, instantApply = false) => {
    const list = [...s.providers];
    const [item] = list.splice(from, 1);
    list.splice(to, 0, item!);
    saveSettings({ providers: list }, instantApply ? { instant: true } : undefined);
  };

  // ---- pointer drag-and-drop on the grip ----
  const down = (e: ReactPointerEvent<HTMLButtonElement>, id: string) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const rects = ids.map((i) => rows.current.get(i)!.getBoundingClientRect());
    geo.current = { rects, startY: e.clientY, gap: rects.length > 1 ? rects[1]!.top - rects[0]!.bottom : 8 };
    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {
      // Capture is only a nicety (moves outside the handle keep arriving); dragging works without it.
    }
    const startIdx = ids.indexOf(id);
    setDrag({ id, startIdx, target: startIdx, dy: 0, settling: false });
  };

  const move = (e: ReactPointerEvent<HTMLButtonElement>) => {
    const g = geo.current;
    if (!drag || !g || drag.settling) return;
    const r = g.rects;
    const me = r[drag.startIdx]!;
    const dy = Math.min(Math.max(e.clientY - g.startY, r[0]!.top - me.top), r[r.length - 1]!.bottom - me.bottom);
    const center = me.top + me.height / 2 + dy;
    // The dragged row's new index is the number of other rows whose middle is above its own middle.
    const target = r.reduce((n, rect, i) => (i !== drag.startIdx && rect.top + rect.height / 2 < center ? n + 1 : n), 0);
    setDrag({ ...drag, dy, target });
  };

  const up = (e: ReactPointerEvent<HTMLButtonElement>) => {
    const g = geo.current;
    if (!drag || !g || drag.settling) return;
    try {
      if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
    } catch {
      // Nothing was captured.
    }
    const r = g.rects;
    const me = r[drag.startIdx]!;
    const slot = r[drag.target]!;
    // Where the row has to land so that its final position equals its slot.
    const snap = drag.target > drag.startIdx ? slot.bottom - me.height - me.top : slot.top - me.top;
    setDrag({ ...drag, dy: drag.target === drag.startIdx ? 0 : snap, settling: true });
    const { startIdx, target } = drag;
    setTimeout(() => {
      // Commit and drop all transforms in the same frame, with transitions off, so nothing visibly jumps.
      setInstant(true);
      if (startIdx !== target) reorder(startIdx, target, true);
      setDrag(null);
      geo.current = null;
      // A timer, not animation frames: frames pause while the window is occluded.
      setTimeout(() => setInstant(false), 60);
    }, 190);
  };

  const key = (e: KeyboardEvent<HTMLButtonElement>, i: number) => {
    const to = e.key === "ArrowUp" ? i - 1 : e.key === "ArrowDown" ? i + 1 : -1;
    if (to < 0 || to >= ids.length) return;
    e.preventDefault();
    reorder(i, to);
    // The grip remounts at its new place; keep the keyboard focus with the moved row.
    requestAnimationFrame(() => rows.current.get(ids[i]!)?.querySelector<HTMLButtonElement>(".grip")?.focus());
  };

  const motionFor = (id: string, i: number): RowProps["motion"] => {
    if (!drag || !geo.current) return { state: "idle" };
    if (id === drag.id) return { transform: `translateY(${drag.dy}px)`, state: drag.settling ? "settling" : "dragging" };
    const hs = geo.current.rects[drag.startIdx]!.height + geo.current.gap;
    if (drag.target > drag.startIdx && i > drag.startIdx && i <= drag.target) return { transform: `translateY(${-hs}px)`, state: "shifted" };
    if (drag.target < drag.startIdx && i >= drag.target && i < drag.startIdx) return { transform: `translateY(${hs}px)`, state: "shifted" };
    return { transform: "translateY(0)", state: "shifted" };
  };

  const resetAll = () =>
    saveSettings(
      { providers: DEFAULT_ORDER.map((id) => ({ id, enabled: true, interval_secs: null, show_in_tray: true, own_tray_icon: false, notify: true, hidden_windows: [], alias: null })) },
    );

  return (
    <div className={instant ? "providers is-instant" : drag ? "providers is-dragging-list" : "providers"}>
      <p className="lead">{t(lang, "providersLead")}</p>
      <div className="ptoolbar">
        <span className="pcount">{t(lang, "provCount", { n: String(enabledCount), m: String(s.providers.length) })}</span>
        <div className="ptoolbar-actions">
          <button className="btn btn-sm btn-accent" disabled={form !== null} onClick={() => setForm({ edit: null })}>
            {t(lang, "provAdd")}
          </button>
          <button
            className="btn btn-sm"
            disabled={enabledCount === s.providers.length}
            onClick={() => saveSettings({ providers: s.providers.map((p) => ({ ...p, enabled: true })) }, { instant: true })}
          >
            {t(lang, "provEnableAll")}
          </button>
          <button className="btn btn-sm" onClick={resetAll}>
            {t(lang, "provResetAll")}
          </button>
        </div>
      </div>

      {form && (
        <CustomForm
          key={form.edit ?? "new"}
          initial={s.custom_providers.find((d) => d.id === form.edit) ?? null}
          lang={lang}
          onClose={() => setForm(null)}
        />
      )}

      <div className="plist">
        {s.providers.map((pref, i) => (
          <ProviderRow
            key={pref.id}
            pref={pref}
            st={data.providers.find((p) => p.id === pref.id)}
            s={s}
            custom={s.custom_providers.find((d) => d.id === pref.id)}
            onEdit={() => setForm({ edit: pref.id })}
            lang={lang}
            open={openId === pref.id}
            onToggleOpen={() => setOpenId(openId === pref.id ? null : pref.id)}
            update={(patch) => update(pref.id, patch)}
            index={i}
            last={i === s.providers.length - 1}
            motion={motionFor(pref.id, i)}
            setRef={(el) => (el ? rows.current.set(pref.id, el) : rows.current.delete(pref.id))}
            onGripDown={(e) => down(e, pref.id)}
            onGripMove={move}
            onGripUp={up}
            onGripKey={(e) => key(e, i)}
          />
        ))}
      </div>
    </div>
  );
}
