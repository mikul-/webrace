// Dev map server: resolves `/maps/<name>.bsp` by scanning the local Warfork
// install directory, extracting the `.bsp` from the matching `.pk3` on demand.
//
// Usage: node server/map-server.mjs [warfork-data-dir]
//   defaults to ~/.local/share/warfork-2.1

import http from "node:http";
import { readFile, readdir } from "node:fs/promises";
import { join, basename, extname } from "node:path";
import { homedir } from "node:os";
import { inflateRaw } from "node:zlib";
import { promisify } from "node:util";

const inflate = promisify(inflateRaw);

const WARFORK_DIR = process.argv[2] || join(homedir(), ".local", "share", "warfork-2.1");
const PORT = Number(process.env.PORT || 4173);

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
          console.log(`[maps] ${name} <- ${basename(pk3)} (${bytes.length} bytes)`);
          return bytes;
        }
      }
    } catch (err) {
      // skip corrupt/incompatible pk3
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

  for (const pk3 of pk3Files) {
    try {
      const buf = await readFile(pk3);
      const entries = readCentralDirectory(buf);
      for (const e of entries) {
        const lower = e.name.toLowerCase();
        if (candidates.includes(lower)) {
          const bytes = await extract(buf, e);
          const ext = lower.split(".").pop();
          texCache.set(key, { bytes, ext });
          console.log(`[tex] ${shaderName} <- ${basename(pk3)}/${e.name} (${bytes.length} bytes)`);
          return texCache.get(key);
        }
      }
    } catch {
      /* skip */
    }
  }
  return null;
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, "http://localhost");
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
});
