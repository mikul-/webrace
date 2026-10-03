use webrace_core::bsp::Plane;
use webrace_core::input::Cmd;
use webrace_core::pmove::{PlayerState, Pmove};
use webrace_core::trace::World;
fn slick_world() -> World {
    let planes = vec![
        Plane { normal: [0.0,0.0,1.0], dist: 0.0 },
        Plane { normal: [0.0,0.0,-1.0], dist: 1000.0 },
        Plane { normal: [1.0,0.0,0.0], dist: 10000.0 },
        Plane { normal: [-1.0,0.0,0.0], dist: 10000.0 },
        Plane { normal: [0.0,1.0,0.0], dist: 10000.0 },
        Plane { normal: [0.0,-1.0,0.0], dist: 10000.0 },
    ];
    World {
        movers: vec![],
        mover_plane_ids: vec![], brush_plane_offsets: vec![0], brush_plane_count: vec![6], brush_plane_ids: (0..6).collect(),
        brush_shaders: vec![0], shader_flags: vec![webrace_core::pmove::SURF_SLICK], shader_contents: vec![0], planes,
        brush_contents: vec![webrace_core::bsp::CONTENTS_SOLID], jumppads: vec![], teleporters: vec![], trigger_plane_ids: vec![] }
}
#[test]
fn slick_move_forward() {
    let mut pmove = Pmove::new(slick_world(), 1.0/250.0);
    let mut ps = PlayerState::default();
    ps.origin=[0.0,0.0,60.0]; ps.viewangles=[0.0,0.0,0.0];
    pmove.drop_to_ground(&mut ps);
    for _ in 0..30 {
        let mut c = Cmd::default(); c.forward=127;
        pmove.step(&mut ps, &c);
    }
    let h = (ps.velocity[0]*ps.velocity[0]+ps.velocity[1]*ps.velocity[1]).sqrt();
    println!("slick move: origin={:?} vel={:?} hspd={h:.1}", ps.origin, ps.velocity);
    assert!(h > 100.0, "player failed to accelerate on slick, hspd={h}");
}
