// Entrypoint: load WASM core, load a map, render it with a free camera.
// Milestone 1 (scaffold + BSP renderer). Movement + multiplayer wire in next.

import { Renderer, perspective, lookAt, multiply } from "./render/renderer";
import { fetchBsp } from "./sim/map";
import init, * as core from "../pkg/webrace_core.js";

const overlay = document.getElementById("overlay")!;
const statusEl = document.getElementById("status")!;
const fpsEl = document.getElementById("fps")!;
const mapInput = document.getElementById("map") as HTMLInputElement;
const playBtn = document.getElementById("play")!;
const logEl = document.getElementById("log")!;
const canvas = document.getElementById("view") as HTMLCanvasElement;

let renderer: Renderer | null = null;
let memory: WebAssembly.Memory | null = null;

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
    const id = core.bsp_parse(name, bytes);

    const vertCount = core.bsp_vertex_count(id);
    const idxCount = core.bsp_index_count(id);
    const triCount = core.bsp_triangle_count(id);
    const brushCount = core.bsp_brush_count(id);

    if (!renderer) {
      renderer = new Renderer(canvas);
      setupControls();
      requestAnimationFrame((t) => loop(t));
    }

    renderer.uploadMap(
      memory,
      core.bsp_vertices_ptr(id),
      vertCount,
      core.bsp_indices_ptr(id),
      idxCount,
    );

    overlay.classList.add("hidden");
    setStatus("");
    log(`${name}: ${triCount} tris, ${idxCount} indices, ${brushCount} brushes`);
  } catch (e) {
    setStatus(`load failed: ${(e as Error).message}`);
    log((e as Error).stack || String(e));
  }
}

// Free-fly camera for the scaffold milestone.
const cam = { pos: [0, 0, 128] as [number, number, number], yaw: 0, pitch: 0 };
let dragging = false;
let lastX = 0;
let lastY = 0;

function setupControls() {
  const update = () => {
    if (!renderer) return;
    canvas.width = canvas.clientWidth;
    canvas.height = canvas.clientHeight;
    renderer.resize(canvas.width, canvas.height);
  };
  window.addEventListener("resize", update);
  update();
  canvas.addEventListener("mousedown", (e) => {
    dragging = true;
    lastX = e.clientX;
    lastY = e.clientY;
  });
  window.addEventListener("mouseup", () => (dragging = false));
  window.addEventListener("mousemove", (e) => {
    if (!dragging) return;
    cam.yaw += (e.clientX - lastX) * 0.003;
    cam.pitch += (e.clientY - lastY) * 0.003;
    cam.pitch = Math.max(-1.5, Math.min(1.5, cam.pitch));
    lastX = e.clientX;
    lastY = e.clientY;
  });
  window.addEventListener("wheel", (e) => {
    const f = Math.cos(cam.yaw);
    const s = Math.sin(cam.yaw);
    cam.pos[0] += f * e.deltaY * 0.1;
    cam.pos[1] += s * e.deltaY * 0.1;
  });
}

let frame = 0;
let lastFpsTime = performance.now();
function loop(_t: number) {
  frame++;
  const now = performance.now();
  if (now - lastFpsTime > 500) {
    const dt = (now - lastFpsTime) / 1000;
    fpsEl.textContent = `${Math.round(frame / dt)} fps`;
    lastFpsTime = now;
    frame = 0;
  }
  if (renderer) {
    const aspect = canvas.width / canvas.height;
    const proj = perspective((75 * Math.PI) / 180, aspect, 8, 200000);
    const view = lookAt(cam.pos, cam.yaw, cam.pitch);
    renderer.draw(multiply(proj, view));
  }
  requestAnimationFrame(loop);
}

void main();
