#!/usr/bin/env bash
# Deploy webrace to the TrueNAS server (Caddy web root + Node backends).
#
# Runs ON the server (192.168.0.107, ssh alias `server`). The site is a git
# clone at /mnt/storage1/www/webrace; this script pulls, rebuilds the WASM core
# and the static client, then (re)starts the two Node backend servers under
# systemd user services.
set -euo pipefail

SITE=/mnt/storage1/www/webrace
cd "$SITE"

echo "==> git pull"
git -c safe.directory="$SITE" pull --ff-only origin master

echo "==> build WASM core"
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
./scripts/build-wasm.sh

echo "==> build static client (base=/webrace/)"
npm install --prefix web --no-audit --no-fund 2>&1 | tail -3
WB_BASE=/webrace/ npm --prefix web run build

echo "==> deploy done. dist at web/dist"
ls -la web/dist
