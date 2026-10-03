// Q3/QFusion `.shader` script parsing (fetched from the map-server's /shader
// endpoint). Used for the skybox and (later) animated/emissive/transparent
// materials. Only the fields we currently need are captured.

import { api } from "../base";

export interface ShaderStage {
  /** Frame texture paths (one for a plain `map`, several for `animmap`). */
  maps: string[];
  /** Frames per second for `animmap` (0 for a static `map`). */
  animFreq: number;
  /** `blendfunc` value if any (e.g. "add", "blend", "filter"). */
  blend?: string;
  /** `rgbGen` value if any (e.g. "identity", "vertex", "wave"). */
  rgbGen?: string;
}

export interface ShaderDef {
  name: string;
  surfaceParms: Set<string>;
  /** skyparms base path (e.g. "env/ame_siege/siege") if this is a sky shader. */
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
      const text = await res.text();
      def = parseShader(`${name} ${text}`, name);
    }
  } catch {
    def = null;
  }
  cache.set(key, def);
  return def;
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

/** Parse a shader script into a `ShaderDef`. Tolerant / best-effort. */
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
  if (tokens[i] === "{") i++; // enter the shader body

  while (i < tokens.length) {
    const raw = tokens[i];
    const t = raw.toLowerCase();
    if (raw === "}") {
      i++; // end of the shader body
      break;
    }
    if (t === "surfaceparm") {
      if (tokens[i + 1]) surfaceParms.add(tokens[i + 1].toLowerCase());
      i += 2;
    } else if (t === "skyparms") {
      skyparmsBase = tokens[i + 1];
      i += 4;
    } else if (raw === "{") {
      // A stage block.
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
          stage.blend = tokens[i + 1];
          i += isBlendOperand(tokens[i + 2]) ? 3 : 2;
        } else if (k === "rgbgen") {
          stage.rgbGen = tokens[i + 1];
          i += 2;
        } else {
          i++;
        }
      }
      i++; // consume '}'
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

function isBlendOperand(t: string | undefined): boolean {
  return (
    t !== undefined &&
    /^gl_/i.test(t)
  );
}
