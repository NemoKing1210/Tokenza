// Brand marks from simple-icons (CC0 icon data), except Command Code (its site's pinned-tab icon) and Devin and
// Antigravity (their sites' favicons); the marks themselves belong to Anthropic, OpenAI, Anysphere, Anomaly (OpenCode),
// Command Code, Cognition (Devin) and Google (Antigravity).
// The data lives in logos.json because the backend paints the same marks into the per-provider tray icons.
// Each path is drawn on a 24x24 canvas (unless it names its own view box) and filled with currentColor.

import logos from "./logos.json";

interface LogoData {
  path: string;
  viewBox?: string;
  /** An SVG `matrix(a b c d e f)` applied to the path before it is fitted to the view box. */
  transform?: string;
  /** The brand color; marks without one follow the text color. */
  color?: string;
}

const ENTRIES = Object.entries(logos as Record<string, LogoData>);

export const LOGO_PATHS: Record<string, string> = Object.fromEntries(ENTRIES.map(([id, l]) => [id, l.path]));

/** Marks that are not drawn on a 24x24 canvas: their own view box and, if needed, a transform for the path. */
export const LOGO_FRAMES: Record<string, { viewBox: string; transform?: string }> = Object.fromEntries(
  ENTRIES.filter(([, l]) => l.viewBox).map(([id, l]) => [id, { viewBox: l.viewBox!, transform: l.transform }]),
);

/** Brand colors, so marks without one can follow the theme on light and dark cards. */
export const LOGO_COLORS: Record<string, string> = Object.fromEntries(
  ENTRIES.filter(([, l]) => l.color).map(([id, l]) => [id, l.color!]),
);
