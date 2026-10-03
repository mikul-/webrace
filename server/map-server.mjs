// Dev map server: resolves `/maps/<name>.bsp` by scanning the local Warfork
// install directory, extracting the `.bsp` from the matching `.pk3` on demand.
//
// Usage: node server/map-server.mjs [warfork-data-dir]
//   defaults to ~/.local/share/warfork-2.1

import http from "node:http";
import { readFile, readdir, mkdir, writeFile } from "node:fs/promises";
import { join, basename } from "node:path";
import { homedir } from "node:os";
import { inflateRaw } from "node:zlib";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import { dirname } from "node:path";

const inflate = promisify(inflateRaw);
const __dirname = dirname(fileURLToPath(import.meta.url));

const WARFORK_DIR = process.argv[2] || join(homedir(), ".local", "share", "warfork-2.1");
const PORT = Number(process.env.MAP_PORT || process.env.PORT || 4173);
const PADPORK = "https://padpork.org";
// Where downloaded padpork pk3s are cached (persists across restarts).
const PK3_CACHE_DIR = process.env.PK3_CACHE_DIR || join(__dirname, ".pk3-cache");

// ---- minimal zip central-directory reader (pk3 files are zips) ----

function u16(b) { return b[0] | (b[1] << 8); }
function u32(b) { return b[0] | (b[1] << 8) | (b[2] << 16) | (b[3] << 24); }

/** Locate the End Of Central Directory record. */
function findEOCD(buf) {
  const min = Math.max(0, buf.length - 65557);
  for (let i = buf.length - 22; i >= min; i--) {
    if (u32(buf.subarray(i, i + 4)) === 0x06054b50) {
      const count = u16(buf.subarray(i + 10, i + 12));
      const cdOffset = u32(buf.subarray(i + 16, i + 20));
      return { count, cdOffset };
    }
  }
  throw new Error("no EOCD");
}

/** List entries (name -> offset) from the central directory. */
function readCentralDirectory(buf) {
  const { count, cdOffset } = findEOCD(buf);
  const entries = [];
  let p = cdOffset;
  for (let i = 0; i < count; i++) {
    if (u32(buf.subarray(p, p + 4)) !== 0x02014b50) break;
    const compMethod = u16(buf.subarray(p + 10, p + 12));
    const compSize = u32(buf.subarray(p + 20, p + 24));
    const nameLen = u16(buf.subarray(p + 28, p + 30));
    const extraLen = u16(buf.subarray(p + 30, p + 32));
    const commentLen = u16(buf.subarray(p + 32, p + 34));
    const localOffset = u32(buf.subarray(p + 42, p + 46));
    const name = buf.subarray(p + 46, p + 46 + nameLen).toString("latin1");
    entries.push({ name, compMethod, compSize, localOffset });
    p += 46 + nameLen + extraLen + commentLen;
  }
  return entries;
}

/** Extract a single entry's raw (decompressed) bytes. */
async function extract(buf, entry) {
  const p = entry.localOffset;
  const nameLen = u16(buf.subarray(p + 26, p + 28));
  const extraLen = u16(buf.subarray(p + 28, p + 30));
  const dataStart = p + 30 + nameLen + extraLen;
  const raw = buf.subarray(dataStart, dataStart + entry.compSize);
  if (entry.compMethod === 0) return raw; // stored
  if (entry.compMethod === 8) return inflate(raw); // deflate
  throw new Error("unsupported compression " + entry.compMethod);
}

// ---- pk3 scanning ----

const pk3Files = [];
for (const sub of ["downloads/racemod_2.1", "downloads__/racemod_2.1", "basewf"]) {
  try {
    const dir = join(WARFORK_DIR, sub);
    for (const f of await readdir(dir)) {
      if (f.toLowerCase().endsWith(".pk3")) pk3Files.push(join(dir, f));
    }
  } catch {
    /* dir may not exist */
  }
}
console.log(`[maps] ${pk3Files.length} pk3 files indexed under ${WARFORK_DIR}`);

// Cache extracted bsp buffers by lowercase map name.
const cache = new Map();

// Cache downloaded padpork pk3 ZIP buffers by map name (also serves textures).
const padporkCache = new Map();

// The pk3 buffer of the most recently-requested map, searched first for its
// textures (maps downloaded from padpork carry their own textures).
let currentPk3 = null;

// Cached padpork catalog JSON (fetched once).
let catalogCache = null;

/** Fetch and cache a map's .pk3 from padpork.org (fallback when not local). */
async function fetchPadporkPk3(name) {
  const key = name.toLowerCase();
  if (padporkCache.has(key)) return padporkCache.get(key);

  // Try the on-disk cache first.
  try {
    await mkdir(PK3_CACHE_DIR, { recursive: true });
    const diskPath = join(PK3_CACHE_DIR, key + ".pk3");
    try {
      const buf = await readFile(diskPath);
      padporkCache.set(key, buf);
      return buf;
    } catch {
      /* not cached on disk */
    }

    const res = await fetch(`${PADPORK}/api/maps/${encodeURIComponent(name)}/download`, {
      headers: { "User-Agent": "webrace map-server" },
    });
    if (!res.ok) return null;
    const buf = Buffer.from(await res.arrayBuffer());
    padporkCache.set(key, buf);
    await writeFile(diskPath, buf).catch(() => {});
    console.log(`[maps] ${name} <- padpork (${buf.length} bytes pk3)`);
    return buf;
  } catch (e) {
    console.error(`[maps] padpork fetch failed for ${name}:`, e.message);
    return null;
  }
}

async function resolveBsp(name) {
  const want = name.toLowerCase();
  if (cache.has(want)) return cache.get(want);

  for (const pk3 of pk3Files) {
    try {
      const buf = await readFile(pk3);
      const entries = readCentralDirectory(buf);
      for (const e of entries) {
        const base = basename(e.name).toLowerCase();
        if (base === want + ".bsp") {
          const bytes = await extract(buf, e);
          cache.set(want, bytes);
          // Also cache this pk3 for its textures (mixed maps use basewf textures).
          padporkCache.set("__local__" + basename(pk3), buf);
          console.log(`[maps] ${name} <- ${basename(pk3)} (${bytes.length} bytes)`);
          return bytes;
        }
      }
    } catch (err) {
      // skip corrupt/incompatible pk3
    }
  }

  // Fallback: padpork.org.
  const pk3 = await fetchPadporkPk3(name);
  if (!pk3) return null;
  currentPk3 = pk3;
  const entries = readCentralDirectory(pk3);
  for (const e of entries) {
    const base = basename(e.name).toLowerCase();
    if (base === want + ".bsp") {
      const bytes = await extract(pk3, e);
      cache.set(want, bytes);
      console.log(`[maps] ${name} <- padpork pk3 (${bytes.length} bytes bsp)`);
      return bytes;
    }
  }
  return null;
}

// Cache raw texture buffers by lowercase path (without extension).
const texCache = new Map();

async function resolveTexture(shaderName) {
  // shaderName like "textures/base_wall/basewall01_owfx" (no extension).
  const key = shaderName.toLowerCase();
  if (texCache.has(key)) return texCache.get(key);

  const candidates = [
    shaderName + ".jpg",
    shaderName + ".tga",
    shaderName + ".png",
    shaderName + ".webp",
  ].map((c) => c.toLowerCase());

  const searchPk3 = (buf, label) => {
    const entries = readCentralDirectory(buf);
    for (const e of entries) {
      const lower = e.name.toLowerCase();
      if (candidates.includes(lower)) {
        return { buf, entry: e, lower, label };
      }
    }
    return null;
  };

  // 1. The current map's pk3 (has its own textures).
  if (currentPk3) {
    const hit = searchPk3(currentPk3, "current");
    if (hit) return extractTex(hit.buf, hit.entry, hit.lower, key, shaderName, hit.label);
  }

  // 2. Local pk3s.
  for (const pk3 of pk3Files) {
    try {
      const buf = await readFile(pk3);
      const hit = searchPk3(buf, basename(pk3));
      if (hit) return extractTex(hit.buf, hit.entry, hit.lower, key, shaderName, hit.label);
    } catch {
      /* skip */
    }
  }

  // 3. All cached padpork pk3s (in case a map references shared textures).
  for (const buf of padporkCache.values()) {
    if (buf === currentPk3) continue;
    const hit = searchPk3(buf, "padpork-cache");
    if (hit) return extractTex(hit.buf, hit.entry, hit.lower, key, shaderName, hit.label);
  }

  return null;
}

async function extractTex(buf, entry, lower, key, shaderName, label) {
  const bytes = await extract(buf, entry);
  const ext = lower.split(".").pop();
  const result = { bytes, ext };
  texCache.set(key, result);
  console.log(`[tex] ${shaderName} <- ${label}/${entry.name} (${bytes.length} bytes)`);
  return result;
}

// ---- shader scripts (.shader) ----

/** Parse `name { ... }` shader blocks from `text` into `index` (name -> block). */
function parseShaderText(text, index) {
  const s = text.replace(/\/\/[^\n]*/g, " ");
  const n = s.length;
  let i = 0;
  while (i < n) {
    while (i < n && /\s/.test(s[i])) i++;
    if (i >= n) break;
    const start = i;
    while (i < n && !/\s/.test(s[i]) && s[i] !== "{") i++;
    const name = s.slice(start, i).trim();
    while (i < n && /\s/.test(s[i])) i++;
    if (s[i] !== "{") continue;
    let depth = 0;
    const bstart = i;
    while (i < n) {
      if (s[i] === "{") depth++;
      else if (s[i] === "}") {
        depth--;
        if (depth === 0) {
          i++;
          break;
        }
      }
      i++;
    }
    if (name) index.set(name.toLowerCase(), s.slice(bstart, i));
  }
}

/** Build a shader index (name -> block) for a pk3 buffer (cached by caller). */
async function pk3ShaderIndex(buf) {
  const index = new Map();
  let entries;
  try {
    entries = readCentralDirectory(buf);
  } catch {
    return index;
  }
  for (const e of entries) {
    if (!e.name.toLowerCase().endsWith(".shader")) continue;
    try {
      const text = (await extract(buf, e)).toString("latin1");
      parseShaderText(text, index);
    } catch {
      /* skip */
    }
  }
  return index;
}

const shaderIndexCache = new Map();

/** Resolve a shader name to its script block, searching current + local pk3s. */
async function resolveShader(name) {
  const want = name.trim().toLowerCase();
  const sources = [];
  if (currentPk3) sources.push({ key: "__current__", buf: currentPk3 });
  for (const p of pk3Files) sources.push({ key: p });
  for (const [k, buf] of padporkCache) sources.push({ key: k, buf });
  for (const src of sources) {
    let idx = shaderIndexCache.get(src.key);
    if (!idx) {
      let buf = src.buf;
      if (!buf) {
        try {
          buf = await readFile(src.key);
        } catch {
          continue;
        }
      }
      idx = await pk3ShaderIndex(buf);
      shaderIndexCache.set(src.key, idx);
    }
    if (idx.has(want)) return idx.get(want);
  }
  return null;
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, "http://localhost");

  // Proxy the padpork map catalog (CORS blocks the browser from hitting it
  // directly). Fetched as a list of all maps once, cached in-memory.
  if (url.pathname === "/catalog") {
    try {
      if (!catalogCache) {
        const PAGE = 100;
        const items = [];
        let offset = 0;
        let total = 0;
        for (;;) {
          const r = await fetch(`${PADPORK}/api/maps?limit=${PAGE}&offset=${offset}`, {
            headers: { "User-Agent": "webrace" },
          });
          if (!r.ok) throw new Error("padpork " + r.status);
          const page = await r.json();
          total = page.total ?? total;
          items.push(...(page.items ?? []));
          if (items.length >= total || (page.items?.length ?? 0) === 0) break;
          offset += PAGE;
        }
        catalogCache = JSON.stringify({ total, items });
        console.log(`[catalog] loaded ${items.length}/${total} maps from padpork`);
      }
      res.writeHead(200, { "Content-Type": "application/json", "Cache-Control": "public, max-age=86400" });
      res.end(catalogCache);
      return;
    } catch (e) {
      res.writeHead(502).end("catalog unavailable: " + e.message);
      return;
    }
  }

  if (url.pathname.startsWith("/maps/") && url.pathname.endsWith(".bsp")) {
    const name = basename(url.pathname, ".bsp");
    try {
      const bytes = await resolveBsp(name);
      if (!bytes) {
        res.writeHead(404).end("map not found: " + name);
        return;
      }
      res.writeHead(200, {
        "Content-Type": "application/octet-stream",
        "Content-Length": bytes.length,
        "Cache-Control": "public, max-age=86400",
      });
      res.end(bytes);
      return;
    } catch (e) {
      res.writeHead(500).end("error: " + e.message);
      return;
    }
  }

  if (url.pathname === "/shader") {
    const name = url.searchParams.get("name") || "";
    try {
      const block = await resolveShader(name);
      if (!block) {
        res.writeHead(404).end("shader not found: " + name);
        return;
      }
      res.writeHead(200, {
        "Content-Type": "text/plain; charset=latin1",
        "Cache-Control": "public, max-age=86400",
      });
      res.end(block);
      return;
    } catch (e) {
      res.writeHead(500).end("error: " + e.message);
      return;
    }
  }

  if (url.pathname.startsWith("/tex/")) {
    const shaderName = decodeURIComponent(url.pathname.slice("/tex/".length));
    try {
      const tex = await resolveTexture(shaderName);
      if (!tex) {
        res.writeHead(404).end("texture not found: " + shaderName);
        return;
      }
      const mime = tex.ext === "png" ? "image/png" : tex.ext === "jpg" ? "image/jpeg" : "application/octet-stream";
      res.writeHead(200, {
        "Content-Type": mime,
        "Content-Length": tex.bytes.length,
        "Cache-Control": "public, max-age=86400",
      });
      res.end(tex.bytes);
      return;
    } catch (e) {
      res.writeHead(500).end("error: " + e.message);
      return;
    }
  }

  res.writeHead(404).end("not found");
});

server.listen(PORT, () => {
  console.log(`[maps] serving on http://127.0.0.1:${PORT}/maps/<name>.bsp`);
  console.log(`[tex]  serving on http://127.0.0.1:${PORT}/tex/<shader-name>`);
  console.log(`[cat]  serving on http://127.0.0.1:${PORT}/catalog`);
});
