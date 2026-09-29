# webrace

A browser-native arena movement game with **exact Warfork/Warsow physics**:
strafe-jumping, dash, wall-jump, overbounce. Loads native Q3/QFusion `.bsp`
maps (the full defrag / Warfork / Warsow catalog), with leaderboards,
nicknames, and Quake-compatible sensitivity/binds.

## Goals (locked in)

- Movement/physics: exact port of Warfork `gs_pmove.cpp` (strafe-jump, dash,
  wall-jump, double-jump, crouch-slide, overbounce).
- Maps: load native `IBSP` QFusion `.bsp` files in the browser (no
  pre-processing). Target catalog = the 28 maps in `~/demos/maps.json`.
- Weapons (Warfork set): Rocket Launcher, Plasmagun, Grenade Launcher,
  Lasergun (and more to follow).
- Sensitivity: Quake/Source compatible — `m_yaw = 0.022`, `m_pitch = 0.022`,
  `sensitivity` multiplier, accel off by default.
- Multiplayer: WebTransport (QUIC/UDP) with WebSocket fallback.
- Tick rate: **250 Hz** simulation.
- Performance target: >360 fps sustained.

## Stack

| Layer | Tech |
|---|---|
| App shell / tooling | Vite + TypeScript |
| Simulation + collision + netcode core | Rust → WebAssembly (`wasm32-unknown-unknown`) |
| Renderer | Raw WebGL2 (no three.js) |
| Maps | Native QFusion BSP parser (in WASM) |
| Multiplayer | WebTransport + WebSocket fallback |
| Backend | Node + Postgres (leaderboards / nicknames / map host) |

## Architecture

```
web/                        # Vite + TypeScript client
  src/
    main.ts                 # entrypoint
    render/                 # WebGL2 renderer (world, players, projectiles)
    ui/                     # HUD, menu, console
    net/                    # WebTransport / WebSocket client
    sim/                    # JS-side glue to the WASM core
  index.html

core/                       # Rust → WASM simulation core
  src/
    pmove.rs                # Warfork movement port (PM_Accelerate, air, dash, wj)
    trace.rs                # AABB hull trace vs BSP brushes (exact pmove)
    bsp.rs                  # QFusion IBSP parser (render + collision)
    weapons.rs              # rocket / plasma / grenade / laser
    netcode.rs              # input snapshots, deterministic tick
    lib.rs

server/                     # Node backend (leaderboards, nicknames, maps)
tools/                      # map catalog sync from ~/demos/maps.json + ws.q3df.org
docs/                       # ADRs and design notes
```

## Key constants (verified against Warfork source)

```
GRAVITY = 850, BASEGRAVITY = 800, GRAVITY_COMPENSATE = 1.0625
pm_accelerate = 12, pm_airaccelerate = 1, pm_airdecelerate = 2
pm_strafebunnyaccel = 70, pm_aircontrol = 150, pm_friction = 8
pm_wishspeed = 30
DEFAULT_WALKSPEED = 160, DEFAULT_CROUCHEDSPEED = 100
pm_dashupspeed = 174 (compensated), pm_wjupspeed = 330 (compensated)
pm_wjbouncefactor = 0.3, PM_OVERBOUNCE = 1.01
PM_DASHJUMP_TIMEDELAY = 1000 ms, PM_WALLJUMP_TIMEDELAY = 1300 ms
PM_WALLJUMP_FAILED_TIMEDELAY = 700 ms
Forward-bunny: airforwardaccel = 1.00001, bunnyaccel = 0.1593,
                bunnytopspeed = 925, turnaccel = 4, backtosideratio = 0.8
```

## BSP format (QFusion `IBSP`, verified against wf-tool/bsp2mesh.py)

- `dvertex_t` = 44 bytes, `dface_t` = 44 bytes (`"<11i"`), `dshaderref_t` = 72 bytes.
- 18 lumps (`HEADER_LUMPS`), lightmaps up to `QF_LIGHTMAP_SIZE` 512×512×3.
- Coordinate transform to three.js/render space: `q2t = (x, z, -y)`.
- Maps live in `basewf/*.pk3` as `maps/<name>.bsp`.

## Development

See `docs/` for architecture decisions. Requires the `wasm32-unknown-unknown`
Rust target (install with `rustup target add wasm32-unknown-unknown` or the
distro `rust-wasm` package).
