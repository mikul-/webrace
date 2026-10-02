use webrace_core::bsp::Plane;
use webrace_core::input::Cmd;
use webrace_core::pmove::{PlayerState, Pmove, SURF_SLICK};
use webrace_core::trace::World;

/// A slick ramp: floor plane tilted (normal not vertical), so the player
/// slides down and accelerates.
fn slick_ramp_world() -> World {
    // A ramp descending toward +X: the ground plane normal points up+back,
    // e.g. normal ( -sin, 0, cos ) with the surface sloping down in +X.
    // We model it as a huge solid below a tilted plane.
    // Plane normal ~ (0.0, 0.0, 1.0) but tilted: use n = (-0.1, 0, 0.995).
    let n = [-0.1_f32, 0.0, 0.995];
    // Normalize.
    let l = (n[0]*n[0] + n[1]*n[1] + n[2]*n[2]).sqrt();
    let n = [n[0]/l, n[1]/l, n[2]/l];
    let planes = vec![
        // tilted ground plane (solid below): empty where dot(n,p) - d > 0
        Plane { normal: n, dist: 0.0 },
        // bottom/back and side bounds to make a convex solid
        Plane { normal: [0.0, 0.0, -1.0], dist: 1000.0 },
        Plane { normal: [1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [-1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
    ];
    World {
        brush_plane_offsets: vec![0],
        brush_plane_count: vec![6],
        brush_plane_ids: (0..6).collect(),
        brush_shaders: vec![0],
        shader_flags: vec![SURF_SLICK],
        shader_contents: vec![0],
        brush_contents: vec![webrace_core::bsp::CONTENTS_SOLID],
        jumppads: vec![],
        teleporters: vec![],
        trigger_plane_ids: vec![],
        planes,
    }
}

#[test]
fn slick_ramp_accelerates_downhill() {
    let mut pmove = Pmove::new(slick_ramp_world(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 60.0];
    ps.viewangles = [0.0, 0.0, 0.0];
    pmove.drop_to_ground(&mut ps);

    // No input — let gravity slide the player down the ramp.
    let mut peak_speed = 0.0f32;
    for _ in 0..500 {
        let cmd = Cmd::default();
        pmove.step(&mut ps, &cmd);
        let h = ps.speed;
        if h > peak_speed { peak_speed = h; }
    }

    println!("slick ramp: peak horizontal speed = {peak_speed:.1} ups, pos={:?}", ps.origin);
    // The player should have slid down the ramp and gained speed beyond the
    // frictionless drift (with no input, frictionless flat floor stays ~0, but
    // a ramp accelerates due to gravity). Expect meaningful downhill speed.
    assert!(
        peak_speed > 100.0,
        "slick ramp should accelerate downhill, got {peak_speed:.1}"
    );
}
