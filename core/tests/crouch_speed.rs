use webrace_core::bsp::Plane;
use webrace_core::input::{Cmd, BUTTON_CROUCH};
use webrace_core::pmove::{PlayerState, Pmove};
use webrace_core::trace::World;

fn flat_world() -> World {
    let planes = vec![
        Plane { normal: [0.0, 0.0, 1.0], dist: 0.0 },
        Plane { normal: [0.0, 0.0, -1.0], dist: 1000.0 },
        Plane { normal: [1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [-1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
    ];
    World {
        brush_plane_offsets: vec![0], brush_plane_count: vec![6],
        brush_plane_ids: (0..6).collect(), brush_shaders: vec![0], shader_flags: vec![0], planes,
        shader_contents: vec![0],
        brush_contents: vec![webrace_core::bsp::CONTENTS_SOLID],
        jumppads: vec![],
        teleporters: vec![],
    }
}

#[test]
fn crouch_reduces_ground_speed_to_160() {
    let mut pmove = Pmove::new(flat_world(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 60.0];
    ps.viewangles = [0.0, 0.0, 0.0];
    pmove.drop_to_ground(&mut ps);

    // Run forward uncrouched -> should reach ~320.
    let mut cmd = Cmd::default();
    cmd.forward = 127;
    for _ in 0..200 {
        pmove.step(&mut ps, &cmd);
    }
    let run_speed = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1]).sqrt();
    println!("run speed (uncrouched): {run_speed:.1} ups");
    assert!(run_speed > 300.0, "uncrouched run should be ~320, got {run_speed}");

    // Now crouch and continue moving — speed should drop to ~160.
    cmd.buttons |= BUTTON_CROUCH;
    for _ in 0..200 {
        pmove.step(&mut ps, &cmd);
    }
    let crouch_speed = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1]).sqrt();
    println!("crouch speed: {crouch_speed:.1} ups");
    assert!(
        crouch_speed < 200.0,
        "crouch should cap speed at 160, got {crouch_speed}"
    );
}
