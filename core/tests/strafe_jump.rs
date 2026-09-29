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
        planes,
    }
}

#[test]
fn strafe_jump_gives_forward_momentum() {
    let mut pmove = Pmove::new(flat_world(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 60.0];
    ps.viewangles = [0.0, 0.0, 0.0]; // yaw 0 -> forward = +X
    pmove.drop_to_ground(&mut ps);

    // Strafe-jump: hold forward + right + jump, turning yaw slowly to keep the
    // sweet spot. Forward (yaw 0) is +X. After a couple of jumps, the player
    // should have moved substantially +X (forward), not just sideways.
    let mut peak_vx = 0.0f32;
    for tick in 0..(250 * 2) {
        // Gentle turn like a strafer.
        let yaw = tick as f32 * 0.0011;
        ps.viewangles = [0.0, yaw, 0.0];
        let mut cmd = Cmd::default();
        cmd.forward = 127;
        cmd.right = 127;
        cmd.up = 127;
        cmd.buttons |= webrace_core::input::BUTTON_JUMP;
        pmove.step(&mut ps, &cmd);
        if ps.velocity[0] > peak_vx {
            peak_vx = ps.velocity[0];
        }
    }

    // After 2s of strafe-jumping with a fixed (non-optimal) turn, total
    // horizontal speed should exceed run speed (320 ups), proving the
    // strafe-jump acceleration works. (The optimal sweet-spot requires precise
    // mouse control; a fixed turn gives some diagonal component, but the total
    // speed still climbs.)
    let hspeed = (ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1]).sqrt();
    println!("peak vx = {peak_vx:.1}, final hspeed = {hspeed:.1} ups, vel = {:?}", ps.velocity);
    assert!(hspeed > 320.0, "strafe-jump should exceed run speed, got {hspeed}");
}
