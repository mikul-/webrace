//! Step-up view smoothing (Warfork `CG_PredictAddStep`): after climbing a
//! step, the camera should sit *below* the new eye height and ease up over
//! `PREDICTED_STEP_TIME` ms, instead of snapping.

use webrace_core::bsp::{Bsp, Plane, SpawnPoint};
use webrace_core::sim::Session;

/// Minimal Bsp: a floor (z<0) plus one step (x in [64,160], z top = 16), a
/// spawn, and no race gates.
fn step_bsp() -> Bsp {
    // brush 0: floor slab, top at z=0.
    // brush 1: step box, x in [64,160], z in [-1000,16].
    let planes = vec![
        Plane { normal: [0.0, 0.0, 1.0], dist: 0.0 },
        Plane { normal: [0.0, 0.0, -1.0], dist: 1000.0 },
        Plane { normal: [1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [-1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
        // step planes (brush 1, 6 planes at indices 6..12)
        Plane { normal: [1.0, 0.0, 0.0], dist: 160.0 },
        Plane { normal: [-1.0, 0.0, 0.0], dist: -64.0 },
        Plane { normal: [0.0, 0.0, 1.0], dist: 16.0 },
        Plane { normal: [0.0, 0.0, -1.0], dist: 1000.0 },
        Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
    ];
    Bsp {
        name: "step".into(),
        positions: vec![],
        indices: vec![],
        chunks: vec![],
        brush_plane_offsets: vec![0, 6],
        brush_plane_count: vec![6, 6],
        brush_plane_ids: (0..12).collect(),
        brush_shaders: vec![0, 0],
        planes,
        shaders: vec!["noshader".into()],
        shader_flags: vec![0],
        shader_contents: vec![1],
        brush_contents: vec![1, 1],
        spawns: vec![SpawnPoint { origin: [0.0, 0.0, 64.0], yaw: 0.0 }],
        race_gates: vec![],
        jumppads: vec![],
        teleporters: vec![],
        trigger_plane_ids: vec![],
        lightmaps: vec![],
        lightmap_atlas: vec![],
        lightmap_atlas_w: 0,
        lightmap_atlas_h: 0,
    }
}

#[test]
fn step_up_smooths_the_view() {
    let mut s = Session::new(&step_bsp(), 0).unwrap();
    s.set_keys(true, false, false, false, false, false, false, false); // forward +X

    let mut min_deficit = 0.0f32; // origin.z + viewheight - eye.z (how far below)
    let mut max_deficit = 0.0f32;
    let mut prev_z = s.origin()[2];
    for _ in 0..400 {
        s.step();
        let o = s.origin();
        let eye = s.eye();
        let ideal = o[2] + 30.0; // stand viewheight
        let deficit = ideal - eye[2];
        if deficit > 0.01 {
            // The view is below the ideal -> a step is being smoothed.
            min_deficit = if min_deficit == 0.0 { deficit } else { min_deficit.min(deficit) };
            max_deficit = max_deficit.max(deficit);
        }
        prev_z = o[2];
    }
    let _ = prev_z;
    println!("step smoothing: deficit min={min_deficit:.2} max={max_deficit:.2}");
    // A ~16-unit step should have produced a visible downward view offset that
    // eases away (not a snap).
    assert!(max_deficit > 5.0, "expected a smoothed step view offset, got {max_deficit}");
    // And it must ease fully back to zero eventually.
    let o = s.origin();
    let eye = s.eye();
    assert!((eye[2] - (o[2] + 30.0)).abs() < 0.01, "view should settle at the ideal height");
}
