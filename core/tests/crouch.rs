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
        movers: vec![],
        mover_plane_ids: vec![],
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

    // Press crouch: the box interpolates (does not snap) — after one tick it is
    // strictly between the stand and crouch heights.
    cmd.buttons |= webrace_core::input::BUTTON_CROUCH;
    pmove.step(&mut ps, &cmd);
    assert!(
        pmove.maxs[2] < 40.0 && pmove.maxs[2] > 16.0,
        "crouch should transition smoothly, got maxs.z={}",
        pmove.maxs[2]
    );
    assert!(ps.crouched);

    // After CROUCHTIME (100ms = 25 ticks) it is fully crouched.
    for _ in 0..30 {
        pmove.step(&mut ps, &cmd);
    }
    assert_eq!(pmove.maxs[2], 16.0, "crouched box should be 16 tall");
    assert_eq!(ps.viewheight, 12.0, "crouched viewheight should be 12");

    println!(
        "crouch: standing maxs.z=40, crouched maxs.z={}, viewheight={}",
        pmove.maxs[2], ps.viewheight
    );
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
        movers: vec![],
        mover_plane_ids: vec![],
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

    // Under the low ceiling, releasing crouch must NOT stand up (head-chomp):
    // the taller box would be blocked, so the player stays crouched.
    let cmd2 = Cmd::default();
    for _ in 0..30 {
        pmove.step(&mut ps, &cmd2); // no crouch button
    }
    println!("after release under ceiling: crouched={} maxs.z={:.1}", ps.crouched, pmove.maxs[2]);
    assert!(ps.crouched, "should stay crouched under a low ceiling");
    assert!(pmove.maxs[2] < 20.0, "should not stand up under the ceiling");
}
