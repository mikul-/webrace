//! Stair/ledge climbing regression: the player must be able to walk up stacked
//! ledges (small steps) without jumping, matching Q3's `PM_StepSlideMove`
//! behavior of stepping up small obstructions and gliding over the top.

use webrace_core::bsp::Plane;
use webrace_core::input::Cmd;
use webrace_core::pmove::{PlayerState, Pmove};
use webrace_core::trace::World;

/// A staircase: three 8-unit steps, each 32 units deep, rising to z=24.
/// The player walks along +X and must climb all three steps without jumping.
///
/// Plane convention (same as the other movement tests): inside = negative side
/// = `dot(point, normal) - dist < 0`.
///   - floor slab: z in [-1000, 0]  -> `normal [0,0,1] dist 0`, `[0,0,-1] dist 1000`
///   - a step [x0,x1] x [h0,h1] z:  -> `[1,0,0] dist x1`, `[-1,0,0] dist -x0`,
///        `[0,0,1] dist -h1` (z < h1), `[0,0,-1] dist -h0` (z > h0)
///   - y bounds: `[0,1,0] dist 10000`, `[0,-1,0] dist 10000`
fn stair_world() -> World {
    let mut planes = vec![
        // brush 0: floor slab z in [-1000, 0]
        Plane { normal: [0.0, 0.0, 1.0], dist: 0.0 },
        Plane { normal: [0.0, 0.0, -1.0], dist: 1000.0 },
        Plane { normal: [1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [-1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
    ];

    // three step brushes, each a box [x0,x1] x [-10000,10000] x [0,h]
    let steps: [(f32, f32, f32); 3] = [(64.0, 96.0, 8.0), (96.0, 128.0, 16.0), (128.0, 160.0, 24.0)];
    for (x0, x1, h) in steps {
        planes.extend_from_slice(&[
            Plane { normal: [1.0, 0.0, 0.0], dist: x1 },       // x < x1
            Plane { normal: [-1.0, 0.0, 0.0], dist: -x0 },     // x > x0
            Plane { normal: [0.0, 0.0, 1.0], dist: h },        // z < h
            Plane { normal: [0.0, 0.0, -1.0], dist: 0.0 },     // z > 0
            Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
            Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
        ]);
    }

    let nbrushes = 4; // 1 floor + 3 steps
    let mut offsets = Vec::new();
    let mut counts = Vec::new();
    let idx: Vec<u32> = (0..(nbrushes * 6)).map(|i| i as u32).collect();
    let mut p = 0u32;
    for _ in 0..nbrushes {
        offsets.push(p);
        counts.push(6);
        p += 6;
    }
    World {
        movers: vec![],
        mover_plane_ids: vec![],
        brush_plane_offsets: offsets,
        brush_plane_count: counts,
        brush_plane_ids: idx,
        brush_shaders: vec![0; nbrushes],
        shader_flags: vec![0],
        planes,
        shader_contents: vec![0],
        brush_contents: vec![webrace_core::bsp::CONTENTS_SOLID; nbrushes],
        jumppads: vec![],
        teleporters: vec![],
        trigger_plane_ids: vec![],
    }
}

#[test]
fn player_walks_up_stairs() {
    let mut pmove = Pmove::new(stair_world(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 40.0];
    ps.viewangles = [0.0, 0.0, 0.0]; // yaw 0 -> forward = +X
    pmove.drop_to_ground(&mut ps);
    let start_z = ps.origin[2];
    println!("start: origin={:?}", ps.origin);

    // Walk forward (hold W) for 2 seconds — should climb all three 8-unit steps
    // to z=24 (origin z=48) without jumping, then descend the far side.
    let mut max_z = start_z;
    for _ in 0..500 {
        let mut cmd = Cmd::default();
        cmd.forward = 127;
        pmove.step(&mut ps, &cmd);
        max_z = max_z.max(ps.origin[2]);
    }
    println!("final: origin={:?} max_z={max_z:.2}", ps.origin);

    // The player should have climbed the full staircase (three 8-unit steps =
    // +24 units, origin z from 24 to ~48), proving step-up works.
    assert!(
        max_z >= start_z + 20.0,
        "player failed to climb stairs: max origin z {max_z} vs start {start_z}"
    );
    // And should have moved substantially forward (+X) past the stairs.
    assert!(ps.origin[0] > 60.0, "player did not advance up the stairs, x={}", ps.origin[0]);
}
