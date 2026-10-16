import { describe, expect, it } from "vitest";
import { PRESETS } from "./presets";

// The backend rejects a definition that breaks these limits, so a preset must satisfy them up front.
describe("provider presets", () => {
  it("have unique ids and names within the backend's length limits", () => {
    expect(new Set(PRESETS.map((p) => p.id)).size).toBe(PRESETS.length);
    for (const p of PRESETS) {
      expect(p.def.name.length, p.id).toBeLessThanOrEqual(24);
      expect(p.site, p.id).not.toMatch(/^https?:/);
    }
  });

  it("send the token only over HTTPS to a plain host", () => {
    for (const p of PRESETS) {
      const url = new URL(p.def.url);
      expect(url.protocol, p.id).toBe("https:");
      expect(url.username + url.password, p.id).toBe("");
    }
  });

  it("describe 1 to 6 windows with unique labels and a value path each", () => {
    for (const p of PRESETS) {
      const w = p.def.windows;
      expect(w.length, p.id).toBeGreaterThanOrEqual(1);
      expect(w.length, p.id).toBeLessThanOrEqual(6);
      expect(new Set(w.map((x) => x.label)).size, p.id).toBe(w.length);
      for (const x of w) {
        expect(x.label.length, `${p.id}/${x.label}`).toBeLessThanOrEqual(24);
        expect(x.used_path, `${p.id}/${x.label}`).toBeTruthy();
      }
    }
  });

  it("name a header or cookie only when the scheme needs one", () => {
    for (const p of PRESETS) {
      const { scheme, name } = p.def.auth;
      expect(scheme === "bearer" ? name === "" : name !== "", p.id).toBe(true);
    }
  });
});
