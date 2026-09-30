// Entrypoint: load WASM core, load a map, and run a first-person
// Warfork-style movement controller (pointer lock + WASD + jump/dash).
//
// Milestone 2: playable movement. Multiplayer + weapons wire in later.

import { Renderer, perspective, lookAt, multiply } from "./render/renderer";
import { fetchBsp } from "./sim/map";
import { api } from "./base";
import { loadTexture } from "./render/textures";
import { getIdentity, registerNickname, submitTime } from "./net/leaderboard";
import { Menu } from "./ui/menu";
import { MovementHud } from "./ui/movement_hud";
import { BindMap, DEFAULT_BINDS, codeToAction, mouseButtonToCode, Action } from "./binds";
import init, * as core from "../pkg/webrace_core.js";

const overlay = document.getElementById("overlay")!;
const statusEl = document.getElementById("status")!;
const fpsEl = document.getElementById("fps")!;
const speedEl = document.getElementById("speed")!;
const mapInput = document.getElementById("map") as HTMLInputElement;
const nicknameInput = document.getElementById("nickname") as HTMLInputElement;
const playBtn = document.getElementById("play")!;
const logEl = document.getElementById("log")!;
const lockEl = document.getElementById("lock")!;
const timerEl = document.getElementById("timer")!;
// Show the unlock state immediately on load (pointerlockchange doesn't fire
// until a lock attempt happens, so we initialize the label ourselves).
lockEl.textContent = "🔓 click to lock mouse";
lockEl.style.color = "#e8603a";
const canvas = document.getElementById("view") as HTMLCanvasElement;

let renderer: Renderer | null = null;
let memory: WebAssembly.Memory | null = null;
let sessionId: number | null = null;
let currentMap = "";
let submitGuard = false; // true once we've submitted the current finish
let fov = 140; // horizontal FOV (updated by the settings menu)
let menu: Menu | null = null;
let binds: BindMap = { ...DEFAULT_BINDS };
let actionByCode = codeToAction(binds);
const moveHud = new MovementHud();

// Key state (held actions).
const keys: Record<string, boolean> = {
  forward: false, back: false, moveleft: false, moveright: false,
  jump: false, crouch: false, special: false, attack: false,
};

function setStatus(s: string) {
  statusEl.textContent = s;
}
function log(s: string) {
  logEl.textContent += s + "\n";
}

async function main() {
  try {
    setStatus("loading WASM core…");
    const initOut = await init();
    memory = initOut.memory;
    core.install_panic_hook();
    setStatus("ready — type a map name and press Play");

    playBtn.addEventListener("click", () => void loadMap());
    mapInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") void loadMap();
    });

    // Settings menu.
    menu = new Menu(
      {
        onFov: (v) => (fov = v),
        onSensitivity: (s) => {
          if (sessionId !== null) core.session_set_sensitivity(sessionId, s);
        },
        onBinds: (b) => {
          binds = b;
          actionByCode = codeToAction(b);
        },
        onPlayMap: (map) => {
          menu?.setOpen(false);
          mapInput.value = map;
          void loadMap(map);
        },
      },
      (open) => {
        // Release pointer lock when opening the menu so the mouse is usable.
        if (open && document.pointerLockElement === canvas) {
          document.exitPointerLock?.();
        }
        // Clear held keys when toggling.
        for (const k of Object.keys(keys) as (keyof typeof keys)[]) keys[k] = false;
      },
    );

    // Prefill nickname from stored identity.
    const existing = getIdentity();
    if (existing) nicknameInput.value = existing.nickname;

    const q = new URLSearchParams(location.search).get("map");
    if (q) {
      mapInput.value = q;
      void loadMap();
    }
  } catch (e) {
    setStatus("failed to initialize: " + (e as Error).message);
    log((e as Error).stack || String(e));
  }
}

async function loadMap(explicitName?: string) {
  const name = (explicitName ?? mapInput.value).trim();
  if (!name || !memory) return;
  setStatus(`loading ${name}…`);
  try {
    // Ensure a nickname is registered (for leaderboard submission).
    const nick = nicknameInput.value.trim();
    if (nick) {
      try {
        await registerNickname(nick);
      } catch (e) {
        log(`nickname: ${(e as Error).message}`);
      }
    }
    currentMap = name.toLowerCase();
    submitGuard = false;

    const url = api(`/maps/${name}.bsp`);
    const bytes = await fetchBsp(url);
    const mapId = core.bsp_parse(name, bytes);

    const vertCount = core.bsp_vertex_count(mapId);
    const idxCount = core.bsp_index_count(mapId);
    const triCount = core.bsp_triangle_count(mapId);
    const brushCount = core.bsp_brush_count(mapId);

    if (!renderer) {
      renderer = new Renderer(canvas);
      setupResize();
    }

    renderer.uploadMap(
      memory,
      core.bsp_vertices_ptr(mapId),
      vertCount,
      core.bsp_indices_ptr(mapId),
      idxCount,
    );

    // Upload the lightmap atlas.
    renderer.uploadLightmap(
      memory,
      core.bsp_lightmap_ptr(mapId),
      core.bsp_lightmap_len(mapId),
      core.bsp_lightmap_w(mapId),
      core.bsp_lightmap_h(mapId),
    );

    // Determine draw chunks + load textures per unique shader.
    const chunkCount = core.bsp_chunk_count(mapId);
    const shaderIdx = new Uint32Array(chunkCount);
    const chunkFirst = new Uint32Array(chunkCount);
    const chunkCountArr = new Uint32Array(chunkCount);
    core.bsp_chunks(mapId, shaderIdx, chunkFirst, chunkCountArr);

    // Unique shaders used by the map.
    const uniqueShaders = Array.from(new Set(Array.from(shaderIdx)));
    const texIdByShader = new Map<number, number>();
    await Promise.all(
      uniqueShaders.map(async (s) => {
        const shaderName = core.bsp_shader_name(mapId, s);
        const img = await loadTexture(shaderName);
        if (img) {
          texIdByShader.set(s, renderer!.registerTexture(img));
        }
      }),
    );

    const chunks = [];
    for (let i = 0; i < chunkCount; i++) {
      chunks.push({
        first: chunkFirst[i],
        count: chunkCountArr[i],
        tex: texIdByShader.get(shaderIdx[i]) ?? -1,
      });
    }
    renderer.setChunks(chunks);

    // Create a playable session at spawn point 0.
    if (sessionId !== null) core.session_drop(sessionId);
    sessionId = core.session_new(mapId, 0);
    // Apply saved sensitivity to the new session.
    menu?.applyAll();

    overlay.classList.add("hidden");
    setStatus("");
    log(`${name}: ${triCount} tris, ${brushCount} brushes`);
    log("CLICK the screen to lock mouse · WASD move · SPACE dash · right-click jump");

    const eye = core.session_eye(sessionId) as unknown as Float32Array;
    log(`spawn eye (render): ${[eye[0], eye[1], eye[2]].map((v) => v.toFixed(1)).join(", ")}`);
    requestAnimationFrame(loop);
  } catch (e) {
    setStatus(`load failed: ${(e as Error).message}`);
    log((e as Error).stack || String(e));
  }
}

function setupResize() {
  const update = () => {
    if (!renderer) return;
    canvas.width = canvas.clientWidth;
    canvas.height = canvas.clientHeight;
    renderer.resize(canvas.width, canvas.height);
  };
  window.addEventListener("resize", update);
  update();
}

// ---- Pointer lock + input ----

function lockPointer() {
  const el = canvas as HTMLCanvasElement & {
    requestPointerLock: () => void | Promise<void>;
  };
  if (typeof el.requestPointerLock !== "function") {
    lockEl.textContent = "browser does not support pointer lock";
    return;
  }
  try {
    const r = el.requestPointerLock();
    if (r && typeof (r as Promise<void>).catch === "function") {
      (r as Promise<void>).catch(() => {
        // Pointer lock refused (e.g. re-locking too soon after exit). Ignore;
        // the user can click again.
      });
    }
  } catch (e) {
    lockEl.textContent = "lock failed: " + (e as Error).message;
  }
}

canvas.addEventListener("click", () => {
  if (overlay.classList.contains("hidden") && document.pointerLockElement !== canvas) {
    lockEl.textContent = "requesting pointer lock…";
    lockPointer();
  }
});

// Also allow clicking anywhere (HUD elements are pointer-events:none), but
// NOT while the menu is open — menu clicks must not re-lock the mouse.
document.addEventListener("click", () => {
  if (menu?.isOpen()) return;
  if (overlay.classList.contains("hidden") && document.pointerLockElement !== canvas) {
    lockEl.textContent = "requesting pointer lock…";
    lockPointer();
  }
});

document.addEventListener("pointerlockerror", () => {
  lockEl.textContent = "🔓 pointer lock refused by browser";
  lockEl.style.color = "#e8603a";
});

document.addEventListener("pointerlockchange", () => {
  if (document.pointerLockElement === canvas) {
    lockEl.textContent = "🔒 mouse locked — right-click jump · SPACE dash";
    lockEl.style.color = "#5ce27a";
  } else {
    lockEl.textContent = "🔓 click to lock mouse";
    lockEl.style.color = "#e8603a";
    for (const k of Object.keys(keys) as (keyof typeof keys)[]) keys[k] = false;
  }
});

// Mouse look (raw movement). Sensitivity is applied inside the sim (Quake
// m_yaw/m_pitch model), so we just forward the raw deltas.
document.addEventListener("mousemove", (e) => {
  if (sessionId === null) return;
  if (document.pointerLockElement === canvas && e.movementX !== undefined) {
    core.session_add_mouse(sessionId, e.movementX, e.movementY);
  }
});

/** Dispatch a discrete (non-held) action. */
function dispatchAction(action: Action): void {
  switch (action) {
    case "restart":
      if (sessionId !== null) core.session_reset(sessionId);
      break;
    case "menu":
      if (menu) {
        menu.setOpen(!menu.isOpen());
        if (menu.isOpen() && currentMap) {
          // Populate the leaderboard map field with the current map.
          menu.prefillLeaderboardMap(currentMap);
          void menu.refreshLeaderboard(currentMap);
        }
      }
      break;
    case "position_save":
      if (sessionId !== null) core.session_position_save(sessionId);
      break;
    default:
      break;
  }
}

// True when the event target is a text input/textarea/contenteditable (so we
// don't hijack typing keys like Space).
function isTypingTarget(e: Event): boolean {
  const t = e.target as HTMLElement | null;
  if (!t) return false;
  const tag = t.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || t.isContentEditable;
}

window.addEventListener("keydown", (e) => {
  if (isTypingTarget(e)) return;
  const action = actionByCode.get(e.code);
  if (!action) return;
  if (e.repeat) return;
  if (e.code === "Space") e.preventDefault();
  if (action in keys) {
    keys[action] = true;
  } else {
    dispatchAction(action);
  }
});
window.addEventListener("keyup", (e) => {
  if (isTypingTarget(e)) return;
  const action = actionByCode.get(e.code);
  if (action && action in keys) keys[action] = false;
});

// Mouse buttons.
document.addEventListener("mousedown", (e) => {
  const code = mouseButtonToCode(e.button);
  const action = actionByCode.get(code);
  if (!action) return;
  if (action in keys) {
    keys[action] = true;
  } else {
    dispatchAction(action);
  }
});
document.addEventListener("mouseup", (e) => {
  const code = mouseButtonToCode(e.button);
  const action = actionByCode.get(code);
  if (action && action in keys) keys[action] = false;
});
document.addEventListener("contextmenu", (e) => e.preventDefault());

// Push held keys into the sim each frame before stepping.
function pushKeys() {
  if (sessionId === null) return;
  core.session_set_keys(
    sessionId,
    keys.forward, keys.back, keys.moveleft, keys.moveright,
    keys.jump, keys.crouch, keys.special, keys.attack,
  );
}

// ---- Main loop ----

let frame = 0;
let lastFpsTime = performance.now();
let lastTick = performance.now();
const TICK_MS = 1000 / 250; // 250 Hz

function loop() {
  frame++;
  const now = performance.now();

  // FPS counter.
  if (now - lastFpsTime > 500) {
    const dt = (now - lastFpsTime) / 1000;
    fpsEl.textContent = `${Math.round(frame / dt)} fps`;
    lastFpsTime = now;
    frame = 0;
  }

  // Step the sim at a fixed 250 Hz using an accumulator.
  const menuOpen = menu?.isOpen() ?? false;
  if (sessionId !== null && !menuOpen) {
    // process queued mouse + keys, then step the fixed-rate loop.
    let steps = 0;
    while (now - lastTick >= TICK_MS && steps < 8) {
      pushKeys();
      core.session_step(sessionId);
      lastTick += TICK_MS;
      steps++;
    }
    if (now - lastTick >= TICK_MS) {
      // We fell behind (e.g. tab was backgrounded) — drop the backlog.
      lastTick = now;
    }

    const speed = core.session_speed(sessionId);
    speedEl.textContent = `${Math.round(speed)} ups`;

    // Movement HUD (strafe/bunny indicators + accel bar).
    const hint = core.session_movement_hint(sessionId) as unknown as Float32Array;
    moveHud.update(hint);

    // Race timer display.
    const ticks = core.session_race_ticks(sessionId);
    const finished = core.session_race_finished(sessionId);
    const running = core.session_race_running(sessionId);
    const totalCp = core.session_race_total_checkpoints(sessionId);
    const splits = core.session_race_splits(sessionId) as unknown as Uint32Array;
    const fmt = (t: number) => {
      const ms = (t * 1000) / 250;
      const m = Math.floor(ms / 60000);
      const s = Math.floor((ms % 60000) / 1000);
      const cs = Math.floor((ms % 1000) / 10);
      return `${m}:${s.toString().padStart(2, "0")}.${cs.toString().padStart(2, "0")}`;
    };
    if (finished) {
      timerEl.textContent = `finish ${fmt(ticks)}`;
      timerEl.style.color = "#5ce27a";
      // Submit the time to the leaderboard (once per finish).
      if (!submitGuard) {
        submitGuard = true;
        const identity = getIdentity();
        if (identity) {
          const timeMs = Math.round((ticks * 1000) / 250);
          void submitTime(identity.token, currentMap, timeMs, Array.from(splits));
        }
      }
    } else if (running) {
      const cpNote = totalCp > 0 ? `  (${splits.length}/${totalCp} cp)` : "";
      timerEl.textContent = `${fmt(ticks)}${cpNote}`;
      timerEl.style.color = "#fa00ff";
    } else {
      timerEl.textContent = "";
    }
  }

  // Render.
  if (renderer && sessionId !== null) {
    const aspect = canvas.width / canvas.height;
    // Quake FOV: the cvar is the HORIZONTAL fov; the vertical fov is derived
    // from the aspect ratio (tan(hfov/2) / aspect). Same model as Warfork/q3.
    const hfov = (fov * Math.PI) / 180;
    const vfov = 2 * Math.atan(Math.tan(hfov / 2) / aspect);
    const proj = perspective(vfov, aspect, 1, 200000);
    const eye = core.session_eye(sessionId) as unknown as Float32Array;
    const angles = core.session_angles(sessionId) as unknown as Float32Array;
    const view = lookAt([eye[0], eye[1], eye[2]], angles[0], angles[1]);
    renderer.draw(multiply(proj, view));
  }

  requestAnimationFrame(loop);
}

void main();
