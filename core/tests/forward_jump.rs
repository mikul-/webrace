//! Regression: a forward jump must not add a spurious ~320 ups on the first
//! airborne tick (the aircontrol bug multiplied velocity by `speed` instead of
//! a unit direction). The player should keep ~320 ups and ramp up slowly.

use webrace_core::bsp::Plane;
use webrace_core::input::Cmd;
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
        brush_plane_offsets: vec![0],
        brush_plane_count: vec![6],
        brush_plane_ids: (0..6).collect(),
        brush_shaders: vec![0],
        shader_flags: vec![0],
        shader_contents: vec![0],
        brush_contents: vec![webrace_core::bsp::CONTENTS_SOLID],
        jumppads: vec![],
        teleporters: vec![],
        trigger_plane_ids: vec![],
        planes,
    }
}

#[test]
fn forward_jump_does_not_add_320() {
    let mut pmove = Pmove::new(flat_world(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 60.0];
    ps.viewangles = [0.0, 0.0, 0.0]; // yaw 0 -> forward = +X
    pmove.drop_to_ground(&mut ps);

    // Walk forward (no jump) for a bit to reach ~run speed.
    for _ in 0..60 {
        let mut cmd = Cmd::default();
        cmd.forward = 127;
        pmove.step(&mut ps, &cmd);
    }
    let hspeed0 = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1]).sqrt();
    println!("pre-jump hspeed = {hspeed0:.1}");

    // First airborne tick: hold forward + jump.
    let mut cmd = Cmd::default();
    cmd.forward = 127;
    cmd.up = 127;
    cmd.buttons |= webrace_core::input::BUTTON_JUMP;
    pmove.step(&mut ps, &cmd);
    let hspeed1 = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1]).sqrt();
    println!("first-airborne hspeed = {hspeed1:.1}");

    // The first airborne tick must NOT add ~320 ups on top; it should stay
    // close to the pre-jump speed (within ~50 ups of a gentle ramp).
    assert!(
        (hspeed1 - hspeed0).abs() < 50.0,
        "forward jump added a spurious speed burst: {} -> {}",
        hspeed0,
        hspeed1
    );
    assert!(hspeed1 < hspeed0 + 100.0, "expected slow ramp, got {hspeed1}");
}
