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
        let mut all_solid = true;

        let delta = [
            end[0] - start[0],
            end[1] - start[1],
            end[2] - start[2],
        ];

        for i in 0..self.brush_plane_offsets.len() {
            let offset = self.brush_plane_offsets[i] as usize;
            let count = self.brush_plane_count[i] as usize;

            // Point to test: the AABB offsets give us the "hull clipping" —
            // for each plane, offset the point that is most inside.
            let mut enter = -f32::INFINITY;
            let mut exit = f32::INFINITY;
            let mut enter_normal = [0.0f32; 3];
            let mut brush_all_solid = true;

            for p in 0..count {
                let pid = self.brush_plane_ids[offset + p] as usize;
                let Some(plane) = self.planes.get(pid) else { continue };
                let n = plane.normal;

                // Offset point for this plane (hull clipping clip point).
                let clip = [
                    if n[0] > 0.0 { maxs[0] } else { mins[0] },
                    if n[1] > 0.0 { maxs[1] } else { mins[1] },
                    if n[2] > 0.0 { maxs[2] } else { mins[2] },
                ];
                let start_dist =
                    clip[0] * n[0] + clip[1] * n[1] + clip[2] * n[2] - plane.dist;
                let end_dist = start_dist + delta[0] * n[0] + delta[1] * n[1] + delta[2] * n[2];

                if start_dist >= 0.0 {
                    brush_all_solid = false;
                }

                if start_dist > 0.0 && end_dist > 0.0 {
                    // Entire motion stays outside this plane; brush can't be hit.
                    enter = f32::INFINITY;
                    exit = -f32::INFINITY;
                    break;
                }

                if start_dist < 0.0 && end_dist < 0.0 {
                    // Both inside plane; doesn't bound the sweep.
                    continue;
                }

                let t = start_dist / (start_dist - end_dist);
                if start_dist > end_dist {
                    // Entering the brush.
                    if t > enter {
                        enter = t;
                        enter_normal = n;
                    }
                } else {
                    // Exiting the brush.
                    if t < exit {
                        exit = t;
                    }
                }
            }

            if enter == f32::INFINITY {
                continue; // no intersection
            }

            let solid = !brush_all_solid || enter == -f32::INFINITY;
            if solid {
                all_solid = false;
            }

            if enter <= 0.0 && solid && !brush_all_solid {
                start_solid = true;
            }

            if enter < best_frac && enter > 0.0 {
                best_frac = enter;
                best_normal = enter_normal;
            }
        }

        if start_solid {
            return TraceResult { fraction: 0.0, normal: [0.0, 0.0, 1.0], all_solid, start_solid: true };
        }

        TraceResult { fraction: best_frac, normal: best_normal, all_solid, start_solid: false }
    }
}
