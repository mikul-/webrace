#!/usr/bin/env bash
# Build the Rust → WASM core and generate JS bindings into web/pkg/.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CORE="$ROOT/core"
PKG="$ROOT/web/pkg"

# Ensure rustup's cargo is on PATH (for the wasm32 target) without clobbering
# the system toolchain usage elsewhere.
export PATH="$HOME/.cargo/bin:$PATH"

echo "==> building WASM core (release)…"
(cd "$CORE" && cargo build --target wasm32-unknown-unknown --release)

echo "==> generating bindings into $PKG …"
mkdir -p "$PKG"
wasm-bindgen \
  "$CORE/target/wasm32-unknown-unknown/release/webrace_core.wasm" \
  --out-dir "$PKG" \
  --target web

echo "==> done. pkg contents:"
ls -la "$PKG"
