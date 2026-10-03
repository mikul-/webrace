# webrace — development journal

Chronological record of what was built, key decisions, and (especially) the
gotchas and subtle bugs that cost time to find. Written so a fresh session can
reconstruct context without re-deriving it.

## Current state (read this first)

### Working / done
- **Movement physics** (Rust/WASM, **faithful port of Warfork `gs_pmove.c`**):
  strafe-jump, air-accel, forward-bunny, dash, wall-jump, jump, crouch, slick
  (ice) surfaces, ramps (up + down), stair-stepping, wall-slide.
  - `PM_SlideMove` + `PM_StepSlideMove` ported verbatim (incl. the ramp slide:
    preserve horizontal speed and `velocity[2] = down_v[2]`).
  - Gravity is **air-only**; `grounded()` uses Warfork `ISWALKABLEPLANE` (0.7)
    and a 0.25-unit ground trace.
  - Constants verified against Warfork: run `maxPlayerSpeed 320`, friction 8,
    air-accel 1, strafe-bunny 70, aircontrol 150, jump 280, dash 450,
    dash-upspeed 174, walljump-upspeed 330.
  - `PM_CheckWallJump` ported verbatim (clip + 0.3 bounce, preserves entry
    hspeed, min 240).
  - Smooth crouch transition (`PM_AdjustBBox`, `CROUCHTIME=100ms`, viewheight
    30→12, head-chomp check).
  - Stair-step **view** smoothing (`PREDICTED_STEP_TIME=150ms`).
- **Collision**: AABB hull trace against BSP brushes (exact pmove semantics), with
  fixes for start-solid boundary cases, wall wedging, inside-corner (>90°)
  sticking, coincident `common/slick` overlays, and **solid brush-model entities**
  (`func_*` submodels).
- **Moving brush entities** (`func_bobbing` / `func_plat` / `func_door` /
  `func_door_rotating` / `func_train` / `func_rotating` / `func_pendulum`):
  parsed with their class keys, animated deterministically in the sim (not the
  renderer), collided at their *current* transform, and they **carry the player**
  (Qfusion `SV_Push` rider handling). Submodel faces are drawn from a separate
  buffer with a per-mover model matrix (via the MODELS lump `firstface`/
  `numfaces`). `func_plat` rises on contact, doors open on proximity, trains
  follow their `path_corner` chain.
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
- **Sound system (first pass)**: a Settings → **Sound** tab with a master volume
  slider and a dropdown per game event (jump, dash, wall dash, land, footstep,
  jumppad, teleport, plus weapon/pickup placeholders). Dropdowns list the drop-in audio files under
  `snd/` (served live by the map-server via `/sounds` + `/snd/<file>`, no
  rebuild), each row has a ▶ preview button, and the assignment persists in
  settings/share-code. The sim emits per-tick event bits (`EV_*` in
  `core/src/lib.rs`) for jump/dash/walljump/land/footstep/jumppad/teleport and
  `main.ts` plays the assigned file. Weapon sounds are assignable now but fire
  once weapons exist.
- **Debug wireframe**: press **P** (rebindable; also `?wire=1`) to toggle a
  green edge-only view of the world + movers. WebGL2 has no polygon mode, so the
  renderer builds a `gl.LINES` index buffer (2× the triangle indices, same order)
  and swaps to a line VAO; skybox/overlays are skipped.
- **Texture filtering**: mipmaps are generated for every world/sky texture and
  the default mode is trilinear + anisotropic (8×) when the extension is
  available. `Settings → Texture filtering` selects Nearest / Bilinear /
  Trilinear / Trilinear+aniso at runtime. The lightmap atlas is excluded (no
  mips, CLAMP) so light doesn't bleed between atlas cells.

### Stubbed / not yet built
- **Shaders** (`.shader` scripts) — skybox, animated (`animmap`), additive
  (emissive) and alpha-blended stages, and `tcMod scroll`. Still missing:
  `tcMod rotate/scale/stretch`, `rgbGen wave`, `alphaGen`, portal/fog stages,
  and shader-only `surfaceparm` overrides for *collision* (rendering honors
  them via `buildPlan`).
- **Lightgrid** (`LUMP_LIGHTGRID`) — only static vertex color; no dynamic lightgrid
  sampling for entities.
- **Visibility** (PVS/`LUMP_VISIBILITY`) — everything rendered every frame.
- **Weapons** (rocket/plasma/grenade/lightning) — `attack` (Mouse0) is bound but no
  weapon logic.
- **Audio** — a settings/assignment + playback pass exists (see above), but the
  weapon/pickup event hooks don't fire yet (placeholders), there's no volume
  control, and no `snd/` sounds are bundled/committed (test files are
  gitignored).
- **Ghosts/replays**, **true multiplayer** (WebTransport), **practice mode**
  (noclip, position-save-anywhere).

## Ordered TODO (recommended next steps)
1. **Paired/one-way teleporters + jumppad/entity models/sprites** (visuals for
   entities; teleporter destination pairing).
2. **Ghosts / replays** — the deterministic sim makes recording/playback nearly free.
3. **Weapons** (rocket/plasma/grenade/lightning).
4. **Audio** — first pass done (drop-in `snd/` menu + jump/dash/walljump/land/
   footstep/jumppad/teleport). Remaining: volume control, weapon/pickup hooks
   (blocked on weapons), and bundling curated sounds.
5. **Perf**: PVS/`LUMP_VISIBILITY` culling, a collision BVH, lightgrid.
6. **Multiplayer** (WebTransport) + **practice mode** (noclip, save-anywhere).

## DONE — moving brush entities (`func_bobbing`/`func_plat`/`func_door`/`func_train`)

Implemented. Summary / where it lives:
- **Parse** (`core/src/bsp.rs`): `parse_movers` reads each animated `func_*`
  entity's keys (`origin`/`angle`/`angles`/`height`/`speed`/`phase`/`wait`/
  `distance`/`spawnflags`/`target`), keeps the submodel brush plane run, and
  follows the `path_corner`/`target_position` chain for `func_train`. Animated
  movers are excluded from the static collision/render set.
- **Animate** (`core/src/trace.rs`): runtime `Mover` with `advance()` ported from
  Warfork `g_func.cpp` — bobbing `sin(2π·frac((t − speed·phase)/speed))`, plat
  bottom→up→top→down FSM, door/rotating-door proximity FSM, train corner chase,
  rotating/pendulum angle integration. The sim owns all of it (deterministic,
  no wall clock).
- **Collide** (`core/src/trace.rs`): mover brush planes are transformed per trace
  (`world = origin + R(angles)·geometry`; plane `n' = R·n`,
  `d' = d + n'·origin`, matching Qfusion `CM_TransformedBoxTrace`). `TraceResult`
  reports the mover index that was hit.
- **Carry** (`core/src/pmove.rs`): `ground_mover` is recorded by `grounded()`; at
  tick start `advance_movers()` advances every mover and applies the Qfusion
  `SV_Push` rider delta (`origin += Δ`, rotate about the mover origin by Δangles,
  add yaw).
- **Render** (`core/src/bsp.rs` + `bindings.rs` + `web/src/render/renderer.ts` +
  `web/src/main.ts`): `dface_t` has no model field, so faces are mapped to a
  submodel via the MODELS lump `firstface`/`numfaces` (dmodel_t bytes 24/28).
  Mover faces are emitted into a **separate** vertex/index buffer; the renderer
  draws them with a `u_model` uniform (origin + `AnglesToAxis` columns).
- **Tests** (`core/tests/movers.rs`): real-map parse (`BardoK-Strafe1` → 6
  `func_bobbing`), bobbing carries the player, plat raises the player, no
  fall-through, door opens/closes, rotating door turns about yaw.

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

### Spawn orientation + step-over-same-height-lip (the "can't pass start line" bug)
- Two independent bugs combined to trap the player at spawns and flat "stuck"
  spots on rek-dire:
  1. `Session::new` initialized `Angles { yaw, .. }` with `yaw = ps.viewangles[1]`
     **in radians**, but `Angles.yaw` is a **16-bit fixed-point angle** (0..65535,
     32768 = forward +X). Sending 0 rad into the field made `to_yaw_rad()` return
     `-PI`, so the player spawned facing **180° backwards**, into the wall behind
     spawn. Fix: `angles.set_view_rad(yaw, 0.0)`.
  2. `slide_move` rejected a step-up by comparing **vertical** change
     (`origin[2] <= start_o[2]+0.01`). A step over a *same-height* thin lip (or
     ledge corner at foot level) advances X without changing Z, so it was
     discarded and the player stuck with the blocked slide. Q3 compares
     **horizontal** distance (`down_dist` vs `up_dist`). Fixed to match.

### Shader stages: animated / emissive / transparent / scroll
- `shader.ts` now parses `tcMod scroll` and full `blendfunc` operands, and
  `buildPlan()` normalizes a shader into a **base pass** (`lit` unless
  `surfaceparm nolightmap`; `animmap` frame cycle; base blend) plus **overlay
  passes** (additive `GL_ONE/GL_ONE`, alpha `blend`, multiply `GL_DST_COLOR/
  GL_ZERO`).
- map-server gained bulk `GET /shaders?names=a,b,c` (JSON name→block) so the
  client fetches all of a map's shaders in one request.
- renderer: base fragment shader gained `u_lit` (lightmap on/off), `u_uv_scroll`
  + `u_time` (tcMod scroll) and alpha output; a new **FX program** draws overlay
  stages with blend modes; draw chunks now carry anim frame ids and overlays.
- `main.ts` bulk-loads shader defs, builds per-shader plans, and registers the
  anim/overlay textures.
- Deploy: added `.dockerignore` and forced `docker compose build --no-cache` for
  the backend (NFS could make the `COPY server/` layer look unchanged).

### `.shader` parsing + skybox
- Added `.shader` script support (first slice of the shader feature):
  - map-server `GET /shader?name=<name>` resolves a shader block by scanning the
    current map pk3 (`currentPk3`, searched first — map pk3s usually bundle their
    shaders) then local pk3s, caching a per-pk3 name index. **Note: Caddy needs a
    `/webrace/shader*` proxy rule** (added to `deploy/Caddyfile`; must be applied
    to the live Caddyfile).
  - client `web/src/render/shader.ts` parses the block (surfaceparm, skyparms,
    stages) — brace-aware tokenizer.
  - WASM `bsp_shader_flags` exposes per-shader surface flags so JS can find
    `SURF_SKY`.
  - renderer builds a skybox from the `skyparms` base (Q3 `rt/bk/lf/ft/up/dn`
    face order via `MakeSkyVec`) and draws it first (writes depth far away, world
    draws over it).
- **Deploy gotcha:** `docker compose up --build` served a *cached* `COPY server/`
  layer, so the container ran old server code. Force with `docker compose build
  --no-cache webrace` when server code changes.

### Stair-step view smoothing (Warfork `CG_PredictAddStep`)
- Climbing a step snapped the camera up abruptly. Ported Warfork's stair
  smoothing: `PM_StepSlideMove` records the step height (`pm.step`), and the
  client carries over the un-eased part of the previous step, adds the new one,
  and offsets the view **down** by it, easing to zero over
  `PREDICTED_STEP_TIME = 150` ms (`CG_ViewSmoothPredictedSteps`). Only the
  *view* is smoothed — collision still snaps up so you climb correctly.
- `PlayerState`/`Pmove` gained `step`; `Session` gained `step_change`/`step_time`
  and `eye()` applies the offset. Regression test `step_smooth.rs` (a 16-unit
  step produces a 16-unit view offset that eases back to 0).

### Falling through brush-model entities (func_bobbing platforms)
- On `BardoK-Strafe1` the player fell through a platform "floating on the water".
  The platform is a **`func_bobbing` brush-model entity** (`model "*4"`), i.e.
  its brushes live in a submodel, not model 0. We only loaded model-0 world
  brushes for collision, so these entities had no collision at all.
- Fix: `parse_solid_brush_models` walks the entities, and for every solid
  brush-model classname (`func_*` except `func_illusionary`/`func_areaportal`/
  `func_portal`/`func_ladder`/`func_water*`) appends that submodel's brushes to
  the collision world at their base position. Triggers (`trigger_*`) are not
  affected (they are handled separately). Moving/animation is not modelled yet
  (`func_bobbing` is static at its base, matching our static rendering).

### Moving brush entities implementation (Warfork-faithful transform + carry)
- The key insight from Qfusion `CM_TransformedBoxTrace`: for inline models the
  BSP brush/vertex geometry is in the entity's **local frame**, and the engine
  applies `world = origin + R(angles)·geometry`. A plane transforms as
  `n' = R·n`, `d' = d + n'·origin`. `func_bobbing` entities on `BardoK-Strafe1`
  have no `origin` key and absolute geometry, so `origin` starts at 0 and the
  bob is a pure translation; origin-brush doors/rotating entities store local
  geometry and a nonzero `origin` (rotation pivot). One formula covers both.
- Movers are **not** appended to the static world brush arrays anymore. The
  trace runs static brushes first, then each mover's planes transformed per
  trace. `TraceResult.mover` (index, `-1` = static) feeds `grounded()`, which
  stores `PlayerState.ground_mover`.
- Carry is `Pmove::advance_movers` at the top of `step`: advance every mover,
  then if the player is grounded on mover `i`, add its per-tick origin delta and
  rotate the player about the mover origin by the angle delta (`SV_Push`), plus
  the yaw carry. A platform rising into the player is handled by the existing
  `start_solid`/`resolve_solid` push-up.
- `dmodel_t` is 40 bytes with `firstface`/`numfaces` at offsets **24/28** (not
  32/36, which are `firstbrush`/`numbrushes`) — the renderer maps each face to
  its submodel with these, then every face of a mover submodel goes into a
  separate mover vertex/index buffer. Renderer applies a `u_model` matrix
  (columns = `AnglesToAxis` rows; translation = current origin).
- `G_SetMovedir` gotcha: `angle == 0` is **not** zero movement — it is yaw 0 →
  `[1,0,0]` via `AngleVectors`. Only `-1`/`-2` mean up/down. Do not special-case
  0 to a zero vector or every un-angled door silently stops moving.
- Door/plat proximity tests use the **authored** position (`base_origin`), not
  the current one: otherwise an opened door slides out of its own trigger and
  immediately closes. (Warfork spawns a separate static trigger volume.)
- `func_door_rotating` derives its axis from spawnflags (default yaw/Z), not the
  `angle` key; `func_rotating` axes: `&4` roll, `&8` pitch, else yaw, `&2`
  reverses.
- Tests in `core/tests/movers.rs` cover parse (real map), bobbing carry, plat
  rise, no fall-through, door open/close, rotating door.
- **Known simplifications** (fine for race/defrag; revisit if needed): doors open
  on player proximity rather than via their spawned trigger/button (`targetname`)
  wiring; a targeted `func_plat` still starts lowered rather than waiting at the
  top for a trigger; door/plat crush damage is not applied (the player is just
  pushed out). `func_button` is still static collision.

### Mover transform was a Y-mirror (Q3 `AnglesToAxis` right-vector sign)
- Symptom: on `BardoK-Strafe1` the `func_bobbing` boxes vanished and the player
  fell through into the water — a regression from extracting movers.
- Cause: `angles_to_axis` was a verbatim `AnglesToAxis`, whose second vector
  `right = forward × up` is `-Y` at zero angles (a *left*-handed basis). Using it
  as the rotation matrix made a zero-angle mover a reflection across Y. The real
  boxes (y `-288..-200`, no origin key, zero angles) were mirrored to y
  `200..288` — off the water and away from where you'd land. Collision and
  rendering both used it, so both broke together.
- Why the synthetic tests missed it: they used boxes symmetric in Y (`±128`),
  which map onto themselves under a Y mirror.
- Fix: negate the middle (`right`) basis vector in `angles_to_axis` so zero
  angles → identity and the transform is a proper right-handed rotation
  (`Rz(yaw)`), matching the camera's yaw convention. `transform_plane`, the
  renderer columns, and the `SV_Push` carry all use this one function, so they
  stay consistent.
- Regression tests: `real_map_bobbing_boxes_have_collision` (down-trace hits
  mover 0, player settles at z≈48) and `zero_angle_rotation_is_identity`
  (identity at 0; +90° yaw maps +X→+Y).

### Sound system (drop-in `snd/` files + per-event assignment)
- The browser can't list a directory, so the **map-server** gained
  `GET /sounds` (JSON list of audio files in `snd/`, no-store) and
  `GET /snd/<file>` (served with the right MIME, no-store so replacing a file
  with the same name is picked up). `web/vite.config.ts` proxies both in dev.
  `SND_DIR = server/.. /snd` (dev) / `/app/snd` (container; `Dockerfile.backend`
  copies it). The endpoint returns `{files:[]}` if the dir is missing.
- WAV/MP3/OGG/etc. under `snd/` are **gitignored** (`/snd/*`, `!/snd/.gitkeep`)
  so large test assets never get committed (the repo once had a 192 MB zip
  incident). Commit curated sounds elsewhere if they need to ship.
- Assignments live in `Settings.sounds` (event id → file name), persisted and
  included in the share code for free. The **Sound tab** (`web/index.html` +
  `Menu.renderSounds`) builds one `<select>` + ▶ preview per `SOUND_EVENTS` entry
  (`web/src/audio.ts`), and a *Reload file list* button re-fetches without a
  page reload.
- Gameplay sounds come from **sim event bits**, not guessed client state:
  `Pmove.events` (`EV_JUMP/DASH/WALLJUMP/LAND/FOOTSTEP/JUMPPAD/TELEPORT` in
  `core/src/lib.rs`), reset each tick, returned by the now-`u32`
  `session_step`. `main.ts` ORs the bits across the frame's fixed ticks and
  `SoundManager.handleBits` plays them. Land uses `was_ground` captured at the
  top of `step`; footsteps accumulate 60 units of ground speed. Bit order is
  mirrored in `web/src/audio.ts` — keep the two in sync.
- Playback uses the **Web Audio API**: each one-shot is a fresh
  `AudioBufferSource` from a decoded+cached `AudioBuffer` (context created and
  resumed on the first pointer/key via `SoundManager.unlock()`). The first cut
  used pooled `HTMLAudioElement`s, but switching files could leave the pool in a
  state where `play()` silently did nothing (real-browser flakiness, hard to
  reproduce headless); Web Audio fixed it and gives reliable overlap.
- Defaults: `jump → FS Ground Civilian Walk N05.wav`, `dash → ljud3.wav`,
  `walljump → FS Ground Civilian Walk N03.wav`. A one-time
  `soundVersion` migration in `loadSettings` re-applies those onto existing
  installs so testers see them without clearing localStorage.
- **Master volume**: a 0..1 slider in the Sound tab (`Settings.volume`, default
  0.8) feeds `SoundManager.setVolume` → the master `GainNode`; if the slider is
  moved before the context exists, the value is stored and applied on unlock.
- Debugging gotcha: headless Chromium needs `HOME`/`XDG_CONFIG_HOME` pointed at a
  writable dir or it crash-loops before opening the DevTools port (the machine's
  `~/.config/chromium` was on a broken mount).
- **Deploy gotcha**: if `snd/` ever ships, Caddy needs `/webrace/sounds` and
  `/webrace/snd/*` → `192.168.0.107:4173` proxy rules (added to the reference
  `deploy/Caddyfile`; must be applied to the live Caddyfile like the shader rule).

### Debug wireframe (WebGL2 has no `glPolygonMode`)
- The renderer keeps a second VAO per mesh (world + movers) that shares the same
  vertex buffer but uses a `gl.LINES` index buffer built from the triangle IBO
  (3 edges per triangle, in order). Because the line buffer is exactly 2× the
  triangle IBO in the same order, a triangle chunk `[first, count)` maps to line
  `[first*2, count*2)` — no per-chunk bookkeeping.
- The base fragment shader gained `u_wire` (flat bright green, early return);
  skybox and overlay passes are skipped while wireframe is on. Toggled by the
  rebindable `wireframe` action (default **P**) and by `?wire=1` on the URL.
- Verified headlessly by wrapping `drawElements` and counting modes: with
  `?wire=1` the frame is all `LINES` (0 `TRIANGLES`), and pressing `P` flips it
  back to `TRIANGLES`.

### Texture filtering (mipmaps + anisotropy)
- The first pass used `MIN_FILTER = LINEAR` with **no `generateMipmap`**, so
  minified surfaces (distance / grazing angles) sampled the full-res texture and
  shimmered. `MAG_FILTER = LINEAR`, wrap `REPEAT`.
- Fix: generate mipmaps on every world/sky/white texture after upload and use
  `MIN_FILTER = LINEAR_MIPMAP_LINEAR` (trilinear). Anisotropic filtering
  (`EXT_texture_filter_anisotropic`, 8×) is enabled when available. Mode is
  switchable at runtime via `Settings → Texture filtering`
  (`Settings.textureMode`: nearest / bilinear / trilinear / anisotropic), which
  re-applies `texParameteri` to every cached texture (no re-upload).
- The **lightmap atlas is excluded** (fixed `LINEAR`, `CLAMP_TO_EDGE`, no mips) —
  mipmapping an atlas grid bleeds light between cells.
- Gotcha: applying filters binds textures; do it on **unit 1**, never unit 0,
  because the lightmap is bound to unit 0 once and never rebound per frame.
- Gotcha: the 1×1 fallback white texture still needs mipmaps (or a
  non-mipmap min filter), otherwise a mipmap min filter makes it incomplete.
- Verified by wrapping `generateMipmap`/`texParameteri`/`texParameterf` in
  headless Chromium: 10 mipmap generations, world/sky textures get
  `LINEAR_MIPMAP_LINEAR`, aniso applied, and switching to Bilinear re-applies
  `LINEAR` to all cached textures.

### Smooth crouch transition (Warfork `PM_AdjustBBox`)
- Crouch was instantaneous (box snapped 40 -> 16). Ported Warfork's transition:
  a `crouchtime` (0..`CROUCHTIME`=100ms) interpolates the box maxs.z (40 -> 16)
  and `viewheight` (30 -> 12) over 100 ms. Standing up is refused while the
  taller box would clip a ceiling (head-chomp): trace the *wish* box at the
  origin and keep the crouched box if `allsolid || startsolid`. `PlayerState`
  gained `crouchtime: f32` and `viewheight: f32`; `eye()` now uses `viewheight`.

### Crouch: eye height + step-up oscillation under low ceilings
- **Eye/viewheight was wrong:** we used stand 26 / crouch 18, but Warfork uses
  `playerbox_stand_viewheight = 30` / `playerbox_crouch_viewheight = 12`. The
  crouch eye (18) sat *above* the crouched box top (origin+16), so the camera
  poked into low objects. Fixed to 30/12. (The collision box itself already
  matched Warfork: mins z -24, crouch maxs z 16.)
- **Stuck at a crouch tunnel mouth:** `PM_StepSlideMove` retries the move from
  `STEPSIZE` (18) higher. Under a low ceiling the raised box is *embedded* in the
  ceiling; we only checked `trace.all_solid` (always false in our trace), not
  `trace.start_solid`, so we stepped from an embedded position and the player
  oscillated at the tunnel mouth (x bounced 183↔184) instead of entering. Now
  reject the step when `all_solid || start_solid`. Crouched players pass tunnels
  > 40 units. Regression test in `crouch.rs`.

### Slick not working on some maps (coincident `common/slick` overlay)
- On maps like `idiotism2_slick`, nothing was slippery. The map places an
  **invisible `textures/common/slick` brush exactly coincident** with the visible
  floor brush (same planes, `flags=0xca2` incl. `SURF_SLICK`, `contents`
  `SOLID|TRANSLUCENT`). Our trace hit both at the same fraction and kept the
  first (`<` strict), which was the visible non-slick brush, so the floor had
  no slick flag.
- Fix: on an (near-)equal fraction, prefer the brush whose surface flags include
  `SURF_SLICK`. This encodes the "invisible slick overlay" convention. Regression
  test `slick_overlay.rs`.

### Dash not registering on ramps + slope speed loss
- **Dash flicker on ramps:** `grounded()` treated `velocity[2] > 20` as airborne,
  but Warfork `PM_CategorizePosition` uses **`velocity[2] > 180`**. Running up a
  ramp redirects horizontal speed into a small `+z` (e.g. ~32), so our code
  flagged the player as airborne and the dash silently failed — intermittently,
  depending on ramp slope/speed. Raised the threshold to 180. Regression test
  `dash.rs`.
- **Slope speed loss:** we had removed Warfork `PM_WalkMove`'s ground-clip
  re-normalize (`vel = VectorLength(velocity); PM_ClipVelocity; VectorNormalize;
  VectorScale(vel)`). Running up a slope then bled speed every tick. Restored it
  (safe now that there is no gravity on the ground), so slopes preserve speed.

### Faithful Warfork walljump (`PM_CheckWallJump`) + 0.25 ground trace
- Our old walljump was a custom "wall-dash" that *reflected* the horizontal
  velocity (`v - 2(v·n)n`), giving the wrong angle and losing speed. Warfork's
  `PM_CheckWallJump` does something different:
  1. Finds the nearest wall via `PlayerTouchWall(12, 0.3)` — 12 directions in a
     circle, flat hull, picks the nearest with `|normal.z| < 0.3`.
  2. Zeroes `velocity[2]`, `hspeed = VectorNormalize2D(velocity)`.
  3. `GS_ClipVelocity(velocity, normal, 1.0005)` (a *slide*, not a reflection).
  4. `velocity += 0.3 * normal` (bounce factor pushes off the wall).
  5. Clamp `hspeed` to `pm_wjminspeed = (walk + runSpeed)/2 = 240`.
  6. Renormalize the 3D velocity and scale back to `hspeed`.
  7. `velocity[2] = max(oldup, pm_wjupspeed)`.
  This preserves entry horizontal speed (verified: fly into a wall at 700 ups →
  exit at 700) and reproduces Warfork's walljump angle. Order is now Warfork's:
  jump → dash → walljump.
- `grounded()` down-trace lowered from **2.0 → 0.25** (Warfork
  `PM_CategorizePosition`). With the faithful slide/step now in place this no
  longer breaks descent, and it lets the player leave a down-slope sooner so
  gravity accelerates them (down-ramp speed). Steeper ramps now clearly gain
  speed (nz 0.6 → ~393 ups).

### Faithful Warfork `PM_SlideMove` + `PM_StepSlideMove` port (ramp slide)
- Replaced our Q3-hybrid slide/step with a **faithful port of Warfork**:
  - `slide_clip` = `PM_SlideMove` (no ground-plane pre-seed; zeroes downward
    velocity only when the ground normal is exactly flat; `PM_OVERBOUNCE` 1.01;
    repeated-plane nudge; crease slide; restore-last-valid-origin on trapped).
  - `slide_move` = `PM_StepSlideMove` (plain slide, retry from `STEPSIZE` up,
    keep the move that advanced farther horizontally, then on a walkable ramp
    preserve horizontal speed and set `velocity[2] = down_v[2]` — the "ramp
    sliding" line).
- **Gravity is now air-only** (Warfork: zero upward velocity on ground, no
  downward gravity). The previous gravity-on-slick hack is gone.
- `grounded()` now uses Warfork `ISWALKABLEPLANE` (`normal.z >= 0.7`) for **all**
  surfaces. This is the key to slick ramps: a *steep* slick ramp is **not**
  walkable, so the player is airborne and gravity accelerates them down (our old
  slick special-case of `0.2` wrongly kept them grounded, so nothing pushed
  them). A *gentle* slick ramp is walkable; a stationary player does not slide
  (Warfork has no ground gravity) — the ramp only redirects existing momentum.

### Gravity-on-ground + residual drift (the "14 ups after stopping" bug)
- Warfork applies gravity in the **air**; NOT on the ground (it zeroes upward
  velocity only). Our code applied gravity *always*, then clipped +
  re-normalized the full 3D speed. That leaked the gravity's ~3.4 ups downward
  component back into horizontal speed, so after releasing the key the player
  asymptoted to a ~14-up drift instead of stopping.
- Fix: apply gravity only when airborne or slick; on ground just clip velocity
  onto the ground plane (no re-normalize, since velocity is already horizontal
  when gravity isn't applied). The player now stops dead at 0 ups, and the
  earlier idle bounce is also naturally gone (same root cause).

### Ledge/gap traversal (getting "stuck on the edge")
- On slick maps (e.g. rek-dire) sliding over a small gap, the player would catch
  the far ledge's leading edge instead of arcing over it. Two Q3-divergent
  behaviours in our code caused it:
  1. `grounded()` used a **2.0-unit** down trace; Q3 `PM_GroundTrace` uses
     **0.25**. The larger tolerance let the player stay "on ground" long after
     leaving a ledge, so they clipped the far edge horizontally instead of
     entering a short ballistic dip ("time to fall a few pixels").
  2. A hand-rolled **anti-float snap-down** (a separate 30-unit trace that
     yanked the player onto whatever surface was below) actively fought the
     natural arc. Q3 has no such step; it relies on the ground-plane clip +
     re-normalize (already added) to stay on surfaces.
- Fixed both: tightened `grounded()` down trace to 0.25, and deleted the
  anti-float snap block. Slick ramp / wall-slide / jump tests still pass.

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
