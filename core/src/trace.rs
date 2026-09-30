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
    /// Surface flags (SURF_SLICK etc.) of the surface that was hit (0 if none).
    pub surface_flags: i32,
}

/// Player collision box (Warfork default bounding box, in world units).
pub const PLAYER_MINS: [f32; 3] = [-16.0, -16.0, -24.0];
pub const PLAYER_MAXS: [f32; 3] = [16.0, 16.0, 40.0];

pub struct World {
    pub brush_plane_offsets: Vec<u32>,
    pub brush_plane_count: Vec<u32>,
    pub brush_plane_ids: Vec<u32>,
    /// Shader index of each collision brush (parallel to the brush arrays).
    pub brush_shaders: Vec<i32>,
    pub planes: Vec<Plane>,
    /// Surface flags (SURF_SLICK etc.) per shader, used for slick/gameplay.
    pub shader_flags: Vec<i32>,
}

impl World {
    pub fn from_bsp(bsp: &Bsp) -> World {
        World {
            brush_plane_offsets: bsp.brush_plane_offsets.clone(),
            brush_plane_count: bsp.brush_plane_count.clone(),
            brush_plane_ids: bsp.brush_plane_ids.clone(),
            brush_shaders: bsp.brush_shaders.clone(),
            planes: bsp.planes.clone(),
            shader_flags: bsp.shader_flags.clone(),
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
            // Surface flags of this brush (via its shader), used for slick etc.
            let brush_surface_flags = self
                .brush_shaders
                .get(i)
                .and_then(|&s| self.shader_flags.get(s as usize).copied())
                .unwrap_or(0);

            let mut enter = -f32::INFINITY;
            let mut exit = f32::INFINITY;
            let mut enter_normal = [0.0f32; 3];
            let mut inside_all = true;
            let mut stays_outside = false;
            let mut deepest = f32::INFINITY; // most-negative start_dist
            let mut deepest_normal = [0.0f32; 3];

            for p in 0..count {
                let pid = self.brush_plane_ids[offset + p] as usize;
                let Some(plane) = self.planes.get(pid) else { continue };
                let n = plane.normal;

                let radius =
                    half_ext[0] * n[0].abs() + half_ext[1] * n[1].abs() + half_ext[2] * n[2].abs();
                let start_dist =
                    c0[0] * n[0] + c0[1] * n[1] + c0[2] * n[2] - plane.dist - radius;
                let end_dist = start_dist
                    + delta[0] * n[0]
                    + delta[1] * n[1]
                    + delta[2] * n[2];

                // Strictly inside = on the solid side (start_dist < 0). A box
                // merely *touching* a plane (start_dist == 0) is on the
                // surface, not inside — this prevents resting-on-floor from
                // being misreported as start_solid.
                if start_dist >= 0.0 {
                    inside_all = false;
                }
                if start_dist < deepest {
                    deepest = start_dist;
                    deepest_normal = n;
                }

                // No crossing along the sweep: doesn't bound the interval.
                if start_dist > 0.0 && end_dist > 0.0 {
                    // The box stays on the empty side of this plane for the
                    // whole sweep, so it never enters this brush at all.
                    stays_outside = true;
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
                    if t > enter {
                        enter = t;
                        enter_normal = n;
                    }
                } else if start_dist < 0.0 {
                    if t < exit {
                        exit = t;
                    }
                }
            }

            if inside_all {
                // The box is fully inside this brush at the start position.
                start_solid = true;
                best_normal = deepest_normal;
                best_surface_flags = brush_surface_flags;
                break;
            }

            if stays_outside {
                // The box remains outside at least one bounding plane, so it
                // never enters this brush. No hit.
                continue;
            }

            if enter > f32::NEG_INFINITY && enter < exit {
                if enter < 0.0 {
                    // The box was already inside this brush before the sweep
                    // began (entered at negative t) — genuinely embedded.
                    start_solid = true;
                    if best_frac > 0.0 {
                        best_frac = 0.0;
                        best_normal = enter_normal;
                        best_surface_flags = brush_surface_flags;
                    }
                } else if enter < best_frac {
                    best_frac = enter;
                    best_normal = enter_normal;
                    best_surface_flags = brush_surface_flags;
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
            };
        }

        TraceResult {
            fraction: best_frac,
            normal: best_normal,
            all_solid,
            start_solid: false,
            surface_flags: best_surface_flags,
        }
    }
}
