use webrace_core::bsp::Plane;
use webrace_core::input::Cmd;
use webrace_core::pmove::{PlayerState, Pmove};
use webrace_core::trace::World;

/// A floor slab (full extent), plus a separate wall brush at x>200.
fn world_with_wall() -> World {
    // Brush 0: floor slab, solid z in [-1000, 0], huge x/y extent.
    // Brush 1: wall, solid x in [200, 260], y in [-10000, 10000], z in [-1000, 1000].
    let planes = vec![
        // brush 0 (floor): 6 planes
        Plane { normal: [0.0, 0.0, 1.0], dist: 0.0 },
        Plane { normal: [0.0, 0.0, -1.0], dist: 1000.0 },
        Plane { normal: [1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [-1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
        // brush 1 (wall x in [200,260]): 6 planes
        Plane { normal: [1.0, 0.0, 0.0], dist: 260.0 },   // x < 260 solid
        Plane { normal: [-1.0, 0.0, 0.0], dist: -200.0 }, // x > 200 solid
        Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, 0.0, 1.0], dist: 1000.0 },  // z < 1000 solid
        Plane { normal: [0.0, 0.0, -1.0], dist: 1000.0 }, // z > -1000 solid
    ];
    World {
        brush_plane_offsets: vec![0, 6],
        brush_plane_count: vec![6, 6],
        brush_plane_ids: (0..12).collect(),
        planes,
    }
}

#[test]
fn sliding_along_wall_does_not_stick() {
    let mut pmove = Pmove::new(world_with_wall(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [100.0, 0.0, 60.0];
    ps.viewangles = [0.0, 0.0, 0.0]; // forward +X
    pmove.drop_to_ground(&mut ps);
    println!("start: {:?} on_ground={}", ps.origin, ps.on_ground);

    // Move forward+right diagonally toward the wall at x=200. The player
    // should be blocked in x (~200) but continue sliding in -y.
    for tick in 0..500 {
        let mut cmd = Cmd::default();
        cmd.forward = 127; // +X toward wall
        cmd.right = 127;   // strafe (right = -Y), so we slide along the wall
        pmove.step(&mut ps, &cmd);
    }
    println!("final pos = {:?}, vel = {:?}, on_ground={}", ps.origin, ps.velocity, ps.on_ground);
    // Player should NOT have gone through the wall (x must stay < 240).
    assert!(ps.origin[0] < 240.0, "player went through wall: x={}", ps.origin[0]);
    // Player should have slid sideways along the wall (y changed meaningfully).
    assert!(ps.origin[1].abs() > 5.0, "player stuck, didn't slide: y={}", ps.origin[1]);
}
