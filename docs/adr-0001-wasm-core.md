# ADR-0001: Rust → WASM core with native QFusion BSP loading

**Status:** Accepted
**Date:** 2026-09-29

## Context

We want Warfork-faithful movement and the ability to load the entire defrag /
Warfork / Warsow `.bsp` catalog directly in the browser, with a >360 fps
render target.

## Decision

- Simulation, collision, and BSP parsing live in a **Rust crate compiled to
  `wasm32-unknown-unknown`**.
- The browser loads maps as **native QFusion `IBSP` v46** files (extracted from
  `.pk3` zip archives by a small dev server, later by the backend).
- The renderer is a thin **WebGL2** layer that reads vertex/index data
  zero-copy from WASM memory via `Float32Array`/`Uint32Array` views.

## Consequences

- Deterministic 250 Hz sim (matches Quake/Warfork integer-tick model).
- Replay/ghosts trivial: store inputs, re-run the sim.
- Single source of truth for physics (Rust) shared by client and (future)
  authoritative server.

## Verified BSP facts (against real Warfork maps)

| Field | Value |
|---|---|
| ident | `IBSP` (`0x50534249`) |
| version | `46` |
| lumps | 18 (`HEADER_LUMPS`) |
| `dvertex_t` | 44 bytes (pos[3] tex_st[2] lm_st[2] normal[3] color[4]) |
| `dface_t` | 104-byte stride; header fields = first 11 ints |
| `dshaderref_t` | 72 bytes |
| `dplane_t` | 16 bytes |
| `dbrush_t` | 12 bytes |
| `dbrushside_t` | 8 bytes |
| `dmodel_t` | 40 bytes (firstbrush @ +32, numbrushes @ +36) |
| coordinate transform | `q2t = (x, z, -y)` |

Validated by `core/tests/bsp_real.rs` against `kool_simple2-wjfix`,
`hoppin`, `KTOTAM-1`, `rek-dire-wjfix-allslick`.
