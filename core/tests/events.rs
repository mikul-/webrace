//! Gameplay event bits (`EV_*`) used by the client's sound system.

use webrace_core::bsp::{Plane, CONTENTS_SOLID};
use webrace_core::input::{Cmd, BUTTON_JUMP, BUTTON_SPECIAL};
use webrace_core::pmove::{PlayerState, Pmove};
use webrace_core::trace::World;
use webrace_core::{EV_DASH, EV_FOOTSTEP, EV_JUMP, EV_LAND};

fn floor_world() -> World {
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
        brush_contents: vec![CONTENTS_SOLID],
        jumppads: vec![],
        teleporters: vec![],
        trigger_plane_ids: vec![],
        movers: vec![],
        mover_plane_ids: vec![],
        planes,
    }
}

#[test]
fn jump_emits_jump_event() {
    let mut pmove = Pmove::new(floor_world(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 24.0];
    ps.on_ground = true;
    let mut cmd = Cmd::default();
    cmd.buttons |= BUTTON_JUMP;

    pmove.step(&mut ps, &cmd);
    assert!(pmove.events & EV_JUMP != 0, "expected EV_JUMP, got {}", pmove.events);
}

#[test]
fn landing_emits_land_event() {
    let mut pmove = Pmove::new(floor_world(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 24.0];
    ps.on_ground = true;
    let mut jump = Cmd::default();
    jump.buttons |= BUTTON_JUMP;
    pmove.step(&mut ps, &jump);
    assert!(!ps.on_ground);

    // Fall back down; a land event should fire on the touchdown tick.
    let idle = Cmd::default();
    let mut landed = false;
    for _ in 0..400 {
        pmove.step(&mut ps, &idle);
        if pmove.events & EV_LAND != 0 {
            landed = true;
            break;
        }
    }
    assert!(landed, "expected EV_LAND while falling back to the floor");
}

#[test]
fn dash_emits_dash_event() {
    let mut pmove = Pmove::new(floor_world(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 24.0];
    ps.on_ground = true;
    let mut cmd = Cmd::default();
    cmd.buttons |= BUTTON_SPECIAL;
    cmd.forward = 127;

    pmove.step(&mut ps, &cmd);
    assert!(pmove.events & EV_DASH != 0, "expected EV_DASH, got {}", pmove.events);
}

#[test]
fn running_emits_footstep_events() {
    let mut pmove = Pmove::new(floor_world(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 24.0];
    ps.on_ground = true;
    let mut cmd = Cmd::default();
    cmd.forward = 127;

    let mut steps = 0;
    for _ in 0..400 {
        pmove.step(&mut ps, &cmd);
        if pmove.events & EV_FOOTSTEP != 0 {
            steps += 1;
        }
    }
    assert!(steps > 0, "expected footstep events while running");
}
