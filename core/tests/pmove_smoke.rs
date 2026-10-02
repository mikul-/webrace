//! Sanity test: strafe-jumping should steadily gain horizontal speed on a
//! flat open map. Uses a synthetic flat world (a single floor plane) rather
//! than a real BSP, so we can drive the pmove headless.

use webrace_core::bsp::Plane;
use webrace_core::pmove::{PlayerState, Pmove};
use webrace_core::trace::World;

/// Build a minimal flat world: one big floor brush at z=0.
fn flat_world() -> World {
    // A box brush: floor plane (normal +Z, dist 0) + 4 walls + ceiling.
    // We need a convex brush; use a big floor slab.
    let planes = vec![
        Plane { normal: [0.0, 0.0, 1.0], dist: 0.0 },    // floor: empty above z=0
        Plane { normal: [0.0, 0.0, -1.0], dist: 10000.0 }, // ceiling above
        Plane { normal: [1.0, 0.0, 0.0], dist: -10000.0 },
        Plane { normal: [-1.0, 0.0, 0.0], dist: -10000.0 },
        Plane { normal: [0.0, 1.0, 0.0], dist: -10000.0 },
        Plane { normal: [0.0, -1.0, 0.0], dist: -10000.0 },
    ];
    // Brush has all 6 planes.
    let brush_plane_ids: Vec<u32> = (0..6).collect();
    World {
        brush_plane_offsets: vec![0],
        brush_plane_count: vec![6],
        brush_plane_ids,
        brush_shaders: vec![0],
        shader_flags: vec![0],
        shader_contents: vec![0],
        brush_contents: vec![webrace_core::bsp::CONTENTS_SOLID],
        jumppads: vec![],
        teleporters: vec![],
        planes,
    }
}

#[test]
fn strafe_jump_gains_speed() {
    let world = flat_world();
    let frametime = 1.0 / 250.0;
    let mut pmove = Pmove::new(world, frametime);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 40.0];
    pmove.drop_to_ground(&mut ps);
    assert!(ps.on_ground, "should start grounded");

    // Alternate: strafe right + forward, holding jump — the classic
    // strafe-jump to gain speed. We'll simulate mouse angles turning.
    let mut peak_hspeed = 0.0f32;

    for tick in 0..(250 * 6) {
        // Simulate turning gradually (like a good strafer maintains angle).
        // We drive the view yaw directly and hold forward+right+jump.
        let t = tick as f32;
        let yaw = t * 0.0009; // smooth turn
        ps.viewangles = [0.0, yaw, 0.0];

        // Build a Cmd: forward + strafe + jump held.
        let mut cmd = webrace_core::input::Cmd::default();
        cmd.forward = 127;
        cmd.right = 127;
        cmd.up = 127;
        cmd.buttons |= webrace_core::input::BUTTON_JUMP;

        pmove.step(&mut ps, &cmd);

        let h = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1]).sqrt();
        if h > peak_hspeed {
            peak_hspeed = h;
        }
    }

    println!("peak horizontal speed after strafe-jump: {peak_hspeed:.1} ups");
    assert!(
        peak_hspeed > 500.0,
        "expected to exceed walk speed ({} ups) via strafe-jumping, got {peak_hspeed}",
        webrace_core::WALK_SPEED
    );
    assert!(peak_hspeed < 2000.0, "speed unrealistically high: {peak_hspeed}");
}
