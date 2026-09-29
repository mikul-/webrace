# webrace — dev run guide

## Prerequisites

- Node ≥ 20 (`node` and `npm` on PATH).
- Rust toolchain with the `wasm32-unknown-unknown` target. Install via:
  ```bash
  rustup target add wasm32-unknown-unknown     # needs rustup
  # or (Arch)  sudo pacman -S rust-wasm
  ```
- `wasm-bindgen-cli` on PATH:
  ```bash
  cargo install wasm-bindgen-cli
  ```
- Local Warfork install (for maps), default `~/.local/share/warfork-2.1`.

## Build the WASM core

```bash
./scripts/build-wasm.sh
```

This produces `web/pkg/webrace_core.js` + `.wasm` bindings.

## Run

```bash
# terminal 1 — map server (serves /maps/<name>.bsp from your Warfork pk3s)
npm run maps            # or: node server/map-server.mjs [warfork-dir]

# terminal 2 — dev server
cd web && npm install && npm run dev
```

Open `http://127.0.0.1:5173/?map=hoppin` — replace `hoppin` with any map in
your catalog (or use the input box). Controls (milestone 1, free camera):
drag to look, scroll to move.

## Map catalog

Your 28 favorited maps are listed in `~/demos/maps.json`. The dev map server
auto-resolves any `<name>.bsp` by scanning `~/.local/share/warfork-2.1`
(`downloads/racemod_2.1`, `downloads__/racemod_2.1`, `basewf`).

## Tests

```bash
cd core && cargo test --test bsp_real   # validates BSP parser on real maps
```
