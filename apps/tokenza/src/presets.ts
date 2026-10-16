import type { CustomDef } from "./types";

/** A ready-made definition for a service with a documented usage endpoint. */
export interface Preset {
  id: string;
  name: string;
  /** Where the user creates the key; shown as text, never opened by the app. */
  site: string;
  def: Pick<CustomDef, "name" | "url" | "auth" | "windows">;
}

// Paths follow each provider's public API reference. The form's Test button checks them against the
// live response, so a provider changing its format shows up as a failed test and not as wrong numbers.
export const PRESETS: Preset[] = [
  {
    id: "openrouter",
    name: "OpenRouter",
    site: "openrouter.ai/keys",
    def: {
      name: "OpenRouter",
      url: "https://openrouter.ai/api/v1/key",
      auth: { scheme: "bearer", name: "" },
      windows: [
        // Spend limit of the key itself; a key without a limit has nothing to show.
        { label: "Key limit", kind: "Monthly", used_path: "data.limit_remaining", limit_path: "data.limit", remaining: true, resets_at_path: null },
      ],
    },
  },
  {
    id: "synthetic",
    name: "Synthetic",
    site: "synthetic.new",
    def: {
      name: "Synthetic",
      url: "https://api.synthetic.new/v2/quotas",
      auth: { scheme: "bearer", name: "" },
      windows: [
        { label: "5 h", kind: "Session5h", used_path: "rollingFiveHourLimit.remaining", limit_path: "rollingFiveHourLimit.max", remaining: true, resets_at_path: null },
        { label: "Week", kind: "Weekly", used_path: "weeklyTokenLimit.percentRemaining", limit_path: null, remaining: true, resets_at_path: null },
      ],
    },
  },
];
