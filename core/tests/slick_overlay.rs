//! Coincident-brush regression: defrag maps often place an invisible
//! `common/slick` brush exactly over a visual (non-slick) floor brush. When the
//! trace hits both at the same fraction, the slick overlay must win, otherwise
//! the floor is wrongly non-slippery (Warfork resolves this via brush order).

use webrace_core::bsp::Plane;
use webrace_core::trace::World;

fn overlay_world(slick_first: bool) -> World {
    // One solid box (x,y in [-10000,10000], z in [-1000,0]).
    let box_planes = |slick: bool| {
        vec![
            Plane { normal: [0.0, 0.0, 1.0], dist: 0.0 },
            Plane { normal: [0.0, 0.0, -1.0], dist: 1000.0 },
            Plane { normal: [1.0, 0.0, 0.0], dist: 10000.0 },
            Plane { normal: [-1.0, 0.0, 0.0], dist: 10000.0 },
            Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
            Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
        ]
    };
    let _ = slick_first;
    let mut planes = box_planes(false);
    planes.extend(box_planes(false));
    // Two switches, same 6 planes each. The slick one uses shader 1.
    World {
        brush_plane_offsets: vec![0, 6],
        brush_plane_count: vec![6, 6],
        brush_plane_ids: (0..12).collect(),
        brush_shaders: vec![0, 1],
        // shader 0 = non-slick, shader 1 = slick
        shader_flags: vec![0, webrace_core::pmove::SURF_SLICK],
        shader_contents: vec![0, 0],
        planes,
        brush_contents: vec![webrace_core::bsp::CONTENTS_SOLID; 2],
        jumppads: vec![],
        teleporters: vec![],
        trigger_plane_ids: vec![],
    }
}

#[test]
fn coincident_slick_overlay_wins() {
    let w = overlay_world(false);
    let mins = webrace_core::trace::PLAYER_MINS;
    let maxs = webrace_core::trace::PLAYER_MAXS;
    // Player resting on the floor: feet (origin.z - 24) at z=0, the floor top.
    let p = [0.0, 0.0, 24.0];
    let tr = w.trace(p, mins, maxs, [p[0], p[1], p[2] - 2.0]);
    println!(
        "coincident trace: fraction={:.3} flags={:#x} slick={}",
        tr.fraction,
        tr.surface_flags,
        tr.surface_flags & webrace_core::pmove::SURF_SLICK != 0
    );
    assert!(
        tr.surface_flags & webrace_core::pmove::SURF_SLICK != 0,
        "coincident slick overlay should win the tie, got flags {:#x}",
        tr.surface_flags
    );
}
