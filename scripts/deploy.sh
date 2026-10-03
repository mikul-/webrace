#!/usr/bin/env bash
# Deploy webrace to the TrueNAS server.
#
# Run on the DEV machine (which has Rust + Node). It pulls latest origin into
# the NFS-mounted clone at /mnt/www/webrace (the same filesystem the server
# sees as /mnt/storage1/www/webrace), builds the WASM core + static client
# there (so `web/dist` automatically appears on the server), then SSHes to the
# server to restart the backend container and reload Caddy.
#
# Usage: ./scripts/deploy.sh
set -euo pipefail

SITE="/mnt/www/webrace"
SERVER="server"   # ssh alias -> 192.168.0.107

if [ ! -d "$SITE/.git" ]; then
  echo "error: $SITE is not a git clone" >&2
  exit 1
fi

echo "==> git pull"
git config --global --add safe.directory "$SITE" 2>/dev/null || true
git -C "$SITE" pull --ff-only origin master

echo "==> build WASM core"
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
"$SITE/scripts/build-wasm.sh"

echo "==> npm install + build static client (base=/webrace/)"
(cd "$SITE/web" && npm install --no-audit --no-fund)
(cd "$SITE/web" && WB_BASE=/webrace/ npx vite build)

echo "==> (re)start backend container + reload Caddy on server"
# `--no-cache`: the NFS mount can make the `COPY server/` layer look unchanged
# to Docker's build cache, so force a fresh copy of the server code.
ssh "$SERVER" '
  set -e
  cd /mnt/storage1/www/webrace
  sudo docker compose build --no-cache webrace
  sudo docker compose up -d webrace
  sudo docker exec ix-webserver-webserver-1 caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile
'

echo "==> done. Live at https://ip.mikul.se/webrace/"
