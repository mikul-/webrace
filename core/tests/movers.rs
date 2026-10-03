//! Moving brush entity tests: parsing from a real map, `func_bobbing` carrying
//! the player, `func_plat` raising the player, and not falling through a
//! moving platform.

use std::path::Path;

use webrace_core::bsp::{Bsp, MoverDef, MoverKind, Plane};
use webrace_core::input::Cmd;
use webrace_core::pmove::{PlayerState, Pmove};
use webrace_core::trace::{angles_to_axis, axis_transform, Mover, PlayerBox, World};

/// A floor slab far below plus room for a mover. Top of the floor at z=-200.
fn base_world() -> World {
    let planes = vec![
        Plane { normal: [0.0, 0.0, 1.0], dist: -200.0 },
        Plane { normal: [0.0, 0.0, -1.0], dist: 10000.0 },
        Plane { normal: [1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [-1.0, 0.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },
        Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },
    ];
    World {
        brush_plane_offsets: vec![0],
        brush_plane_count: vec![6],
        brush_plane_ids: (0..6).collect(),
        brush_shaders: vec![0],
        shader_flags: vec![0],
        shader_contents: vec![0],
        brush_contents: vec![webrace_core::bsp::CONTENTS_SOLID],
        jumppads: vec![],
        teleporters: vec![],
        trigger_plane_ids: vec![],
        movers: vec![],
        mover_plane_ids: vec![],
        planes,
    }
}

/// Append an axis-aligned solid box to the mover plane store, returning the
/// `(plane_off, plane_count)` run.
fn add_mover_box(world: &mut World, mins: [f32; 3], maxs: [f32; 3]) -> (u32, u32) {
    let planes = [
        Plane { normal: [1.0, 0.0, 0.0], dist: maxs[0] },
        Plane { normal: [-1.0, 0.0, 0.0], dist: -mins[0] },
        Plane { normal: [0.0, 1.0, 0.0], dist: maxs[1] },
        Plane { normal: [0.0, -1.0, 0.0], dist: -mins[1] },
        Plane { normal: [0.0, 0.0, 1.0], dist: maxs[2] },
        Plane { normal: [0.0, 0.0, -1.0], dist: -mins[2] },
    ];
    let off = world.mover_plane_ids.len() as u32;
    for p in planes {
        world.planes.push(p);
        world.mover_plane_ids.push(world.planes.len() as u32 - 1);
    }
    (off, 6)
}

fn add_mover(world: &mut World, kind: MoverKind, height: f32, speed: f32) {
    let mins = [-128.0, -128.0, -16.0];
    let maxs = [128.0, 128.0, 0.0];
    let (plane_off, plane_count) = add_mover_box(world, mins, maxs);
    let def = MoverDef {
        kind,
        model: 1,
        origin: [0.0, 0.0, 0.0],
        angles: [0.0, 0.0, 0.0],
        angle: 0.0,
        height,
        speed,
        phase: 0.0,
        wait: 0.0,
        distance: 0.0,
        spawnflags: 0,
        path: vec![],
        path_wait: vec![],
        plane_off,
        plane_count,
        mins,
        maxs,
    };
    world.movers.push(Mover::from_def(&def));
}

#[test]
fn parses_bobbing_movers_from_real_map() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../maps/BardoK-Strafe1.bsp");
    let Ok(data) = std::fs::read(&p) else {
        eprintln!("skipping: {:?} not present", p);
        return;
    };
    let bsp = Bsp::parse("BardoK-Strafe1", &data).expect("parse");
    assert_eq!(bsp.movers.len(), 6, "expected 6 func_bobbing movers");
    for m in &bsp.movers {
        assert_eq!(m.kind, MoverKind::Bobbing);
        assert!(m.plane_count > 0);
        assert!((m.height - 4.0).abs() < 1e-3, "height key = 4");
    }
    // Mover render geometry is separated out of the static world buffers.
    assert!(!bsp.mover_positions.is_empty());
    assert!(!bsp.mover_indices.is_empty());
    assert_eq!(bsp.mover_indices.len() % 3, 0);
    assert!(!bsp.mover_chunks.is_empty());
}

#[test]
fn bobbing_platform_carries_the_player() {
    let mut world = base_world();
    add_mover(&mut world, MoverKind::Bobbing, 32.0, 1.0);
    let mut pmove = Pmove::new(world, 1.0 / 250.0);
    let mut ps = PlayerState::default();
    // Stand on the platform's top (z=0): feet at origin.z-24 = 0.
    ps.origin = [0.0, 0.0, 24.0];
    ps.on_ground = true;
    ps.viewangles = [0.0, 0.0, 0.0];
    let cmd = Cmd::default();

    let start_z = ps.origin[2];
    // Quarter of the bob period (speed=1 → 1 s): platform is near its apex.
    for _ in 0..63 {
        pmove.step(&mut ps, &cmd);
    }
    let risen = ps.origin[2] - start_z;
    println!("bobbing carry: start={start_z} end={} risen={risen}", ps.origin[2]);
    assert!(
        (risen - 32.0).abs() < 3.0,
        "player should be carried up ~32 units with the bob, got {risen}"
    );
    assert!(ps.on_ground, "player should still be grounded on the platform");
    // Feet must not sink below the platform top.
    assert!(ps.origin[2] - 24.0 > -1.0);
}

#[test]
fn bob_platform_never_lets_the_player_fall_through() {
    let mut world = base_world();
    add_mover(&mut world, MoverKind::Bobbing, 48.0, 0.7);
    let mut pmove = Pmove::new(world, 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 24.0];
    ps.on_ground = true;
    let cmd = Cmd::default();

    for _ in 0..500 {
        pmove.step(&mut ps, &cmd);
        let top = pmove.world.movers[0].origin[2];
        let feet = ps.origin[2] - 24.0;
        assert!(
            feet > top - 2.0,
            "player fell through the moving platform: feet={feet} top={top}"
        );
    }
}

#[test]
fn plat_raises_the_player() {
    let mut world = base_world();
    add_mover(&mut world, MoverKind::Plat, 64.0, 128.0);
    let mut pmove = Pmove::new(world, 1.0 / 250.0);
    let mut ps = PlayerState::default();
    // Plat spawns lowered: top at z=-64, player stands on it.
    ps.origin = [0.0, 0.0, -40.0];
    ps.on_ground = true;
    let cmd = Cmd::default();

    let start_z = ps.origin[2];
    for _ in 0..400 {
        pmove.step(&mut ps, &cmd);
    }
    let raised = ps.origin[2] - start_z;
    println!("plat: start={start_z} end={} raised={raised}", ps.origin[2]);
    assert!(raised > 50.0, "plat should raise the player, got {raised}");
    assert!(ps.on_ground, "player should ride the plat");
    // Plat should have reached the top (origin.z == 0) and the player with it.
    assert!(ps.origin[2] > 15.0);
}

#[test]
fn sliding_door_opens_when_near_and_closes_after_waiting() {
    let mut world = base_world();
    let mins = [-16.0, -64.0, -64.0];
    let maxs = [16.0, 64.0, 0.0];
    let (plane_off, plane_count) = add_mover_box(&mut world, mins, maxs);
    let def = MoverDef {
        kind: MoverKind::Door,
        model: 1,
        origin: [0.0; 3],
        angles: [0.0; 3],
        angle: 0.0, // +X slide
        height: 0.0,
        speed: 200.0,
        phase: 0.0,
        wait: 0.05,
        distance: 0.0,
        spawnflags: 0,
        path: vec![],
        path_wait: vec![],
        plane_off,
        plane_count,
        mins,
        maxs,
    };
    world.movers.push(Mover::from_def(&def));

    let near = PlayerBox {
        origin: [40.0, 0.0, 24.0],
        mins: [-16.0, -16.0, -24.0],
        maxs: [16.0, 16.0, 40.0],
    };
    for _ in 0..120 {
        world.advance_movers(1.0 / 250.0, Some(&near));
    }
    assert!(
        world.movers[0].origin[0] > 20.0,
        "door should open, got x={}",
        world.movers[0].origin[0]
    );

    // Leave: the door waits then closes.
    let far = PlayerBox {
        origin: [2000.0, 0.0, 24.0],
        mins: [-16.0, -16.0, -24.0],
        maxs: [16.0, 16.0, 40.0],
    };
    for _ in 0..600 {
        world.advance_movers(1.0 / 250.0, Some(&far));
    }
    assert!(
        world.movers[0].origin[0] < 1.0,
        "door should close, got x={}",
        world.movers[0].origin[0]
    );
}

#[test]
fn rotating_door_turns_about_yaw() {
    let mut world = base_world();
    let mins = [-64.0, -16.0, -64.0];
    let maxs = [64.0, 16.0, 0.0];
    let (plane_off, plane_count) = add_mover_box(&mut world, mins, maxs);
    let def = MoverDef {
        kind: MoverKind::DoorRotating,
        model: 1,
        origin: [0.0; 3],
        angles: [0.0; 3],
        angle: 0.0,
        height: 0.0,
        speed: 180.0,
        phase: 0.0,
        wait: 5.0,
        distance: 90.0,
        spawnflags: 0, // default Z/yaw axis
        path: vec![],
        path_wait: vec![],
        plane_off,
        plane_count,
        mins,
        maxs,
    };
    world.movers.push(Mover::from_def(&def));
    let near = PlayerBox {
        origin: [40.0, 0.0, 24.0],
        mins: [-16.0, -16.0, -24.0],
        maxs: [16.0, 16.0, 40.0],
    };
    for _ in 0..200 {
        world.advance_movers(1.0 / 250.0, Some(&near));
    }
    let yaw = world.movers[0].angles[1].to_degrees();
    assert!(
        (yaw - 90.0).abs() < 2.0,
        "rotating door should turn ~90° about yaw, got {yaw}"
    );
}

#[test]
fn real_map_bobbing_boxes_have_collision() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../maps/BardoK-Strafe1.bsp");
    let Ok(data) = std::fs::read(&p) else {
        eprintln!("skipping: {p:?} not present");
        return;
    };
    let bsp = Bsp::parse("BardoK-Strafe1", &data).expect("parse");
    let world = World::from_bsp(&bsp);
    assert_eq!(world.movers.len(), 6);
    // A zero-angle mover must not be mirrored: its top plane is at the authored z.
    let tr = world.trace(
        [1128.0, -244.0, 48.0],
        webrace_core::trace::PLAYER_MINS,
        webrace_core::trace::PLAYER_MAXS,
        [1128.0, -244.0, 47.75],
    );
    assert_eq!(tr.mover, 0, "down-trace should hit bobbing mover 0");
    assert!(tr.normal[2] > 0.9, "hit normal should be the box top");

    // And the player settles on top of it (z=24 + player half-height 24).
    let mut pmove = Pmove::new(world, 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [1128.0, -244.0, 200.0];
    ps.on_ground = false;
    pmove.drop_to_ground(&mut ps);
    assert!(
        ps.origin[2] > 40.0,
        "should land on the bobbing box (top z=24), got {:?}",
        ps.origin
    );
}

#[test]
fn zero_angle_rotation_is_identity() {
    let a = angles_to_axis([0.0, 0.0, 0.0]);
    for r in 0..3 {
        for c in 0..3 {
            let want = if r == c { 1.0 } else { 0.0 };
            assert!(
                (a[r][c] - want).abs() < 1e-6,
                "identity basis mismatch at [{r}][{c}]: {:?}",
                a
            );
        }
    }
    let v = [3.0, -7.0, 2.5];
    let t = axis_transform(&a, v);
    assert!((t[0] - v[0]).abs() < 1e-6);
    assert!((t[1] - v[1]).abs() < 1e-6);
    assert!((t[2] - v[2]).abs() < 1e-6);
    // A +90° yaw rotates +X toward +Y (matches the camera's yaw convention).
    let y = angles_to_axis([0.0, std::f32::consts::FRAC_PI_2, 0.0]);
    let x = axis_transform(&y, [1.0, 0.0, 0.0]);
    assert!(x[0].abs() < 1e-6 && (x[1] - 1.0).abs() < 1e-6, "yaw 90 gave {x:?}");
}
