use webrace_core::bsp::Plane;
use webrace_core::input::Cmd;
use webrace_core::pmove::{PlayerState, Pmove};
use webrace_core::trace::World;

/// Floor slab (full extent) + a wall brush at x in [0, 32] (solid), so the
/// player can wall-dash off its face at x=0 from the right (x>32).
fn wall_world() -> World {
    let planes = vec![
        // brush 0: floor slab, solid z in [-1000, -100] (floor top at z=-100)
        Plane { normal: [0.0, 0.0, 1.0], dist: -100.0 },
        Plane { normal: [0.0, 0.0, -1.0], dist: 1000.0 },
        Plane { normal: [1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [-1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
        // brush 1: wall, solid x in [0, 32], y in [-10000,10000], z in [-1000,1000]
        Plane { normal: [1.0, 0.0, 0.0], dist: 32.0 },   // x < 32 solid
        Plane { normal: [-1.0, 0.0, 0.0], dist: 0.0 },   // x > 0 solid
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
        planes,
    }
}

#[test]
fn wall_dash_pushes_away_and_up() {
    let mut pmove = Pmove::new(wall_world(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    // Player airborne next to the wall. Wall face at x=0 (solid x in [0,32]).
    // Player box spans x in [48-16, 48+16] = [32, 64], so 32 units from wall.
    ps.origin = [48.0, 0.0, 0.0];
    ps.viewangles = [0.0, std::f32::consts::PI, 0.0]; // facing -X (toward wall)
    ps.on_ground = false;

    for tick in 0..30 {
        let mut cmd = Cmd::default();
        cmd.forward = 127; // move -X toward the wall
        cmd.right = 0;
        if tick >= 3 {
            cmd.buttons |= webrace_core::input::BUTTON_SPECIAL;
        }
        pmove.step(&mut ps, &cmd);
        if ps.wjtime > 0 {
            break; // walljump fired
        }
    }

    println!("wall-dash: vel={:?} pos={:?} wjtime={}", ps.velocity, ps.origin, ps.wjtime);
    // Wall-jump should push away from the wall (+X) and upward.
    assert!(ps.wjtime > 0, "walljump did not fire");
    assert!(ps.velocity[0] > 0.0, "push away +X, got vx={}", ps.velocity[0]);
    assert!(ps.velocity[2] > 100.0, "upward speed, got vz={}", ps.velocity[2]);
}
