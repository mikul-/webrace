# webrace — development journal

Chronological record of what was built, key decisions, and (especially) the
gotchas and subtle bugs that cost time to find. Written so a fresh session can
reconstruct context without re-deriving it.

## Current state (read this first)

### Working / done
- **Movement physics** (Rust/WASM, ported from Warfork `gs_pmove.cpp`): strafe-jump,
  air-accel, forward-bunny, dash, wall-jump, jump, crouch, slick (ice) surfaces,
  ramps (uphill + downhill), stair-stepping, wall-slide (multi-pass `PM_SlideMove`).
- **Collision**: AABB hull trace against BSP brushes (exact pmove semantics), with
  fixes for start-solid boundary cases, wall wedging, and inside-corner (>90°)
  sticking.
- **BSP loading**: native QFusion/Quake3 `IBSP` v46 — geometry, textures, lightmaps,
  collision brushes, spawn points, and **race gates** (start/checkpoint/finish).
- **Bezier patches** (`FACETYPE_PATCH`) — control-point grids tessellated into
  8×8 triangle meshes via Bernstein blending (position + tex + lightmap + normal +
  color), fixing the curved geometry (arches/pipes/pillars) that planar-only parsing
  skipped.
- **Physical contents** (LUMP_SHADERREFS `contents`): water/lava/slime swimming
  (Q3 `PM_WaterMove` + waterlevel 0–3 detection via point-contents), jumppads
  (`trigger_push` ballistic launch), teleporters (`trigger_teleport` → target
  origin). Non-solid liquid brushes are skipped for collision.
- **Rendering**: raw WebGL2, per-shader draw chunks, texture + lightmap sampling,
  TGA decoder for `.tga` textures.
- **Race mode**: timer, checkpoints/splits, position-save (Mouse 3, behind start
  line, saves yaw+pitch), restart (key 4).
- **Nicknames + leaderboard**: node:sqlite, `POST /api/nickname`, `POST /api/times`,
  `GET /api/leaderboard`, `GET /api/personal`.
- **Map catalog/voting**: padpork.org proxied via `/catalog` (4744 maps), search,
  `random`, `random <keyword>` (slick/rocket/plasma/grenade/teleporter/jumppad/water),
  favorites, clickable vote chips.
- **Settings menu** (`Esc`): FOV (125 default), sensitivity (2.0 default), crosshair
  color/size, full key bind editing, config share-code (import/export).
- **Movement HUD**: acceleration bar (green gain/red loss) + strafe-jump triangle +
  bunny-hop marker (both sweet-spot indicators that shrink with speed).
- **Live deployment** on TrueNAS (see `docs/deploy.md`).

### Stubbed / not yet built
- **Shaders** (`.shader` scripts) — not parsed at all. No animated textures, emissive
  glows, multi-stage materials, or skybox.
- **Lightgrid** (`LUMP_LIGHTGRID`) — only static vertex color; no dynamic lightgrid
  sampling for entities.
- **Visibility** (PVS/`LUMP_VISIBILITY`) — everything rendered every frame.
- **Weapons** (rocket/plasma/grenade/lightning) — `attack` (Mouse0) is bound but no
  weapon logic.
- **Ghosts/replays**, **true multiplayer** (WebTransport), **practice mode**
  (noclip, position-save-anywhere).

## Ordered TODO (recommended next steps)
1. ~~Bezier patch tessellation~~ (done — see below).
2. Shader parsing (sky + animated/emissive textures).
3. ~~Physical `contents` (water/lava/jumppad/teleporter)~~ (done — see below).
4. Ghosts/replays (deterministic sim makes this nearly free).
5. Weapons.
6. Lightgrid + PVS (perf).

---

## History / decisions / gotchas

### Bezier patch tessellation
- `FACETYPE_PATCH` faces carry a control-point grid (dimensions in the last two
  `dface_t` fields `patch_cp[2]` at byte offsets 96/100); `numverts == cp_w*cp_h`
  and the points live at `firstvert..firstvert+numverts` as ordinary `dvertex_t`s.
- Tessellation uses bilinear Bernstein blending of ALL attributes (position, tex,
  lightmap, normal, color), subdivided 8×8 by default. The blended normal is
  re-normalized (blending shrinks it). Emitted into the same shader-grouped
  triangle soup, so patches draw as one chunk like planar faces.
- Patch triangulation winds `[i0,i1,i2, i1,i3,i2]` per cell (i0=top-left in a
  (px,py) grid) to match the planar face winding (culling is currently off).

### Physical contents (LUMP_SHADERREFS + entity triggers)
- `dshaderref_t` is `name[64] + flags(i32) + contents(i32)` (72 bytes); the
  `contents` word at offset 68 was previously unread. It's what classifies
  water/lava/slime (and `CONTENTS_JUMPPAD`-style surfaces).
- **Two data sources** feed trigger semantics, not one: (1) per-shader `contents`
  for liquid volumes, and (2) entity blocks `trigger_push`/`trigger_teleport`/
  `trigger_hurt` that reference a submodel (`model "*N"`) whose AABB bounds the
  trigger, plus a `target` entity (`target_position`/`info_notnull`/`misc_teleporter_dest`).
- **World-brush contents decode** (verified on real maps): solid = `1`
  (CONTENTS_SOLID), playerclip = `0x20010000` (PLAYERCLIP|TRANSLUCENT), liquid-
  surface brushes = `0x20000000` (TRANSLUCENT only, non-solid). So
  `is_solid_contents = contents & (SOLID|PLAYERCLIP) != 0 && contents & MASK_WATER == 0`.
- **Jumppad velocity** is precomputed from the target apex by `trigger_push_setup`:
  `time = sqrt(height/(0.5*g))`, horizontal speed = `dist/time`, `vz = time*g`.
- Tests: the `World` literal now carries `brush_contents` parallel to the brush
  arrays — keep the length in sync with `brush_plane_count` or collision silently
  drops brushes (a one-element-too-short `brush_contents` cost a wall-dash test).

### Forward-jump speed bug
  Warfork lands ~350:
  1. `aircontrol` ported C `VectorNormalize` (which normalizes *in place* and
     returns the length) wrong: it multiplied the *full* velocity by `speed`
     (`320*320`) instead of a unit direction — instant ~320 boost on the first
     airborne tick.
  2. The bigger one: `max_player_speed` was set to **600**, but Warfork's
     `maxPlayerSpeed` is **320** (`DEFAULT_PLAYERSPEED` = 320 for race/instagib/
     standard — there is no separate 600 "air-bunny" reference speed). The
     forward-bunny `PM_AirAccelerate` uses `curspeed.max(maxPlayerSpeed)` as its
     target, so with 600 it ramps every forward jump toward 600+.
- Fix: normalize before `aircontrol`, and set `max_player_speed = 320.0`.
  Result: forward jump lands at ~354 (320 → +34 over one arc), matching Warfork.
  Strafe-jump still climbs past 320 via the air-accel sweet spot.
- Regression test `forward_jump.rs` asserts a forward jump lands < 400 ups.

### Wall clipping / stairs / ledges (Q3 `PM_StepSlideMove` + `PM_SlideMove`)
- The player could "barely walk up" stacked ledges/steps. Compared our code to
  Warfork `gs_pmove.c` and Q3 `bg_pmove.c`/`bg_slidemove.c`:
  - **Q3 (the best)** `PM_SlideMove` pre-seeds `planes[0] = groundTrace.plane.normal`
    (and a normalized-velocity plane) so the box never turns against the ground,
    and `PM_WalkMove` clips velocity to the ground plane then **re-normalizes**
    (`VectorNormalize` + `VectorScale` back to the saved magnitude) so speed is
    preserved while gliding up steps. `PM_StepSlideMove` also has a "**never step
    up while still rising**" guard (down-trace + up-velocity check) that makes
    ledge traversal clean instead of auto-climbing mid-jump.
  - Warfork shares the step-up but lacks the re-normalize-on-ground-clip and the
    rising guard, and its SlideMove starts cold (`numplanes=0`) — so it catches
    toes on steps too.
  - Ours additionally had fewer clip planes and a hardcoded epsilon nudge instead
    of Q3's "re-add the plane normal when the same plane recurs".
- Implemented the Q3 model: `slide_clip` = `PM_SlideMove` (ground-plane pre-seed,
  `OVERCLIP` 1.001, repeated-plane nudge, crease slide), `slide_move` =
  `PM_StepSlideMove` (slide → step up by actual `stepSize` → slide → trace down →
  clip, with the never-step-while-rising guard), and `ground_move`/`step` now clip
  velocity to the ground normal then re-normalize (speed preserved).
- **Latent bug found**: `trace.rs` hardcoded `all_solid = true` (never updated).
  The old slide code never read it, but the faithful `PM_SlideMove` does
  (`if trace.allsolid` returns early), so every move was killed. Fixed to `false`
  (our trace returns on `start_solid` rather than probing an exit, so the two
  coincide).
- Regression test `stairs.rs`: the player climbs three 8-unit steps (z 24 → 48)
  and descends the far side while running, no jumping.

### Jumppad/teleporter trigger geometry fix
- Trigger volumes were first detected by their **submodel AABB**, which is wrong
  for **diagonal/slanted** jumppads (a "ramp" pad): the AABB is the bounding box
  of a wedge, so the player standing at the bottom of the ramp is outside the
  AABB and never triggers, while the top of the ramp over-triggers.
- Fix: `parse_triggers` now extracts each trigger submodel's **actual brush
  planes** (via `brush_planes_into`, reading BRUSHES/BRUSHSIDES/PLANES) into a
  shared `trigger_plane_ids`, and `jumppad_at`/`teleporter_at` test the player
  AABB against the convex brush using the same slab method as `trace`
  (`box_intersects_brush`). Handles axis-aligned and diagonal pads alike.
- Gotcha: `drop_to_ground` on a *diagonal ramp pad* slides the player down the
  ramp (off the trigger) — that's correct collision; such pads are meant to be
  run *up*, not landed on from above. Flat pads (pornstar, coldrun sjp1/sjp2)
  trigger on contact.

### Scaffold & BSP (start)
- Started with Vite+TS + Rust→WASM, validated the BSP parser against **real Warfork
  maps** (extracted `.bsp` from local `~/.local/share/warfork-2.1/**/*.pk3`).
- **Critical BSP fact**: QFusion `IBSP` is version **46**, `dface_t` stride **104
  bytes** (header fields = first 11 ints = `"<11i"`), `dvertex_t` 44 bytes, plane
  coords are **native Z-up** (we removed an early `q2t=(x,z,-y)` three.js transform
  that caused "maps loaded laying down"). See `docs/adr-0001-wasm-core.md`.

### Movement (the big port)
- Ported `PM_Accelerate`/`PM_AirAccelerate`/`PM_Aircontrol`, dash, wall-jump. Key
  fix: **input must be -127..127** (`PM_CmdScale`), not -1..1 — ground run was stuck
  at 1 ups until this was fixed.
- Strafe-jump routing: forward+strafe → `PM_AIRACCELERATE=1`; forward-only →
  forward-bunny; pure-strafe → `PM_STRANGE_BUNNY_ACCEL=70`.
- Wall-slide: reimplemented as Quake's multi-pass `PM_SlideMove` (clip against up to
  5 planes, re-tracing); this fixed "stuck on every wall".
- Wall-wedge bug: the trace's `start_solid` was returning `normal=[0,0,1]` (up)
  instead of the real deepest-normal, so recovery pushed "up" uselessly. Fixed by
  returning `best_normal` and scanning all 6 axes in `resolve_solid`.

### Trace/collision subtleties (took the most time)
- Slab-method boundary bugs: "both inside" vs "touching" (`start_dist == 0`) vs
  "start_solid" (`enter < 0`) — each had to be distinguished carefully.
- `grounded()` false-positives: originally a 2-unit down-trace; needed `normal.z`
  walkability threshold AND "never grounded while rising (`vz > 20`)" to stop
  double-jump ("every jump was 2x") and wall-float.
- Steep slick ramps: `grounded()` used a hard 45° cap (`normal.z > 0.7`) but defrag
  ramps are 45-80°. Changed slick surfaces to allow down to `normal.z > 0.2`.
- Slick accelerate: briefly changed slick ground to `airaccelerate=1` (matching
  Q3 `SURF_SLICK`), but this *killed* slick acceleration — **reverted** to
  `pm_accelerate=12`.

### Input / settings
- Sensitivity uses Quake model: `m_yaw=m_pitch=0.022`, 16-bit angle circle
  (65536). A `DEG_TO_ANGLE` conversion bug made mouse feel ~0.1x ("0.1 sens").
- Key bind system (`binds.ts`): named actions, remappable in menu, `Esc`=menu
  (was `M`, but `M` collided with typing nicknames), Space=special (dash), jump=
  right-click, restart=4, position-save=Mouse3.

### Race / leaderboard
- Race gates parsed from entities: `trigger_multiple` + `model *N` (submodel AABB)
  → `target_startTimer`/`target_checkpoint`/`target_stopTimer`.
- Position-save restricted to *behind* the start line (start gate's thin axis vs
  spawn side).

### Maps/voting
- padpork API: `GET /api/maps` (paginated, `limit` max 100), `GET /api/maps/<name>`
  (metadata), `GET /api/maps/<name>/download` (the **.pk3** zip). CORS blocks the
  browser, so we proxy `/catalog` through the map-server.
- Map server: local Warfork pk3s first, padpork `.pk3` download + cache fallback;
  textures also resolved from the current map's pk3.

### Deployment (TrueNAS)
- Server = TrueNAS SCALE at `192.168.0.107` (NOT `.7`). No Node/Rust on host, so
  webrace runs in a node:22 Docker container; Caddy (a separate container) serves
  static + reverse-proxies `/webrace/*`.
- Client had to become **base-path aware** for `/webrace/` subdir (Vite `base` +
  `api()` helper).
- A stray committed **`webrace.zip` (192MB)** blocked the GitHub push; purged via
  `git filter-branch` (all hashes rewritten — the current SHAs are post-rewrite).
- `docker compose` reverse_proxy targets initially used `127.0.0.1` (wrong, cross-
  container); fixed to `192.168.0.107`.

## Conventions
- Rust tests live in `core/tests/*.rs`; run via `npm test` (or `cargo test`).
- Movement constants live in `core/src/lib.rs`; never hardcode them in `pmove.rs`.
- Client fetches use `api("/path")` from `web/src/base.ts` (never a bare `/path`).
- New WASM-exposed functions go in `core/src/session.rs` (session state) or
  `core/src/bindings.rs` (BSP data).
