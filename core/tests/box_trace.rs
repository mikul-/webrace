use webrace_core::bsp::Plane;
use webrace_core::trace::{World, PLAYER_MINS, PLAYER_MAXS};
use webrace_core::trace::TraceResult;

// A solid box from x in [100,200], y in [100,200], z in [0,100].
// Planes with normals pointing OUTWARD (empty on the positive side).
fn box_world() -> World {
    // Solid where x in [100,200], etc. Empty elsewhere.
    // Plane (n, d) means empty where dot(n,p) > d (positive side is empty).
    // For solid x in [100,200]: left wall is x=100 (empty x<100 -> normal -x),
    // right wall x=200 (empty x>200 -> normal +x).
    let planes = vec![
        Plane { normal: [-1.0, 0.0, 0.0], dist: -100.0 }, // x <= 100 solid side
        Plane { normal: [1.0, 0.0, 0.0], dist: 200.0 },   // x >= 200 solid side
        Plane { normal: [0.0, -1.0, 0.0], dist: -100.0 },
        Plane { normal: [0.0, 1.0, 0.0], dist: 200.0 },
        Plane { normal: [0.0, 0.0, -1.0], dist: -0.0 },
        Plane { normal: [0.0, 0.0, 1.0], dist: 100.0 },
    ];
    World {
        brush_plane_offsets: vec![0],
        brush_plane_count: vec![6],
        brush_plane_ids: (0..6).collect(),
        brush_shaders: vec![0],
        shader_flags: vec![0],
        planes,
    }
}

fn probe(world: &World, label: &str, start: [f32; 3], end: [f32; 3]) {
    let r: TraceResult = world.trace(start, PLAYER_MINS, PLAYER_MAXS, end);
    println!("{label}: frac={:.3} normal={:?} start_solid={}", r.fraction, r.normal, r.start_solid);
}

#[test]
fn box_trace_sanity() {
    let w = box_world();
    // Player box center at (150,150,50), box spans x[134,166] etc. Inside the solid box.
    probe(&w, "center inside box  ", [150.0, 150.0, 50.0], [150.0, 150.0, 50.0]);
    // Far outside, tracing toward the box.
    probe(&w, "far -x -> +x      ", [50.0, 150.0, 50.0], [160.0, 150.0, 50.0]);
    // Above the box, tracing down onto it.
    probe(&w, "above -> down     ", [150.0, 150.0, 200.0], [150.0, 150.0, 50.0]);
    // Clear trace (no hit).
    probe(&w, "clear (no hit)    ", [0.0, 0.0, 500.0], [0.0, 0.0, 400.0]);
}
