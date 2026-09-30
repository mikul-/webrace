// Settings menu wiring: syncs the menu controls with the Settings object and
// applies changes to the game + localStorage.

import { Settings, saveSettings, loadSettings, encodeConfig, decodeConfig } from "../settings";
import { fetchLeaderboard } from "../net/leaderboard";
import { ACTIONS, BindMap, displayCode } from "../binds";

export interface SettingsCallbacks {
  onFov: (fov: number) => void;
  onSensitivity: (sens: number) => void;
  onBinds: (binds: BindMap) => void;
}

export class Menu {
  private settings: Settings;
  private menuEl: HTMLElement;
  private fovSlider: HTMLInputElement;
  private fovInput: HTMLInputElement;
  private fovValue: HTMLElement;
  private sensSlider: HTMLInputElement;
  private sensInput: HTMLInputElement;
  private sensValue: HTMLElement;
  private xhairColor: HTMLInputElement;
  private xhairSize: HTMLInputElement;
  private xhairSizeValue: HTMLElement;
  private xhairEl: HTMLElement;
  private leaderboardEl: HTMLElement;
  private bindsEl: HTMLElement;
  private cb: SettingsCallbacks;
  private onToggle: (open: boolean) => void;
  private captureAction: string | null = null;

  constructor(cb: SettingsCallbacks, onToggle: (open: boolean) => void) {
    this.cb = cb;
    this.onToggle = onToggle;
    this.settings = loadSettings();
    this.menuEl = document.getElementById("menu")!;
    this.fovSlider = document.getElementById("fov-slider") as HTMLInputElement;
    this.fovInput = document.getElementById("fov-input") as HTMLInputElement;
    this.fovValue = document.getElementById("fov-value")!;
    this.sensSlider = document.getElementById("sens-slider") as HTMLInputElement;
    this.sensInput = document.getElementById("sens-input") as HTMLInputElement;
    this.sensValue = document.getElementById("sens-value")!;
    this.xhairColor = document.getElementById("xhair-color") as HTMLInputElement;
    this.xhairSize = document.getElementById("xhair-size") as HTMLInputElement;
    this.xhairSizeValue = document.getElementById("xhair-size-value")!;
    this.xhairEl = document.getElementById("crosshair")!;
    this.leaderboardEl = document.getElementById("leaderboard")!;
    this.bindsEl = document.getElementById("binds")!;

    this.bind();
    this.bindConfigButtons();
    this.applyAll();
  }

  /** Wire the share-config copy/apply buttons. */
  private bindConfigButtons() {
    const shareBtn = document.getElementById("share-config");
    const codeInput = document.getElementById("config-code") as HTMLInputElement;
    const applyBtn = document.getElementById("apply-config");
    shareBtn?.addEventListener("click", () => {
      const code = encodeConfig(this.settings);
      if (!code) return;
      try {
        void navigator.clipboard.writeText(code);
        shareBtn!.textContent = "copied!";
        setTimeout(() => (shareBtn!.textContent = "Copy share code"), 1500);
      } catch {
        codeInput.value = code;
      }
    });
    applyBtn?.addEventListener("click", () => {
      const decoded = decodeConfig(codeInput.value.trim());
      if (!decoded) {
        codeInput.value = "";
        codeInput.placeholder = "invalid code";
        return;
      }
      this.settings = { ...this.settings, ...decoded };
      saveSettings(this.settings);
      this.applyAll();
      codeInput.value = "";
    });
  }

  private bind() {
    // FOV
    this.fovSlider.addEventListener("input", () => this.setFov(Number(this.fovSlider.value)));
    this.fovInput.addEventListener("input", () => {
      const v = Number(this.fovInput.value);
      if (Number.isFinite(v)) this.setFov(v);
    });

    // Sensitivity
    this.sensSlider.addEventListener("input", () => this.setSensitivity(Number(this.sensSlider.value)));
    this.sensInput.addEventListener("input", () => {
      const v = Number(this.sensInput.value);
      if (Number.isFinite(v)) this.setSensitivity(v);
    });

    // Crosshair color
    this.xhairColor.addEventListener("input", () => this.setCrosshairColor(this.xhairColor.value));

    // Crosshair size
    this.xhairSize.addEventListener("input", () => this.setCrosshairSize(Number(this.xhairSize.value)));
  }

  private setFov(v: number) {
    this.settings.fov = v;
    this.fovValue.textContent = `${v}°`;
    if (Number(this.fovInput.value) !== v) this.fovInput.value = String(v);
    if (this.fovSlider.value !== String(v)) this.fovSlider.value = String(v);
    this.cb.onFov(v);
    this.save();
  }

  private setSensitivity(v: number) {
    this.settings.sensitivity = v;
    this.sensValue.textContent = v.toFixed(2);
    if (Number(this.sensInput.value) !== v) this.sensInput.value = String(v);
    if (Number(this.sensSlider.value) !== v) this.sensSlider.value = String(v);
    this.cb.onSensitivity(v);
    this.save();
  }

  private setCrosshairColor(c: string) {
    this.settings.crosshairColor = c;
    this.xhairEl.style.setProperty("--xhair-color", c);
    this.save();
  }

  private setCrosshairSize(s: number) {
    this.settings.crosshairSize = s;
    this.xhairSizeValue.textContent = `${s}px`;
    this.xhairEl.style.width = `${s}px`;
    this.xhairEl.style.height = `${s}px`;
    this.save();
  }

  private save() {
    saveSettings(this.settings);
  }

  /** Sync all UI + game state from the stored settings. */
  applyAll() {
    this.fovSlider.value = String(this.settings.fov);
    this.fovInput.value = String(this.settings.fov);
    this.fovValue.textContent = `${this.settings.fov}°`;
    this.cb.onFov(this.settings.fov);

    this.sensSlider.value = String(this.settings.sensitivity);
    this.sensInput.value = String(this.settings.sensitivity);
    this.sensValue.textContent = this.settings.sensitivity.toFixed(2);
    this.cb.onSensitivity(this.settings.sensitivity);

    this.xhairColor.value = this.settings.crosshairColor;
    this.xhairEl.style.setProperty("--xhair-color", this.settings.crosshairColor);

    this.xhairSize.value = String(this.settings.crosshairSize);
    this.xhairSizeValue.textContent = `${this.settings.crosshairSize}px`;
    this.xhairEl.style.width = `${this.settings.crosshairSize}px`;
    this.xhairEl.style.height = `${this.settings.crosshairSize}px`;

    this.cb.onBinds({ ...this.settings.binds });
    this.renderBinds();
  }

  /** Open/close the menu. */
  setOpen(open: boolean) {
    this.menuEl.classList.toggle("open", open);
    this.onToggle(open);
  }

  isOpen(): boolean {
    return this.menuEl.classList.contains("open");
  }

  /** Load and render the leaderboard for a map. */
  async refreshLeaderboard(map: string) {
    try {
      const entries = await fetchLeaderboard(map);
      if (entries.length === 0) {
        this.leaderboardEl.textContent = "no times yet";
        return;
      }
      const fmt = (ms: number) => {
        const m = Math.floor(ms / 60000);
        const s = Math.floor((ms % 60000) / 1000);
        const cs = Math.floor((ms % 1000) / 10);
        return `${m}:${s.toString().padStart(2, "0")}.${cs.toString().padStart(2, "0")}`;
      };
      this.leaderboardEl.innerHTML = entries
        .map((e: { nickname: string; time_ms: number }, i: number) => {
          const row = document.createElement("div");
          row.className = "lb-row";
          const pos = document.createElement("span");
          pos.className = "lb-pos";
          pos.textContent = `${i + 1}.`;
          const name = document.createElement("span");
          name.textContent = e.nickname;
          const time = document.createElement("span");
          time.textContent = fmt(e.time_ms);
          row.append(pos, name, time);
          return row.outerHTML;
        })
        .join("");
    } catch (e) {
      this.leaderboardEl.textContent = "leaderboard unavailable";
    }
  }

  /** Render the bind list with editable entries. */
  private renderBinds() {
    if (!this.bindsEl) return;
    this.bindsEl.innerHTML = "";
    for (const a of ACTIONS) {
      // Find the code bound to this action.
      let code = "";
      for (const [c, act] of Object.entries(this.settings.binds)) {
        if (act === a.id) {
          code = c;
          break;
        }
      }
      const row = document.createElement("div");
      row.className = "bind-row";
      const label = document.createElement("span");
      label.className = "bind-label";
      label.textContent = a.label;
      const btn = document.createElement("button");
      btn.className = "bind-key";
      btn.textContent = this.captureAction === a.id ? "press a key…" : displayCode(code);
      btn.addEventListener("click", () => this.startCapture(a.id));
      row.append(label, btn);
      this.bindsEl.append(row);
    }
  }

  /** Begin capturing a new key/mouse bind for an action. */
  private startCapture(action: string) {
    this.captureAction = action;
    this.renderBinds();

    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const code = e.code;
      if (code === "Escape") {
        this.captureAction = null;
      } else {
        this.setBind(action, code);
      }
      cleanup();
      this.renderBinds();
    };
    const onMouse = (e: MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const code = "Mouse" + e.button;
      this.setBind(action, code);
      cleanup();
      this.renderBinds();
    };
    const cleanup = () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("mousedown", onMouse, true);
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("mousedown", onMouse, true);
  }

  /** Set a bind, removing any conflicting binding for the same code. */
  private setBind(action: string, code: string) {
    // Remove any existing binding for this code.
    for (const c of Object.keys(this.settings.binds)) {
      if (c === code) delete this.settings.binds[c];
    }
    // Remove any existing binding for this action.
    for (const [c] of Object.entries(this.settings.binds)) {
      const a = this.settings.binds[c];
      if (a === action) delete this.settings.binds[c];
    }
    this.settings.binds[code] = action;
    this.captureAction = null;
    saveSettings(this.settings);
    this.cb.onBinds({ ...this.settings.binds });
  }
}
