use webrace_core::bsp::Plane;
use webrace_core::input::Cmd;
use webrace_core::pmove::{PlayerState, Pmove};
use webrace_core::trace::World;

/// Floor slab with SURF_SLICK flag on the floor surface (shader 0 has SLICK).
fn slick_world() -> World {
    let planes = vec![
        Plane { normal: [0.0, 0.0, 1.0], dist: 0.0 },
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
        shader_flags: vec![webrace_core::pmove::SURF_SLICK],
        planes,
    }
}

#[test]
fn slick_floor_preserves_speed() {
    let mut pmove = Pmove::new(slick_world(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 60.0];
    ps.viewangles = [0.0, 0.0, 0.0]; // forward +X
    pmove.drop_to_ground(&mut ps);

    // Build up forward speed holding W, then release and check it persists
    // (slick = no friction, so momentum is preserved).
    for tick in 0..500 {
        let mut cmd = Cmd::default();
        if tick < 250 {
            cmd.forward = 127;
        }
        pmove.step(&mut ps, &cmd);
    }
    let hspeed = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1]).sqrt();
    println!("slick: hspeed after 250 ticks release = {hspeed:.1} ups, pos={:?}", ps.origin);
    // On slick (no friction) the player keeps their speed after releasing.
    // accelerate is airaccelerate (1) so build-up is slow, but once moving the
    // speed must not decay.
    assert!(hspeed > 100.0, "slick floor should preserve/build speed, got {hspeed}");
    assert!(ps.origin[0] > 20.0, "player didn't slide, x={}", ps.origin[0]);
}
