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
//! Coordinates are used in Quake's native Z-up space directly (no rotation),
//! matching the physics (`pmove`) and camera (`lookAt`) which are both Z-up.

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

/// Contents flags (Q3/QFusion `qfiles.h`) carried per-shader by LUMP_SHADERREFS.
/// Used to classify liquid / jumppad / teleporter surfaces for gameplay.
pub const CONTENTS_SOLID: i32 = 1;
pub const CONTENTS_STRUCTURAL: i32 = 0x1000_0000;
pub const CONTENTS_LAVA: i32 = 8;
pub const CONTENTS_SLIME: i32 = 16;
pub const CONTENTS_WATER: i32 = 32;
pub const CONTENTS_FOG: i32 = 64;
pub const CONTENTS_PLAYERCLIP: i32 = 0x10000;
pub const CONTENTS_TELEPORTER: i32 = 0x40000;
pub const CONTENTS_JUMPPAD: i32 = 0x80000;
pub const CONTENTS_TRIGGER: i32 = 0x4000_0000;

/// Surface flags (Q3/QFusion `qfiles.h`).
pub const SURF_NODRAW: i32 = 0x80;

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
    /// Shader index of each collision brush (parallel to the brush arrays).
    pub brush_shaders: Vec<i32>,
    pub planes: Vec<Plane>,
    pub shaders: Vec<String>,
    /// Surface flags (SURF_SLICK etc.) per shader.
    pub shader_flags: Vec<i32>,
    /// Contents flags (CONTENTS_WATER etc.) per shader, from LUMP_SHADERREFS.
    pub shader_contents: Vec<i32>,
    /// Contents flags per collision brush (parallel to brush arrays), derived
    /// from the brush's shader. Used for PointContents (water/lava/etc.).
    pub brush_contents: Vec<i32>,
    pub spawns: Vec<SpawnPoint>,
    /// Race checkpoint triggers (start, checkpoints, finish). Ordered by the
    /// map's intended sequence (start first, then checkpoints, then finish).
    pub race_gates: Vec<RaceGate>,
    /// Jumppad trigger volumes (`trigger_push`).
    pub jumppads: Vec<Jumppad>,
    /// Teleporter trigger volumes (`trigger_teleport`).
    pub teleporters: Vec<Teleporter>,
    /// Plane ids of trigger brushes (shared backing store for `Jumppad`/
    /// `Teleporter` plane runs).
    pub trigger_plane_ids: Vec<u32>,
    /// Animated solid brush entities (`func_bobbing`/`func_plat`/...).
    pub movers: Vec<MoverDef>,
    /// Plane ids backing the mover brush runs.
    pub mover_plane_ids: Vec<u32>,
    /// Render geometry for movers (separate buffer from the static world so
    /// each mover can be drawn with its own model matrix).
    pub mover_positions: Vec<f32>,
    pub mover_indices: Vec<u32>,
    pub mover_chunks: Vec<MoverChunk>,
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

/// A race gate: the trigger volume (AABB) and its kind (start/checkpoint/finish).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaceGateKind {
    Start,
    Checkpoint,
    Finish,
}

#[derive(Clone, Copy, Debug)]
pub struct RaceGate {
    pub kind: RaceGateKind,
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
}

/// A jumppad (`trigger_push`): a trigger volume that launches the player along
/// the precomputed ballistic velocity `velocity` (reaching the target origin).
///
/// The volume is described by its actual brush (`plane range` into the shared
/// `planes` array) rather than just an AABB, so diagonal/slanted pads trigger
/// correctly. `mins`/`maxs` are the brush AABB for fast rejection.
#[derive(Clone, Copy, Debug)]
pub struct Jumppad {
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    pub plane_off: u32,
    pub plane_count: u32,
    /// The velocity to set on the player (origin2 in Q3 terms).
    pub velocity: [f32; 3],
}

/// A teleporter (`trigger_teleport`): a trigger volume that instantly moves the
/// player to `dest_origin`, preserving velocity and horizontal view direction.
#[derive(Clone, Copy, Debug)]
pub struct Teleporter {
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    pub plane_off: u32,
    pub plane_count: u32,
    pub dest_origin: [f32; 3],
}

/// A class of animated solid brush entity. `Static` is not produced by
/// `parse_movers`; non-animated `func_*` entities keep their existing static
/// collision/rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoverKind {
    Bobbing,
    Plat,
    Door,
    DoorRotating,
    Train,
    Rotating,
    Pendulum,
}

/// A parsed animated brush entity: its class keys plus the submodel's brush
/// plane run. The runtime mover (`trace::Mover`) is built from this and
/// animated in the sim.
#[derive(Clone, Debug)]
pub struct MoverDef {
    pub kind: MoverKind,
    /// Submodel index (`model "*N"` → N).
    pub model: usize,
    /// Entity `origin` key (spawn pivot/local-frame origin), default [0,0,0].
    pub origin: [f32; 3],
    /// Entity `angles` key in radians `[pitch, yaw, roll]` (mostly zero).
    pub angles: [f32; 3],
    /// Entity `angle` key in degrees (door movedir; -1 up, -2 down).
    pub angle: f32,
    pub height: f32,
    pub speed: f32,
    pub phase: f32,
    pub wait: f32,
    /// `func_door_rotating` `distance` (degrees).
    pub distance: f32,
    pub spawnflags: i32,
    /// `func_train` path (corner origins + per-corner waits), in order.
    pub path: Vec<[f32; 3]>,
    pub path_wait: Vec<f32>,
    /// Plane run into `Bsp::mover_plane_ids`.
    pub plane_off: u32,
    pub plane_count: u32,
    /// Raw submodel AABB.
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
}

/// A draw range of a moving brush entity: `(mover index, shader, first index,
/// index count)` into the mover render buffers.
#[derive(Clone, Copy, Debug)]
pub struct MoverChunk {
    pub mover: usize,
    pub shader: i32,
    pub first: u32,
    pub count: u32,
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

        let (shaders, shader_flags, shader_contents) = parse_shaders(data, &lumps[LUMP_SHADERREFS])?;
        let planes = parse_planes(data, &lumps[LUMP_PLANES]);
        let lightmaps = parse_lightmaps(data, &lumps[LUMP_LIGHTING]);

        // Animated solid brush entities, parsed before the drawable pass so the
        // renderer can separate their faces from the static world geometry.
        let (movers, mover_plane_ids) =
            parse_movers(data, &lumps, &lumps[LUMP_ENTITIES]);
        let mover_model_to_idx: std::collections::HashMap<usize, usize> = movers
            .iter()
            .enumerate()
            .map(|(i, m)| (m.model, i))
            .collect();

        let (positions, indices, chunks, mover_positions, mover_indices, mover_chunks) =
            parse_drawable(data, &lumps, &shaders, &lightmaps, &mover_model_to_idx)?;
        let (
            mut brush_plane_offsets,
            mut brush_plane_count,
            mut brush_plane_ids,
            mut brush_shaders,
            mut brush_contents,
        ) = parse_brushes(data, &lumps, &shader_contents)?;
        // Add *static* solid brush-model entities (func_static, func_button, ...)
        // to the collision world at their base position. Animated movers are
        // handled separately (they collide at their current transform).
        {
            let (models, model_planes) = parse_solid_brush_models(data, &lumps, &shader_contents);
            let base = brush_plane_ids.len() as u32;
            for (off, count, shader, contents) in models {
                brush_plane_offsets.push(base + off);
                brush_plane_count.push(count);
                brush_shaders.push(shader);
                brush_contents.push(contents);
            }
            brush_plane_ids.extend_from_slice(&model_planes);
        }
        let spawns = parse_spawns(data, &lumps[LUMP_ENTITIES]);
        let models = parse_models(data, &lumps[LUMP_MODELS]);
        let race_gates = parse_race(data, &lumps[LUMP_ENTITIES], &models);
        let submodels = parse_submodels(data, &lumps[LUMP_MODELS]);
        let (jumppads, teleporters, trigger_plane_ids) =
            parse_triggers(data, &lumps, &lumps[LUMP_ENTITIES], &submodels);
        let (lightmap_atlas, atlas_w, atlas_h) = build_lightmap_atlas(&lightmaps);

        Ok(Bsp {
            name: name.to_string(),
            positions,
            indices,
            chunks,
            mover_positions,
            mover_indices,
            mover_chunks,
            brush_plane_offsets,
            brush_plane_count,
            brush_plane_ids,
            brush_shaders,
            planes,
            shaders,
            shader_flags,
            shader_contents,
            brush_contents,
            spawns,
            race_gates,
            jumppads,
            teleporters,
            trigger_plane_ids,
            movers,
            mover_plane_ids,
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

fn parse_shaders(data: &[u8], lump: &(u32, u32)) -> Result<(Vec<String>, Vec<i32>, Vec<i32>), String> {
    let (off, len) = *lump;
    let n = len as usize / DSHADERREF_SIZE;
    let mut names = Vec::with_capacity(n);
    let mut flags = Vec::with_capacity(n);
    let mut contents = Vec::with_capacity(n);
    let mut i = off as usize;
    for _ in 0..n {
        let name_bytes = &data[i..i + 64];
        let end = name_bytes.iter().position(|&b| b == 0).unwrap_or(64);
        let name = String::from_utf8_lossy(&name_bytes[..end]).into_owned();
        let surf_flags = read_i32(data, i + 64);
        let contents_flags = read_i32(data, i + 68);
        names.push(name);
        flags.push(surf_flags);
        contents.push(contents_flags);
        i += DSHADERREF_SIZE;
    }
    Ok((names, flags, contents))
}

fn parse_planes(data: &[u8], lump: &(u32, u32)) -> Vec<Plane> {
    let (off, len) = *lump;
    let n = len as usize / DPLANE_SIZE;
    let mut out = Vec::with_capacity(n);
    let mut i = off as usize;
    for _ in 0..n {
        // Raw Quake plane (Z-up), unchanged — matches our physics/camera space.
        let (nx, ny, nz) = (read_f32(data, i), read_f32(data, i + 4), read_f32(data, i + 8));
        let dist = read_f32(data, i + 12);
        out.push(Plane { normal: [nx, ny, nz], dist });
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
    mover_models: &std::collections::HashMap<usize, usize>,
) -> Result<
    (
        Vec<f32>,
        Vec<u32>,
        Vec<(i32, u32, u32)>,
        Vec<f32>,
        Vec<u32>,
        Vec<MoverChunk>,
    ),
    String,
> {
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
        // Keep Quake's native Z-up coordinates (matches our physics/camera).
        raw_v.push([x, y, z, tu, tv, lu, lv, nx, ny, nz, r, g, b, a]);
    }

    let lm_count = lightmaps.len().max(1);
    let atlas_cols = 4usize;
    let atlas_rows = lightmap_atlas_rows(lm_count, atlas_cols);
    let atlas_w = (atlas_cols * LIGHTMAP_W) as f32;
    let atlas_h = (atlas_rows * LIGHTMAP_H) as f32;

    // Map each face to its submodel (dmodel_t `firstface`/`numfaces` at byte
    // offsets 24/28). Model 0 is the world; models 1..n are entities.
    let (moff, mlen) = lumps[LUMP_MODELS];
    let nmodels = mlen as usize / DMODEL_SIZE;
    let mut face_model = vec![0usize; nfaces];
    for mi in 0..nmodels {
        let p = moff as usize + mi * DMODEL_SIZE;
        let firstface = read_i32(data, p + 24).max(0) as usize;
        let numfaces = read_i32(data, p + 28).max(0) as usize;
        let end = (firstface + numfaces).min(nfaces);
        for fm in face_model.iter_mut().take(end).skip(firstface) {
            *fm = mi;
        }
    }

    // First pass: collect drawable faces grouped by shader.
    // Each face is either planar/trisurf (an element-index run) or a bezier
    // patch (a control-point grid, tessellated during the emit pass).
    #[derive(Clone)]
    enum FaceRef {
        Planar(Vec<usize>),
        Patch { firstvert: usize, cp_w: usize, cp_h: usize },
    }
    let mut groups: Vec<Vec<(FaceRef, i32, usize)>> = vec![Vec::new(); shaders.len()];
    for fi in 0..nfaces {
        let f = foff as usize + fi * DFACE_SIZE;
        let shadernum = read_i32(data, f) as usize;
        let facetype = read_i32(data, f + 8);
        let firstvert = read_i32(data, f + 12) as usize;
        let numverts = read_i32(data, f + 16) as usize;
        let firstelem = read_i32(data, f + 20) as usize;
        let numelems = read_i32(data, f + 24) as usize;
        let lm_texnum = read_i32(data, f + 28);
        let model = face_model[fi];

        let shader_name = shaders.get(shadernum).map(|s| s.as_str()).unwrap_or("");
        if is_nodraw(shader_name) {
            continue;
        }

        match facetype {
            FACETYPE_PLANAR | FACETYPE_TRISURF => {
                let mut idxs = Vec::with_capacity(numelems);
                for e in 0..numelems {
                    let ei = eoff as usize + (firstelem + e) * 4;
                    let raw = read_i32(data, ei);
                    idxs.push((raw + firstvert as i32) as usize);
                }
                groups[shadernum].push((FaceRef::Planar(idxs), lm_texnum, model));
            }
            FACETYPE_PATCH => {
                // Control-point grid dimensions are the last two fields of the
                // 104-byte `dface_t` (patch_cp[2] at byte offsets 96 and 100).
                let cp_w = read_i32(data, f + 96) as usize;
                let cp_h = read_i32(data, f + 100) as usize;
                if cp_w < 2 || cp_h < 2 || numverts != cp_w * cp_h {
                    continue; // malformed patch
                }
                groups[shadernum].push((
                    FaceRef::Patch { firstvert, cp_w, cp_h },
                    lm_texnum,
                    model,
                ));
            }
            _ => continue,
        }
    }

    let mut rv: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut chunks: Vec<(i32, u32, u32)> = Vec::new();
    // Movers get their own buffer so each can be drawn with its own transform.
    let mut mover_rv: Vec<f32> = Vec::new();
    let mut mover_indices: Vec<u32> = Vec::new();
    let mut mover_chunks: Vec<MoverChunk> = Vec::new();

    // Emit one face's geometry into the given vertex/index buffers.
    let emit_face = |face_ref: &FaceRef,
                     lm_texnum: i32,
                     rv: &mut Vec<f32>,
                     indices: &mut Vec<u32>| {
        let (atlas_x, atlas_y) = lightmap_atlas_origin(lm_texnum, atlas_cols);
        match face_ref {
            FaceRef::Planar(face_verts) => {
                let base = rv.len() as u32 / 14;
                for &vi in face_verts.iter() {
                    emit_vertex(rv, raw_v[vi], atlas_x, atlas_y, atlas_w, atlas_h);
                }
                for k in 0..face_verts.len() as u32 {
                    indices.push(base + k);
                }
            }
            FaceRef::Patch { firstvert, cp_w, cp_h } => {
                let base = rv.len() as u32 / 14;
                let tess = 8usize; // subdivisions per patch dimension
                patch_tessellate(
                    rv,
                    &raw_v,
                    *firstvert,
                    *cp_w,
                    *cp_h,
                    tess,
                    atlas_x,
                    atlas_y,
                    atlas_w,
                    atlas_h,
                );
                // n×n sub-patches → indices for a triangle grid.
                let n = tess + 1;
                for py in 0..tess {
                    for px in 0..tess {
                        let i0 = base + (py * n + px) as u32;
                        let i1 = base + (py * n + px + 1) as u32;
                        let i2 = base + ((py + 1) * n + px) as u32;
                        let i3 = base + ((py + 1) * n + px + 1) as u32;
                        indices.extend_from_slice(&[i0, i1, i2, i1, i3, i2]);
                    }
                }
            }
        }
    };

    for shadernum in 0..shaders.len() {
        if groups[shadernum].is_empty() {
            continue;
        }
        let world_first = indices.len() as u32;
        let mut world_any = false;
        for (face_ref, lm_texnum, model) in &groups[shadernum] {
            if let Some(&mi) = mover_models.get(model) {
                let mfirst = mover_indices.len() as u32;
                emit_face(face_ref, *lm_texnum, &mut mover_rv, &mut mover_indices);
                mover_chunks.push(MoverChunk {
                    mover: mi,
                    shader: shadernum as i32,
                    first: mfirst,
                    count: mover_indices.len() as u32 - mfirst,
                });
            } else {
                emit_face(face_ref, *lm_texnum, &mut rv, &mut indices);
                world_any = true;
            }
        }
        if world_any {
            chunks.push((
                shadernum as i32,
                world_first,
                indices.len() as u32 - world_first,
            ));
        }
    }

    Ok((rv, indices, chunks, mover_rv, mover_indices, mover_chunks))
}

/// Append a single interleaved render vertex (14 f32) to `rv`, remapping the
/// per-face lightmap (lu,lv) into atlas-space UVs.
#[inline]
fn emit_vertex(
    rv: &mut Vec<f32>,
    v: [f32; 14],
    atlas_x: usize,
    atlas_y: usize,
    atlas_w: f32,
    atlas_h: f32,
) {
    let au = (atlas_x as f32 + v[5] * LIGHTMAP_W as f32) / atlas_w;
    let av = (atlas_y as f32 + v[6] * LIGHTMAP_H as f32) / atlas_h;
    // 14 floats per vertex: pos(3) tex(2) lm(2) normal(3) color(4)
    rv.extend_from_slice(&[
        v[0], v[1], v[2],   // pos (Z-up)
        v[3], v[4],          // tex uv
        au, av,              // lightmap atlas uv
        v[7], v[8], v[9],    // normal (Z-up)
        v[10], v[11], v[12], v[13], // r,g,b,a
    ]);
}

/// Tessellate a Q3-style bicubic Bézier patch (control points from the vertex
/// lump, laid out as a `cp_w × cp_h` grid) into a `tess × tess` grid of
/// vertices, appended to `rv` row-major. Bilinear interpolation of all vertex
/// attributes (position, tex, lightmap, normal, color) via Bernstein blending.
fn patch_tessellate(
    rv: &mut Vec<f32>,
    raw_v: &[[f32; 14]],
    firstvert: usize,
    cp_w: usize,
    cp_h: usize,
    tess: usize,
    atlas_x: usize,
    atlas_y: usize,
    atlas_w: f32,
    atlas_h: f32,
) {
    let n = tess + 1; // vertices per side
    for py in 0..n {
        let v = py as f32 / tess as f32;
        for px in 0..n {
            let u = px as f32 / tess as f32;
            // Evaluate the Bézier surface at (u, v).
            let mut acc = [0.0f32; 14];
            let mut wsum = 0.0f32;
            // Blend over control points (u across columns, v across rows).
            for cpy in 0..cp_h {
                let bv = bernstein(v, cpy, cp_h - 1);
                if bv == 0.0 {
                    continue;
                }
                for cpx in 0..cp_w {
                    let bu = bernstein(u, cpx, cp_w - 1);
                    let w = bu * bv;
                    if w == 0.0 {
                        continue;
                    }
                    let cp = raw_v[firstvert + cpy * cp_w + cpx];
                    for k in 0..14 {
                        acc[k] += cp[k] * w;
                    }
                    wsum += w;
                }
            }
            // Normalize (should be ~1.0, but guard against degenerate patches).
            let inv = if wsum > 0.0 { 1.0 / wsum } else { 1.0 };
            let mut out = acc;
            // Re-normalize the interpolated normal (blending shrinks it).
            let nx = out[7] * inv;
            let ny = out[8] * inv;
            let nz = out[9] * inv;
            let nlen = (nx * nx + ny * ny + nz * nz).sqrt();
            if nlen > 1e-8 {
                out[7] = nx / nlen;
                out[8] = ny / nlen;
                out[9] = nz / nlen;
            } else {
                out[7] = 0.0;
                out[8] = 0.0;
                out[9] = 1.0;
            }
            for k in 0..13 {
                out[k] *= inv;
            }
            emit_vertex(rv, out, atlas_x, atlas_y, atlas_w, atlas_h);
        }
    }
}

/// Nth-order Bernstein polynomial basis for degree `deg`.
fn bernstein(t: f32, i: usize, deg: usize) -> f32 {
    if i > deg {
        return 0.0;
    }
    // C(deg, i) * t^i * (1-t)^(deg-i)
    let c = binomial(deg, i) as f32;
    c * t.powi(i as i32) * (1.0 - t).powi((deg - i) as i32)
}

fn binomial(n: usize, k: usize) -> usize {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut r = 1usize;
    for i in 0..k {
        r = r * (n - i) / (i + 1);
    }
    r
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
    shader_contents: &[i32],
) -> Result<(Vec<u32>, Vec<u32>, Vec<u32>, Vec<i32>, Vec<i32>), String> {
    let (_moff, mlen) = lumps[LUMP_MODELS];
    let (boff, blen) = lumps[LUMP_BRUSHES];
    let (soff, slen) = lumps[LUMP_BRUSHSIDES];

    if mlen < DMODEL_SIZE as u32 {
        return Err("missing model lump".into());
    }
    // Model 0 is the world; its `firstbrush`/`numbrushes` give the world hull.
    let firstbrush = read_i32(data, _moff as usize + 32) as u32;
    let numbrushes = read_i32(data, _moff as usize + 36) as u32;
    let _nbrushes_total = blen as usize / DBRUSH_SIZE;
    let _nbrushsides = slen as usize / DBRUSHSIDE_SIZE;

    let mut offsets = Vec::new();
    let mut counts = Vec::new();
    let mut plane_ids = Vec::new();
    let mut shaders = Vec::new();
    let mut contents = Vec::new();

    for bi in 0..numbrushes {
        let b = boff as usize + (firstbrush as usize + bi as usize) * DBRUSH_SIZE;
        let bsoff = read_i32(data, b) as u32;
        let bsnum = read_i32(data, b + 4) as u32;
        let shadernum = read_i32(data, b + 8);

        // All brushes referenced by model 0 are structural (solid) world
        // geometry. The brush's `shadernum` names its surface texture, whose
        // surfaceflags (SURF_SLICK etc.) drive gameplay (e.g. ice sliding).
        // `contents` also rides along so water/lava volumes can still be
        // detected via PointContents even though they collide as solid here.
        offsets.push(plane_ids.len() as u32);
        counts.push(bsnum);
        shaders.push(shadernum);
        contents.push(
            shader_contents
                .get(shadernum as usize)
                .copied()
                .unwrap_or(0),
        );
        for s in 0..bsnum {
            let sp = soff as usize + (bsoff as usize + s as usize) * DBRUSHSIDE_SIZE;
            let planenum = read_i32(data, sp) as u32;
            plane_ids.push(planenum);
        }
    }

    Ok((offsets, counts, plane_ids, shaders, contents))
}

/// Enumerate the submodels (models 1..n) as `(firstbrush, numbrushes, mins,
/// maxs)` so trigger entities (`model "*N"`) can be mapped to their volume.
fn parse_submodels(data: &[u8], lump: &(u32, u32)) -> Vec<(u32, u32, [f32; 3], [f32; 3])> {
    let (off, len) = *lump;
    let n = len as usize / DMODEL_SIZE;
    let mut out = Vec::with_capacity(n.saturating_sub(1));
    for i in 1..n {
        let p = off as usize + i * DMODEL_SIZE;
        let mins = [read_f32(data, p), read_f32(data, p + 4), read_f32(data, p + 8)];
        let maxs = [read_f32(data, p + 12), read_f32(data, p + 16), read_f32(data, p + 20)];
        let firstbrush = read_i32(data, p + 32) as u32;
        let numbrushes = read_i32(data, p + 36) as u32;
        out.push((firstbrush, numbrushes, mins, maxs));
    }
    out
}

/// Extract the plane indices of a brush (given its firstbrush) from the
/// BRUSHES/BRUSHSIDES lumps, appending them to `out` and returning the
/// `(offset, count)` into `out`.
fn brush_planes_into(
    data: &[u8],
    lumps: &[(u32, u32)],
    firstbrush: u32,
    numbrushes: u32,
    out: &mut Vec<u32>,
) -> (u32, u32) {
    let (boff, _blen) = lumps[LUMP_BRUSHES];
    let (soff, _slen) = lumps[LUMP_BRUSHSIDES];
    let offset = out.len() as u32;
    for bi in 0..numbrushes {
        let b = boff as usize + (firstbrush as usize + bi as usize) * DBRUSH_SIZE;
        let bsoff = read_i32(data, b) as u32;
        let bsnum = read_i32(data, b + 4) as u32;
        for s in 0..bsnum {
            let sp = soff as usize + (bsoff as usize + s as usize) * DBRUSHSIDE_SIZE;
            let planenum = read_i32(data, sp) as u32;
            out.push(planenum);
        }
    }
    (offset, out.len() as u32 - offset)
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
            // Quake Z-up coordinates (unchanged) — matches physics/camera.
            let render = [x, y, z];
            // Quake "angle" is yaw in degrees; 0 = +X, positive = CCW from +Z.
            // Our yaw convention matches (forward = [cos yaw, sin yaw, 0]).
            let yaw = angle.to_radians();
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

/// Parse submodel AABBs from the MODELS lump. Returns `(mins, maxs)` per model
/// index (model 0 = world, ignored for triggers).
fn parse_models(data: &[u8], lump: &(u32, u32)) -> Vec<([f32; 3], [f32; 3])> {
    let (off, len) = *lump;
    let n = len as usize / DMODEL_SIZE;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let p = off as usize + i * DMODEL_SIZE;
        let mins = [read_f32(data, p), read_f32(data, p + 4), read_f32(data, p + 8)];
        let maxs = [read_f32(data, p + 12), read_f32(data, p + 16), read_f32(data, p + 20)];
        // Coordinates are Quake Z-up (identity), matching our space.
        out.push((mins, maxs));
    }
    out
}

/// Parse race gates from the entity lump. `trigger_multiple` entities with
/// `model "*N"` reference a submodel AABB; their `target` names a `target_*`
/// entity that tells whether it is a start/checkpoint/finish line.
fn parse_race(
    data: &[u8],
    lump: &(u32, u32),
    models: &[([f32; 3], [f32; 3])],
) -> Vec<RaceGate> {
    let (off, len) = *lump;
    let text = String::from_utf8_lossy(&data[off as usize..(off + len) as usize]);

    // First pass: build a map from targetname -> kind.
    let mut target_kinds: std::collections::HashMap<String, RaceGateKind> =
        std::collections::HashMap::new();
    for block in text.split('{').skip(1) {
        let Some(end) = block.find('}') else { continue };
        let block = &block[..end];
        let mut classname = "";
        let mut targetname = "";
        // Tokenize key/value pairs.
        let mut kv: Vec<&str> = Vec::new();
        for chunk in block.split('"') {
            let c = chunk.trim();
            if !c.is_empty() {
                kv.push(c);
            }
        }
        let mut i = 0;
        while i + 1 < kv.len() {
            match kv[i] {
                "classname" => classname = kv[i + 1],
                "targetname" => targetname = kv[i + 1],
                _ => {}
            }
            i += 2;
        }
        let kind = match classname {
            "target_startTimer" => Some(RaceGateKind::Start),
            "target_stopTimer" => Some(RaceGateKind::Finish),
            "target_checkpoint" => Some(RaceGateKind::Checkpoint),
            _ => None,
        };
        if let Some(k) = kind {
            target_kinds.insert(targetname.to_string(), k);
        }
    }

    // Second pass: trigger_multiple -> model AABB + target -> kind.
    let mut gates: Vec<(RaceGateKind, [f32; 3], [f32; 3])> = Vec::new();
    for block in text.split('{').skip(1) {
        let Some(end) = block.find('}') else { continue };
        let block = &block[..end];
        let mut classname = "";
        let mut target = "";
        let mut model = "";
        let mut kv: Vec<&str> = Vec::new();
        for chunk in block.split('"') {
            let c = chunk.trim();
            if !c.is_empty() {
                kv.push(c);
            }
        }
        let mut i = 0;
        while i + 1 < kv.len() {
            match kv[i] {
                "classname" => classname = kv[i + 1],
                "target" => target = kv[i + 1],
                "model" => model = kv[i + 1],
                _ => {}
            }
            i += 2;
        }
        if classname != "trigger_multiple" {
            continue;
        }
        let Some(kind) = target_kinds.get(target).copied() else {
            continue;
        };
        // model "*N" -> submodel index N.
        let Some(num) = model.strip_prefix('*').and_then(|s| s.parse::<usize>().ok()) else {
            continue;
        };
        let Some((mins, maxs)) = models.get(num) else {
            continue;
        };
        gates.push((kind, *mins, *maxs));
    }

    // Order: start first, then checkpoints in entity order, then finish.
    gates.sort_by_key(|(k, _, _)| match k {
        RaceGateKind::Start => 0,
        RaceGateKind::Checkpoint => 1,
        RaceGateKind::Finish => 2,
    });
    gates
        .into_iter()
        .map(|(kind, mins, maxs)| RaceGate { kind, mins, maxs })
        .collect()
}

/// Parse jumppads (`trigger_push`) and teleporters (`trigger_teleport`) from
/// the entity lump. Trigger volumes reference a submodel (`model "*N"`) whose
/// brush geometry bounds the trigger; each also references a target entity that
/// supplies either the launch destination (`target_position`/`info_notnull`) or
/// the teleport destination (`misc_teleporter_dest`/`target_teleporter`).
fn parse_triggers(
    data: &[u8],
    lumps: &[(u32, u32)],
    entity_lump: &(u32, u32),
    submodels: &[(u32, u32, [f32; 3], [f32; 3])],
) -> (Vec<Jumppad>, Vec<Teleporter>, Vec<u32>) {
    let (off, len) = *entity_lump;
    let text = String::from_utf8_lossy(&data[off as usize..(off + len) as usize]);

    // First pass: collect target origins (targetname -> origin).
    let mut target_origins: std::collections::HashMap<String, [f32; 3]> =
        std::collections::HashMap::new();
    for block in text.split('{').skip(1) {
        let Some(end) = block.find('}') else { continue };
        let block = &block[..end];
        let kv = tokenize_entity(block);
        let classname = get_entity(&kv, "classname");
        let is_target = matches!(
            classname,
            "target_position"
                | "info_notnull"
                | "misc_teleporter_dest"
                | "target_teleporter"
        );
        if !is_target {
            continue;
        }
        let targetname = get_entity(&kv, "targetname");
        if targetname.is_empty() {
            continue;
        }
        if let Some(origin) = parse_origin(get_entity(&kv, "origin")) {
            target_origins.insert(targetname.to_string(), origin);
        }
    }

    let mut jumppads = Vec::new();
    let mut teleporters = Vec::new();
    let mut trigger_plane_ids: Vec<u32> = Vec::new();

    for block in text.split('{').skip(1) {
        let Some(end) = block.find('}') else { continue };
        let block = &block[..end];
        let kv = tokenize_entity(block);
        let classname = get_entity(&kv, "classname");
        let target = get_entity(&kv, "target");
        let Some(num) = get_entity(&kv, "model")
            .strip_prefix('*')
            .and_then(|s| s.parse::<usize>().ok())
        else {
            continue;
        };
        // submodels is indexed from model 1.
        let Some((firstbrush, numbrushes, mins, maxs)) = submodels.get(num.wrapping_sub(1)).copied()
        else {
            continue;
        };
        let (plane_off, plane_count) =
            brush_planes_into(data, lumps, firstbrush, numbrushes, &mut trigger_plane_ids);

        match classname {
            "trigger_push" => {
                // Target is the apex of the jump; compute the ballistic launch
                // velocity that arcs the player there (Q3 trigger_push_setup).
                if let Some(t) = target_origins.get(target) {
                    let origin = [
                        (mins[0] + maxs[0]) * 0.5,
                        (mins[1] + maxs[1]) * 0.5,
                        (mins[2] + maxs[2]) * 0.5,
                    ];
                    let height = t[2] - origin[2];
                    let gravity = crate::GRAVITY;
                    let time = (height / (0.5 * gravity)).sqrt();
                    if time > 0.0 {
                        let mut vel = [t[0] - origin[0], t[1] - origin[1], 0.0];
                        let dist = (vel[0] * vel[0] + vel[1] * vel[1]).sqrt();
                        let nspeed = if dist > 0.0 { dist / time } else { 0.0 };
                        vel[0] = if dist > 0.0 { vel[0] / dist * nspeed } else { 0.0 };
                        vel[1] = if dist > 0.0 { vel[1] / dist * nspeed } else { 0.0 };
                        vel[2] = time * gravity;
                        jumppads.push(Jumppad {
                            mins,
                            maxs,
                            plane_off,
                            plane_count,
                            velocity: vel,
                        });
                    }
                }
            }
            "trigger_teleport" => {
                if let Some(dest) = target_origins.get(target) {
                    teleporters.push(Teleporter {
                        mins,
                        maxs,
                        plane_off,
                        plane_count,
                        dest_origin: *dest,
                    });
                }
            }
            _ => {}
        }
    }

    (jumppads, teleporters, trigger_plane_ids)
}

/// Map an entity classname to its animated-mover kind, if any. These are pulled
/// out of the static collision set and animated by the sim.
fn is_animated_mover(classname: &str) -> Option<MoverKind> {
    match classname {
        "func_bobbing" => Some(MoverKind::Bobbing),
        "func_plat" => Some(MoverKind::Plat),
        "func_door" => Some(MoverKind::Door),
        "func_door_rotating" => Some(MoverKind::DoorRotating),
        "func_train" => Some(MoverKind::Train),
        "func_rotating" => Some(MoverKind::Rotating),
        "func_pendulum" => Some(MoverKind::Pendulum),
        _ => None,
    }
}

/// Parse animated brush entities with their class-specific keys. Returns the
/// mover defs plus the shared plane-id backing store for their brush runs.
fn parse_movers(
    data: &[u8],
    lumps: &[(u32, u32)],
    entity_lump: &(u32, u32),
) -> (Vec<MoverDef>, Vec<u32>) {
    let submodels = parse_submodels(data, &lumps[LUMP_MODELS]);
    let (eoff, elen) = *entity_lump;
    let text = String::from_utf8_lossy(&data[eoff as usize..(eoff + elen) as usize]);

    // First pass: index `path_corner` nodes by targetname (origin, wait,
    // next-target), used to build func_train paths.
    let mut corners: std::collections::HashMap<String, ([f32; 3], f32, String)> =
        std::collections::HashMap::new();
    for block in text.split('{').skip(1) {
        let Some(end) = block.find('}') else { continue };
        let kv = tokenize_entity(&block[..end]);
        let cn = get_entity(&kv, "classname");
        if cn != "path_corner" && cn != "target_position" {
            continue;
        }
        let name = get_entity(&kv, "targetname");
        if name.is_empty() {
            continue;
        }
        let origin = parse_origin(get_entity(&kv, "origin")).unwrap_or([0.0; 3]);
        let wait = parse_f32(get_entity(&kv, "wait")).unwrap_or(0.0);
        let next = get_entity(&kv, "target").to_string();
        corners.insert(name.to_string(), (origin, wait, next));
    }

    let mut movers = Vec::new();
    let mut plane_ids: Vec<u32> = Vec::new();

    for block in text.split('{').skip(1) {
        let Some(end) = block.find('}') else { continue };
        let kv = tokenize_entity(&block[..end]);
        let classname = get_entity(&kv, "classname");
        let Some(kind) = is_animated_mover(classname) else {
            continue;
        };
        let Some(model) = get_entity(&kv, "model")
            .strip_prefix('*')
            .and_then(|s| s.parse::<usize>().ok())
        else {
            continue;
        };
        let Some((firstbrush, numbrushes, mins, maxs)) =
            submodels.get(model.wrapping_sub(1)).copied()
        else {
            continue;
        };

        let origin = parse_origin(get_entity(&kv, "origin")).unwrap_or([0.0; 3]);
        let angle = parse_f32(get_entity(&kv, "angle")).unwrap_or(0.0);
        let angles_deg = parse_vec3(get_entity(&kv, "angles")).unwrap_or([0.0; 3]);
        let angles = [
            angles_deg[0].to_radians(),
            angles_deg[1].to_radians(),
            angles_deg[2].to_radians(),
        ];
        let mut height = parse_f32(get_entity(&kv, "height")).unwrap_or(0.0);
        let mut speed = parse_f32(get_entity(&kv, "speed")).unwrap_or(0.0);
        let phase = parse_f32(get_entity(&kv, "phase")).unwrap_or(0.0);
        let mut wait = parse_f32(get_entity(&kv, "wait")).unwrap_or(0.0);
        let mut distance = parse_f32(get_entity(&kv, "distance")).unwrap_or(0.0);
        let spawnflags = parse_int(get_entity(&kv, "spawnflags"));

        // Class defaults (Warfork SP_func_*).
        match kind {
            MoverKind::Bobbing => {
                if height <= 0.0 {
                    height = 32.0;
                }
                if speed <= 0.0 {
                    speed = 4.0;
                }
            }
            MoverKind::Plat => {
                if speed <= 0.0 {
                    speed = 300.0;
                }
            }
            MoverKind::Door => {
                if speed <= 0.0 {
                    speed = 600.0;
                }
                if wait <= 0.0 {
                    wait = 2.0;
                }
            }
            MoverKind::DoorRotating => {
                if speed <= 0.0 {
                    speed = 100.0;
                }
                if wait <= 0.0 {
                    wait = 3.0;
                }
                if distance <= 0.0 {
                    distance = 90.0;
                }
            }
            MoverKind::Train => {
                if speed <= 0.0 {
                    speed = 100.0;
                }
            }
            MoverKind::Rotating => {
                if speed <= 0.0 {
                    speed = 100.0;
                }
            }
            MoverKind::Pendulum => {
                if speed <= 0.0 {
                    speed = 30.0;
                }
            }
        }

        // func_train: follow the `target` chain through path_corner nodes.
        let (path, path_wait) = if kind == MoverKind::Train {
            let mut path = Vec::new();
            let mut waits = Vec::new();
            let mut target = get_entity(&kv, "target").to_string();
            let mut guard = 0;
            while !target.is_empty() && guard < 256 {
                let Some((o, w, next)) = corners.get(&target) else {
                    break;
                };
                path.push(*o);
                waits.push(*w);
                target = next.clone();
                guard += 1;
            }
            (path, waits)
        } else {
            (Vec::new(), Vec::new())
        };

        let (plane_off, plane_count) = brush_planes_into(
            data,
            lumps,
            firstbrush,
            numbrushes,
            &mut plane_ids,
        );

        movers.push(MoverDef {
            kind,
            model,
            origin,
            angles,
            angle,
            height,
            speed,
            phase,
            wait,
            distance,
            spawnflags,
            path,
            path_wait,
            plane_off,
            plane_count,
            mins,
            maxs,
        });
    }

    (movers, plane_ids)
}

/// Is this entity classname a solid brush model (collides with the player)?
/// `func_*` movers/brush entities are solid; triggers and portals are not.
fn is_solid_brush_model(classname: &str) -> bool {
    if is_animated_mover(classname).is_some() {
        // Animated movers are handled by `parse_movers` (they collide at their
        // current transform, not statically at the authored position).
        return false;
    }
    if !classname.starts_with("func_") {
        return false;
    }
    !matches!(
        classname,
        "func_areaportal"
            | "func_areaportalwindow"
            | "func_portal"
            | "func_ladder"
            | "func_water"
            | "func_water_analog"
            | "func_illusionary"
    )
}

/// Parse the brushes of solid brush-model entities (`func_bobbing`,
/// `func_plat`, `func_door`, ...) so they can be added to the collision world.
/// Returns `(plane_off, plane_count, shader, contents)` per brush plus the
/// shared plane-id array (empty if there are none).
fn parse_solid_brush_models(
    data: &[u8],
    lumps: &[(u32, u32)],
    shader_contents: &[i32],
) -> (Vec<(u32, u32, i32, i32)>, Vec<u32>) {
    let submodels = parse_submodels(data, &lumps[LUMP_MODELS]);
    let (eoff, elen) = lumps[LUMP_ENTITIES];
    let text = String::from_utf8_lossy(&data[eoff as usize..(eoff + elen) as usize]);

    // Submodel numbers (from `model "*N"`) that are solid brush models.
    let mut solid = Vec::new();
    for block in text.split('{').skip(1) {
        let Some(end) = block.find('}') else { continue };
        let kv = tokenize_entity(&block[..end]);
        if !is_solid_brush_model(get_entity(&kv, "classname")) {
            continue;
        }
        if let Some(num) = get_entity(&kv, "model")
            .strip_prefix('*')
            .and_then(|s| s.parse::<usize>().ok())
        {
            solid.push(num);
        }
    }

    let (boff, _blen) = lumps[LUMP_BRUSHES];
    let (soff, _slen) = lumps[LUMP_BRUSHSIDES];
    let mut brushes = Vec::new();
    let mut plane_ids: Vec<u32> = Vec::new();
    for num in solid {
        let Some((firstbrush, numbrushes, _, _)) = submodels.get(num.wrapping_sub(1)).copied() else {
            continue;
        };
        for bi in 0..numbrushes {
            let b = boff as usize + (firstbrush as usize + bi as usize) * DBRUSH_SIZE;
            let bsoff = read_i32(data, b) as u32;
            let bsnum = read_i32(data, b + 4) as u32;
            let shadernum = read_i32(data, b + 8);
            let offset = plane_ids.len() as u32;
            for s in 0..bsnum {
                let sp = soff as usize + (bsoff as usize + s as usize) * DBRUSHSIDE_SIZE;
                plane_ids.push(read_i32(data, sp) as u32);
            }
            let count = plane_ids.len() as u32 - offset;
            let contents = shader_contents.get(shadernum as usize).copied().unwrap_or(0);
            brushes.push((offset, count, shadernum, contents));
        }
    }
    (brushes, plane_ids)
}

/// Get an entity key's string value (empty if absent).
fn get_entity<'a>(kv: &'a std::collections::HashMap<String, String>, key: &str) -> &'a str {
    kv.get(key).map(String::as_str).unwrap_or("")
}

/// Tokenize an entity `{ ... }` block into a lowercase key -> value map.
fn tokenize_entity(block: &str) -> std::collections::HashMap<String, String> {
    let mut kv = std::collections::HashMap::new();
    let mut tokens: Vec<&str> = Vec::new();
    for chunk in block.split('"') {
        let c = chunk.trim();
        if !c.is_empty() {
            tokens.push(c);
        }
    }
    let mut i = 0;
    while i + 1 < tokens.len() {
        kv.insert(tokens[i].to_string(), tokens[i + 1].to_string());
        i += 2;
    }
    kv
}

fn parse_origin(s: &str) -> Option<[f32; 3]> {
    let mut parts = s.split_whitespace();
    let x = parts.next()?.parse().ok()?;
    let y = parts.next()?.parse().ok()?;
    let z = parts.next()?.parse().ok()?;
    Some([x, y, z])
}

/// Parse a `"x y z"` vector of f32 (empty if the string is malformed).
fn parse_vec3(s: &str) -> Option<[f32; 3]> {
    let mut parts = s.split_whitespace();
    let x = parts.next()?.parse().ok()?;
    let y = parts.next()?.parse().ok()?;
    let z = parts.next()?.parse().ok()?;
    Some([x, y, z])
}

/// Parse a scalar, accepting decimal or `0x`-prefixed hexadecimal.
fn parse_int(s: &str) -> i32 {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        i32::from_str_radix(hex, 16).unwrap_or(0)
    } else {
        s.parse::<i32>().unwrap_or(0)
    }
}

fn parse_f32(s: &str) -> Option<f32> {
    s.trim().parse::<f32>().ok()
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
