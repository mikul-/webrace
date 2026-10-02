use webrace_core::bsp::Plane;
use webrace_core::input::Cmd;
use webrace_core::pmove::{PlayerState, Pmove, SURF_SLICK};
use webrace_core::trace::World;

/// Build a solid whose top surface is the tilted plane with the given normal
/// (a ramp). The surface descends toward +X when the normal has a -X component.
fn ramp_world(normal: [f32; 3]) -> World {
    let l = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    let n = [normal[0] / l, normal[1] / l, normal[2] / l];
    let planes = vec![
        Plane { normal: n, dist: 0.0 },
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

/// A moving player on a *walkable* slick surface keeps their horizontal speed
/// (slick = no friction). This is Warfork's core slick behavior.
#[test]
fn slick_surface_preserves_moving_speed() {
    // Gentle ramp (normal.z ~= 0.995, walkable).
    let mut pmove = Pmove::new(ramp_world([-0.1, 0.0, 0.995]), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 60.0];
    ps.viewangles = [0.0, 0.0, 0.0];
    pmove.drop_to_ground(&mut ps);
    // Give a running start (into the hill, +X).
    ps.velocity = [400.0, 0.0, 0.0];

    for _ in 0..250 {
        pmove.step(&mut ps, &Cmd::default());
    }
    let h = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1]).sqrt();
    println!("slick moving: hspeed={h:.1} pos={:?}", ps.origin);
    // No friction on slick: the player should retain most of their speed.
    assert!(h > 300.0, "slick should preserve moving speed, got {h}");
}

/// A stationary player on a gentle slick ramp does NOT slide (Warfork applies
/// no gravity on the ground); the ramp only redirects existing momentum.
#[test]
fn stationary_on_gentle_slick_ramp_does_not_slide() {
    let mut pmove = Pmove::new(ramp_world([-0.1, 0.0, 0.995]), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 60.0];
    ps.viewangles = [0.0, 0.0, 0.0];
    pmove.drop_to_ground(&mut ps);
    let start = ps.origin;
    for _ in 0..300 {
        pmove.step(&mut ps, &Cmd::default());
    }
    let drift = ((ps.origin[0] - start[0]).powi(2) + (ps.origin[1] - start[1]).powi(2)).sqrt();
    println!("stationary slick ramp drift = {drift:.2}");
    assert!(drift < 1.0, "stationary player should not slide, drifted {drift}");
}

/// A *steep* slick ramp is not walkable (normal.z < 0.7), so the player is
/// airborne and gravity accelerates them downhill.
#[test]
fn steep_slick_ramp_accelerates_downhill() {
    // Steep ramp: normal.z ~= 0.6 (< 0.7 => not walkable).
    let mut pmove = Pmove::new(ramp_world([-0.8, 0.0, 0.6]), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    // Start above the ramp surface and let gravity take over.
    ps.origin = [0.0, 0.0, 5.0];
    ps.viewangles = [0.0, 0.0, 0.0];
    let mut peak = 0.0f32;
    for _ in 0..500 {
        pmove.step(&mut ps, &Cmd::default());
        let h = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1]).sqrt();
        if h > peak {
            peak = h;
        }
    }
    println!("steep slick ramp peak hspeed = {peak:.1}, pos={:?}", ps.origin);
    assert!(peak > 100.0, "steep slick ramp should accelerate downhill, got {peak}");
}
