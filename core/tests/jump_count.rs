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
        brush_plane_offsets: vec![0], brush_plane_count: vec![6],
        brush_plane_ids: (0..6).collect(), brush_shaders: vec![0], shader_flags: vec![0], planes,
        shader_contents: vec![0],
        brush_contents: vec![webrace_core::bsp::CONTENTS_SOLID],
        jumppads: vec![],
        teleporters: vec![],
    }
}

#[test]
fn single_jump_height() {
    let mut pmove = Pmove::new(flat_world(), 1.0/250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 60.0];
    ps.viewangles = [0.0, 0.0, 0.0];
    pmove.drop_to_ground(&mut ps);
    let start_z = ps.origin[2];
    let mut peak = start_z;
    let mut max_vz = 0.0f32;
    for t in 0..250 {
        let mut cmd = Cmd::default();
        if t == 0 { cmd.up = 127; cmd.buttons |= webrace_core::input::BUTTON_JUMP; }
        pmove.step(&mut ps, &cmd);
        if ps.velocity[2] > max_vz { max_vz = ps.velocity[2]; }
        if ps.origin[2] > peak { peak = ps.origin[2]; }
    }
    let expected_vz = 280.0 * webrace_core::GRAVITY_COMPENSATE;
    let expected_peak = expected_vz * expected_vz / (2.0 * webrace_core::GRAVITY);
    println!("rise={:.2} max_vz={:.1} expected_vz={:.1} expected_peak={:.2}", peak - start_z, max_vz, expected_vz, expected_peak);
    assert!(max_vz < expected_vz + 5.0, "single jump vz too high (double jump): {max_vz}");
    assert!((peak - start_z) < expected_peak + 8.0, "jump too high: {}", peak - start_z);
}
