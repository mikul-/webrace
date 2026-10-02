# webrace — deployment topology

How the live demo is deployed on the TrueNAS server. **Read this before touching
any server config.**

## Infrastructure

- **TrueNAS SCALE** box at **192.168.0.107** (hostname `truenas`).
  - SSH alias `server` (see below).
  - This dev machine is at `192.168.0.185`.
- The appliance host has **no Node/Rust/npm** (SCALE is minimal). Apps run in
  **Docker**; the web front is a **Caddy container**.
- `192.168.0.107` is **not** `192.168.0.7` (common typo).

### SSH

In `~/.ssh/config`:
```
Host server
    HostName 192.168.0.107
    User mikul
    IdentityFile ~/.ssh/id_ed25519_server
```
Passwordless `sudo` is available on the box (`sudo -n`).

## Filesystem layout

- TrueNAS side: `/mnt/storage1/www` (web content) and `/mnt/storage1/caddy` (Caddyfile).
- `/mnt/storage1/www` is **NFS-mounted on the dev machine at `/mnt/www`** — the
  *same* filesystem. So building on the dev machine into `/mnt/www/webrace`
  immediately appears on the server.

## The Caddy container

- Container `ix-webserver-webserver-1`, image `caddy:latest`.
- Mounts:
  - `/mnt/storage1/www` → `/usr/share/caddy` (site content)
  - `/mnt/storage1/caddy` → `/etc/caddy` (Caddyfile)
- Host ports: `8080 -> 80`, `8443 -> 443`.
- Reload config (after editing Caddyfile):
  ```bash
  sudo docker exec ix-webserver-webserver-1 caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile
  ```

### The Caddyfile (live)

Serves `ip.mikul.se` (static) + `memos.*`, `nc.mikul.se` (proxies). webrace is a
subdirectory site `ip.mikul.se/webrace/`:

- static files → `/usr/share/caddy/webrace/web/dist`
- `/webrace/maps/*`, `/webrace/tex/*`, `/webrace/catalog` → `192.168.0.107:4173` (map-server), with `uri strip_prefix /webrace`
- `/webrace/api/*` → `192.168.0.107:4174` (leaderboard)

The reference copy lives in this repo at `deploy/Caddyfile`.

## The webrace backend container

- Built from `Dockerfile.backend` + `docker-compose.yml` (project dir
  `/mnt/storage1/www/webrace`).
- Image `web/webrace` (node:22-alpine) running `map-server.mjs` (4173) +
  `leaderboard.mjs` (4174) together.
- Persisted volume `webrace-data` → `/app/data` (cached padpork pk3s + sqlite db).

## How to deploy (the update flow)

Run on the **dev machine** (which has Rust + Node):

```bash
./scripts/deploy.sh
```

This does, in order:
1. `git pull` into the NFS clone at `/mnt/www/webrace`.
2. Build WASM (`scripts/build-wasm.sh`) + static client (`WB_BASE=/webrace/ vite build`).
3. SSH to server: `docker compose up -d --build webrace` + reload Caddy.

The built client lands at `/mnt/storage1/www/webrace/web/dist` (via NFS).

## Gotchas

- `git` in `/mnt/www/webrace` needs `safe.directory` (owned by uid 3000, not the
  local user). The deploy script adds it.
- Maps are **padpork fallback only** (no local Warfork install on the server).
  The map-server downloads `.pk3` on demand and caches them in the container
  volume.
- The client was made **base-path aware** (`web/src/base.ts`, `WB_BASE` env) so
  it works under the `/webrace/` subdirectory. All API/asset fetches use `api()`.
- Vite's `base` only affects bundled assets; the `api()` helper prefixes the
  hand-written `fetch()` paths. Both must agree.
