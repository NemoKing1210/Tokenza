import { Switch } from "@fluentui/react-components";
import { selectLang, useStore } from "./store";
import { t, type Key, type Lang } from "./i18n";
import { providerName } from "./format";
import { Row, Segmented } from "./controls";
import { ProviderLogo } from "./Logo";
import {
  SAMPLE_LEVELS,
  TRAY_KINDS,
  TRAY_STYLES,
  TRAY_TRACK,
  numberCells,
  trayLevels,
  trayTone,
  type TrayLevels,
} from "./tray";
import type { ProviderPref, Settings, TrayKind, TraySource, TrayStyle } from "./types";

const STYLE_LABEL: Record<TrayStyle, Key> = { ring: "ring", bars: "bars", pie: "pie", number: "number" };
const SOURCE_LABEL: Record<TraySource, Key> = { all: "srcAll", selected: "srcSelected", single: "srcSingle" };
const KIND_LABEL: Record<TrayKind, Key> = { session: "winSession", daily: "winDaily", weekly: "winWeekly", monthly: "trayKindMonthly" };

const RING_R = 12;
const RING_LEN = 2 * Math.PI * RING_R;

/** The icon as the backend paints it (32 px canvas), so the choice can be judged before it is made. */
function TrayGlyph({ style, levels }: { style: TrayStyle; levels: TrayLevels }) {
  const worst = levels.worst;
  const track = TRAY_TRACK;
  if (style === "bars") {
    const rows: [number | null, number][] = [
      [levels.session ?? worst, 10],
      [levels.weekly ?? worst, 22],
    ];
    return (
      <svg viewBox="0 0 32 32" aria-hidden>
        {rows.map(([pct, y]) => (
          <g key={y}>
            <line x1={5} x2={27} y1={y} y2={y} stroke={track} strokeWidth={7} strokeLinecap="round" />
            {pct !== null && pct > 0 && (
              <line x1={5} x2={Math.max(5.1, 5 + 22 * Math.min(1, pct / 100))} y1={y} y2={y} stroke={trayTone(pct)} strokeWidth={7} strokeLinecap="round" />
            )}
          </g>
        ))}
      </svg>
    );
  }
  if (style === "pie") {
    const frac = worst === null ? 0 : Math.min(1, worst / 100);
    const a = -Math.PI / 2 + 2 * Math.PI * frac;
    return (
      <svg viewBox="0 0 32 32" aria-hidden>
        <circle cx={16} cy={16} r={14} fill={track} />
        {worst !== null &&
          frac > 0 &&
          (frac >= 1 ? (
            <circle cx={16} cy={16} r={14} fill={trayTone(worst)} />
          ) : (
            <path d={`M16 16 L16 2 A14 14 0 ${frac > 0.5 ? 1 : 0} 1 ${16 + 14 * Math.cos(a)} ${16 + 14 * Math.sin(a)} Z`} fill={trayTone(worst)} />
          ))}
      </svg>
    );
  }
  if (style === "number") {
    return (
      <svg viewBox="0 0 32 32" aria-hidden shapeRendering="crispEdges">
        {worst === null ? (
          <rect x={8} y={14} width={16} height={4} fill={track} />
        ) : (
          numberCells(worst).map((c) => <rect key={`${c.x}-${c.y}`} x={c.x} y={c.y} width={c.size} height={c.size} fill={trayTone(worst)} />)
        )}
      </svg>
    );
  }
  return (
    <svg viewBox="0 0 32 32" aria-hidden>
      <circle cx={16} cy={16} r={RING_R} fill="none" stroke={track} strokeWidth={5} />
      {worst !== null && worst > 0 && (
        <circle
          cx={16}
          cy={16}
          r={RING_R}
          fill="none"
          stroke={trayTone(worst)}
          strokeWidth={5}
          strokeLinecap="round"
          strokeDasharray={`${RING_LEN * Math.min(1, worst / 100)} ${RING_LEN}`}
          transform="rotate(-90 16 16)"
        />
      )}
    </svg>
  );
}

function StylePicker({ s, lang, levels }: { s: Settings; lang: Lang; levels: TrayLevels }) {
  const saveSettings = useStore((st) => st.saveSettings);
  return (
    <div className="tstyles" role="radiogroup" aria-label={t(lang, "trayStyle")}>
      {TRAY_STYLES.map((style) => (
        <button
          key={style}
          role="radio"
          aria-checked={s.tray_style === style}
          className={s.tray_style === style ? "tstyle tstyle-on" : "tstyle"}
          onClick={() => saveSettings({ tray_style: style })}
        >
          <span className="tstyle-tile">
            <TrayGlyph style={style} levels={levels} />
          </span>
          <span className="tstyle-name">{t(lang, STYLE_LABEL[style])}</span>
        </button>
      ))}
    </div>
  );
}

function ProviderPicker({ s, lang, single }: { s: Settings; lang: Lang; single: boolean }) {
  const saveSettings = useStore((st) => st.saveSettings);
  const name = (p: ProviderPref) => p.alias?.trim() || providerName(p.id, s.custom_providers);
  const setFlag = (id: string, show: boolean) =>
    saveSettings({ providers: s.providers.map((p) => (p.id === id ? { ...p, show_in_tray: show } : p)) }, { instant: true });
  return (
    <div className="wlist tproviders" role={single ? "radiogroup" : "group"} aria-label={t(lang, single ? "traySingleHint" : "traySelectHint")}>
      {s.providers.map((p) => (
        <label className={p.enabled ? "wrow" : "wrow is-off"} key={p.id}>
          <ProviderLogo id={p.id} name={name(p)} small />
          <span className="wrow-name">{name(p)}</span>
          {!p.enabled && <span className="wrow-pct">{t(lang, "off")}</span>}
          {single ? (
            <input
              type="radio"
              className="tradio"
              name="tray-provider"
              checked={s.tray_provider === p.id}
              onChange={() => saveSettings({ tray_provider: p.id })}
              aria-label={name(p)}
            />
          ) : (
            <Switch checked={p.show_in_tray} onChange={(_, d) => setFlag(p.id, d.checked)} aria-label={name(p)} />
          )}
        </label>
      ))}
    </div>
  );
}

export default function TrayPane() {
  const data = useStore((st) => st.data);
  const saveSettings = useStore((st) => st.saveSettings);
  if (!data) return null;
  const s = data.settings;
  const lang = selectLang(data);
  const levels = trayLevels(s, data.providers);
  // Until real numbers exist the previews show made-up ones, so every style still reads as a gauge.
  const shown = levels.worst === null ? SAMPLE_LEVELS : levels;

  const toggleKind = (kind: TrayKind, on: boolean) => {
    const next = TRAY_KINDS.filter((k) => (k === kind ? on : s.tray_kinds.includes(k)));
    if (next.length > 0) saveSettings({ tray_kinds: next }, { instant: true });
  };

  return (
    <>
      <p className="lead">{t(lang, "trayLead")}</p>

      <h2 className="subhead">{t(lang, "trayStyle")}</h2>
      <p className="lead">{t(lang, "trayStyleHint")}</p>
      <StylePicker s={s} lang={lang} levels={shown} />

      <Row title={t(lang, "traySource")} hint={t(lang, "traySourceHint")}>
        <Segmented
          label={t(lang, "traySource")}
          value={s.tray_source}
          onChange={(v) => saveSettings({ tray_source: v })}
          options={(Object.keys(SOURCE_LABEL) as TraySource[]).map((v) => ({ value: v, label: t(lang, SOURCE_LABEL[v]) }))}
        />
      </Row>
      {s.tray_source !== "all" && (
        <>
          <p className="lead tray-sub">{t(lang, s.tray_source === "single" ? "traySingleHint" : "traySelectHint")}</p>
          <ProviderPicker s={s} lang={lang} single={s.tray_source === "single"} />
        </>
      )}

      <h2 className="subhead">{t(lang, "trayKinds")}</h2>
      <p className="lead">{t(lang, "trayKindsHint")}</p>
      <div className="wlist">
        {TRAY_KINDS.map((kind) => {
          const on = s.tray_kinds.includes(kind);
          const onlyOne = on && s.tray_kinds.length === 1;
          return (
            <label className="wrow" key={kind}>
              <span className="wrow-name">{t(lang, KIND_LABEL[kind])}</span>
              <Switch checked={on} disabled={onlyOne} onChange={(_, d) => toggleKind(kind, d.checked)} aria-label={t(lang, KIND_LABEL[kind])} />
            </label>
          );
        })}
      </div>
    </>
  );
}
