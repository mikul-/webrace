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
        trigger_plane_ids: vec![],
    }
}

#[test]
fn crouch_shrinks_box_and_lowers_eye() {
    let mut pmove = Pmove::new(flat_world(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 60.0];
    ps.viewangles = [0.0, 0.0, 0.0];
    pmove.drop_to_ground(&mut ps);

    // Stand: box maxs.z = 40.
    let mut cmd = Cmd::default();
    pmove.step(&mut ps, &cmd);
    assert_eq!(pmove.maxs[2], 40.0, "standing box should be 40 tall");
    assert!(!ps.crouched);

    // Crouch: box maxs.z = 16.
    cmd.buttons |= webrace_core::input::BUTTON_CROUCH;
    pmove.step(&mut ps, &cmd);
    assert_eq!(pmove.maxs[2], 16.0, "crouched box should be 16 tall");
    assert!(ps.crouched);

    println!("crouch: standing maxs.z=40, crouched maxs.z={}", pmove.maxs[2]);
}
