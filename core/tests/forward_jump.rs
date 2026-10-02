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

    // Hold forward + jump through a full jump arc and land, tracking hspeed.
    let mut peak = 0.0f32;
    let mut landed = None;
    for tick in 0..200 {
        let mut cmd = Cmd::default();
        cmd.forward = 127;
        cmd.up = 127;
        cmd.buttons |= webrace_core::input::BUTTON_JUMP;
        pmove.step(&mut ps, &cmd);
        let h = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1]).sqrt();
        if h > peak {
            peak = h;
        }
        if ps.on_ground && tick > 30 {
            landed = Some(h);
            break;
        }
    }
    let land = landed.unwrap();
    println!("forward-jump: pre={hspeed0:.1} peak={peak:.1} landed={land:.1}");

    // A forward jump must NOT add a spurious ~320 ups (landing at ~640). It
    // should climb gently, landing near ~350 (Warfork maxPlayerSpeed=320).
    assert!(
        land < 400.0,
        "forward jump landed too fast: {land} (expected ~350, was ~640)"
    );
    assert!(land > hspeed0, "should still gain a little speed while airborne");
}
