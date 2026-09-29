//! AABB clipping-hull trace against BSP brushes (exact `pmove` semantics).
//!
//! This is the collision primitive the movement code depends on. It traces an
//! axis-aligned box (the player's mins/maxs) against the set of convex solid
//! brushes parsed from the BSP, returning the closest plane hit (fraction,
//! normal). Port of the logic in Warfork's `cm_trace` / `PM_SlideMove`.

#![allow(dead_code)]

use crate::bsp::{Bsp, Plane};

#[derive(Clone, Copy, Debug, Default)]
pub struct TraceResult {
    pub fraction: f32,
    pub normal: [f32; 3],
    pub all_solid: bool,
    pub start_solid: bool,
}

/// Player collision box (Warfork default bounding box, in world units).
pub const PLAYER_MINS: [f32; 3] = [-16.0, -16.0, -24.0];
pub const PLAYER_MAXS: [f32; 3] = [16.0, 16.0, 40.0];

pub struct World {
    // Reference to the collision brushes.
    pub brush_plane_offsets: Vec<u32>,
    pub brush_plane_count: Vec<u32>,
    pub brush_plane_ids: Vec<u32>,
    pub planes: Vec<Plane>,
}

impl World {
    pub fn from_bsp(bsp: &Bsp) -> World {
        World {
            brush_plane_offsets: bsp.brush_plane_offsets.clone(),
            brush_plane_count: bsp.brush_plane_count.clone(),
            brush_plane_ids: bsp.brush_plane_ids.clone(),
            planes: bsp.planes.clone(),
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
        let mut start_solid = false;
        let all_solid = true;

        let delta = [
            end[0] - start[0],
            end[1] - start[1],
            end[2] - start[2],
        ];

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

        for i in 0..self.brush_plane_offsets.len() {
            let offset = self.brush_plane_offsets[i] as usize;
            let count = self.brush_plane_count[i] as usize;

            let mut enter = -f32::INFINITY;
            let mut exit = f32::INFINITY;
            let mut enter_normal = [0.0f32; 3];

            for p in 0..count {
                let pid = self.brush_plane_ids[offset + p] as usize;
                let Some(plane) = self.planes.get(pid) else { continue };
                let n = plane.normal;

                // Signed distance from box center to plane, minus box radius
                // projected on the normal (support function).
                let radius =
                    half_ext[0] * n[0].abs() + half_ext[1] * n[1].abs() + half_ext[2] * n[2].abs();
                let start_dist =
                    c0[0] * n[0] + c0[1] * n[1] + c0[2] * n[2] - plane.dist - radius;
                let end_dist = start_dist
                    + delta[0] * n[0]
                    + delta[1] * n[1]
                    + delta[2] * n[2];

                // No crossing of this plane along the sweep: doesn't bound.
                if start_dist > 0.0 && end_dist > 0.0 {
                    continue;
                }
                if start_dist < 0.0 && end_dist < 0.0 {
                    continue;
                }
                if start_dist == 0.0 && end_dist == 0.0 {
                    continue;
                }

                let denom = start_dist - end_dist;
                let t = if denom.abs() < 1e-12 { 0.0 } else { start_dist / denom };

                if start_dist > end_dist {
                    // Moving into the solid side: entering this half-space.
                    if t > enter {
                        enter = t;
                        enter_normal = n;
                    }
                } else if start_dist < 0.0 {
                    // Currently inside (start_dist < 0) and moving out
                    // (end_dist > start_dist): this bounds the exit.
                    if t < exit {
                        exit = t;
                    }
                }
                // start_dist == 0 and moving away (end_dist > 0): the box is
                // touching but leaves — imposes no constraint, skip.
            }

            if enter > f32::NEG_INFINITY && enter < exit {
                if enter <= 0.0 {
                    start_solid = true;
                    if best_frac > 0.0 {
                        best_frac = 0.0;
                        best_normal = enter_normal;
                    }
                } else if enter < best_frac {
                    best_frac = enter;
                    best_normal = enter_normal;
                }
            }
        }

        if start_solid {
            return TraceResult { fraction: 0.0, normal: [0.0, 0.0, 1.0], all_solid, start_solid: true };
        }

        TraceResult { fraction: best_frac, normal: best_normal, all_solid, start_solid: false }
    }
}
