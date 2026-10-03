// Q3/QFusion `.shader` script parsing (fetched from the map-server's
// /shader and /shaders endpoints). Used for the skybox and animated /
// emissive / transparent materials.

import { api } from "../base";

export interface ShaderStage {
  /** Frame texture paths (one for `map`, several for `animmap`). */
  maps: string[];
  animFreq: number;
  /** Raw `blendfunc` string (e.g. "add", "blend", "GL_DST_COLOR GL_ZERO"). */
  blend?: string;
  rgbGen?: string;
  /** tcMod scroll (s,t per second) if present. */
  scroll?: [number, number];
}

export interface ShaderDef {
  name: string;
  surfaceParms: Set<string>;
  skyparmsBase?: string;
  stages: ShaderStage[];
}

const cache = new Map<string, ShaderDef | null>();

/** Fetch + parse a shader script block by name (cached). */
export async function loadShader(name: string): Promise<ShaderDef | null> {
  const key = name.toLowerCase();
  if (cache.has(key)) return cache.get(key)!;
  let def: ShaderDef | null = null;
  try {
    const res = await fetch(api(`/shader?name=${encodeURIComponent(name)}`));
    if (res.ok) {
      def = parseShader(`${name} ${await res.text()}`, name);
    }
  } catch {
    def = null;
  }
  cache.set(key, def);
  return def;
}

/**
 * Bulk-fetch + parse shader defs for many names (one request), caching the
 * results. Returns a map of name -> def (absent/falsey if not found).
 */
export async function loadShaders(names: string[]): Promise<Map<string, ShaderDef | null>> {
  const out = new Map<string, ShaderDef | null>();
  const missing: string[] = [];
  for (const n of names) {
    const key = n.toLowerCase();
    if (cache.has(key)) out.set(key, cache.get(key)!);
    else missing.push(n);
  }
  if (missing.length) {
    try {
      const res = await fetch(api(`/shaders?names=${encodeURIComponent(missing.join(","))}`));
      if (res.ok) {
        const json = (await res.json()) as Record<string, string>;
        for (const n of missing) {
          const block = json[n];
          const def = block ? parseShader(`${n} ${block}`, n) : null;
          cache.set(n.toLowerCase(), def);
          out.set(n.toLowerCase(), def);
        }
      } else {
        for (const n of missing) cache.set(n.toLowerCase(), null);
      }
    } catch {
      /* leave missing unresolved */
    }
  }
  return out;
}

/** Tokenize, keeping `{`/`}` as tokens (C-style `//` comments stripped). */
function tokenize(source: string): string[] {
  const s = source.replace(/\/\/[^\n]*/g, " ");
  const tokens: string[] = [];
  let i = 0;
  while (i < s.length) {
    const c = s[i];
    if (/\s/.test(c)) {
      i++;
      continue;
    }
    if (c === "{" || c === "}") {
      tokens.push(c);
      i++;
      continue;
    }
    let j = i;
    while (j < s.length && !/\s/.test(s[j]) && s[j] !== "{" && s[j] !== "}") j++;
    tokens.push(s.slice(i, j).replace(/^"|"$/g, ""));
    i = j;
  }
  return tokens;
}

export function parseShader(source: string, fallbackName: string): ShaderDef {
  const tokens = tokenize(source);
  const surfaceParms = new Set<string>();
  const stages: ShaderStage[] = [];
  let skyparmsBase: string | undefined;
  let name = fallbackName;

  let i = 0;
  if (tokens[i] && tokens[i] !== "{") {
    name = tokens[i];
    i++;
  }
  if (tokens[i] === "{") i++;

  while (i < tokens.length) {
    const raw = tokens[i];
    const t = raw.toLowerCase();
    if (raw === "}") {
      i++;
      break;
    }
    if (t === "surfaceparm") {
      if (tokens[i + 1]) surfaceParms.add(tokens[i + 1].toLowerCase());
      i += 2;
    } else if (t === "skyparms") {
      skyparmsBase = tokens[i + 1];
      i += 4;
    } else if (raw === "{") {
      const stage: ShaderStage = { maps: [], animFreq: 0 };
      i++;
      while (i < tokens.length && tokens[i] !== "}") {
        const k = tokens[i].toLowerCase();
        if (k === "map" || k === "clampmap") {
          if (tokens[i + 1]) stage.maps.push(tokens[i + 1]);
          i += 2;
        } else if (k === "animmap") {
          stage.animFreq = parseFloat(tokens[i + 1] ?? "0") || 0;
          i += 2;
          while (i < tokens.length && tokens[i] !== "}" && !isStageKeyword(tokens[i])) {
            stage.maps.push(tokens[i]);
            i++;
          }
        } else if (k === "blendfunc") {
          const a = tokens[i + 1];
          const b = tokens[i + 2];
          if (a && b && isGlConst(b)) {
            stage.blend = `${a} ${b}`.toLowerCase();
            i += 3;
          } else {
            stage.blend = a?.toLowerCase();
            i += 2;
          }
        } else if (k === "rgbgen") {
          stage.rgbGen = tokens[i + 1];
          i += 2;
        } else if (k === "tcmod") {
          if (tokens[i + 1]?.toLowerCase() === "scroll") {
            stage.scroll = [parseFloat(tokens[i + 2] ?? "0") || 0, parseFloat(tokens[i + 3] ?? "0") || 0];
            i += 4;
          } else if (tokens[i + 1]?.toLowerCase() === "rotate") {
            i += 3;
          } else if (tokens[i + 1]?.toLowerCase() === "scale") {
            i += 4;
          } else {
            i += 2;
          }
        } else {
          i++;
        }
      }
      i++;
      if (stage.maps.length) stages.push(stage);
    } else {
      i++;
    }
  }

  return { name, surfaceParms, skyparmsBase, stages };
}

function isStageKeyword(t: string): boolean {
  return [
    "map",
    "clampmap",
    "animmap",
    "blendfunc",
    "rgbgen",
    "alphagen",
    "tcmod",
    "tcgen",
    "depthfunc",
    "depthwrite",
    "detail",
  ].includes(t.toLowerCase());
}

function isGlConst(t: string): boolean {
  return /^gl_/i.test(t);
}

export type BlendMode = "opaque" | "add" | "blend" | "filter";

function blendMode(blend?: string): BlendMode {
  if (!blend) return "opaque";
  const s = blend.toLowerCase();
  if (s === "add" || s.includes("gl_one gl_one")) return "add";
  if (s === "blend") return "blend";
  if (s === "filter" || s.includes("gl_dst_color gl_zero")) return "filter";
  return "opaque";
}

/**
 * A normalized render plan derived from a shader script: a base diffuse pass
 * (optionally lit by the lightmap, animated, scrolled, blended) plus additive
 * / blended overlay passes, in order.
 */
export interface ShaderPlan {
  lit: boolean;
  baseTex?: string;
  baseAnim?: string[];
  baseFreq: number;
  baseBlend: BlendMode;
  baseScroll?: [number, number];
  overlays: Array<{ tex: string; blend: BlendMode }>;
  noDraw: boolean;
}

/** Translate a `ShaderDef` into a `ShaderPlan` (null if not a drawable override). */
export function buildPlan(def: ShaderDef): ShaderPlan {
  const noDraw =
    def.surfaceParms.has("nodraw") ||
    def.surfaceParms.has("sky") ||
    def.surfaceParms.has("trigger") ||
    def.surfaceParms.has("playerclip") ||
    def.surfaceParms.has("clip");

  // Which stages carry a real texture vs `$lightmap`.
  const hasLightmapStage = def.stages.some((s) => s.maps.some((m) => m === "$lightmap"));
  const textureStages = def.stages.filter((s) => s.maps.some((m) => m !== "$lightmap" && m !== "$whiteimage"));

  const base = textureStages[0];
  const baseTex = base?.maps[0];
  const baseAnim = base && base.maps.length > 1 ? base.maps : undefined;
  const rawBlend = base ? blendMode(base.blend) : "opaque";
  // A "filter" base would multiply against the cleared background; treat it as
  // opaque (the lightmap multiply is handled separately).
  const baseBlend: BlendMode = rawBlend === "filter" ? "opaque" : rawBlend;

  // Apply the lightmap unless explicitly disabled, or unless the shader is
  // purely additive/transparent with no lightmap stage.
  const nolightmap = def.surfaceParms.has("nolightmap");
  const lit = !nolightmap && (hasLightmapStage || def.stages.length === 0 || baseBlend === "opaque");

  const overlays = textureStages.slice(1).map((s) => ({
    tex: s.maps[0],
    blend: blendMode(s.blend),
  }));

  return {
    lit,
    baseTex,
    baseAnim,
    baseFreq: base?.animFreq ?? 0,
    baseBlend,
    baseScroll: base?.scroll,
    overlays,
    noDraw,
  };
}
