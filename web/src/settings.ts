// Client settings (persisted to localStorage).

import { BindMap, DEFAULT_BINDS } from "./binds";

export interface Settings {
  fov: number;
  sensitivity: number;
  crosshairColor: string;
  crosshairSize: number;
  binds: BindMap;
}

const KEY = "webrace.settings";

const DEFAULTS: Settings = {
  fov: 125,
  sensitivity: 2.0,
  crosshairColor: "#fa00ff",
  crosshairSize: 18,
  binds: { ...DEFAULT_BINDS },
};

function clamp(v: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, v));
}

/** Load settings from localStorage (with validation/clamping). */
export function loadSettings(): Settings {
  let s: Settings = { ...DEFAULTS, binds: { ...DEFAULT_BINDS } };
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (parsed && typeof parsed === "object") {
        s.fov = clamp(Number(parsed.fov) || DEFAULTS.fov, 60, 160);
        s.sensitivity = clamp(Number(parsed.sensitivity) || DEFAULTS.sensitivity, 0.1, 50);
        s.crosshairColor =
          typeof parsed.crosshairColor === "string" && /^#[0-9a-f]{6}$/i.test(parsed.crosshairColor)
            ? parsed.crosshairColor
            : DEFAULTS.crosshairColor;
        s.crosshairSize = clamp(Number(parsed.crosshairSize) || DEFAULTS.crosshairSize, 4, 64);
        if (parsed.binds && typeof parsed.binds === "object") {
          // Merge with defaults so missing binds fall back.
          s.binds = { ...DEFAULT_BINDS, ...parsed.binds };
        }
      }
    }
  } catch {
    /* ignore */
  }
  return s;
}

/** Persist settings to localStorage. */
export function saveSettings(s: Settings): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(s));
  } catch {
    /* ignore */
  }
}

/** Encode settings into a compact share code (base64 of JSON). */
export function encodeConfig(s: Settings): string {
  try {
    const json = JSON.stringify(s);
    // Encode to base64 URL-safe.
    const bytes = new TextEncoder().encode(json);
    let bin = "";
    bytes.forEach((b) => (bin += String.fromCharCode(b)));
    return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  } catch {
    return "";
  }
}

/** Decode a share code back into settings (partial merge onto defaults). */
export function decodeConfig(code: string): Settings | null {
  try {
    const b64 = code.replace(/-/g, "+").replace(/_/g, "/");
    const bin = atob(b64);
    const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
    const json = new TextDecoder().decode(bytes);
    const parsed = JSON.parse(json);
    if (!parsed || typeof parsed !== "object") return null;
    return parsed as Settings;
  } catch {
    return null;
  }
}

export { DEFAULTS };
