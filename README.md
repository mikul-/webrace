# webrace

A browser-native arena movement game with **Warfork/Warsow-faithful physics**:
strafe-jumping, dash, wall-jump, overbounce, slick (ice) surfaces, and ramps.
Loads native Q3/QFusion `.bsp` maps (the full defrag / Warfork / Warsow
catalog), with race timers, leaderboards, nicknames, map voting, and
Quake-compatible sensitivity/binds.

## Goals (locked in)

- Movement/physics: port of Warfork `gs_pmove.cpp` (strafe-jump, dash,
  wall-jump, double-jump, crouch, slick, overbounce).
- Maps: load native `IBSP` v46 `.bsp` files in the browser (no pre-processing).
  Served from a local Warfork install first, with a fallback to
  [padpork.org](https://padpork.org) (the Warsow/Warfork race archive, ~4700
  maps) for anything not installed locally.
- Weapons (Warfork set): Rocket Launcher, Plasmagun, Grenade Launcher,
  Lasergun (and more to follow).
- Sensitivity: Quake/Source compatible — `m_yaw = 0.022`, `m_pitch = 0.022`,
  `sensitivity` multiplier, accel off by default.
- Multiplayer: WebTransport (QUIC/UDP) with WebSocket fallback (planned).
- Tick rate: **250 Hz** deterministic simulation.
- Performance target: >360 fps sustained.

## Stack

| Layer | Tech |
|---|---|
| App shell / tooling | Vite + TypeScript |
| Simulation + collision | Rust → WebAssembly (`wasm32-unknown-unknown`) |
| Renderer | Raw WebGL2 (no three.js) |
| Maps | Native QFusion BSP parser (in WASM) + padpork archive |
| Backend | Node (leaderboards via built-in `node:sqlite`) |

## Architecture

```
web/                        # Vite + TypeScript client
  src/
    main.ts                 # entrypoint, input loop, HUD
    render/                 # WebGL2 renderer (world, textures, lightmaps)
    ui/                     # menu (settings / leaderboard / maps / favorites)
    net/                    # leaderboard client
    sim/                    # map loading glue
    binds.ts, settings.ts, maps.ts
  index.html

core/                       # Rust → WASM simulation core
  src/
    pmove.rs                # Warfork movement port
    trace.rs                # AABB hull trace vs BSP brushes
    bsp.rs                  # QFusion IBSP parser (render + collision + race gates)
    input.rs                # Quake m_yaw/m_pitch angle handling
    sim.rs, session.rs      # session + race state + wasm-bindgen surface
    bindings.rs, lib.rs

server/
    map-server.mjs          # serves /maps, /tex, /catalog (local + padpork)
    leaderboard.mjs         # nicknames + race times (node:sqlite)

scripts/
    build-wasm.sh           # rustc -> wasm32 -> wasm-bindgen

docs/                       # ADRs and design notes
```

## Setup

### Prerequisites

- **Node.js ≥ 22** (uses built-in `node:sqlite` and `fetch`).
- **Rust** toolchain with the `wasm32-unknown-unknown` target:
  ```bash
  rustup target add wasm32-unknown-unknown
  # or, on Arch: sudo pacman -S rust-wasm
  ```
- **wasm-bindgen CLI**:
  ```bash
  cargo install wasm-bindgen-cli
  ```
- **Optional:** a local Warfork/Warsow install so maps resolve offline. Without
  it, maps are downloaded on demand from [padpork.org](https://padpork.org)
  (requires internet).

### Install & build

```bash
git clone https://github.com/mikul-/webrace
cd webrace
npm install --prefix web    # or: cd web && npm install
npm run wasm                # builds core/ -> web/pkg/ (Rust -> WASM)
```

### Run (three terminals, from the repo root)

```bash
npm run maps          # map server         -> http://127.0.0.1:4173
npm run leaderboard   # leaderboard server -> http://127.0.0.1:4174
npm run dev           # vite dev server    -> http://127.0.0.1:5173
```

Open **http://127.0.0.1:5173** and press Play. Use the menu (`Esc`) to change
settings, browse maps, vote (`random slick`, `random rocket`, …), and view
leaderboards / favorites.

### Providing your own maps (offline / local catalog)

The map server resolves `<name>.bsp` by scanning `.pk3` files under a Warfork
data directory. The default path is Linux XDG (`~/.local/share/warfork-2.1`),
but you can pass any directory:

```bash
npm run maps -- /path/to/warfork-data
```

Standard Warfork game data directories by OS (from the Warfork wiki):

| OS | Data directory |
|---|---|
| Linux | `~/.local/share/warfork-2.1` (also `/usr/share/warfork-2.1`) |
| Windows | `%USERPROFILE%\My Documents\My Games\Warfork 2.1` |
| macOS | `~/Library/Application Support/Warfork-2.1` |

Any folder containing `basewf/*.pk3` (and `downloads/**/*.pk3`) works. The
server scans `basewf`, `downloads/racemod_2.1`, and `downloads__/racemod_2.1`
subdirectories. Maps not found locally fall back to padpork.

### Tests

```bash
npm test                 # runs the Rust test suite (cargo test)
```

## Controls (defaults)

| Input | Action |
|---|---|
| WASD | move |
| Space | dash / wall-jump |
| Right click | jump |
| Shift / Ctrl | crouch |
| Mouse 3 | save position (in the start zone) |
| 4 | restart race |
| Esc | menu |

All binds are remappable in Settings → Key binds.

## Key constants (verified against Warfork source)

```
GRAVITY = 850, BASEGRAVITY = 800, GRAVITY_COMPENSATE = 1.0625
pm_accelerate = 12, pm_airaccelerate = 1, pm_airdecelerate = 2
pm_strafebunnyaccel = 70, pm_aircontrol = 150, pm_friction = 8
pm_wishspeed = 30
Walk / crouch = 160, run = 320, dash = 450
pm_dashupspeed = 174 (compensated), pm_wjupspeed = 330 (compensated)
pm_wjbouncefactor = 0.3, PM_OVERBOUNCE = 1.01
Forward-bunny: airforwardaccel = 1.00001, bunnyaccel = 0.1593,
                bunnytopspeed = 925, turnaccel = 4, backtosideratio = 0.8
```

## BSP format (QFusion `IBSP`, verified against wf-tool/bsp2mesh.py)

- `dvertex_t` = 44 bytes, `dface_t` = 104-byte stride (`"<11i"` header),
  `dshaderref_t` = 72 bytes.
- 18 lumps (`HEADER_LUMPS`), lightmaps up to `QF_LIGHTMAP_SIZE` 512×512×3.
- Coordinates used in Quake's native Z-up space (identity transform).
- Maps live in `maps/<name>.bsp` inside `.pk3` archives.

## Development

See `docs/` for architecture decisions and the run guide.
