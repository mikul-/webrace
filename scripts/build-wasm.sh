#!/usr/bin/env bash
# Build the Rust → WASM core and generate JS bindings into web/pkg/.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CORE="$ROOT/core"
PKG="$ROOT/web/pkg"

# Use the system cargo + the distro `rust-wasm` package (wasm32-unknown-unknown
# target lives in /usr/lib/rustlib). Prefer /usr/bin/cargo so a possible
# rustup-installed cargo (~/.cargo/bin) doesn't shadow the system toolchain.
if [ -x /usr/bin/cargo ]; then
  export PATH="/usr/bin:$PATH"
fi

if ! rustc --print target-list 2>/dev/null | grep -q wasm32-unknown-unknown; then
  echo "error: wasm32-unknown-unknown target not available." >&2
  echo "       install the distro 'rust-wasm' package (e.g. sudo pacman -S rust-wasm)." >&2
  exit 1
fi

echo "==> building WASM core (release)…"
(cd "$CORE" && cargo build --target wasm32-unknown-unknown --release)

echo "==> generating bindings into $PKG …"
mkdir -p "$PKG"

# `wasm-bindgen` CLI; commonly installed via `cargo install wasm-bindgen-cli`
# into ~/.cargo/bin (not on the default PATH).
WASM_BINDGEN="$(command -v wasm-bindgen 2>/dev/null || true)"
if [ -z "$WASM_BINDGEN" ] && [ -x "$HOME/.cargo/bin/wasm-bindgen" ]; then
  WASM_BINDGEN="$HOME/.cargo/bin/wasm-bindgen"
fi
if [ -z "$WASM_BINDGEN" ]; then
  echo "error: wasm-bindgen not found." >&2
  echo "       install it with: cargo install wasm-bindgen-cli" >&2
  exit 1
fi

"$WASM_BINDGEN" \
  "$CORE/target/wasm32-unknown-unknown/release/webrace_core.wasm" \
  --out-dir "$PKG" \
  --target web

echo "==> done. pkg contents:"
ls -la "$PKG"
