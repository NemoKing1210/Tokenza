import { useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { selectLang, useCustomProviders, useStore } from "./store";
import { t, type Key } from "./i18n";
import { ProviderLogo } from "./Logo";
import { secretFieldsFor } from "./Connect";
import { EnterContext, useCountUp } from "./motion";
import { useNow } from "./clock";
import {
  providerName,
  formatDuration,
  formatReset,
  formatSubscriptionEnd,
  planLabel,
  toneFor,
  windowLabel,
  type Lang,
} from "./format";
import type { CustomDef, ProviderPref, ProviderState, UsageWindow } from "./types";
import { AlertIcon, GearIcon, LockIcon, OfflineIcon, RefreshIcon, SearchIcon } from "./icons";

/** The user's alias when set, else the built-in provider name. */
export const displayName = (id: string, pref?: ProviderPref, custom?: CustomDef[]) => pref?.alias?.trim() || providerName(id, custom);

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
        <div className={`fill tone-${toneFor(pct)}`} style={{ width: `${pct > 0 ? Math.max(pct, 1) : 0}%` }} />
      </div>
      <span className="crow-pct">{Math.round(shown)}%</span>
      <span className="crow-reset">{left !== null && left > 0 ? formatDuration(left, lang) : ""}</span>
    </div>
  );
}

export function CompactCard({ p, lang, now, index = 0, pref, focused = false }: { p: ProviderState; lang: Lang; now: number; index?: number; pref?: ProviderPref; focused?: boolean }) {
  const refresh = useStore((s) => s.refresh);
  const name = displayName(p.id, pref, useCustomProviders());
  const windows = visibleWindows(p, pref);
  const account = p.snapshot?.account === "claude.ai" ? "" : (p.snapshot?.account ?? "");
  const plan = p.status === "not_found" ? "" : planLabel(account);

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
    const connect = kind === "NotLoggedIn" && secretFieldsFor(p.id).length > 0;
    const [icon, tone, title] =
      kind === "NotLoggedIn" || kind === "TokenExpired"
        ? [<LockIcon size={16} />, "warning", t(lang, connect ? "connectTitle" : "loginTitle")]
        : kind === "NoSubscription"
          ? [<LockIcon size={16} />, "secondary", t(lang, "noSubTitle")]
          : kind === "SchemaChanged"
          ? [<AlertIcon size={16} />, "critical", t(lang, "schemaTitle")]
          : kind === "RateLimited"
            ? [<AlertIcon size={16} />, "warning", t(lang, "rateTitle")]
            : [<OfflineIcon size={16} />, "secondary", t(lang, "offlineTitle")];
    status = (
      <div className="cstatus" role="status" title={t(lang, kind === "NoSubscription" ? "noSubHint" : kind === "SchemaChanged" ? "schemaHint" : connect ? "connectHint" : "offlineHint", { name }) + (kind === "SchemaChanged" && p.error ? ` (${p.error.message})` : "")}>
        <span className={`text-${tone}`}>{icon}</span>
        <span className="cstatus-title">{title}</span>
        <button className="link-btn" onClick={refresh}>
          {t(lang, "retry")}
        </button>
      </div>
    );
  }

  return (
    <section
      className={`ccard stagger${p.status === "not_found" ? " card-muted" : ""}${focused ? " is-focus" : ""}`}
      aria-label={name}
      data-provider={p.id}
      style={cardStyle(index, p.id)}
    >
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
      {p.snapshot && p.status === "error" && (
        <div className="ccard-sub">{t(lang, "staleSince", { x: formatDuration(now - new Date(p.snapshot.fetched_at).getTime(), lang) })}</div>
      )}
      {p.snapshot?.subscription_ends_at && p.status !== "not_found" && (
        <div className="ccard-sub">{formatSubscriptionEnd(p.snapshot.subscription_ends_at, now, lang)}</div>
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
  // Countdowns only matter while the flyout is on screen.
  const now = useNow(10_000, flyout.phase === "shown");

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
      if (h === last) return;
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

  // A tall flyout scrolls: each show starts at the top, or at the highlighted card.
  useEffect(() => {
    if (flyout.phase !== "shown" || !data) return;
    const card = flyout.focus && document.querySelector(`[data-provider="${CSS.escape(flyout.focus)}"]`);
    if (card) card.scrollIntoView({ block: "nearest" });
    else window.scrollTo(0, 0);
  }, [flyout.phase, flyout.enter, flyout.focus, data === null]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && hideFlyout();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [hideFlyout]);

  const lang = selectLang(data);
  const visible = data?.providers.filter((p) => p.enabled) ?? [];
  const prefOf = (id: string) => data?.settings.providers.find((x) => x.id === id);

  return (
    <EnterContext.Provider value={enterCtx}>
    <div
      className={`flyout${entering ? " is-entering" : ""}`}
      ref={rootRef}
      data-phase={flyout.phase}
      data-enter={flyout.enter % 2 ? "a" : "b"}
      style={{ viewTransitionName: "flyout" } as CSSProperties}
    >
      {data && visible.length === 0 && <div className="card-note">{t(lang, "empty")}</div>}
      {visible.map((p, i) => (
        <CompactCard key={p.id} p={p} lang={lang} now={now} index={i + 1} pref={prefOf(p.id)} focused={flyout.focus === p.id} />
      ))}

      <footer className="flyout-foot stagger" style={{ "--i": visible.length } as CSSProperties}>
        <button
          className={refreshing ? "icon-btn is-spinning" : "icon-btn"}
          onClick={refresh}
          aria-label={t(lang, "refresh")}
          title={t(lang, "refresh")}
          aria-busy={refreshing}
        >
          <RefreshIcon />
        </button>
        <span className="spacer" />
        <button className="icon-btn" onClick={openSettings} aria-label={t(lang, "settings")} title={t(lang, "settings")}>
          <GearIcon />
        </button>
      </footer>
    </div>
    </EnterContext.Provider>
  );
}
