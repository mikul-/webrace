//! QFusion `IBSP` parser.
//!
//! Layouts are verified against the Warfork `qfiles.h` header and the working
//! `bsp2mesh.py` in the author's `wf-tool`:
//!
//! - `dvertex_t`   = 44 bytes (point[3] f32, tex_st[2] f32, lm_st[2] f32,
//!   normal[3] f32, color[4] u8)
//! - `dface_t`     = 44 bytes (`"<11i"`: shadernum, fognum, facetype,
//!   firstvert, numverts, firstelem, numelems, lm_texnum, lm_offset[2],
//!   lm_size[2])
//! - `dshaderref_t`= 72 bytes (name[64] + flags i32 + contents i32)
//!
//! Coordinate transform to render space (three.js Y-up / Z-forward):
//! `q2t = (x, z, -y)` — same mapping `wf-tool` uses so ghosts line up.

#![allow(dead_code)]

const IDENT_IBSP: u32 = 0x5053_4249; // "IBSP"
const LUMP_ENTITIES: usize = 0;
const LUMP_SHADERREFS: usize = 1;
const LUMP_PLANES: usize = 2;
const LUMP_VERTEXES: usize = 10;
const LUMP_ELEMENTS: usize = 11;
const LUMP_FACES: usize = 13;
const LUMP_MODELS: usize = 7;
const LUMP_BRUSHES: usize = 8;
const LUMP_BRUSHSIDES: usize = 9;
const LUMP_LIGHTING: usize = 14;

pub const MAX_LUMPS: usize = 18;

const DVERTEX_SIZE: usize = 44;
// QFusion extends the Q3 104-byte dface_t; the header fields we care about
// (shadernum..numelems) are the first 11 ints (44 bytes). The stride is 104.
const DFACE_SIZE: usize = 104;
const DSHADERREF_SIZE: usize = 72;
const DPLANE_SIZE: usize = 16;
const DBRUSH_SIZE: usize = 12;
const DBRUSHSIDE_SIZE: usize = 8;
const DMODEL_SIZE: usize = 40;

pub const FACETYPE_PLANAR: i32 = 1;
pub const FACETYPE_PATCH: i32 = 2;
pub const FACETYPE_TRISURF: i32 = 3;

const CONTENTS_SOLID: i32 = 1;

pub const LIGHTMAP_W: usize = 128;
pub const LIGHTMAP_H: usize = 128;
pub const LIGHTMAP_BYTES: usize = 3;

/// A parsed map. Holds everything needed for both rendering and collision.
pub struct Bsp {
    pub name: String,
    /// Render-space triangle soup (x, y, z, u, v, lm_u, lm_v, nx, ny, nz, r, g, b, a)
    /// for every drawable face, sorted by shader. `indices` indexes into it.
    pub positions: Vec<f32>,
    pub indices: Vec<u32>,
    /// Draw chunks: (shader index, first index, index count). Faces are
    /// grouped by shader so each chunk can be drawn with one bound texture.
    pub chunks: Vec<(i32, u32, u32)>,
    /// Collision brushes: parallel planes, each brush is a convex hull defined
    /// by a run of planes in `brush_planes` (`brush_plane_offsets`).
    pub brush_plane_offsets: Vec<u32>,
    pub brush_plane_count: Vec<u32>,
    pub brush_plane_ids: Vec<u32>,
    pub planes: Vec<Plane>,
    pub shaders: Vec<String>,
    pub spawns: Vec<SpawnPoint>,
    /// Individual lightmap images (each LIGHTMAP_W×LIGHTMAP_H×3 bytes).
    pub lightmaps: Vec<Vec<u8>>,
    /// Packed lightmap atlas (RGB), and its width/height in pixels.
    pub lightmap_atlas: Vec<u8>,
    pub lightmap_atlas_w: u32,
    pub lightmap_atlas_h: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Plane {
    pub normal: [f32; 3],
    pub dist: f32,
}

/// A player spawn point (from `info_player_start` / `info_player_deathmatch`).
#[derive(Clone, Copy, Debug)]
pub struct SpawnPoint {
    /// Render-space origin (already `q2t = (x, z, -y)` transformed).
    pub origin: [f32; 3],
    /// Yaw in radians (from the entity `angle`, Quake convention).
    pub yaw: f32,
}

impl Bsp {
    /// Parse a raw `.bsp` byte buffer (already extracted from any `.pk3`).
    pub fn parse(name: &str, data: &[u8]) -> Result<Bsp, String> {
        if data.len() < 8 {
            return Err("bsp too short".into());
        }
        let ident = read_u32(data, 0);
        let version = read_i32(data, 4);
        if ident != IDENT_IBSP {
            return Err(format!("bad ident 0x{ident:08x} (want IBSP)"));
        }
        if version != 46 {
            return Err(format!("unexpected bsp version {version}"));
        }

        // Lump directory.
        let mut lumps = [(0u32, 0u32); MAX_LUMPS];
        let mut base = 8;
        for (_i, lump) in lumps.iter_mut().enumerate().take(MAX_LUMPS) {
            let off = read_u32(data, base);
            let len = read_u32(data, base + 4);
            *lump = (off, len);
            base += 8;
        }

        let shaders = parse_shaders(data, &lumps[LUMP_SHADERREFS])?;
        let planes = parse_planes(data, &lumps[LUMP_PLANES]);
        let lightmaps = parse_lightmaps(data, &lumps[LUMP_LIGHTING]);
        let (positions, indices, chunks) =
            parse_drawable(data, &lumps, &shaders, &lightmaps)?;
        let (brush_plane_offsets, brush_plane_count, brush_plane_ids) =
            parse_brushes(data, &lumps)?;
        let spawns = parse_spawns(data, &lumps[LUMP_ENTITIES]);
        let (lightmap_atlas, atlas_w, atlas_h) = build_lightmap_atlas(&lightmaps);

        Ok(Bsp {
            name: name.to_string(),
            positions,
            indices,
            chunks,
            brush_plane_offsets,
            brush_plane_count,
            brush_plane_ids,
            planes,
            shaders,
            spawns,
            lightmaps,
            lightmap_atlas,
            lightmap_atlas_w: atlas_w,
            lightmap_atlas_h: atlas_h,
        })
    }

    /// Total drawable triangle count.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

fn parse_shaders(data: &[u8], lump: &(u32, u32)) -> Result<Vec<String>, String> {
    let (off, len) = *lump;
    let n = len as usize / DSHADERREF_SIZE;
    let mut out = Vec::with_capacity(n);
    let mut i = off as usize;
    for _ in 0..n {
        let name_bytes = &data[i..i + 64];
        let end = name_bytes.iter().position(|&b| b == 0).unwrap_or(64);
        let name = String::from_utf8_lossy(&name_bytes[..end]).into_owned();
        out.push(name);
        i += DSHADERREF_SIZE;
    }
    Ok(out)
}

fn parse_planes(data: &[u8], lump: &(u32, u32)) -> Vec<Plane> {
    let (off, len) = *lump;
    let n = len as usize / DPLANE_SIZE;
    let mut out = Vec::with_capacity(n);
    let mut i = off as usize;
    for _ in 0..n {
        // Raw Quake plane; transform to render space with the same rigid
        // `q2t = (x, z, -y)` mapping applied to vertices and spawns.
        let (nx, ny, nz) = (read_f32(data, i), read_f32(data, i + 4), read_f32(data, i + 8));
        let dist = read_f32(data, i + 12);
        out.push(Plane { normal: [nx, nz, -ny], dist });
        i += DPLANE_SIZE;
    }
    out
}

/// Split the LIGHTING lump into individual 128×128×3 lightmap images.
fn parse_lightmaps(data: &[u8], lump: &(u32, u32)) -> Vec<Vec<u8>> {
    let (off, len) = *lump;
    let per = LIGHTMAP_W * LIGHTMAP_H * LIGHTMAP_BYTES;
    let n = len as usize / per;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let s = off as usize + i * per;
        out.push(data[s..s + per].to_vec());
    }
    out
}

/// Pack all lightmaps into a single RGB atlas (grid of LIGHTMAP_W cells).
fn build_lightmap_atlas(lightmaps: &[Vec<u8>]) -> (Vec<u8>, u32, u32) {
    let cols = 4usize;
    let rows = lightmaps.len().div_ceil(cols).max(1);
    let w = cols * LIGHTMAP_W;
    let h = rows * LIGHTMAP_H;
    let mut atlas = vec![0u8; w * h * LIGHTMAP_BYTES];
    for (i, lm) in lightmaps.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;
        let ox = col * LIGHTMAP_W;
        let oy = row * LIGHTMAP_H;
        for y in 0..LIGHTMAP_H {
            let dst = ((oy + y) * w + ox) * LIGHTMAP_BYTES;
            let src = y * LIGHTMAP_W * LIGHTMAP_BYTES;
            atlas[dst..dst + LIGHTMAP_W * LIGHTMAP_BYTES]
                .copy_from_slice(&lm[src..src + LIGHTMAP_W * LIGHTMAP_BYTES]);
        }
    }
    (atlas, w as u32, h as u32)
}

fn parse_drawable(
    data: &[u8],
    lumps: &[(u32, u32)],
    shaders: &[String],
    lightmaps: &[Vec<u8>],
) -> Result<(Vec<f32>, Vec<u32>, Vec<(i32, u32, u32)>), String> {
    let (voff, vlen) = lumps[LUMP_VERTEXES];
    let (eoff, _elen) = lumps[LUMP_ELEMENTS];
    let (foff, flen) = lumps[LUMP_FACES];

    let nverts = vlen as usize / DVERTEX_SIZE;
    let nfaces = flen as usize / DFACE_SIZE;

    // Read raw vertex data once.
    // raw_v[vi] = (x, y, z, tu, tv, lu, lv, nx, ny, nz, r, g, b, a)
    let mut raw_v: Vec<[f32; 14]> = Vec::with_capacity(nverts);
    for vi in 0..nverts {
        let p = voff as usize + vi * DVERTEX_SIZE;
        let (x, y, z) = (read_f32(data, p), read_f32(data, p + 4), read_f32(data, p + 8));
        let (tu, tv) = (read_f32(data, p + 12), read_f32(data, p + 16));
        let (lu, lv) = (read_f32(data, p + 20), read_f32(data, p + 24));
        let (nx, ny, nz) = (read_f32(data, p + 28), read_f32(data, p + 32), read_f32(data, p + 36));
        let (r, g, b, a) = (
            data[p + 40] as f32 / 255.0,
            data[p + 41] as f32 / 255.0,
            data[p + 42] as f32 / 255.0,
            data[p + 43] as f32 / 255.0,
        );
        raw_v.push([x, y, z, tu, tv, lu, lv, nx, ny, nz, r, g, b, a]);
    }

    let lm_count = lightmaps.len().max(1);
    let atlas_cols = 4usize;
    let atlas_rows = lightmap_atlas_rows(lm_count, atlas_cols);
    let atlas_w = (atlas_cols * LIGHTMAP_W) as f32;
    let atlas_h = (atlas_rows * LIGHTMAP_H) as f32;

    // First pass: collect drawable faces grouped by shader.
    // Group drawable faces by shader, carrying each face's lm_texnum.
    let mut groups: Vec<Vec<Vec<usize>>> = vec![Vec::new(); shaders.len()];
    let mut lm_per_face: Vec<Vec<i32>> = vec![Vec::new(); shaders.len()];
    for fi in 0..nfaces {
        let f = foff as usize + fi * DFACE_SIZE;
        let shadernum = read_i32(data, f) as usize;
        let facetype = read_i32(data, f + 8);
        let firstvert = read_i32(data, f + 12) as usize;
        let firstelem = read_i32(data, f + 20) as usize;
        let numelems = read_i32(data, f + 24) as usize;
        let lm_texnum = read_i32(data, f + 28);

        if facetype != FACETYPE_PLANAR && facetype != FACETYPE_TRISURF {
            continue;
        }
        let shader_name = shaders.get(shadernum).map(|s| s.as_str()).unwrap_or("");
        if is_nodraw(shader_name) {
            continue;
        }
        let mut idxs = Vec::with_capacity(numelems);
        for e in 0..numelems {
            let ei = eoff as usize + (firstelem + e) * 4;
            let raw = read_i32(data, ei);
            idxs.push((raw + firstvert as i32) as usize);
        }
        groups[shadernum].push(idxs);
        lm_per_face[shadernum].push(lm_texnum);
    }

    let mut rv: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut chunks: Vec<(i32, u32, u32)> = Vec::new();

    for shadernum in 0..shaders.len() {
        if groups[shadernum].is_empty() {
            continue;
        }
        let first_index = indices.len() as u32;
        let face_list = &groups[shadernum];
        let lm_list = &lm_per_face[shadernum];
        for (face_idx, face_verts) in face_list.iter().enumerate() {
            let lm_texnum = lm_list[face_idx];
            let (atlas_x, atlas_y) = lightmap_atlas_origin(lm_texnum, atlas_cols);
            let base = rv.len() as u32 / 14;
            for &vi in face_verts {
                let v = raw_v[vi];
                let au = (atlas_x as f32 + v[5] * LIGHTMAP_W as f32) / atlas_w;
                let av = (atlas_y as f32 + v[6] * LIGHTMAP_H as f32) / atlas_h;
                rv.extend_from_slice(&[
                    v[0], v[2], -v[1],
                    v[3], v[4],
                    au, av,
                    v[7], v[9], -v[8],
                    v[10], v[11], v[12], v[13],
                ]);
            }
            for k in 0..face_verts.len() as u32 {
                indices.push(base + k);
            }
        }
        let first = first_index;
        let count = indices.len() as u32 - first;
        chunks.push((shadernum as i32, first, count));
    }

    Ok((rv, indices, chunks))
}

fn lightmap_atlas_origin(lm_texnum: i32, atlas_cols: usize) -> (usize, usize) {
    let idx = lm_texnum.max(0) as usize;
    let col = idx % atlas_cols;
    let row = idx / atlas_cols;
    (col * LIGHTMAP_W, row * LIGHTMAP_H)
}

fn lightmap_atlas_rows(lm_count: usize, atlas_cols: usize) -> usize {
    lm_count.div_ceil(atlas_cols).max(1)
}

fn parse_brushes(
    data: &[u8],
    lumps: &[(u32, u32)],
) -> Result<(Vec<u32>, Vec<u32>, Vec<u32>), String> {
    let (_moff, mlen) = lumps[LUMP_MODELS];
    let (boff, blen) = lumps[LUMP_BRUSHES];
    let (soff, slen) = lumps[LUMP_BRUSHSIDES];

    if mlen < DMODEL_SIZE as u32 {
        return Err("missing model lump".into());
    }
    // Model 0 is the world; its `firstbrush`/`numbrushes` give the world hull.
    // dmodel_t layout: mins[3] maxs[3] firstface numfaces firstbrush numbrushes
    // (10 ints = 40 bytes).
    let firstbrush = read_i32(data, _moff as usize + 32) as u32;
    let numbrushes = read_i32(data, _moff as usize + 36) as u32;
    let _nbrushes_total = blen as usize / DBRUSH_SIZE;
    let _nbrushsides = slen as usize / DBRUSHSIDE_SIZE;

    let mut offsets = Vec::new();
    let mut counts = Vec::new();
    let mut plane_ids = Vec::new();

    for bi in 0..numbrushes {
        let b = boff as usize + (firstbrush as usize + bi as usize) * DBRUSH_SIZE;
        let _bsoff = read_i32(data, b) as u32;
        let bsnum = read_i32(data, b + 4) as u32;
        let contents = read_i32(data, b + 8);

        // Only solid brushes matter for pmove traces; skip triggers/water/etc.
        // Playerclip is handled separately (kept here as solid for simplicity).
        if contents & CONTENTS_SOLID == 0 && contents & 0x10000 == 0 {
            continue;
        }

        offsets.push(plane_ids.len() as u32);
        counts.push(bsnum);
        for s in 0..bsnum {
            let sp = soff as usize + (_bsoff as usize + s as usize) * DBRUSHSIDE_SIZE;
            let planenum = read_i32(data, sp) as u32;
            plane_ids.push(planenum);
        }
    }

    Ok((offsets, counts, plane_ids))
}

/// Parse player spawn points from the entity lump (text key/value blocks).
fn parse_spawns(data: &[u8], lump: &(u32, u32)) -> Vec<SpawnPoint> {
    let (off, len) = *lump;
    let text = String::from_utf8_lossy(&data[off as usize..(off + len) as usize]);
    let mut out = Vec::new();

    // Entities are `{ ... }` blocks of `"key" "value"` pairs.
    for block in text.split('{').skip(1) {
        let Some(_end) = block.find('}') else { continue };
        let block = &block[..block.find('}').unwrap_or(block.len())];

        let mut classname = None;
        let mut origin: Option<[f32; 3]> = None;
        let mut angle: f32 = 0.0;

        let mut tokens: Vec<&str> = Vec::new();
        for chunk in block.split('"') {
            let c = chunk.trim();
            if !c.is_empty() {
                tokens.push(c);
            }
        }
        let mut i = 0;
        while i + 1 < tokens.len() {
            let key = tokens[i];
            let val = tokens[i + 1];
            match key {
                "classname" => classname = Some(val.to_string()),
                "origin" => {
                    let mut parts = val.split_whitespace();
                    if let (Some(x), Some(y), Some(z)) = (
                        parts.next().and_then(|s| s.parse().ok()),
                        parts.next().and_then(|s| s.parse().ok()),
                        parts.next().and_then(|s| s.parse().ok()),
                    ) {
                        origin = Some([x, y, z]);
                    }
                }
                "angle" => {
                    angle = val.trim().parse::<f32>().unwrap_or(0.0);
                }
                _ => {}
            }
            i += 2;
        }

        let is_start = matches!(
            classname.as_deref(),
            Some("target_startTimer") | Some("target_start")
        );
        let is_spawn = matches!(
            classname.as_deref(),
            Some("info_player_start") | Some("info_player_deathmatch") | Some("info_player_intermission")
        );
        if (is_start || is_spawn) && origin.is_some() {
            let [x, y, z] = origin.unwrap();
            // Quake -> render space: q2t = (x, z, -y).
            let render = [x, z, -y];
            // Quake "angle" is yaw in degrees. Convert to radians.
            let yaw = -angle.to_radians();
            let point = SpawnPoint { origin: render, yaw };
            if is_spawn {
                // Prefer actual player spawns (info_player_*); they are where
                // the player physically stands. Start timers (virtual race
                // start lines) are only a fallback if no spawn exists.
                out.insert(0, point);
            } else {
                out.push(point);
            }
        }
    }
    out
}

/// Shaders matching these substrings are not drawn (same skip list as wf-tool).
fn is_nodraw(name: &str) -> bool {
    const SKIP: &[&str] = &[
        "sky", "nodraw", "caulk", "trigger", "weaponclip", "clip", "common/ladder",
        "hint", "skip", "portal", "areaportal", "playerclip", "monsterclip",
        "do_not_enter",
    ];
    SKIP.iter().any(|s| name.to_ascii_lowercase().contains(s))
}

#[inline]
fn read_u32(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}
#[inline]
fn read_i32(d: &[u8], o: usize) -> i32 {
    i32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}
#[inline]
fn read_f32(d: &[u8], o: usize) -> f32 {
    f32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}
