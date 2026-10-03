// Settings menu wiring: syncs the menu controls with the Settings object and
// applies changes to the game + localStorage.

import { Settings, saveSettings, loadSettings, encodeConfig, decodeConfig } from "../settings";
import { fetchLeaderboard, getIdentity } from "../net/leaderboard";
import { ACTIONS, BindMap, displayCode } from "../binds";
import { SOUND_EVENTS } from "../audio";
import type { TextureMode } from "../render/renderer";
import {
  MapInfo,
  getCatalog,
  searchMaps,
  parseVote,
  resolveVote,
  getFavorites,
  toggleFavorite,
  isFavorite,
} from "../maps";

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]!));
}

function getNickname(): string | null {
  const id = getIdentity();
  return id ? id.nickname : null;
}

export interface SettingsCallbacks {
  onFov: (fov: number) => void;
  onSensitivity: (sens: number) => void;
  onBinds: (binds: BindMap) => void;
  onPlayMap: (map: string) => void;
  /** Texture filtering quality changed. */
  onTextureMode: (mode: TextureMode) => void;
  /** Sound assignment (event id -> file name) changed. */
  onSounds: (sounds: Record<string, string>) => void;
  /** Master sound volume changed (0..1). */
  onVolume: (volume: number) => void;
  /** Preview a specific `snd/` file. */
  onPlaySound: (file: string) => void;
  /** Fetch the live list of files available under `snd/`. */
  onListSounds: () => Promise<string[]>;
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
  private textureModeSelect: HTMLSelectElement;
  private leaderboardEl: HTMLElement;
  private bindsEl: HTMLElement;
  private lbMapInput: HTMLInputElement;
  private lbRefreshBtn: HTMLButtonElement;
  private voteInput: HTMLInputElement;
  private votePlayBtn: HTMLButtonElement;
  private voteResultEl: HTMLElement;
  private mapsSearchInput: HTMLInputElement;
  private mapsListEl: HTMLElement;
  private favoritesListEl: HTMLElement;
  // Sound tab.
  private soundListEl: HTMLElement;
  private soundStatusEl: HTMLElement;
  private soundReloadBtn: HTMLButtonElement;
  private volumeSlider: HTMLInputElement;
  private volumeValue: HTMLElement;
  private soundSelects = new Map<string, HTMLSelectElement>();
  private soundFiles: string[] = [];
  private soundLoaded = false;
  private soundLoading = false;
  private cb: SettingsCallbacks;
  private onToggle: (open: boolean) => void;
  private captureAction: string | null = null;
  private lbMap = "";
  private catalog: MapInfo[] | null = null;
  private mapResults: MapInfo[] = [];
  private mapRendered = 0;

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
    this.textureModeSelect = document.getElementById("texture-mode") as HTMLSelectElement;
    this.leaderboardEl = document.getElementById("leaderboard")!;
    this.bindsEl = document.getElementById("binds")!;
    this.lbMapInput = document.getElementById("lb-map-input") as HTMLInputElement;
    this.lbRefreshBtn = document.getElementById("lb-refresh") as HTMLButtonElement;
    this.voteInput = document.getElementById("vote-input") as HTMLInputElement;
    this.votePlayBtn = document.getElementById("vote-play") as HTMLButtonElement;
    this.voteResultEl = document.getElementById("vote-result")!;
    this.mapsSearchInput = document.getElementById("maps-search-input") as HTMLInputElement;
    this.mapsListEl = document.getElementById("maps-list")!;
    this.favoritesListEl = document.getElementById("favorites-list")!;
    this.soundListEl = document.getElementById("sound-list")!;
    this.soundStatusEl = document.getElementById("sound-status")!;
    this.soundReloadBtn = document.getElementById("sound-reload") as HTMLButtonElement;
    this.volumeSlider = document.getElementById("volume-slider") as HTMLInputElement;
    this.volumeValue = document.getElementById("volume-value")!;

    this.bind();
    this.bindTabs();
    this.bindConfigButtons();
    this.bindLeaderboardControls();
    this.bindMapsControls();
    this.bindSoundControls();
    this.applyAll();
    // Preload the drop-in sound list so the dropdowns are ready when opened.
    void this.ensureSoundsLoaded();
  }

  /** Tab/page switching. */
  private bindTabs() {
    const tabs = Array.from(this.menuEl.querySelectorAll<HTMLButtonElement>(".tab"));
    const pages = Array.from(this.menuEl.querySelectorAll<HTMLElement>(".page"));
    for (const tab of tabs) {
      tab.addEventListener("click", () => {
        const pageId = tab.dataset.page;
        for (const t of tabs) t.classList.toggle("active", t === tab);
        for (const p of pages) p.classList.toggle("active", p.id === `page-${pageId}`);
        if (pageId === "maps") this.ensureCatalogAndRender();
        if (pageId === "favorites") this.ensureCatalogAndRenderFavorites();
        if (pageId === "sound") void this.ensureSoundsLoaded();
      });
    }
  }

  /** Load the catalog (once) and render the maps list. */
  async ensureCatalogAndRender() {
    try {
      await this.getCatalog();
      this.renderMapsList();
    } catch {
      this.mapsListEl.innerHTML = `<div class="map-row" style="color:#5a6472">catalog unavailable</div>`;
    }
  }

  async ensureCatalogAndRenderFavorites() {
    try {
      await this.getCatalog();
      this.renderFavorites();
    } catch {
      this.favoritesListEl.innerHTML = `<div class="map-row" style="color:#5a6472">catalog unavailable</div>`;
    }
  }

  /** Leaderboard controls (map input + refresh). */
  private bindLeaderboardControls() {
    this.lbRefreshBtn.addEventListener("click", () => {
      const map = this.lbMapInput.value.trim();
      if (map) void this.refreshLeaderboard(map);
    });
    this.lbMapInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        const map = this.lbMapInput.value.trim();
        if (map) void this.refreshLeaderboard(map);
      }
    });
  }

  /** Map voting + search + favorites controls. */
  private bindMapsControls() {
    const doVote = async (explicit?: string) => {
      const input = (explicit ?? this.voteInput.value).trim();
      if (!input) return;
      if (explicit === undefined) this.voteInput.value = input;
      const maps = await this.getCatalog();
      const req = parseVote(input);
      if (!req) {
        this.voteResultEl.textContent = "invalid vote";
        return;
      }
      const map = resolveVote(maps, req);
      if (!map) {
        this.voteResultEl.textContent = `no map matched: ${input}`;
        return;
      }
      this.voteResultEl.textContent = `voting… ${map}`;
      this.cb.onPlayMap(map);
    };
    this.votePlayBtn.addEventListener("click", () => void doVote());
    this.voteInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") void doVote();
    });

    // Clickable example vote chips.
    this.menuEl.querySelectorAll<HTMLButtonElement>(".vote-chip").forEach((chip) => {
      chip.addEventListener("click", () => void doVote(chip.dataset.vote));
    });

    this.mapsSearchInput.addEventListener("input", () => this.renderMapsList());

    this.mapsListEl.addEventListener("scroll", this.onMapsListScroll);
  }

  /** Sound tab: reload button + live file list. */
  private bindSoundControls() {
    this.soundReloadBtn.addEventListener("click", () => void this.ensureSoundsLoaded(true));
  }

  /**
   * Fetch the list of files under `snd/` (once, or when forced by Reload) and
   * render the per-event dropdowns.
   */
  async ensureSoundsLoaded(force = false) {
    if (this.soundLoading) return;
    if (this.soundLoaded && !force) {
      this.renderSounds();
      return;
    }
    this.soundLoading = true;
    this.soundStatusEl.textContent = "loading…";
    try {
      this.soundFiles = await this.cb.onListSounds();
      this.soundLoaded = true;
      this.soundStatusEl.textContent = this.soundFiles.length
        ? `${this.soundFiles.length} file${this.soundFiles.length === 1 ? "" : "s"}`
        : "no files found in snd/";
    } catch {
      this.soundFiles = [];
      this.soundStatusEl.textContent = "could not load snd/ (is the map-server running?)";
    } finally {
      this.soundLoading = false;
    }
    this.renderSounds();
  }

  /** Build a labelled dropdown + preview button for every sound event. */
  private renderSounds() {
    if (!this.soundListEl) return;
    this.soundListEl.innerHTML = "";
    this.soundSelects.clear();

    for (const ev of SOUND_EVENTS) {
      const row = document.createElement("div");
      row.className = "sound-row";

      const label = document.createElement("span");
      label.className = "sound-label";
      label.textContent = ev.label;

      const select = document.createElement("select");
      select.className = "sound-select";
      const none = document.createElement("option");
      none.value = "";
      none.textContent = this.soundFiles.length ? "— none —" : "(no files in snd/)";
      select.append(none);
      for (const f of this.soundFiles) {
        const opt = document.createElement("option");
        opt.value = f;
        opt.textContent = f;
        select.append(opt);
      }
      const cur = (this.settings.sounds ?? {})[ev.id] ?? "";
      if (cur && this.soundFiles.includes(cur)) select.value = cur;
      select.addEventListener("change", () => {
        if (!this.settings.sounds || typeof this.settings.sounds !== "object") {
          this.settings.sounds = {};
        }
        if (select.value) this.settings.sounds[ev.id] = select.value;
        else delete this.settings.sounds[ev.id];
        this.save();
        this.cb.onSounds({ ...this.settings.sounds });
      });
      this.soundSelects.set(ev.id, select);

      const play = document.createElement("button");
      play.className = "sound-play";
      play.textContent = "▶";
      play.title = "preview selected file";
      play.addEventListener("click", () => {
        const file = select.value;
        if (file) this.cb.onPlaySound(file);
      });

      row.append(label, select, play);
      this.soundListEl.append(row);
    }
  }

  async getCatalog(): Promise<MapInfo[]> {
    if (!this.catalog) this.catalog = await getCatalog();
    return this.catalog;
  }

  /** Render the searchable map list (incremental: load more on scroll). */
  renderMapsList() {
    if (!this.catalog) return;
    const q = this.mapsSearchInput.value.trim();
    const results = searchMaps(this.catalog, q);
    this.mapResults = results;
    this.mapRendered = 0;
    this.mapsListEl.innerHTML = "";
    this.appendMapBatch();
  }

  private appendMapBatch() {
    if (!this.catalog) return;
    const BATCH = 100;
    const end = Math.min(this.mapRendered + BATCH, this.mapResults.length);
    for (let i = this.mapRendered; i < end; i++) {
      this.mapsListEl.appendChild(this.makeMapRow(this.mapResults[i]));
    }
    this.mapRendered = end;
  }

  /** Load more when the list is scrolled near the bottom. */
  private onMapsListScroll = () => {
    const el = this.mapsListEl;
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - 200) {
      this.appendMapBatch();
    }
  };

  renderFavorites() {
    if (!this.catalog) {
      this.favoritesListEl.innerHTML = `<div class="map-row" style="color:#5a6472">loading…</div>`;
      return;
    }
    const favs = getFavorites();
    if (favs.length === 0) {
      this.favoritesListEl.innerHTML = `<div class="map-row" style="color:#5a6472">no favorites yet — click ☆ on a map</div>`;
      return;
    }
    this.favoritesListEl.innerHTML = "";
    const byName = new Map(this.catalog.map((m) => [m.map_name.toLowerCase(), m]));
    for (const name of favs) {
      const m = byName.get(name);
      if (m) this.favoritesListEl.appendChild(this.makeMapRow(m));
    }
  }

  private makeMapRow(m: MapInfo): HTMLButtonElement {
    const row = document.createElement("button");
    row.className = "map-row";

    // Star (favorite toggle).
    const star = document.createElement("span");
    star.className = "fav-star";
    star.textContent = isFavorite(m.map_name) ? "★" : "☆";
    star.title = "toggle favorite";

    const name = document.createElement("span");
    name.className = "map-name";
    name.textContent = m.map_name;

    const author = document.createElement("span");
    author.className = "map-author";
    author.textContent = m.author ?? m.game ?? "";

    const tags: string[] = [];
    if (m.has_slick) tags.push("slick");
    if (m.has_rocket_launcher) tags.push("rocket");
    if (m.has_plasmagun) tags.push("plasma");
    if (m.has_grenade_launcher) tags.push("grenade");
    if (m.has_jumppad) tags.push("jp");
    const tagsEl = document.createElement("span");
    tagsEl.className = "map-tags";
    tagsEl.textContent = tags.join(" ");

    row.append(star, name, author, tagsEl);

    // Click = play the map.
    row.addEventListener("click", () => this.cb.onPlayMap(m.map_name));

    // Star click = toggle favorite (stop propagation).
    star.addEventListener("click", (e) => {
      e.stopPropagation();
      toggleFavorite(m.map_name);
      star.textContent = isFavorite(m.map_name) ? "★" : "☆";
      this.renderFavorites();
    });

    return row;
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

    // Texture filtering quality
    this.textureModeSelect.addEventListener("change", () =>
      this.setTextureMode(this.textureModeSelect.value as TextureMode),
    );

    // Master sound volume
    this.volumeSlider.addEventListener("input", () => this.setVolume(Number(this.volumeSlider.value)));
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

  private setTextureMode(mode: TextureMode) {
    this.settings.textureMode = mode;
    this.cb.onTextureMode(mode);
    this.save();
  }

  private setVolume(v: number) {
    const clamped = Math.min(1, Math.max(0, v));
    this.settings.volume = clamped;
    this.volumeValue.textContent = `${Math.round(clamped * 100)}%`;
    if (Number(this.volumeSlider.value) !== clamped) this.volumeSlider.value = String(clamped);
    this.cb.onVolume(clamped);
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

    this.textureModeSelect.value = this.settings.textureMode;
    this.cb.onTextureMode(this.settings.textureMode);

    const volume = Math.min(1, Math.max(0, this.settings.volume ?? 0.8));
    this.volumeSlider.value = String(volume);
    this.volumeValue.textContent = `${Math.round(volume * 100)}%`;
    this.cb.onVolume(volume);

    this.cb.onBinds({ ...this.settings.binds });
    this.renderBinds();

    // Sound assignments: ensure the object exists and push to the audio system.
    if (!this.settings.sounds || typeof this.settings.sounds !== "object") {
      this.settings.sounds = {};
    }
    this.cb.onSounds({ ...this.settings.sounds });
    if (this.soundLoaded) this.renderSounds();
  }

  /** Open/close the menu. */
  setOpen(open: boolean) {
    this.menuEl.classList.toggle("open", open);
    this.onToggle(open);
  }

  /** Prefill the leaderboard map input with the given map. */
  prefillLeaderboardMap(map: string) {
    if (this.lbMapInput.value.trim() === "") {
      this.lbMapInput.value = map.toLowerCase();
    }
  }

  isOpen(): boolean {
    return this.menuEl.classList.contains("open");
  }

  /** Load and render the leaderboard for a map (formatted table). */
  async refreshLeaderboard(map: string) {
    this.lbMap = map.toLowerCase();
    if (this.lbMapInput.value !== this.lbMap) this.lbMapInput.value = this.lbMap;
    try {
      const entries = await fetchLeaderboard(this.lbMap);
      if (entries.length === 0) {
        this.leaderboardEl.innerHTML = `<div class="lb-row" style="justify-content:center;color:#5a6472">no times yet</div>`;
        return;
      }
      const fmt = (ms: number) => {
        const m = Math.floor(ms / 60000);
        const s = Math.floor((ms % 60000) / 1000);
        const cs = Math.floor((ms % 1000) / 10);
        return `${m}:${s.toString().padStart(2, "0")}.${cs.toString().padStart(2, "0")}`;
      };
      const me = getNickname();
      // Header + rows.
      const head = `<div class="lb-head"><span>#</span><span>player</span><span class="lb-time">time</span></div>`;
      const rows = entries.map((e, i) => {
        const isMe = me !== null && e.nickname === me;
        const wr = i === 0 ? ' <span style="color:#ffb238">WR</span>' : "";
        return `<div class="lb-row${isMe ? " me" : ""}">
          <span class="lb-pos">${i + 1}.</span>
          <span>${escapeHtml(e.nickname)}${isMe ? ' <span style="opacity:.6">(you)</span>' : ""}${wr}</span>
          <span class="lb-time">${fmt(e.time_ms)}</span>
        </div>`;
      });
      this.leaderboardEl.innerHTML = head + rows.join("");
    } catch {
      this.leaderboardEl.innerHTML = `<div class="lb-row" style="justify-content:center;color:#5a6472">leaderboard unavailable</div>`;
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
