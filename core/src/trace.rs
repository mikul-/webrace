//! AABB clipping-hull trace against BSP brushes (exact `pmove` semantics).
//!
//! This is the collision primitive the movement code depends on. It traces an
//! axis-aligned box (the player's mins/maxs) against the set of convex solid
//! brushes parsed from the BSP, returning the closest plane hit (fraction,
//! normal). Port of the logic in Warfork's `cm_trace` / `PM_SlideMove`.
//!
//! Moving brush entities (`func_bobbing`/`func_plat`/...) are stored separately
//! from the static world brushes and traced with their *current* transform
//! (`world = origin + R(angles)·geometry`, matching Qfusion
//! `CM_TransformedBoxTrace`).

#![allow(dead_code)]

use crate::bsp::{Bsp, Jumppad, MoverDef, MoverKind, Plane, Teleporter};

#[derive(Clone, Copy, Debug, Default)]
pub struct TraceResult {
    pub fraction: f32,
    pub normal: [f32; 3],
    pub all_solid: bool,
    pub start_solid: bool,
    /// Surface flags (SURF_SLICK etc.) of the surface that was hit (0 if none).
    pub surface_flags: i32,
    /// Contents flags (CONTENTS_WATER etc.) of the surface that was hit.
    pub contents: i32,
    /// Index into `World.movers` of the moving brush entity that was hit, or
    /// `-1` for static world geometry. Used by `PM_CategorizePosition` to know
    /// which mover (if any) the player is standing on, so it can be carried.
    pub mover: i32,
}

/// Player collision box (Warfork default bounding box, in world units).
pub const PLAYER_MINS: [f32; 3] = [-16.0, -16.0, -24.0];
pub const PLAYER_MAXS: [f32; 3] = [16.0, 16.0, 40.0];

/// Crouched player box (Warfork playerbox_crouch: maxs z = 16).
pub const CROUCH_MINS: [f32; 3] = [-16.0, -16.0, -24.0];
pub const CROUCH_MAXS: [f32; 3] = [16.0, 16.0, 16.0];

/// A query box for mover trigger detection (player origin + run box).
#[derive(Clone, Copy, Debug)]
pub struct PlayerBox {
    pub origin: [f32; 3],
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
}

/// Generic mover finite-state machine. `AtBottom`/`MovingUp`/`AtTop`/
/// `MovingDown` are used by plats and doors; `Waiting`/`Moving` by trains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoverState {
    AtBottom,
    MovingUp,
    AtTop,
    MovingDown,
    Waiting,
    Moving,
}

/// Which axis a `func_bobbing`/`func_rotating`/`func_pendulum` acts on, from
/// its spawnflags (Warfork `SP_func_bobbing` / `SP_func_rotating`).
fn bobbing_axis(spawnflags: i32) -> usize {
    if spawnflags & 1 != 0 {
        0
    } else if spawnflags & 2 != 0 {
        1
    } else {
        2
    }
}

/// Q3/Qfusion `G_SetMovedir`: `angle` in degrees, where -1 = up and -2 = down,
/// otherwise the angle is a yaw (0° → +X).
pub fn movedir_from_angle(angle_deg: f32) -> [f32; 3] {
    if (angle_deg + 1.0).abs() < 1e-6 {
        [0.0, 0.0, 1.0]
    } else if (angle_deg + 2.0).abs() < 1e-6 {
        [0.0, 0.0, -1.0]
    } else {
        let r = angle_deg.to_radians();
        [r.cos(), r.sin(), 0.0]
    }
}

/// Rotation matrix for `angles` (`[pitch, yaw, roll]` radians): `world = R·local`.
/// Rows are the matrix rows (`axis_transform` computes `R·v`).
///
/// Q3's `AnglesToAxis` returns `(forward, right, up)` where `right = forward×up`
/// is `-Y` at zero angles — i.e. a *left*-handed basis. Using it directly made a
/// zero-angle mover a Y-mirror (bobbing platforms moved across the map and
/// vanished). We negate the middle (right) basis vector so zero angles → identity
/// and the transform is a proper right-handed rotation `Rz(yaw)`.
pub fn angles_to_axis(angles: [f32; 3]) -> [[f32; 3]; 3] {
    let (sp, cp) = (angles[0].sin(), angles[0].cos());
    let (sy, cy) = (angles[1].sin(), angles[1].cos());
    let (sr, cr) = (angles[2].sin(), angles[2].cos());
    [
        [cp * cy, cp * sy, -sp],
        [sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, sr * cp],
        [cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp],
    ]
}

/// Apply the rotation matrix implied by `axis` to a vector:
/// `(R·v)[i] = axis[0][i]*v[0] + axis[1][i]*v[1] + axis[2][i]*v[2]`.
pub fn axis_transform(axis: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        axis[0][0] * v[0] + axis[1][0] * v[1] + axis[2][0] * v[2],
        axis[0][1] * v[0] + axis[1][1] * v[1] + axis[2][1] * v[2],
        axis[0][2] * v[0] + axis[1][2] * v[1] + axis[2][2] * v[2],
    ]
}

/// A moving solid brush entity. `world = origin + R(angles)·raw_geometry`, so
/// the raw BSP brush planes are transformed by `n' = R·n`,
/// `d' = d + n'·origin` (Qfusion `CM_TransformedBoxTrace`).
#[derive(Clone, Debug)]
pub struct Mover {
    pub kind: MoverKind,
    /// Submodel index (`model "*N"` → N). Also the render mapping key.
    pub model: usize,
    /// Current origin (local-frame origin in world space).
    pub origin: [f32; 3],
    /// Current angles `[pitch, yaw, roll]` radians.
    pub angles: [f32; 3],
    /// Previous-tick transform, for rider carry (`SV_Push`).
    pub prev_origin: [f32; 3],
    pub prev_angles: [f32; 3],
    /// Entity `origin` key (spawn pivot / local-frame origin).
    pub base_origin: [f32; 3],
    /// Entity `angles` key (radians).
    pub base_angles: [f32; 3],
    pub plane_off: u32,
    pub plane_count: u32,
    /// Raw submodel AABB (in the local frame) for trigger/contact tests.
    pub bounds_mins: [f32; 3],
    pub bounds_maxs: [f32; 3],
    pub height: f32,
    pub speed: f32,
    pub phase: f32,
    pub wait: f32,
    pub distance: f32,
    pub spawnflags: i32,
    /// Normalized direction (`G_SetMovedir`) for doors/trains, or zero.
    pub movedir: [f32; 3],
    /// `func_train` path corner origins + per-corner wait.
    pub path: Vec<[f32; 3]>,
    pub path_wait: Vec<f32>,
    // Runtime state.
    pub state: MoverState,
    pub timer: f32,
    pub path_node: usize,
    pub t: f32,
}

impl Mover {
    pub fn from_def(def: &MoverDef) -> Mover {
        let kind = def.kind;
        // `func_door_rotating` derives its rotation axis from spawnflags
        // (Z/yaw default), unlike sliding doors which use the `angle` key.
        let mut movedir = match kind {
            MoverKind::DoorRotating => {
                let mut m = [0.0f32; 3];
                if def.spawnflags & 64 != 0 {
                    m[2] = 1.0;
                } else if def.spawnflags & 128 != 0 {
                    m[0] = 1.0;
                } else {
                    m[1] = 1.0;
                }
                if def.spawnflags & 2 != 0 {
                    m = [-m[0], -m[1], -m[2]];
                }
                m
            }
            _ => movedir_from_angle(def.angle),
        };
        // `func_door` REVERSE spawnflag negates the slide direction.
        if kind == MoverKind::Door && def.spawnflags & 2 != 0 {
            movedir = [-movedir[0], -movedir[1], -movedir[2]];
        }
        let size = [
            def.maxs[0] - def.mins[0],
            def.maxs[1] - def.mins[1],
            def.maxs[2] - def.mins[2],
        ];
        let lip = 8.0f32;
        // Height (plat) defaults to the model's vertical size minus the lip.
        let mut height = def.height;
        if height <= 0.0 {
            height = (size[2] - lip).max(0.0);
        }
        let mut origin = def.origin;
        let angles = def.angles;
        let mut state = MoverState::AtBottom;
        let mut path_node = 0;
        let mut timer = 0.0;

        match kind {
            MoverKind::Plat => {
                // Plats are authored in the raised position and spawn lowered.
                origin[2] -= height;
            }
            MoverKind::Train => {
                if let Some(first) = def.path.first() {
                    origin = [
                        first[0] - def.mins[0],
                        first[1] - def.mins[1],
                        first[2] - def.mins[2],
                    ];
                    path_node = 0;
                    state = MoverState::Waiting;
                    timer = def.path_wait.first().copied().unwrap_or(0.0);
                }
            }
            MoverKind::Door | MoverKind::DoorRotating => {
                state = MoverState::AtBottom;
            }
            _ => {}
        }

        Mover {
            kind,
            model: def.model,
            origin,
            angles,
            prev_origin: origin,
            prev_angles: angles,
            base_origin: def.origin,
            base_angles: def.angles,
            plane_off: def.plane_off,
            plane_count: def.plane_count,
            bounds_mins: def.mins,
            bounds_maxs: def.maxs,
            height,
            speed: def.speed,
            phase: def.phase,
            wait: def.wait,
            distance: def.distance,
            spawnflags: def.spawnflags,
            movedir,
            path: def.path.clone(),
            path_wait: def.path_wait.clone(),
            state,
            timer,
            path_node,
            t: 0.0,
        }
    }

    /// Current rotation matrix (columns are the world-space basis vectors; also
    /// the GL model-matrix columns for rendering).
    pub fn axis(&self) -> [[f32; 3]; 3] {
        angles_to_axis(self.angles)
    }

    /// Transform a raw BSP plane into world space.
    pub fn transform_plane(&self, normal: [f32; 3], dist: f32) -> ([f32; 3], f32) {
        let r = self.axis();
        let n = axis_transform(&r, normal);
        let d = dist + n[0] * self.origin[0] + n[1] * self.origin[1] + n[2] * self.origin[2];
        (n, d)
    }

    /// World-space AABB of the (unrotated) model at the current origin. Good
    /// enough for trigger/contact proximity tests.
    fn world_aabb(&self) -> ([f32; 3], [f32; 3]) {
        (
            [
                self.origin[0] + self.bounds_mins[0],
                self.origin[1] + self.bounds_mins[1],
                self.origin[2] + self.bounds_mins[2],
            ],
            [
                self.origin[0] + self.bounds_maxs[0],
                self.origin[1] + self.bounds_maxs[1],
                self.origin[2] + self.bounds_maxs[2],
            ],
        )
    }

    /// Advance one tick. Deterministic: depends only on `dt` and the player box.
    fn advance(&mut self, dt: f32, player: Option<&PlayerBox>) {
        self.t += dt;
        match self.kind {
            MoverKind::Bobbing => self.advance_bobbing(),
            MoverKind::Rotating => self.advance_rotating(dt),
            MoverKind::Pendulum => self.advance_pendulum(),
            MoverKind::Plat => self.advance_plat(dt, player),
            MoverKind::Door | MoverKind::DoorRotating => self.advance_door(dt, player),
            MoverKind::Train => self.advance_train(dt),
        }
    }

    /// `func_bobbing_think`: `origin = start + dir·sin(2π·frac((t − speed·phase)/speed))`.
    fn advance_bobbing(&mut self) {
        let speed = self.speed.max(1e-3);
        let mut delta = (self.t - speed * self.phase) / speed;
        delta -= delta.floor();
        let s = (delta * std::f32::consts::TAU).sin();
        let axis = bobbing_axis(self.spawnflags);
        let mut dir = [0.0f32; 3];
        dir[axis] = self.height;
        self.origin = [
            self.base_origin[0] + dir[0] * s,
            self.base_origin[1] + dir[1] * s,
            self.base_origin[2] + dir[2] * s,
        ];
    }

    /// `func_rotating`: constant angular speed about the spawnflag axis.
    /// Warfork axes: `&4` → roll, `&8` → pitch, otherwise yaw (Z by default).
    fn advance_rotating(&mut self, _dt: f32) {
        let axis = if self.spawnflags & 4 != 0 {
            2
        } else if self.spawnflags & 8 != 0 {
            0
        } else {
            1
        };
        let sign = if self.spawnflags & 2 != 0 { -1.0 } else { 1.0 };
        self.angles[axis] = self.base_angles[axis]
            + sign * self.speed.to_radians() * self.t;
        self.origin = self.base_origin;
    }

    /// `func_pendulum_think`: sinusoidal angle swing (amplitude `speed`,
    /// frequency derived from gravity + arm length; approximated by `phase`).
    fn advance_pendulum(&mut self) {
        let freq = if self.phase > 0.0 { self.phase } else { 0.5 };
        let s = (std::f32::consts::TAU * freq * self.t).sin();
        // Warfork stores the swing on the roll channel (`dir[2] = speed`).
        self.angles[2] = self.base_angles[2] + self.speed.to_radians() * s;
        self.origin = self.base_origin;
    }

    /// Can the player interact with this mover (stands on / is next to it)?
    /// The AABB is evaluated at `at_origin` (usually the authored position, so
    /// an already-open door still sees a player in its doorway).
    fn player_contact_at(&self, at_origin: [f32; 3], player: Option<&PlayerBox>, margin: f32) -> bool {
        let Some(p) = player else { return false };
        let bmin = [
            at_origin[0] + self.bounds_mins[0],
            at_origin[1] + self.bounds_mins[1],
            at_origin[2] + self.bounds_mins[2],
        ];
        let bmax = [
            at_origin[0] + self.bounds_maxs[0],
            at_origin[1] + self.bounds_maxs[1],
            at_origin[2] + self.bounds_maxs[2],
        ];
        let pmin = [
            p.origin[0] + p.mins[0],
            p.origin[1] + p.mins[1],
            p.origin[2] + p.mins[2],
        ];
        let pmax = [
            p.origin[0] + p.maxs[0],
            p.origin[1] + p.maxs[1],
            p.origin[2] + p.maxs[2],
        ];
        pmin[0] <= bmax[0] + margin
            && pmax[0] >= bmin[0] - margin
            && pmin[1] <= bmax[1] + margin
            && pmax[1] >= bmin[1] - margin
            && pmin[2] <= bmax[2] + margin
            && pmax[2] >= bmin[2] - margin
    }

    /// `func_plat` FSM: bottom → (touch) up → top → (3 s after leaving) down.
    fn advance_plat(&mut self, dt: f32, player: Option<&PlayerBox>) {
        let top = self.base_origin;
        let bottom = [top[0], top[1], top[2] - self.height];
        // A player standing on the plat keeps it up; but only trigger the
        // initial rise from a small contact margin so adjacent geometry
        // doesn't set it off.
        let touching = self.player_contact_at(self.base_origin, player, 4.0);
        match self.state {
            MoverState::AtBottom => {
                if touching {
                    self.state = MoverState::MovingUp;
                }
            }
            MoverState::MovingUp => {
                let step = self.speed.max(1.0) * dt;
                self.origin[2] = (self.origin[2] + step).min(top[2]);
                if self.origin[2] >= top[2] - 1e-3 {
                    self.origin = top;
                    self.state = MoverState::AtTop;
                    self.timer = 3.0;
                }
            }
            MoverState::AtTop => {
                if touching {
                    self.timer = 1.0; // still riding: delay the return
                } else {
                    self.timer -= dt;
                    if self.timer <= 0.0 {
                        self.state = MoverState::MovingDown;
                    }
                }
            }
            MoverState::MovingDown => {
                let step = self.speed.max(1.0) * dt;
                self.origin[2] = (self.origin[2] - step).max(bottom[2]);
                if self.origin[2] <= bottom[2] + 1e-3 {
                    self.origin = bottom;
                    self.state = MoverState::AtBottom;
                }
            }
            _ => {}
        }
    }

    /// `func_door` / `func_door_rotating` FSM: closed → (near) open → wait → close.
    fn advance_door(&mut self, dt: f32, player: Option<&PlayerBox>) {
        let size = [
            self.bounds_maxs[0] - self.bounds_mins[0],
            self.bounds_maxs[1] - self.bounds_mins[1],
            self.bounds_maxs[2] - self.bounds_mins[2],
        ];
        let lip = 8.0f32;
        let near = self.player_contact_at(self.base_origin, player, 48.0);

        let (start_o, end_o) = if self.kind == MoverKind::DoorRotating {
            let end = [
                self.base_angles[0] + self.distance.to_radians() * self.movedir[0],
                self.base_angles[1] + self.distance.to_radians() * self.movedir[1],
                self.base_angles[2] + self.distance.to_radians() * self.movedir[2],
            ];
            (self.base_angles, end)
        } else {
            let dist = (size[0] * self.movedir[0].abs()
                + size[1] * self.movedir[1].abs()
                + size[2] * self.movedir[2].abs()
                - lip)
                .max(0.0);
            let end = [
                self.base_origin[0] + self.movedir[0] * dist,
                self.base_origin[1] + self.movedir[1] * dist,
                self.base_origin[2] + self.movedir[2] * dist,
            ];
            (self.base_origin, end)
        };

        let step = self.speed.max(1.0) * dt;
        match self.state {
            MoverState::AtBottom => {
                if near {
                    self.state = MoverState::MovingUp;
                }
            }
            MoverState::MovingUp => {
                let done = if self.kind == MoverKind::DoorRotating {
                    move_angles_toward(&mut self.angles, end_o, step)
                } else {
                    move_vec_toward(&mut self.origin, end_o, step)
                };
                if done {
                    self.origin = if self.kind == MoverKind::DoorRotating {
                        self.origin
                    } else {
                        end_o
                    };
                    if self.kind == MoverKind::DoorRotating {
                        self.angles = end_o;
                    }
                    self.state = MoverState::AtTop;
                    self.timer = self.wait.max(0.0);
                }
            }
            MoverState::AtTop => {
                if near {
                    self.timer = self.wait.max(0.0);
                } else {
                    self.timer -= dt;
                    if self.timer <= 0.0 {
                        self.state = MoverState::MovingDown;
                    }
                }
            }
            MoverState::MovingDown => {
                let done = if self.kind == MoverKind::DoorRotating {
                    move_angles_toward(&mut self.angles, start_o, step)
                } else {
                    move_vec_toward(&mut self.origin, start_o, step)
                };
                if done {
                    if self.kind == MoverKind::DoorRotating {
                        self.angles = start_o;
                    } else {
                        self.origin = start_o;
                    }
                    self.state = MoverState::AtBottom;
                }
            }
            _ => {}
        }
    }

    /// `func_train`: move corner-to-corner along the `path_corner` chain.
    fn advance_train(&mut self, dt: f32) {
        if self.path.is_empty() {
            return;
        }
        let node = self.path_node % self.path.len();
        let dest = [
            self.path[node][0] - self.bounds_mins[0],
            self.path[node][1] - self.bounds_mins[1],
            self.path[node][2] - self.bounds_mins[2],
        ];
        let step = self.speed.max(1.0) * dt;
        match self.state {
            MoverState::Waiting => {
                self.timer -= dt;
                if self.timer <= 0.0 {
                    self.state = MoverState::Moving;
                }
            }
            MoverState::Moving => {
                if move_vec_toward(&mut self.origin, dest, step) {
                    self.origin = dest;
                    self.state = MoverState::Waiting;
                    self.timer = self.path_wait.get(node).copied().unwrap_or(0.0);
                    self.path_node = (node + 1) % self.path.len();
                }
            }
            _ => {
                self.state = MoverState::Moving;
            }
        }
    }
}

/// Move `cur` toward `dest` by at most `step`; returns true when it arrives.
fn move_vec_toward(cur: &mut [f32; 3], dest: [f32; 3], step: f32) -> bool {
    let d = [dest[0] - cur[0], dest[1] - cur[1], dest[2] - cur[2]];
    let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if dist <= step || dist < 1e-6 {
        *cur = dest;
        true
    } else {
        let s = step / dist;
        cur[0] += d[0] * s;
        cur[1] += d[1] * s;
        cur[2] += d[2] * s;
        false
    }
}

/// Move `cur` angles toward `dest` shortest-arc by at most `step` radians.
fn move_angles_toward(cur: &mut [f32; 3], dest: [f32; 3], step: f32) -> bool {
    let mut done = true;
    for i in 0..3 {
        let mut d = dest[i] - cur[i];
        while d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        }
        while d < -std::f32::consts::PI {
            d += std::f32::consts::TAU;
        }
        if d.abs() <= step || d.abs() < 1e-6 {
            cur[i] = dest[i];
        } else {
            cur[i] += d.signum() * step;
            done = false;
        }
    }
    done
}

pub struct World {
    pub brush_plane_offsets: Vec<u32>,
    pub brush_plane_count: Vec<u32>,
    pub brush_plane_ids: Vec<u32>,
    /// Shader index of each collision brush (parallel to the brush arrays).
    pub brush_shaders: Vec<i32>,
    /// Contents flags (CONTENTS_WATER etc.) of each brush (parallel arrays).
    pub brush_contents: Vec<i32>,
    pub planes: Vec<Plane>,
    /// Surface flags (SURF_SLICK etc.) per shader, used for slick/gameplay.
    pub shader_flags: Vec<i32>,
    /// Contents flags (CONTENTS_WATER etc.) per shader.
    pub shader_contents: Vec<i32>,
    /// Jumppad trigger volumes.
    pub jumppads: Vec<Jumppad>,
    /// Teleporter trigger volumes.
    pub teleporters: Vec<Teleporter>,
    /// Plane ids of trigger brushes (backing store for trigger plane runs).
    pub trigger_plane_ids: Vec<u32>,
    /// Moving solid brush entities (traced at their current transform).
    pub movers: Vec<Mover>,
    /// Plane ids backing the mover brush runs.
    pub mover_plane_ids: Vec<u32>,
}

impl World {
    pub fn from_bsp(bsp: &Bsp) -> World {
        World {
            brush_plane_offsets: bsp.brush_plane_offsets.clone(),
            brush_plane_count: bsp.brush_plane_count.clone(),
            brush_plane_ids: bsp.brush_plane_ids.clone(),
            brush_shaders: bsp.brush_shaders.clone(),
            brush_contents: bsp.brush_contents.clone(),
            planes: bsp.planes.clone(),
            shader_flags: bsp.shader_flags.clone(),
            shader_contents: bsp.shader_contents.clone(),
            jumppads: bsp.jumppads.clone(),
            teleporters: bsp.teleporters.clone(),
            trigger_plane_ids: bsp.trigger_plane_ids.clone(),
            movers: bsp.movers.iter().map(Mover::from_def).collect(),
            mover_plane_ids: bsp.mover_plane_ids.clone(),
        }
    }

    /// Advance every mover one tick (deterministic; no wall clock).
    pub fn advance_movers(&mut self, dt: f32, player: Option<&PlayerBox>) {
        for m in self.movers.iter_mut() {
            m.prev_origin = m.origin;
            m.prev_angles = m.angles;
            m.advance(dt, player);
        }
    }

    /// Per-mover render transforms: 12 floats each = origin(3) + axis[0](3) +
    /// axis[1](3) + axis[2](3). Column-major basis for the GL model matrix.
    pub fn mover_transforms(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.movers.len() * 12);
        for m in &self.movers {
            let r = m.axis();
            out.extend_from_slice(&[
                m.origin[0], m.origin[1], m.origin[2],
                r[0][0], r[0][1], r[0][2],
                r[1][0], r[1][1], r[1][2],
                r[2][0], r[2][1], r[2][2],
            ]);
        }
        out
    }
}

/// Does this contents word block the player? Mirrors Q3's `MASK_PLAYERSOLID`
/// (`CONTENTS_SOLID|CONTENTS_PLAYERCLIP|CONTENTS_BODY`), minus liquids: a brush
/// bearing only water/lava/slime/translucent contents is a non-solid volume.
pub fn is_solid_contents(contents: i32) -> bool {
    const MASK_WATER: i32 = crate::bsp::CONTENTS_WATER
        | crate::bsp::CONTENTS_LAVA
        | crate::bsp::CONTENTS_SLIME;
    if contents & MASK_WATER != 0 {
        return false;
    }
    contents & (crate::bsp::CONTENTS_SOLID | crate::bsp::CONTENTS_PLAYERCLIP) != 0
}

/// Per-brush hemisphere/interval accumulator for the slab trace. Kept separate
/// from `World::trace` so static and transformed (mover) brushes share the exact
/// same logic.
struct BrushTrace {
    inside_all: bool,
    stays_outside: bool,
    enter: f32,
    exit: f32,
    enter_normal: [f32; 3],
    deepest: f32,
    deepest_normal: [f32; 3],
}

impl BrushTrace {
    fn new() -> Self {
        BrushTrace {
            inside_all: true,
            stays_outside: false,
            enter: f32::NEG_INFINITY,
            exit: f32::INFINITY,
            enter_normal: [0.0; 3],
            deepest: f32::INFINITY,
            deepest_normal: [0.0; 3],
        }
    }

    fn add_plane(
        &mut self,
        n: [f32; 3],
        dist: f32,
        c0: [f32; 3],
        delta: [f32; 3],
        half_ext: [f32; 3],
    ) {
        let radius =
            half_ext[0] * n[0].abs() + half_ext[1] * n[1].abs() + half_ext[2] * n[2].abs();
        let start_dist =
            c0[0] * n[0] + c0[1] * n[1] + c0[2] * n[2] - dist - radius;
        let end_dist = start_dist
            + delta[0] * n[0]
            + delta[1] * n[1]
            + delta[2] * n[2];

        if start_dist >= 0.0 {
            self.inside_all = false;
        }
        if start_dist < self.deepest {
            self.deepest = start_dist;
            self.deepest_normal = n;
        }

        if start_dist > 0.0 && end_dist > 0.0 {
            self.stays_outside = true;
            return;
        }
        if start_dist < 0.0 && end_dist < 0.0 {
            return;
        }
        if start_dist == 0.0 && end_dist == 0.0 {
            return;
        }

        let denom = start_dist - end_dist;
        let t = if denom.abs() < 1e-12 { 0.0 } else { start_dist / denom };

        if start_dist > end_dist {
            if t > self.enter {
                self.enter = t;
                self.enter_normal = n;
            }
        } else if start_dist < 0.0 && t < self.exit {
            self.exit = t;
        }
    }
}

impl World {
    /// Fast rejection: does the AABB overlap the brush's AABB? For now we test
    /// against every brush via plane distances; enough for correctness, will
    /// add a BVH later as an optimization.
    pub fn trace(
        &self,
        start: [f32; 3],
        mins: [f32; 3],
        maxs: [f32; 3],
        end: [f32; 3],
    ) -> TraceResult {
        let mut best_frac = 1.0f32;
        let mut best_normal = [0.0f32; 3];
        let mut best_surface_flags = 0i32;
        let mut best_contents = 0i32;
        let mut best_mover = -1i32;
        let mut start_solid = false;
        // `all_solid` (Q3 "trapped in solid with no exit") is approximated by
        // `start_solid` in this trace: we return immediately on embedding rather
        // than probing for an exit, so the two coincide. It is never `true`
        // for a non-embedded trace.
        let all_solid = false;

        let delta = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];

        // Box center offset (mins/maxs are relative to `start`) and half-extents.
        let center_off = [
            (mins[0] + maxs[0]) * 0.5,
            (mins[1] + maxs[1]) * 0.5,
            (mins[2] + maxs[2]) * 0.5,
        ];
        let half_ext = [
            (maxs[0] - mins[0]) * 0.5,
            (maxs[1] - mins[1]) * 0.5,
            (maxs[2] - mins[2]) * 0.5,
        ];
        let c0 = [
            start[0] + center_off[0],
            start[1] + center_off[1],
            start[2] + center_off[2],
        ];

        // ---- Static world brushes ----
        for i in 0..self.brush_plane_offsets.len() {
            let offset = self.brush_plane_offsets[i] as usize;
            let count = self.brush_plane_count[i] as usize;
            // Skip non-solid (liquid/trigger) brushes for movement collision.
            let brush_contents = self.brush_contents.get(i).copied().unwrap_or(0);
            if !is_solid_contents(brush_contents) {
                continue;
            }
            // Surface flags of this brush (via its shader), used for slick etc.
            let brush_surface_flags = self
                .brush_shaders
                .get(i)
                .and_then(|&s| self.shader_flags.get(s as usize).copied())
                .unwrap_or(0);

            let mut bt = BrushTrace::new();
            for p in 0..count {
                let pid = self.brush_plane_ids[offset + p] as usize;
                let Some(plane) = self.planes.get(pid) else { continue };
                bt.add_plane(plane.normal, plane.dist, c0, delta, half_ext);
            }

            if bt.inside_all {
                start_solid = true;
                best_normal = bt.deepest_normal;
                best_surface_flags = brush_surface_flags;
                best_contents = brush_contents;
                best_mover = -1;
                break;
            }
            if bt.stays_outside {
                continue;
            }
            if bt.enter > f32::NEG_INFINITY && bt.enter < bt.exit {
                if bt.enter < 0.0 {
                    start_solid = true;
                    if best_frac > 0.0 {
                        best_frac = 0.0;
                        best_normal = bt.enter_normal;
                        best_surface_flags = brush_surface_flags;
                        best_contents = brush_contents;
                        best_mover = -1;
                    }
                } else if bt.enter < best_frac - 1e-4 {
                    best_frac = bt.enter;
                    best_normal = bt.enter_normal;
                    best_surface_flags = brush_surface_flags;
                    best_contents = brush_contents;
                    best_mover = -1;
                } else if (bt.enter - best_frac).abs() <= 1e-4
                    && brush_surface_flags & crate::pmove::SURF_SLICK != 0
                    && best_surface_flags & crate::pmove::SURF_SLICK == 0
                {
                    // Coincident surfaces: defrag maps often place an invisible
                    // `common/slick` brush exactly over a visual floor brush. The
                    // slick overlay must win the tie so the floor is actually
                    // slippery (Warfork does this via its brush ordering).
                    best_frac = bt.enter;
                    best_normal = bt.enter_normal;
                    best_surface_flags = brush_surface_flags;
                    best_contents = brush_contents;
                    best_mover = -1;
                }
            }
        }

        // ---- Moving brush entities (transformed planes) ----
        for (mi, m) in self.movers.iter().enumerate() {
            let offset = m.plane_off as usize;
            let count = m.plane_count as usize;
            let mut bt = BrushTrace::new();
            for p in 0..count {
                let Some(&pid) = self.mover_plane_ids.get(offset + p) else {
                    continue;
                };
                let Some(plane) = self.planes.get(pid as usize) else {
                    continue;
                };
                let (n, d) = m.transform_plane(plane.normal, plane.dist);
                bt.add_plane(n, d, c0, delta, half_ext);
            }
            if bt.inside_all {
                start_solid = true;
                best_normal = bt.deepest_normal;
                best_surface_flags = 0;
                best_contents = crate::bsp::CONTENTS_SOLID;
                best_mover = mi as i32;
                break;
            }
            if bt.stays_outside {
                continue;
            }
            if bt.enter > f32::NEG_INFINITY && bt.enter < bt.exit {
                if bt.enter < 0.0 {
                    start_solid = true;
                    if best_frac > 0.0 {
                        best_frac = 0.0;
                        best_normal = bt.enter_normal;
                        best_surface_flags = 0;
                        best_contents = crate::bsp::CONTENTS_SOLID;
                        best_mover = mi as i32;
                    }
                } else if bt.enter < best_frac - 1e-4 {
                    best_frac = bt.enter;
                    best_normal = bt.enter_normal;
                    best_surface_flags = 0;
                    best_contents = crate::bsp::CONTENTS_SOLID;
                    best_mover = mi as i32;
                }
            }
        }

        if start_solid {
            // Return the direction the box is deepest inside (the plane it
            // overlaps most), so the caller can push the player OUT correctly.
            return TraceResult {
                fraction: 0.0,
                normal: best_normal,
                all_solid,
                start_solid: true,
                surface_flags: best_surface_flags,
                contents: best_contents,
                mover: best_mover,
            };
        }

        TraceResult {
            fraction: best_frac,
            normal: best_normal,
            all_solid,
            start_solid: false,
            surface_flags: best_surface_flags,
            contents: best_contents,
            mover: best_mover,
        }
    }

    /// Return the contents flags of the topmost non-solid (content) brush
    /// containing the point, OR of zero (empty). Mirrors Q3 `PointContents`:
    /// the point is inside a brush iff it is on the negative side of every
    /// bounding plane (with a small inward epsilon for boundary tolerance).
    pub fn point_contents(&self, point: [f32; 3]) -> i32 {
        let mut result = 0i32;
        for i in 0..self.brush_plane_offsets.len() {
            let contents = self.brush_contents.get(i).copied().unwrap_or(0);
            // Only non-solid content volumes contribute contents here; the
            // caller combines this with solid collision separately.
            if is_solid_contents(contents) {
                continue;
            }
            let offset = self.brush_plane_offsets[i] as usize;
            let count = self.brush_plane_count[i] as usize;
            let mut inside = true;
            for p in 0..count {
                let pid = self.brush_plane_ids[offset + p] as usize;
                let Some(plane) = self.planes.get(pid) else {
                    inside = false;
                    break;
                };
                // Inside = on the negative side (plane dist = dot(point,n)-d).
                let d = point[0] * plane.normal[0]
                    + point[1] * plane.normal[1]
                    + point[2] * plane.normal[2]
                    - plane.dist;
                if d > 0.0 {
                    inside = false;
                    break;
                }
            }
            if inside {
                result |= contents;
            }
        }
        result
    }

    /// First jumppad whose trigger brush overlaps the player's AABB. Uses the
    /// brush's actual convex planes (not just its AABB) so diagonal/slanted
    /// pads trigger correctly.
    pub fn jumppad_at(&self, origin: [f32; 3], mins: [f32; 3], maxs: [f32; 3]) -> Option<&Jumppad> {
        self.jumppads.iter().find(|jp| {
            aabb_overlap(origin, mins, maxs, jp.mins, jp.maxs)
                && self.box_intersects_brush(origin, mins, maxs, jp.plane_off, jp.plane_count)
        })
    }

    /// First teleporter whose trigger brush overlaps the player.
    pub fn teleporter_at(&self, origin: [f32; 3], mins: [f32; 3], maxs: [f32; 3]) -> Option<&Teleporter> {
        self.teleporters.iter().find(|tp| {
            aabb_overlap(origin, mins, maxs, tp.mins, tp.maxs)
                && self.box_intersects_brush(origin, mins, maxs, tp.plane_off, tp.plane_count)
        })
    }

    /// Does the player's AABB intersect the convex brush defined by the given
    /// plane run? Uses the same slab method as `trace`: the box is inside iff
    /// every plane's (box-extent-expanded) distance can bound the interval.
    fn box_intersects_brush(
        &self,
        origin: [f32; 3],
        mins: [f32; 3],
        maxs: [f32; 3],
        plane_off: u32,
        plane_count: u32,
    ) -> bool {
        let center = [
            origin[0] + (mins[0] + maxs[0]) * 0.5,
            origin[1] + (mins[1] + maxs[1]) * 0.5,
            origin[2] + (mins[2] + maxs[2]) * 0.5,
        ];
        let half = [
            (maxs[0] - mins[0]) * 0.5,
            (maxs[1] - mins[1]) * 0.5,
            (maxs[2] - mins[2]) * 0.5,
        ];
        for p in 0..plane_count as usize {
            let pid = self.trigger_plane_ids[plane_off as usize + p] as usize;
            let Some(plane) = self.planes.get(pid) else {
                return false;
            };
            let n = plane.normal;
            let radius = half[0] * n[0].abs() + half[1] * n[1].abs() + half[2] * n[2].abs();
            let dist = center[0] * n[0] + center[1] * n[1] + center[2] * n[2] - plane.dist;
            // The box is entirely OUTSIDE if it's on the positive side of any
            // plane beyond its radius.
            if dist > radius {
                return false;
            }
        }
        true
    }
}

fn aabb_overlap(
    origin: [f32; 3],
    mins: [f32; 3],
    maxs: [f32; 3],
    bmins: [f32; 3],
    bmaxs: [f32; 3],
) -> bool {
    let amin = [origin[0] + mins[0], origin[1] + mins[1], origin[2] + mins[2]];
    let amax = [origin[0] + maxs[0], origin[1] + maxs[1], origin[2] + maxs[2]];
    amin[0] <= bmaxs[0] && amax[0] >= bmins[0]
        && amin[1] <= bmaxs[1] && amax[1] >= bmins[1]
        && amin[2] <= bmaxs[2] && amax[2] >= bmins[2]
}
