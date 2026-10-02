use webrace_core::bsp::Plane;
use webrace_core::pmove::{PlayerState, Pmove};
use webrace_core::trace::World;

fn world_with_wall() -> World {
    let planes = vec![
        Plane { normal: [0.0, 0.0, 1.0], dist: 0.0 },
        Plane { normal: [0.0, 0.0, -1.0], dist: 1000.0 },
        Plane { normal: [1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [-1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
        Plane { normal: [1.0, 0.0, 0.0], dist: 260.0 },
        Plane { normal: [-1.0, 0.0, 0.0], dist: -200.0 },
        Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, 0.0, 1.0], dist: 1000.0 },
        Plane { normal: [0.0, 0.0, -1.0], dist: 1000.0 },
    ];
    World {
        brush_plane_offsets: vec![0, 6],
        brush_plane_count: vec![6, 6],
        brush_plane_ids: (0..12).collect(),
        brush_shaders: vec![0, 0],
        shader_flags: vec![0],
        shader_contents: vec![0],
        brush_contents: vec![webrace_core::bsp::CONTENTS_SOLID; 2],
        jumppads: vec![],
        teleporters: vec![],
        trigger_plane_ids: vec![],
        planes,
    }
}

#[test]
fn trace_at_wall() {
    use webrace_core::input::Cmd;
    let mut pmove = Pmove::new(world_with_wall(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [100.0, 0.0, 60.0];
    ps.viewangles = [0.0, 0.0, 0.0];
    pmove.drop_to_ground(&mut ps);

    // Step until we're at the wall (~x=184), then inspect one tick's traces.
    for _ in 0..100 {
        let mut cmd = Cmd::default();
        cmd.forward = 127;
        pmove.step(&mut ps, &cmd);
    }
    println!("at wall: pos={:?} vel={:?} on_ground={}", ps.origin, ps.velocity, ps.on_ground);

    // Manually trace the next movement.
    let start = ps.origin;
    let end = [start[0] + ps.velocity[0] * (1.0/250.0), start[1], start[2]];
    let tr = pmove.world.trace(start, webrace_core::trace::PLAYER_MINS, webrace_core::trace::PLAYER_MAXS, end);
    println!("movement trace +x: frac={:.4} normal={:?} start_solid={}", tr.fraction, tr.normal, tr.start_solid);

    // Trace straight down (grounded check).
    let tr2 = pmove.world.trace(start, webrace_core::trace::PLAYER_MINS, webrace_core::trace::PLAYER_MAXS, [start[0], start[1], start[2]-2.0]);
    println!("down trace: frac={:.4} normal={:?}", tr2.fraction, tr2.normal);
}
