use webrace_core::bsp::Plane;
use webrace_core::trace::World;

fn floor_world() -> World {
    let planes = vec![
        Plane { normal: [0.0,0.0,1.0], dist: 0.0 },      // solid below z=0
        Plane { normal: [0.0,0.0,-1.0], dist: 1000.0 },  // above z=-1000
        Plane { normal: [1.0,0.0,0.0], dist: 10000.0 },
        Plane { normal: [-1.0,0.0,0.0], dist: 10000.0 },
        Plane { normal: [0.0,1.0,0.0], dist: 10000.0 },
        Plane { normal: [0.0,-1.0,0.0], dist: 10000.0 },
    ];
    World {
        movers: vec![],
        mover_plane_ids: vec![], brush_plane_offsets: vec![0], brush_plane_count: vec![6], brush_plane_ids: (0..6).collect(),
        brush_shaders: vec![0], shader_flags: vec![0], shader_contents: vec![0], planes,
        brush_contents: vec![webrace_core::bsp::CONTENTS_SOLID], jumppads: vec![], teleporters: vec![], trigger_plane_ids: vec![] }
}

#[test]
fn box_overlapping_floor_is_start_solid() {
    let w = floor_world();
    let mins = webrace_core::trace::PLAYER_MINS;   // [-16,-16,-24]
    let maxs = webrace_core::trace::PLAYER_MAXS;   // [16,16,40]
    // Player origin at z=10 => box from z=-14 to z=50, overlapping floor (z<0).
    let p = [0.0, 0.0, 10.0];
    let tr = w.trace(p, mins, maxs, p);
    println!("origin z=10 (overlap): fraction={} start_solid={} all_solid={} normal={:?}", tr.fraction, tr.start_solid, tr.all_solid, tr.normal);
    // The box bottom (z=-14) is inside the solid floor, so start_solid should be true.
    assert!(tr.start_solid, "box overlapping floor should be start_solid, got {}", tr.start_solid);
}
