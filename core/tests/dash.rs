//! Dash regression: dashing while running up a ramp must register. Our old
//! `grounded()` treated `velocity[2] > 20` as airborne, so on an up-ramp (which
//! redirects horizontal speed into a small +z) the dash intermittently failed.
//! Warfork only treats `velocity[2] > 180` as airborne.

use webrace_core::bsp::Plane;
use webrace_core::input::Cmd;
use webrace_core::pmove::{PlayerState, Pmove};
use webrace_core::trace::World;

/// A ramp rising toward +X: normal (-nx, 0, nz) so the surface climbs with x.
fn ramp_world() -> World {
    let nz = 0.995_f32;
    let nx = (1.0 - nz * nz).sqrt();
    let planes = vec![
        Plane { normal: [-nx, 0.0, nz], dist: 0.0 },
        Plane { normal: [0.0, 0.0, -1.0], dist: 5000.0 },
        Plane { normal: [1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [-1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
    ];
    World {
        movers: vec![],
        mover_plane_ids: vec![],
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
fn dash_registers_on_uphill_ramp() {
    let mut pmove = Pmove::new(ramp_world(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 60.0];
    ps.viewangles = [0.0, 0.0, 0.0]; // forward = +X (up the ramp)
    pmove.drop_to_ground(&mut ps);

    // Run up the ramp to build some upward z (the ramp redirects into +z).
    for _ in 0..40 {
        let mut cmd = Cmd::default();
        cmd.forward = 127;
        pmove.step(&mut ps, &cmd);
    }
    println!("pre-dash: vel={:?} ground={}", ps.velocity, ps.on_ground);
    let pre = ps.velocity;

    // Press dash (special) once while on the ramp.
    for _ in 0..3 {
        let mut cmd = Cmd::default();
        cmd.forward = 127;
        cmd.buttons |= webrace_core::input::BUTTON_SPECIAL;
        pmove.step(&mut ps, &cmd);
        if ps.doshtime > 0 {
            break;
        }
    }
    println!("post-dash: vel={:?} doshtime={}", ps.velocity, ps.doshtime);

    // The dash must have fired (doshtime set), and it should have redirected
    // the horizontal velocity (not left it unchanged).
    assert!(ps.doshtime > 0, "dash did not register on the uphill ramp");
    let moved = (ps.velocity[0] - pre[0]).abs() + (ps.velocity[1] - pre[1]).abs();
    assert!(moved > 1.0 || ps.velocity[2] > pre[2] + 50.0, "dash had no effect");
}
