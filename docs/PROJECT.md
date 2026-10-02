# webrace — project overview

> This file is the **master orientation document**. Read this first when
> starting work on this project. For the full history of decisions/gotchas see
> `docs/JOURNAL.md`. For deployment topology see `docs/deploy.md`.

## What this is

A **browser-native first-person arena movement game** with Warfork/Warsow-faithful
physics (strafe-jumping, dash, wall-jump, bunny-hopping, slick/ice surfaces,
ramps). Loads native Quake3/QFusion `.bsp` maps (the full defrag / Warfork /
Warsow catalog via [padpork.org](https://padpork.org)), with race timers,
checkpoints, leaderboards, nicknames, map voting, and Quake-compatible input.

Live demo: **https://ip.mikul.se/webrace/**

## Core architecture (the big idea)

The **entire deterministic game simulation runs in Rust compiled to WebAssembly**.
The browser client only:
1. Sends inputs (buttons + view angles) into the WASM sim.
2. Steps the sim at a fixed **250 Hz**.
3. Pulls back the resulting player state (position, velocity, angles).
4. Renders the BSP world + the player camera.

This is intentionally the same shape as the reference game this project was
inspired by (dinoblast.net), because it's the clean way to hit the >360fps
target and to get deterministic replays/ghosts for free.

### Stack

| Layer | Tech |
|---|---|
| Client shell / tooling | Vite + TypeScript (`web/`) |
| Simulation + collision + BSP parsing | Rust → WebAssembly (`core/`, `wasm32-unknown-unknown`) |
| Renderer | Raw WebGL2 (no three.js) |
| Maps | Native QFusion/Quake3 `IBSP` v46 parser + pk3 texture extraction |
| Map + leaderboard backend | Node (`server/map-server.mjs`, `server/leaderboard.mjs`) |

### Directory map

```
core/                 Rust crate -> WASM (the sim: pmove, trace, bsp, input)
  src/
    pmove.rs          Warfork movement physics port
    trace.rs          AABB hull trace vs BSP brushes (exact pmove collision)
    bsp.rs            QFusion IBSP parser (render mesh + collision + race gates)
    input.rs          Quake m_yaw/m_pitch (0.022) angle handling
    sim.rs            Session: player + world + race state
    session.rs        wasm-bindgen surface (JS -> Rust)
    bindings.rs       wasm-bindgen surface for BSP data (render/collision)
  tests/              Rust integration tests

web/                  Vite + TypeScript client
  src/
    main.ts           entrypoint: input loop, HUD, loadMap
    render/           WebGL2 renderer, textures (incl. TGA decoder), lightmaps
    ui/               menu (settings/leaderboard/maps/favorites), movement HUD
    net/              leaderboard client
    sim/              map fetch helpers
    binds.ts          key/mouse bind system (remappable)
    settings.ts       persisted settings (localStorage + share code)
    maps.ts           padpork catalog, search, random-vote, favorites
    base.ts           base-path helper (for /webrace/ subdir deploy)

server/
    map-server.mjs    serves /maps/*.bsp, /tex/*, /catalog (local + padpork)
    leaderboard.mjs   nickname + times (node:sqlite)

scripts/
    build-wasm.sh     rustc -> wasm32 -> wasm-bindgen -> web/pkg/
    deploy.sh         deploy to TrueNAS server

deploy/
    Caddyfile         reference copy of the live Caddy reverse-proxy config

docs/                 ADRs + this overview + journal + deploy notes
```

## How to run (dev)

From the repo root:
```bash
npm run wasm          # build Rust -> WASM core (needs Rust + wasm32 target)
npm run maps          # map server on :4173
npm run leaderboard   # leaderboard server on :4174
npm run dev           # vite dev server on :5173
```

Open http://127.0.0.1:5173.

Testing:
```bash
npm test              # cargo test (the Rust sim integration tests)
```

## Key movement constants (verified against Warfork source)

```
GRAVITY 850, BASEGRAVITY 800, GRAVITY_COMPENSATE 1.0625
pm_accelerate 12, pm_airaccelerate 1, pm_friction 8, pm_aircontrol 150
pm_wishspeed 30, pm_strafebunnyaccel 70
walk/crouch 160, run 320, dash 450
jump 280 (DEFAULT_JUMPSPEED), dash-upspeed 174, walljump-upspeed 330
walljump bounce 0.3, overbounce 1.01
forward-bunny: airforwardaccel 1.00001, bunnyaccel 0.1593, bunnytopspeed 925
```

## Known state / what's next

See the "Current state" section in `docs/JOURNAL.md` — it lists what works,
what's stubbed, and the ordered TODO list (bezier patch tessellation is the
most-wanted next fix: it's why some curved maps have missing geometry).
