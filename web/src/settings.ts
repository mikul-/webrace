// Client settings (persisted to localStorage).

export interface Settings {
  fov: number;
  sensitivity: number;
  crosshairColor: string;
  crosshairSize: number;
}

const KEY = "webrace.settings";

const DEFAULTS: Settings = {
  fov: 140,
  sensitivity: 1.72,
  crosshairColor: "#fa00ff",
  crosshairSize: 18,
};

function clamp(v: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, v));
}

/** Load settings from localStorage (with validation/clamping). */
export function loadSettings(): Settings {
  let s: Settings = { ...DEFAULTS };
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

export { DEFAULTS };
