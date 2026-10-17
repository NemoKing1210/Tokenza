import { Select, Switch } from "@fluentui/react-components";
import { selectLang, useStore } from "./store";
import { t, type Key } from "./i18n";
import { formatDuration, providerName, type Lang } from "./format";
import type { ProviderPref, ProviderState, Settings, TuneState } from "./types";
import { Row, Segmented } from "./controls";
import { useNow } from "./clock";

const MIN = 60;
export const BASE_OPTIONS = [1, 2, 3, 5, 10, 15, 30, 60].map((m) => m * MIN);
const LOW_OPTIONS = [30, 60, 120, 180, 300, 600].map((s) => s);
const HIGH_OPTIONS = [5, 10, 15, 30, 60, 120, 360].map((m) => m * MIN);

const STATE_TEXT: Record<TuneState, Key> = {
  fixed: "tuneFixed",
  steady: "tuneSteady",
  boosted: "tuneBoosted",
  relaxed: "tuneRelaxed",
  rate_limited: "tuneRateLimited",
};

const fmt = (secs: number, lang: Lang) => formatDuration(secs * 1000, lang);

export function SecondsSelect({
  value,
  options,
  lang,
  onChange,
  label,
  extra,
}: {
  value: number | null;
  options: number[];
  lang: Lang;
  onChange: (v: number | null) => void;
  label: string;
  extra?: { value: string; label: string };
}) {
  // A stored value that is not in the preset list must still be selectable.
  const list = value !== null && !options.includes(value) ? [...options, value].sort((a, b) => a - b) : options;
  return (
    <Select
      aria-label={label}
      value={value === null ? "" : String(value)}
      onChange={(_, d) => onChange(d.value === "" ? null : Number(d.value))}
    >
      {extra && <option value={extra.value}>{extra.label}</option>}
      {list.map((v) => (
        <option key={v} value={v}>
          {fmt(v, lang)}
        </option>
      ))}
    </Select>
  );
}

function countdown(iso: string | null, now: number): string | null {
  if (!iso) return null;
  const s = Math.max(0, Math.round((new Date(iso).getTime() - now) / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

function ProviderTuning({ pref, st, s, now, lang }: { pref: ProviderPref; st: ProviderState | undefined; s: Settings; now: number; lang: Lang }) {
  const { saveSettings, resetTuning } = useStore.getState();
  const poll = st?.poll;
  const name = providerName(pref.id, s.custom_providers);
  const setOwn = (v: number | null) =>
    saveSettings({ providers: s.providers.map((p) => (p.id === pref.id ? { ...p, interval_secs: v } : p)) });

  const lines: string[] = [];
  if (!pref.enabled) {
    lines.push(t(lang, "pollingOff"));
  } else if (poll) {
    lines.push(t(lang, "nowEvery", { x: fmt(poll.interval_secs, lang) }) + " · " + t(lang, STATE_TEXT[poll.state]));
    const next = countdown(poll.next_refresh_at, now);
    const bits = [next && t(lang, "nextIn", { x: next }), t(lang, "providerMin", { x: fmt(poll.min_secs, lang) })];
    if (poll.floor_secs) bits.push(t(lang, "safeFloor", { x: fmt(poll.floor_secs, lang) }));
    if (poll.rate_limit_hits > 0) bits.push(t(lang, "hits429", { n: String(poll.rate_limit_hits) }));
    lines.push(bits.filter(Boolean).join(" · "));
  }
  const tuned = !!poll && (poll.floor_secs !== null || poll.rate_limit_hits > 0);

  return (
    <div className="srow srow-tuning">
      <div className="srow-text">
        <div className="srow-title">{name}</div>
        {lines.map((l, i) => (
          <div className="srow-hint" key={i}>
            {l}
          </div>
        ))}
        {tuned && pref.enabled && (
          <button className="link-btn link-btn-left" onClick={() => resetTuning(pref.id)}>
            {t(lang, "resetTuning")}
          </button>
        )}
      </div>
      <div className="srow-control">
        <SecondsSelect
          value={pref.interval_secs}
          options={BASE_OPTIONS}
          lang={lang}
          onChange={setOwn}
          label={`${name}: ${t(lang, "baseInterval")}`}
          extra={{ value: "", label: t(lang, "asGlobal") }}
        />
      </div>
    </div>
  );
}

export default function UpdatePane() {
  const data = useStore((s) => s.data);
  const saveSettings = useStore((s) => s.saveSettings);
  const now = useNow(1000);
  if (!data) return null;
  const s = data.settings;
  const lang = selectLang(data);
  const smart = s.update_mode === "smart";

  return (
    <>
      <Row title={t(lang, "updMode")} hint={t(lang, "updModeHint")}>
        <Segmented
          label={t(lang, "updMode")}
          value={s.update_mode}
          onChange={(v) => saveSettings({ update_mode: v })}
          options={[
            { value: "smart", label: t(lang, "modeSmart") },
            { value: "fixed", label: t(lang, "modeFixed") },
          ]}
        />
      </Row>
      <Row title={t(lang, "baseInterval")} hint={t(lang, "baseIntervalHint")}>
        <SecondsSelect
          value={s.interval_secs}
          options={BASE_OPTIONS}
          lang={lang}
          onChange={(v) => v !== null && saveSettings({ interval_secs: v })}
          label={t(lang, "baseInterval")}
        />
      </Row>

      {smart && (
        <>
          <h2 className="subhead">{t(lang, "smartTitle")}</h2>
          <Row title={t(lang, "minInterval")} hint={t(lang, "minIntervalHint")}>
            <SecondsSelect
              value={s.min_interval_secs}
              options={LOW_OPTIONS}
              lang={lang}
              onChange={(v) => v !== null && saveSettings({ min_interval_secs: v })}
              label={t(lang, "minInterval")}
            />
          </Row>
          <Row title={t(lang, "maxInterval")} hint={t(lang, "maxIntervalHint")}>
            <SecondsSelect
              value={s.max_interval_secs}
              options={HIGH_OPTIONS}
              lang={lang}
              onChange={(v) => v !== null && saveSettings({ max_interval_secs: v })}
              label={t(lang, "maxInterval")}
            />
          </Row>
          <Row title={t(lang, "boostNear")} hint={t(lang, "boostNearHint")}>
            <Switch checked={s.boost_near_limit} onChange={(_, d) => saveSettings({ boost_near_limit: d.checked })} aria-label={t(lang, "boostNear")} />
          </Row>
          <Row title={t(lang, "slowIdle")} hint={t(lang, "slowIdleHint")}>
            <Switch checked={s.slow_when_idle} onChange={(_, d) => saveSettings({ slow_when_idle: d.checked })} aria-label={t(lang, "slowIdle")} />
          </Row>
        </>
      )}

      <h2 className="subhead">{t(lang, "perProvider")}</h2>
      <p className="lead">{t(lang, "perProviderHint")}</p>
      {s.providers.map((pref) => (
        <ProviderTuning key={pref.id} pref={pref} st={data.providers.find((p) => p.id === pref.id)} s={s} now={now} lang={lang} />
      ))}
    </>
  );
}
