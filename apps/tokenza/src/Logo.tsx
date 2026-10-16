import { LOGO_FRAMES, LOGO_PATHS } from "./logoPaths";

/** Brand colors; marks without one follow the text color so they work on light and dark cards. */
const BRAND_COLOR: Record<string, string> = {
  claude: "#D97757",
};

export function ProviderLogo({ id, name, small = false }: { id: string; name: string; small?: boolean }) {
  const path = LOGO_PATHS[id];
  const frame = LOGO_FRAMES[id];
  const cls = small ? "logo logo-sm" : "logo";
  if (!path) {
    // Unknown provider: a neutral initial, so the card still has an anchor.
    return (
      <span className={`${cls} logo-initial`} aria-hidden>
        {name.charAt(0)}
      </span>
    );
  }
  return (
    <span className={cls} style={{ color: BRAND_COLOR[id] }} aria-hidden>
      <svg viewBox={frame?.viewBox ?? "0 0 24 24"} width="100%" height="100%" fill="currentColor" role="presentation">
        <path d={path} transform={frame?.transform} />
      </svg>
    </span>
  );
}
