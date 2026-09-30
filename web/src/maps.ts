// Map catalog: fetches the padpork map list (via our server proxy) and
// provides search, random selection (with keywords), and favorites.

export interface MapInfo {
  map_name: string;
  normalized_name: string;
  game: string;
  longname: string | null;
  author: string | null;
  has_rocket_launcher: number;
  has_plasmagun: number;
  has_grenade_launcher: number;
  has_slick: number;
  has_water: number;
  has_slime: number;
  has_lava: number;
  has_fog: number;
  has_teleporter: number;
  has_jumppad: number;
  has_timer: number;
  record_count: number;
  best_time_ms: number | null;
  best_player_name: string | null;
}

interface Catalog {
  total: number;
  items: MapInfo[];
}

const CATALOG_KEY = "webrace.catalog";
const CATALOG_TS_KEY = "webrace.catalog.ts";
const FAVORITES_KEY = "webrace.favorites";
const CATALOG_MAX_AGE_MS = 7 * 24 * 60 * 60 * 1000; // re-fetch weekly

let catalogPromise: Promise<MapInfo[]> | null = null;

/** Fetch (and cache) the map catalog. */
async function fetchCatalog(): Promise<MapInfo[]> {
  const res = await fetch("/catalog");
  if (!res.ok) throw new Error(`catalog ${res.status}`);
  const data = (await res.json()) as Catalog;
  return data.items ?? [];
}

/** Load the catalog, using localStorage cache when fresh. */
export async function getCatalog(force = false): Promise<MapInfo[]> {
  if (catalogPromise) return catalogPromise;
  catalogPromise = (async () => {
    try {
      const ts = Number(localStorage.getItem(CATALOG_TS_KEY) ?? 0);
      const cached = localStorage.getItem(CATALOG_KEY);
      if (!force && cached && Date.now() - ts < CATALOG_MAX_AGE_MS) {
        return JSON.parse(cached) as MapInfo[];
      }
      const maps = await fetchCatalog();
      localStorage.setItem(CATALOG_KEY, JSON.stringify(maps));
      localStorage.setItem(CATALOG_TS_KEY, String(Date.now()));
      return maps;
    } catch (e) {
      // Fall back to cache if the network fetch failed.
      const cached = localStorage.getItem(CATALOG_KEY);
      if (cached) return JSON.parse(cached) as MapInfo[];
      console.warn("catalog unavailable:", e);
      return [];
    }
  })();
  return catalogPromise;
}

// ---- Favorites ----

export function getFavorites(): string[] {
  try {
    const raw = localStorage.getItem(FAVORITES_KEY);
    return raw ? (JSON.parse(raw) as string[]) : [];
  } catch {
    return [];
  }
}

export function isFavorite(mapName: string): boolean {
  return getFavorites().includes(mapName.toLowerCase());
}

export function toggleFavorite(mapName: string): string[] {
  const name = mapName.toLowerCase();
  const favs = getFavorites();
  const idx = favs.indexOf(name);
  if (idx >= 0) favs.splice(idx, 1);
  else favs.push(name);
  localStorage.setItem(FAVORITES_KEY, JSON.stringify(favs));
  return favs;
}

// ---- Search & random ----

const KEYWORD_FLAGS: Record<string, (m: MapInfo) => boolean> = {
  slick: (m) => !!m.has_slick,
  rocket: (m) => !!m.has_rocket_launcher,
  plasma: (m) => !!m.has_plasmagun,
  grenade: (m) => !!m.has_grenade_launcher,
  teleporter: (m) => !!m.has_teleporter,
  teleport: (m) => !!m.has_teleporter,
  jumppad: (m) => !!m.has_jumppad,
  water: (m) => !!m.has_water,
  slime: (m) => !!m.has_slime,
  lava: (m) => !!m.has_lava,
  fog: (m) => !!m.has_fog,
  timer: (m) => !!m.has_timer,
};

/** Search maps by a free-text query (substring match on name/longname/author). */
export function searchMaps(maps: MapInfo[], query: string): MapInfo[] {
  const q = query.trim().toLowerCase();
  if (!q) return maps;
  return maps.filter(
    (m) =>
      m.map_name.toLowerCase().includes(q) ||
      (m.normalized_name ?? "").toLowerCase().includes(q) ||
      (m.longname ?? "").toLowerCase().includes(q) ||
      (m.author ?? "").toLowerCase().includes(q),
  );
}

/** Parse a vote string like "random slick" or "mapname". */
export interface VoteRequest {
  kind: "specific" | "random" | "random-keyword";
  map?: string;
  keyword?: string;
}

export function parseVote(input: string): VoteRequest | null {
  const s = input.trim();
  if (!s) return null;
  const parts = s.split(/\s+/);
  const head = parts[0].toLowerCase();
  if (head === "random" || head === "randmap" || head === "vr") {
    if (parts.length >= 2) {
      const keyword = parts.slice(1).join(" ").toLowerCase();
      return { kind: "random-keyword", keyword };
    }
    return { kind: "random" };
  }
  if (head === "map" || head === "vote" || head === "vm") {
    const map = parts.slice(1).join(" ");
    if (!map) return null;
    return { kind: "specific", map };
  }
  // Bare map name -> specific vote.
  return { kind: "specific", map: s };
}

/** Resolve a vote request to a concrete map name (or null). */
export function resolveVote(maps: MapInfo[], req: VoteRequest): string | null {
  if (req.kind === "specific" && req.map) {
    const found = maps.find((m) => m.map_name.toLowerCase() === req.map!.toLowerCase());
    return found ? found.map_name : null;
  }
  if (req.kind === "random") {
    return randomMap(maps);
  }
  if (req.kind === "random-keyword" && req.keyword) {
    return randomMap(maps, req.keyword);
  }
  return null;
}

/** Pick a random map, optionally filtered by keyword. */
export function randomMap(maps: MapInfo[], keyword?: string): string | null {
  let pool = maps;
  if (keyword) {
    const flag = KEYWORD_FLAGS[keyword];
    if (flag) {
      pool = maps.filter(flag);
    } else {
      // Fall back to name/longname substring.
      pool = maps.filter(
        (m) =>
          m.map_name.toLowerCase().includes(keyword) ||
          (m.longname ?? "").toLowerCase().includes(keyword),
      );
    }
  }
  if (pool.length === 0) return null;
  return pool[Math.floor(Math.random() * pool.length)].map_name;
}
