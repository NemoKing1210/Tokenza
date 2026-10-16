import { useState, type FormEvent } from "react";
import { Select, Switch } from "@fluentui/react-components";
import { useStore } from "./store";
import { t, type Key, type Lang } from "./i18n";
import type { AuthScheme, CustomDef, CustomWindowDef } from "./types";
import { PRESETS } from "./presets";

/** Credential fields of the built-in providers that accept a value typed into the app. Custom ones have a single token. */
const BUILTIN_FIELDS: Record<string, string[]> = {
  opencode: ["workspace_id", "cookie"],
  commandcode: ["api_key"],
};

const FIELD_LABEL: Record<string, Key> = {
  workspace_id: "fieldWorkspaceId",
  cookie: "fieldCookie",
  api_key: "fieldApiKey",
  token: "fieldToken",
};

export const isCustomId = (id: string) => id.startsWith("custom-");
export const secretFieldsFor = (id: string): string[] => (isCustomId(id) ? ["token"] : (BUILTIN_FIELDS[id] ?? []));

/** Backend errors are short codes, or an English validation message for a definition that was rejected. */
function errorText(lang: Lang, raw: string): string {
  const code = raw.replace(/^Error:\s*/, "");
  if (code === "empty") return t(lang, "secretErrEmpty");
  if (code === "too_long") return t(lang, "secretErrLong");
  if (code.startsWith("store_failed") || code === "unknown_field" || code === "unknown provider") return t(lang, "secretErrStore");
  return t(lang, "customErr", { msg: code });
}

function SecretField({ id, field, isSet, lang }: { id: string; field: string; isSet: boolean; lang: Lang }) {
  const setSecret = useStore((s) => s.setSecret);
  const clearSecret = useStore((s) => s.clearSecret);
  const [draft, setDraft] = useState("");
  const [replacing, setReplacing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const label = t(lang, FIELD_LABEL[field] ?? "fieldToken");
  // A workspace id is not a secret; everything else is masked while typing.
  const masked = field !== "workspace_id";

  const run = async (job: () => Promise<string | null>, after?: () => void) => {
    setBusy(true);
    const err = await job();
    setBusy(false);
    setError(err ? errorText(lang, err) : null);
    if (!err) after?.();
  };
  const save = (e: FormEvent) => {
    e.preventDefault();
    void run(() => setSecret(id, field, draft), () => (setDraft(""), setReplacing(false)));
  };

  const editing = !isSet || replacing;
  return (
    <div className="dline dline-wrap">
      <div className="dline-text">
        <div className="dline-title">{label}</div>
        {error && <div className="dline-hint dline-error" role="alert">{error}</div>}
      </div>
      {editing ? (
        <form className="secret-ctl" onSubmit={save}>
          <div className="field-wrap">
            <input
              className="field"
              type={masked ? "password" : "text"}
              value={draft}
              autoComplete="off"
              spellCheck={false}
              aria-label={label}
              onChange={(e) => setDraft(e.target.value)}
            />
          </div>
          <button className="btn btn-sm btn-accent" type="submit" disabled={busy || !draft.trim()}>
            {t(lang, "secretSave")}
          </button>
          {replacing && (
            <button className="btn btn-sm" type="button" onClick={() => (setReplacing(false), setDraft(""), setError(null))}>
              {t(lang, "customCancel")}
            </button>
          )}
        </form>
      ) : (
        <div className="secret-ctl">
          <span className="pill">{t(lang, "secretSaved")}</span>
          <button className="btn btn-sm" onClick={() => setReplacing(true)}>
            {t(lang, "secretReplace")}
          </button>
          <button className="btn btn-sm" disabled={busy} onClick={() => void run(() => clearSecret(id, field))}>
            {t(lang, "secretRemove")}
          </button>
        </div>
      )}
    </div>
  );
}

/** Credentials typed into the app for one provider, kept in the OS credential store. */
export function ConnectSection({ id, secrets, lang }: { id: string; secrets: string[]; lang: Lang }) {
  const fields = secretFieldsFor(id);
  if (!fields.length) return null;
  const hint: Key = id === "opencode" ? "connectOpencode" : id === "commandcode" ? "connectCommandcode" : "connectCustom";
  return (
    <div className="dgroup">
      <div className="dline-title">{t(lang, "provConnect")}</div>
      <div className="dline-hint">{t(lang, hint)}</div>
      <div className="dline-hint">{t(lang, "provConnectStore")}</div>
      {fields.map((f) => (
        <SecretField key={f} id={id} field={f} isSet={secrets.includes(f)} lang={lang} />
      ))}
    </div>
  );
}

const blankWindow = (): CustomWindowDef => ({ label: "", kind: "Monthly", used_path: "", limit_path: null, remaining: false, resets_at_path: null });

const KIND_LABEL: Record<CustomWindowDef["kind"], Key> = { Session5h: "winSession", Daily: "winDaily", Weekly: "winWeekly", Monthly: "winMonthly" };

function WindowEditor({ w, lang, onChange, onRemove, canRemove }: { w: CustomWindowDef; lang: Lang; onChange: (w: CustomWindowDef) => void; onRemove: () => void; canRemove: boolean }) {
  const text = (key: Key, value: string, set: (v: string) => void, placeholder?: string) => (
    <label className="cfield">
      <span>{t(lang, key)}</span>
      <input className="field" value={value} placeholder={placeholder} spellCheck={false} onChange={(e) => set(e.target.value)} />
    </label>
  );
  return (
    <div className="cwin">
      <div className="cwin-grid">
        {text("customWinLabel", w.label, (label) => onChange({ ...w, label }), "Month")}
        <label className="cfield">
          <span>{t(lang, "customWinKind")}</span>
          <Select aria-label={t(lang, "customWinKind")} value={w.kind} onChange={(_, d) => onChange({ ...w, kind: d.value as CustomWindowDef["kind"] })}>
            {(Object.keys(KIND_LABEL) as CustomWindowDef["kind"][]).map((k) => (
              <option key={k} value={k}>
                {t(lang, KIND_LABEL[k])}
              </option>
            ))}
          </Select>
        </label>
        {text("customWinUsed", w.used_path, (used_path) => onChange({ ...w, used_path }), "data.usage.used")}
        {text("customWinLimit", w.limit_path ?? "", (limit_path) => onChange({ ...w, limit_path }), "data.usage.limit")}
        {text("customWinReset", w.resets_at_path ?? "", (resets_at_path) => onChange({ ...w, resets_at_path }), "data.usage.resets_at")}
      </div>
      <div className="cwin-foot">
        <label className="cswitch">
          <Switch checked={w.remaining} onChange={(_, d) => onChange({ ...w, remaining: d.checked })} aria-label={t(lang, "customWinRemaining")} />
          <span>{t(lang, "customWinRemaining")}</span>
        </label>
        <button type="button" className="btn btn-sm" disabled={!canRemove} onClick={onRemove}>
          {t(lang, "customRemoveWindow")}
        </button>
      </div>
    </div>
  );
}

/** Form for a provider the app has no built-in support for: one HTTPS request, JSON paths to the numbers. */
export function CustomForm({ initial, lang, onClose }: { initial: CustomDef | null; lang: Lang; onClose: () => void }) {
  const saveCustom = useStore((s) => s.saveCustom);
  const testCustom = useStore((s) => s.testCustom);
  const [name, setName] = useState(initial?.name ?? "");
  const [url, setUrl] = useState(initial?.url ?? "");
  const [scheme, setScheme] = useState<AuthScheme>(initial?.auth.scheme ?? "bearer");
  const [authName, setAuthName] = useState(initial?.auth.name ?? "");
  const [token, setToken] = useState("");
  const [windows, setWindows] = useState<CustomWindowDef[]>(initial?.windows ?? [blankWindow()]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [preset, setPreset] = useState<string | null>(null);
  const [showToken, setShowToken] = useState(false);
  const [testing, setTesting] = useState(false);
  const [test, setTest] = useState<{ ok: boolean; text: string } | null>(null);

  const current = (): CustomDef => ({
    id: initial?.id ?? "",
    name,
    url,
    auth: { scheme, name: scheme === "bearer" ? "" : authName },
    windows,
  });

  /** Fills every field from a preset (or empties them); the token stays, it is the user's. */
  const applyPreset = (id: string | null) => {
    const p = PRESETS.find((x) => x.id === id);
    setPreset(id);
    setName(p?.def.name ?? "");
    setUrl(p?.def.url ?? "");
    setScheme(p?.def.auth.scheme ?? "bearer");
    setAuthName(p?.def.auth.name ?? "");
    setWindows(p ? p.def.windows.map((w) => ({ ...w })) : [blankWindow()]);
    setError(null);
    setTest(null);
  };

  const runTest = async () => {
    setTesting(true);
    setTest(null);
    const res = await testCustom(current(), token.trim() ? token : null);
    setTesting(false);
    if (res.windows) {
      const lines = res.windows.map((w) => `${w.label} ${w.used_pct === null ? "—" : `${Math.round(w.used_pct)}%`}`).join(" · ");
      setTest({ ok: true, text: t(lang, "customTestOk", { x: lines }) });
    } else {
      const raw = (res.error ?? "").replace(/^Error:\s*/, "");
      setTest({ ok: false, text: raw === "empty" ? t(lang, "customTestNeedToken") : t(lang, "customTestFail", { msg: raw }) });
    }
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    const def = current();
    const err = await saveCustom(def, token.trim() ? token : null);
    setBusy(false);
    if (err) setError(errorText(lang, err));
    else onClose();
  };

  const schemes: { value: AuthScheme; key: Key }[] = [
    { value: "bearer", key: "customAuthBearer" },
    { value: "header", key: "customAuthHeader" },
    { value: "cookie", key: "customAuthCookie" },
  ];

  return (
    <form className="cform" onSubmit={submit}>
      <div className="cform-title">{t(lang, initial ? "customTitleEdit" : "customTitleNew")}</div>
      <p className="dline-hint">{t(lang, "customLead")}</p>
      {!initial && (
        <div className="cpresets" role="group" aria-label={t(lang, "customPresets")}>
          <span className="cpresets-label">{t(lang, "customPresets")}</span>
          {PRESETS.map((p) => (
            <button key={p.id} type="button" className={preset === p.id ? "btn btn-sm btn-accent" : "btn btn-sm"} aria-pressed={preset === p.id} onClick={() => applyPreset(p.id)}>
              {p.name}
            </button>
          ))}
          {preset && (
            <button type="button" className="btn btn-sm" onClick={() => applyPreset(null)}>
              {t(lang, "customPresetBlank")}
            </button>
          )}
        </div>
      )}
      {preset && (
        <p className="dline-hint">
          {t(lang, "customPresetKey", { site: PRESETS.find((p) => p.id === preset)!.site })} {t(lang, "customPresetNote")}
        </p>
      )}
      <div className="cform-grid">
        <label className="cfield">
          <span>{t(lang, "customName")}</span>
          <input className="field" value={name} maxLength={24} required onChange={(e) => setName(e.target.value)} />
        </label>
        <label className="cfield cfield-wide">
          <span>{t(lang, "customUrl")}</span>
          <input className="field" type="url" value={url} placeholder="https://api.example.com/v1/usage" required spellCheck={false} onChange={(e) => setUrl(e.target.value)} />
        </label>
        <label className="cfield">
          <span>{t(lang, "customAuth")}</span>
          <Select aria-label={t(lang, "customAuth")} value={scheme} onChange={(_, d) => setScheme(d.value as AuthScheme)}>
            {schemes.map((s) => (
              <option key={s.value} value={s.value}>
                {t(lang, s.key)}
              </option>
            ))}
          </Select>
        </label>
        {scheme !== "bearer" && (
          <label className="cfield">
            <span>{t(lang, "customAuthName")}</span>
            <input className="field" value={authName} required spellCheck={false} placeholder={scheme === "header" ? "X-Api-Key" : "session"} onChange={(e) => setAuthName(e.target.value)} />
          </label>
        )}
        <label className="cfield cfield-wide">
          <span>{t(lang, "customToken")}</span>
          <div className="token-row">
            <input
              className="field"
              type={showToken ? "text" : "password"}
              value={token}
              autoComplete="off"
              spellCheck={false}
              required={!initial}
              placeholder={initial ? t(lang, "customTokenKeep") : ""}
              onChange={(e) => setToken(e.target.value)}
            />
            <button type="button" className="btn btn-sm" aria-pressed={showToken} onClick={() => setShowToken(!showToken)}>
              {t(lang, showToken ? "customHide" : "customShow")}
            </button>
          </div>
        </label>
      </div>

      <div className="dline-title cform-sub">{t(lang, "customWindows")}</div>
      <div className="dline-hint">{t(lang, "customWindowsHint")}</div>
      {windows.map((w, i) => (
        <WindowEditor
          key={i}
          w={w}
          lang={lang}
          canRemove={windows.length > 1}
          onChange={(nw) => setWindows(windows.map((o, j) => (j === i ? nw : o)))}
          onRemove={() => setWindows(windows.filter((_, j) => j !== i))}
        />
      ))}
      {windows.length < 6 && (
        <button type="button" className="btn btn-sm" onClick={() => setWindows([...windows, blankWindow()])}>
          {t(lang, "customAddWindow")}
        </button>
      )}

      {error && (
        <div className="cform-error" role="alert">
          {error}
        </div>
      )}
      {test && (
        <div className={test.ok ? "cform-ok" : "cform-error"} role="status">
          {test.text}
        </div>
      )}
      <div className="cform-actions">
        <button type="button" className="btn btn-sm cform-test" disabled={testing || !url.trim()} onClick={() => void runTest()}>
          {t(lang, testing ? "customTesting" : "customTest")}
        </button>
        <button type="button" className="btn btn-sm" onClick={onClose}>
          {t(lang, "customCancel")}
        </button>
        <button type="submit" className="btn btn-sm btn-accent" disabled={busy}>
          {t(lang, "secretSave")}
        </button>
      </div>
    </form>
  );
}

/** Edit and remove for a user-defined provider; removal asks for a second click. */
export function CustomActions({ def, lang, onEdit }: { def: CustomDef; lang: Lang; onEdit: () => void }) {
  const removeCustom = useStore((s) => s.removeCustom);
  const [confirm, setConfirm] = useState(false);
  return (
    <div className="ptoolbar-actions">
      <button className="btn btn-sm" onClick={onEdit}>
        {t(lang, "customEdit")}
      </button>
      <button
        className="btn btn-sm"
        onClick={() => (confirm ? void removeCustom(def.id) : setConfirm(true))}
        onBlur={() => setConfirm(false)}
      >
        {t(lang, confirm ? "customRemoveConfirm" : "customRemove")}
      </button>
    </div>
  );
}
