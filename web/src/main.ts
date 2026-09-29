// Entrypoint: load WASM core, load a map, and run a first-person
// Warfork-style movement controller (pointer lock + WASD + jump/dash).
//
// Milestone 2: playable movement. Multiplayer + weapons wire in later.

import { Renderer, perspective, lookAt, multiply } from "./render/renderer";
import { fetchBsp } from "./sim/map";
import { loadTexture } from "./render/textures";
import init, * as core from "../pkg/webrace_core.js";

const overlay = document.getElementById("overlay")!;
const statusEl = document.getElementById("status")!;
const fpsEl = document.getElementById("fps")!;
const speedEl = document.getElementById("speed")!;
const mapInput = document.getElementById("map") as HTMLInputElement;
const playBtn = document.getElementById("play")!;
const logEl = document.getElementById("log")!;
const lockEl = document.getElementById("lock")!;
// Show the unlock state immediately on load (pointerlockchange doesn't fire
// until a lock attempt happens, so we initialize the label ourselves).
lockEl.textContent = "🔓 click to lock mouse";
lockEl.style.color = "#e8603a";
const canvas = document.getElementById("view") as HTMLCanvasElement;

let renderer: Renderer | null = null;
let memory: WebAssembly.Memory | null = null;
let sessionId: number | null = null;

// Key state.
const keys = { forward: false, back: false, left: false, right: false, jump: false, crouch: false, special: false };

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

async function loadMap() {
  const name = mapInput.value.trim();
  if (!name || !memory) return;
  setStatus(`loading ${name}…`);
  try {
    const url = `/maps/${name}.bsp`;
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

// Also allow clicking anywhere (HUD elements are pointer-events:none).
document.addEventListener("click", () => {
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

const KEYMAP: Record<string, keyof typeof keys> = {
  KeyW: "forward", ArrowUp: "forward",
  KeyS: "back", ArrowDown: "back",
  KeyA: "left", ArrowLeft: "left",
  KeyD: "right", ArrowRight: "right",
  ShiftLeft: "crouch", ControlLeft: "crouch",
  // Space = dash / wall-jump (`+special`), matching your Warfork config
  // (`bind SPACE "+special"`, `bind MOUSE2 "+moveup"`). Jump = right-click.
  Space: "special",
};

window.addEventListener("keydown", (e) => {
  const k = KEYMAP[e.code];
  if (k && !e.repeat) keys[k] = true;
  if (e.code === "Space") e.preventDefault();

  // 4 = restart the race (reset to spawn).
  if (e.code === "Digit4" && !e.repeat) {
    if (sessionId !== null) core.session_reset(sessionId);
  }
});
window.addEventListener("keyup", (e) => {
  const k = KEYMAP[e.code];
  if (k) keys[k] = false;
});

// Mouse buttons: right-click = jump (Warfork default), left = attack (later).
document.addEventListener("mousedown", (e) => {
  if (e.button === 2) keys.jump = true;
});
document.addEventListener("mouseup", (e) => {
  if (e.button === 2) keys.jump = false;
});
document.addEventListener("contextmenu", (e) => e.preventDefault());

// Push held keys into the sim each frame before stepping.
function pushKeys() {
  if (sessionId === null) return;
  core.session_set_keys(
    sessionId,
    keys.forward, keys.back, keys.left, keys.right,
    keys.jump, keys.crouch, keys.special, false,
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
  if (sessionId !== null) {
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
  }

  // Render.
  if (renderer && sessionId !== null) {
    const aspect = canvas.width / canvas.height;
    // Quake FOV: the cvar is the HORIZONTAL fov; the vertical fov is derived
    // from the aspect ratio (tan(hfov/2) / aspect). Same model as Warfork/q3.
    const HFOV_DEG = 140;
    const hfov = (HFOV_DEG * Math.PI) / 180;
    const vfov = 2 * Math.atan(Math.tan(hfov / 2) / aspect);
    const proj = perspective(vfov, aspect, 8, 200000);
    const eye = core.session_eye(sessionId) as unknown as Float32Array;
    const angles = core.session_angles(sessionId) as unknown as Float32Array;
    const view = lookAt([eye[0], eye[1], eye[2]], angles[0], angles[1]);
    renderer.draw(multiply(proj, view));
  }

  requestAnimationFrame(loop);
}

void main();
