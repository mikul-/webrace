// Map loading: find and extract a `.bsp` from a Warfork basewf/pk3 directory,
// or download from ws.q3df.org. For now this is a dev-only helper that reads
// a local directory (via the Vite dev server proxy to the browser filesystem
// is not possible), so it fetches maps via HTTP.

export interface MapMeta {
  name: string;
  url: string;
}

const WS_Q3DF = "https://ws.q3df.org/maps/";

function slug(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9_-]+/g, "");
}

/** Build a download URL for a map from ws.q3df.org. */
export function q3dfUrl(name: string): string {
  return `${WS_Q3DF}${slug(name)}.zip`;
}

/**
 * For the local dev flow, maps are served from `/maps/<name>.bsp` (a static
 * directory the server host can point at ~/.local/share/warfork-2.1).
 * In production we'll fetch from the backend which unzips pk3s on demand.
 */
export function localBspUrl(name: string): string {
  return `/maps/${name}.bsp`;
}

/**
 * Extract the `.bsp` bytes from a pk3 (zip). pk3 files are zip archives where
 * maps live at `maps/<name>.bsp`. In the MVP, the server extracts `.bsp`
 * files, so this is a placeholder for later client-side extraction.
 */
export async function bspFromPk3(_pk3Bytes: Uint8Array, _mapName: string): Promise<Uint8Array> {
  throw new Error("pk3 extraction happens server-side in MVP; load .bsp directly");
}

/** Fetch raw .bsp bytes. */
export async function fetchBsp(url: string): Promise<Uint8Array> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`fetch ${url}: ${res.status}`);
  const buf = await res.arrayBuffer();
  return new Uint8Array(buf);
}
