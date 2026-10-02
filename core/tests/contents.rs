//! Tests for physical contents: water movement, jumppads, and teleporters.

use webrace_core::bsp::{
    Plane, CONTENTS_SOLID, CONTENTS_WATER, Jumppad, Teleporter,
};
use webrace_core::input::Cmd;
use webrace_core::pmove::{PlayerState, Pmove};
use webrace_core::trace::World;

/// A solid floor slab (top at z=-100) with no special contents.
fn solid_world() -> World {
    let planes = vec![
        Plane { normal: [0.0, 0.0, 1.0], dist: -100.0 },
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
        planes,
    }
}

/// A solid floor plus a water volume brush above it (non-solid contents).
fn water_world() -> World {
    let planes = vec![
        // brush 0: solid floor slab (top at z=-100).
        Plane { normal: [0.0, 0.0, 1.0], dist: -100.0 },
        Plane { normal: [0.0, 0.0, -1.0], dist: 1000.0 },
        Plane { normal: [1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [-1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
        // brush 1: water volume z in [-100, 1000].
        Plane { normal: [0.0, 0.0, 1.0], dist: 1000.0 },
        Plane { normal: [0.0, 0.0, -1.0], dist: 100.0 },
        Plane { normal: [1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [-1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
    ];
    World {
        brush_plane_offsets: vec![0, 6],
        brush_plane_count: vec![6, 6],
        brush_plane_ids: (0..12).collect(),
        brush_shaders: vec![0, 0],
        shader_flags: vec![0],
        shader_contents: vec![0, CONTENTS_WATER],
        brush_contents: vec![CONTENTS_SOLID, CONTENTS_WATER],
        jumppads: vec![],
        teleporters: vec![],
        trigger_plane_ids: vec![],
        planes,
    }
}

#[test]
fn water_volume_is_detected_and_slows_fall() {
    let mut pmove = Pmove::new(water_world(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    // Player falls from above into the water volume (water spans z -100..1000,
    // so origin z=500 is fully submerged).
    ps.origin = [0.0, 0.0, 500.0];
    ps.on_ground = false;
    ps.viewangles = [0.0, 0.0, 0.0];

    // Step a few ticks: waterlevel should be >= 2 (submerged).
    let cmd = Cmd::default();
    pmove.step(&mut ps, &cmd);
    assert!(ps.waterlevel >= 2, "expected submerged, got level {}", ps.waterlevel);
    assert_eq!(ps.watertype, CONTENTS_WATER, "watertype should be CONTENTS_WATER");

    // With no input, the player should drift down (not fall at full gravity).
    for _ in 0..60 {
        pmove.step(&mut ps, &cmd);
    }
    // Full gravity over 60 ticks would be ~ -204 ups; water caps it far smaller.
    assert!(ps.velocity[2] > -200.0, "water should slow the sink, vz={}", ps.velocity[2]);
}

/// Append the 6 planes of an axis-aligned box to the world's plane + trigger
/// plane-id arrays, returning the `(plane_off, plane_count)` trigger run.
fn add_box_trigger(world: &mut World, mins: [f32; 3], maxs: [f32; 3]) -> (u32, u32) {
    // Box planes (inside = negative side).
    let planes = [
        Plane { normal: [1.0, 0.0, 0.0], dist: maxs[0] },   // x < maxs
        Plane { normal: [-1.0, 0.0, 0.0], dist: -mins[0] }, // x > mins
        Plane { normal: [0.0, 1.0, 0.0], dist: maxs[1] },
        Plane { normal: [0.0, -1.0, 0.0], dist: -mins[1] },
        Plane { normal: [0.0, 0.0, 1.0], dist: maxs[2] },
        Plane { normal: [0.0, 0.0, -1.0], dist: -mins[2] },
    ];
    let off = world.trigger_plane_ids.len() as u32;
    for p in planes {
        world.planes.push(p);
        world.trigger_plane_ids.push(world.planes.len() as u32 - 1);
    }
    (off, 6)
}

#[test]
fn jumppad_launches_player() {
    let mut pmove = Pmove::new(solid_world(), 1.0 / 250.0);
    let (off, count) = add_box_trigger(&mut pmove.world, [-32.0, -32.0, -100.0], [32.0, 32.0, 100.0]);
    pmove.world.jumppads.push(Jumppad {
        mins: [-32.0, -32.0, -100.0],
        maxs: [32.0, 32.0, 100.0],
        plane_off: off,
        plane_count: count,
        velocity: [0.0, 0.0, 600.0],
    });
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 0.0];
    ps.on_ground = true;

    let cmd = Cmd::default();
    pmove.step(&mut ps, &cmd);

    // Jump pad should set vertical velocity to ~600 and leave the ground.
    assert!(ps.velocity[2] > 500.0, "expected launch, vz={}", ps.velocity[2]);
    assert!(!ps.on_ground, "jumppad should clear ground flag");
}

#[test]
fn teleporter_moves_player_to_destination() {
    let mut pmove = Pmove::new(solid_world(), 1.0 / 250.0);
    let (off, count) = add_box_trigger(&mut pmove.world, [-32.0, -32.0, -100.0], [32.0, 32.0, 100.0]);
    pmove.world.teleporters.push(Teleporter {
        mins: [-32.0, -32.0, -100.0],
        maxs: [32.0, 32.0, 100.0],
        plane_off: off,
        plane_count: count,
        dest_origin: [1000.0, 500.0, -100.0],
    });
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 0.0];

    let cmd = Cmd::default();
    pmove.step(&mut ps, &cmd);

    // The teleporter should have moved the player to the destination (plus drop
    // to ground). The X/Y should be at the destination.
    assert!(
        (ps.origin[0] - 1000.0).abs() < 1.0 && (ps.origin[1] - 500.0).abs() < 1.0,
        "expected teleport to destination, got {:?}",
        ps.origin
    );
}
