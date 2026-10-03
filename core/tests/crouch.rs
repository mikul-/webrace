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

/// A crouched player must fit through a low tunnel (e.g. 44 units) but a
/// standing one must not. Regression for the step-up oscillation at the tunnel
/// mouth (the raised step-up box embedded in the ceiling).
#[test]
fn crouch_fits_under_low_ceiling() {
    // Floor z<0, then a ceiling slab leaving a `gap`-unit tunnel for x>200.
    let make = |gap: f32| {
        let mut planes = vec![
            Plane { normal: [0.0, 0.0, 1.0], dist: 0.0 },
            Plane { normal: [0.0, 0.0, -1.0], dist: 1000.0 },
            Plane { normal: [1.0, 0.0, 0.0], dist: 10000.0 },
            Plane { normal: [-1.0, 0.0, 0.0], dist: 10000.0 },
            Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
            Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
        ];
        planes.extend_from_slice(&[
            Plane { normal: [0.0, 0.0, 1.0], dist: gap + 100.0 },
            Plane { normal: [0.0, 0.0, -1.0], dist: -gap },
            Plane { normal: [1.0, 0.0, 0.0], dist: 10000.0 },
            Plane { normal: [-1.0, 0.0, 0.0], dist: -200.0 },
            Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
            Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
        ]);
        World {
            brush_plane_offsets: vec![0, 6],
            brush_plane_count: vec![6, 6],
            brush_plane_ids: (0..12).collect(),
            brush_shaders: vec![0, 0],
            shader_flags: vec![0],
            shader_contents: vec![0],
            planes,
            brush_contents: vec![webrace_core::bsp::CONTENTS_SOLID; 2],
            jumppads: vec![],
            teleporters: vec![],
            trigger_plane_ids: vec![],
        }
    };

    // 44-unit tunnel: crouch passes, stand is blocked.
    let mut pmove = Pmove::new(make(44.0), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 60.0];
    ps.viewangles = [0.0, 0.0, 0.0];
    pmove.drop_to_ground(&mut ps);
    let mut cmd = Cmd::default();
    cmd.forward = 127;
    cmd.buttons |= webrace_core::input::BUTTON_CROUCH;
    for _ in 0..1200 {
        pmove.step(&mut ps, &cmd);
    }
    println!("crouch under 44: final x={:.0}", ps.origin[0]);
    assert!(ps.origin[0] > 250.0, "crouched player should pass a 44-unit tunnel, x={}", ps.origin[0]);
}
