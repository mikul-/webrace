use webrace_core::bsp::Plane;
use webrace_core::pmove::{PlayerState, Pmove};
use webrace_core::trace::World;

fn flat_world() -> World {
    // A large solid slab: x,y in [-10000,10000], z in [-1000, 0]. The player
    // stands on the top surface at z=0 (empty above). Solid = negative side
    // of each plane (dot(n,p) - d < 0), same convention as box_trace.
    let planes = vec![
        Plane { normal: [0.0, 0.0, 1.0], dist: 0.0 },       // below z=0 solid (floor)
        Plane { normal: [0.0, 0.0, -1.0], dist: 1000.0 },   // above z=-1000 solid
        Plane { normal: [1.0, 0.0, 0.0], dist: 10000.0 },   // x < 10000 solid
        Plane { normal: [-1.0, 0.0, 0.0], dist: 10000.0 },  // x > -10000 solid
        Plane { normal: [0.0, 1.0, 0.0], dist: 10000.0 },   // y < 10000 solid
        Plane { normal: [0.0, -1.0, 0.0], dist: 10000.0 },  // y > -10000 solid
    ];
    World {
        brush_plane_offsets: vec![0],
        brush_plane_count: vec![6],
        brush_plane_ids: (0..6).collect(),
        planes,
    }
}

#[test]
fn jump_on_flat() {
    let mut pmove = Pmove::new(flat_world(), 1.0 / 250.0);
    let mut ps = PlayerState::default();
    ps.origin = [0.0, 0.0, 60.0];
    ps.viewangles = [0.0, 0.0, 0.0];
    pmove.drop_to_ground(&mut ps);
    println!("start: origin={:?} on_ground={}", ps.origin, ps.on_ground);

    // Tap jump on tick 0, then compare peak height.
    let mut peak = ps.origin[2];
    for tick in 0..120 {
        let mut cmd = webrace_core::input::Cmd::default();
        if tick == 0 {
            cmd.up = 127;
            cmd.buttons |= webrace_core::input::BUTTON_JUMP;
        }
        pmove.step(&mut ps, &cmd);
        if ps.origin[2] > peak {
            peak = ps.origin[2];
        }
        if tick % 20 == 0 {
            println!("tick {tick}: z={:.1} vz={:.1} on_ground={}", ps.origin[2], ps.velocity[2], ps.on_ground);
        }
    }
    println!("peak z = {peak:.1}, rise = {:.1}", peak - 24.0);
    assert!(peak > 40.0, "jump should rise, peak={peak}");
}
