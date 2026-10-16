import { useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { selectLang, useStore } from "./store";
import { t, type Key } from "./i18n";
import { ProviderLogo } from "./Logo";
import { EnterContext, useCountUp } from "./motion";
import {
  PROVIDER_NAMES,
  formatAgo,
  formatDuration,
  formatReset,
  planLabel,
  toneFor,
  windowLabel,
  type Lang,
} from "./format";
import type { ProviderPref, ProviderState, UsageWindow } from "./types";
import { AlertIcon, DensityIcon, GearIcon, LockIcon, OfflineIcon, RefreshIcon, SearchIcon } from "./icons";

/** The user's alias when set, else the built-in provider name. */
export const displayName = (id: string, pref?: ProviderPref) => pref?.alias?.trim() || PROVIDER_NAMES[id] || id;

/** Limit windows of a provider minus the ones the user hid. */
const visibleWindows = (p: ProviderState, pref?: ProviderPref): UsageWindow[] =>
  (p.snapshot?.windows ?? []).filter((w) => !pref?.hidden_windows.includes(w.label));

/** Entrance order and a stable name so the card morphs (instead of jumping) when the view changes. */
const cardStyle = (index: number, id: string): CSSProperties =>
  ({ "--i": index, viewTransitionName: `card-${id}` }) as CSSProperties;

/** How long a smaller flyout waits before the window shrinks: the morph runs about this long. */
const SHRINK_DELAY_MS = 300;
/** Longest entrance (last card's delay plus its duration); after it, remounts do not replay it. */
const ENTER_MS = 1100;

function useNow(intervalMs = 10_000) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), intervalMs);
    return () => clearInterval(id);
  }, [intervalMs]);
  return now;
}

export function UsageRow({ w, lang, now }: { w: UsageWindow; lang: Lang; now: number }) {
  const label = windowLabel(w, lang);
  if (w.used_pct === null) {
    // Credit balance has no percentage, so no bar.
    return (
      <div className="usage-row">
        <div className="usage-head">
          <span className="usage-label">{label}</span>
          <span className="usage-pct">{w.detail ?? "—"}</span>
        </div>
      </div>
    );
  }
  return <PercentRow w={w} pctValue={w.used_pct} label={label} lang={lang} now={now} />;
}

function PercentRow({ w, pctValue, label, lang, now }: { w: UsageWindow; pctValue: number; label: string; lang: Lang; now: number }) {
  const pct = Math.max(0, Math.min(100, pctValue));
  const tone = toneFor(pct);
  const shown = useCountUp(pct);
  return (
    <div className="usage-row">
      <div className="usage-head">
        <span className="usage-label">{label}</span>
        <span className="usage-pct">{Math.round(shown)}%</span>
      </div>
      <div
        className="track"
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(pct)}
      >
        <div className={`fill tone-${tone}`} style={{ width: `${Math.max(pct, 1)}%` }} />
      </div>
      <div className="usage-reset">{formatReset(w.resets_at, now, lang, w.kind === "Monthly")}</div>
    </div>
  );
}

function Banner({ icon, tone, title, hint }: { icon: ReactNode; tone: string; title: string; hint: string }) {
  return (
    <div className="banner" role="status">
      <span className={`banner-icon text-${tone}`}>{icon}</span>
      <div>
        <div className="banner-title">{title}</div>
        <div className="banner-hint">{hint}</div>
      </div>
    </div>
  );
}

function errorBanner(p: ProviderState, lang: Lang) {
  const name = PROVIDER_NAMES[p.id] ?? p.id;
  const kind = p.error?.kind;
  const pick = (title: Key, hint: Key, icon: ReactNode, tone: string) => (
    <Banner icon={icon} tone={tone} title={t(lang, title)} hint={t(lang, hint, { name })} />
  );
  switch (kind) {
    case "NotLoggedIn":
    case "TokenExpired":
      return pick("loginTitle", "loginHint", <LockIcon />, "warning");
    case "SchemaChanged":
      return pick("schemaTitle", "schemaHint", <AlertIcon />, "critical");
    case "RateLimited":
      return pick("rateTitle", "rateHint", <AlertIcon />, "warning");
    default:
      return pick("offlineTitle", "offlineHint", <OfflineIcon />, "secondary");
  }
}

export function ProviderCard({ p, lang, now, index = 0, pref }: { p: ProviderState; lang: Lang; now: number; index?: number; pref?: ProviderPref }) {
  const refresh = useStore((s) => s.refresh);
  const name = displayName(p.id, pref);
  const windows = visibleWindows(p, pref);
  // Claude's usage endpoint reports no plan; the placeholder label is not worth a pill.
  const account = p.snapshot?.account === "claude.ai" ? "" : (p.snapshot?.account ?? "");
  const plan = p.status === "not_found" ? t(lang, "notFound") : planLabel(account);

  return (
    <section className={`card stagger ${p.status === "not_found" ? "card-muted" : ""}`} aria-label={name} style={cardStyle(index, p.id)}>
      <header className="card-head">
        <ProviderLogo id={p.id} name={name} />
        <span className="card-name">{name}</span>
        {plan && <span className="pill">{plan}</span>}
      </header>

      {p.status === "not_found" && (
        <Banner icon={<SearchIcon />} tone="secondary" title={t(lang, "notFoundTitle")} hint={t(lang, "notFoundHint", { name })} />
      )}

      {p.status === "pending" && <div className="card-note">{t(lang, "pending")}</div>}

      {p.snapshot && p.status !== "not_found" && (
        <div className={`rows ${p.status === "error" ? "dimmed" : ""}`}>
          {windows.map((w, i) => (
            <UsageRow key={`${w.kind}-${w.label}-${i}`} w={w} lang={lang} now={now} />
          ))}
        </div>
      )}

      {p.status === "error" && (
        <>
          {errorBanner(p, lang)}
          <button className="btn" onClick={refresh}>
            {t(lang, "retry")}
          </button>
        </>
      )}

      {p.status === "ok" && (
        <div className="card-foot">
          {formatAgo(p.snapshot?.fetched_at ?? p.last_attempt, now, lang)}
          {p.poll && p.poll.state !== "steady" && p.poll.state !== "fixed" && ` · ${t(lang, "everyShort", { x: formatDuration(p.poll.interval_secs * 1000, lang) })}`}
        </div>
      )}
    </section>
  );
}

function CompactRow({ w, lang, now }: { w: UsageWindow; lang: Lang; now: number }) {
  // The label column is narrow, so the long monthly name gets a short form (full text stays in the tooltip).
  const label = windowLabel(w, lang, true);
  if (w.used_pct === null) {
    return (
      <div className="crow">
        <span className="crow-label">{label}</span>
        <span className="crow-detail">{w.detail ?? "—"}</span>
      </div>
    );
  }
  const pct = Math.max(0, Math.min(100, w.used_pct));
  const shown = useCountUp(pct);
  const left = w.resets_at ? new Date(w.resets_at).getTime() - now : null;
  return (
    <div className="crow" title={formatReset(w.resets_at, now, lang, w.kind === "Monthly")}>
      <span className="crow-label">{label}</span>
      <div
        className="track track-thin"
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(pct)}
      >
        <div className={`fill tone-${toneFor(pct)}`} style={{ width: `${Math.max(pct, 1)}%` }} />
      </div>
      <span className="crow-pct">{Math.round(shown)}%</span>
      <span className="crow-reset">{left !== null && left > 0 ? formatDuration(left, lang) : ""}</span>
    </div>
  );
}

export function CompactCard({ p, lang, now, index = 0, pref }: { p: ProviderState; lang: Lang; now: number; index?: number; pref?: ProviderPref }) {
  const refresh = useStore((s) => s.refresh);
  const name = displayName(p.id, pref);
  const windows = visibleWindows(p, pref);
  const account = p.snapshot?.account === "claude.ai" ? "" : (p.snapshot?.account ?? "");
  const plan = p.status === "not_found" ? "" : planLabel(account);
  const updated = p.status === "ok" ? formatAgo(p.snapshot?.fetched_at ?? p.last_attempt, now, lang) : "";

  let status: ReactNode = null;
  if (p.status === "not_found") {
    status = (
      <div className="cstatus text-secondary">
        <SearchIcon size={16} />
        <span>{t(lang, "notFoundTitle")}</span>
      </div>
    );
  } else if (p.status === "pending") {
    status = <div className="cstatus text-secondary">{t(lang, "pending")}</div>;
  } else if (p.status === "error") {
    const kind = p.error?.kind;
    const [icon, tone, title] =
      kind === "NotLoggedIn" || kind === "TokenExpired"
        ? [<LockIcon size={16} />, "warning", t(lang, "loginTitle")]
        : kind === "SchemaChanged"
          ? [<AlertIcon size={16} />, "critical", t(lang, "schemaTitle")]
          : kind === "RateLimited"
            ? [<AlertIcon size={16} />, "warning", t(lang, "rateTitle")]
            : [<OfflineIcon size={16} />, "secondary", t(lang, "offlineTitle")];
    status = (
      <div className="cstatus" role="status" title={t(lang, kind === "SchemaChanged" ? "schemaHint" : "offlineHint", { name })}>
        <span className={`text-${tone}`}>{icon}</span>
        <span className="cstatus-title">{title}</span>
        <button className="link-btn" onClick={refresh}>
          {t(lang, "retry")}
        </button>
      </div>
    );
  }

  return (
    <section className={`ccard stagger ${p.status === "not_found" ? "card-muted" : ""}`} aria-label={name} title={updated} style={cardStyle(index, p.id)}>
      <header className="ccard-head">
        <ProviderLogo id={p.id} name={name} small />
        <span className="ccard-name">{name}</span>
        {plan && <span className="pill pill-sm">{plan}</span>}
      </header>
      {p.snapshot && p.status !== "not_found" && (
        <div className={`crows ${p.status === "error" ? "dimmed" : ""}`}>
          {windows.map((w, i) => (
            <CompactRow key={`${w.kind}-${w.label}-${i}`} w={w} lang={lang} now={now} />
          ))}
        </div>
      )}
      {status}
    </section>
  );
}

export default function Flyout() {
  const data = useStore((s) => s.data);
  const refreshing = useStore((s) => s.refreshing);
  const flyout = useStore((s) => s.flyout);
  const { refresh, openSettings, hideFlyout, setFlyoutHeight, saveSettings } = useStore.getState();
  const rootRef = useRef<HTMLDivElement>(null);
  const now = useNow();

  // The entrance plays once per show. Derived (not an effect) so the very first frame already has it.
  const [settledEnter, setSettledEnter] = useState(0);
  const entering = flyout.phase === "shown" && settledEnter !== flyout.enter;
  const enterCtx = useMemo(() => ({ token: flyout.enter, entering }), [flyout.enter, entering]);
  useEffect(() => {
    if (flyout.phase !== "shown") return;
    const id = setTimeout(() => setSettledEnter(flyout.enter), ENTER_MS);
    return () => clearTimeout(id);
  }, [flyout.phase, flyout.enter]);

  useLayoutEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    let last = 0;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const report = () => {
      const h = Math.ceil(el.getBoundingClientRect().height);
      clearTimeout(timer);
      // Growing is applied at once; shrinking waits for the morph so the window does not clip it.
      if (h < last) timer = setTimeout(() => setFlyoutHeight(h), SHRINK_DELAY_MS);
      else setFlyoutHeight(h);
      last = h;
    };
    report();
    const ro = new ResizeObserver(report);
    ro.observe(el);
    return () => {
      clearTimeout(timer);
      ro.disconnect();
    };
  }, [setFlyoutHeight, data === null]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && hideFlyout();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [hideFlyout]);

  const lang = selectLang(data);
  const visible = data?.providers.filter((p) => p.enabled) ?? [];
  const compact = data?.settings.view_mode === "compact";
  const prefOf = (id: string) => data?.settings.providers.find((x) => x.id === id);
  const toggleLabel = t(lang, compact ? "switchToStandard" : "switchToCompact");

  return (
    <EnterContext.Provider value={enterCtx}>
    <div
      className={`flyout${compact ? " flyout-compact" : ""}${entering ? " is-entering" : ""}`}
      ref={rootRef}
      data-phase={flyout.phase}
      data-enter={flyout.enter % 2 ? "a" : "b"}
      style={{ viewTransitionName: "flyout" } as CSSProperties}
    >
      <header className="flyout-head stagger" style={{ "--i": 0 } as CSSProperties}>
        <h1>Tokenza</h1>
        <div className="head-actions">
          <button
            className="icon-btn"
            onClick={() => saveSettings({ view_mode: compact ? "standard" : "compact" })}
            aria-label={toggleLabel}
            title={toggleLabel}
          >
            <DensityIcon compact={compact} />
          </button>
          <button
            className={refreshing ? "icon-btn is-spinning" : "icon-btn"}
            onClick={refresh}
            aria-label={t(lang, "refresh")}
            title={t(lang, "refresh")}
            aria-busy={refreshing}
          >
            <RefreshIcon />
          </button>
        </div>
      </header>

      {data && visible.length === 0 && <div className="card-note">{t(lang, "empty")}</div>}
      {visible.map((p, i) =>
        compact ? (
          <CompactCard key={p.id} p={p} lang={lang} now={now} index={i + 1} pref={prefOf(p.id)} />
        ) : (
          <ProviderCard key={p.id} p={p} lang={lang} now={now} index={i + 1} pref={prefOf(p.id)} />
        ),
      )}

      <footer className="flyout-foot stagger" style={{ "--i": visible.length + 1 } as CSSProperties}>
        <span>{compact ? "" : t(lang, "footer")}</span>
        <button className="icon-btn" onClick={openSettings} aria-label={t(lang, "settings")} title={t(lang, "settings")}>
          <GearIcon />
        </button>
      </footer>
    </div>
    </EnterContext.Provider>
  );
}
