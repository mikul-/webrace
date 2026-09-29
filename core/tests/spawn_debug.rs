//! Debug: load a real map, place the player at spawn, and check that
//! drop_to_ground finds a floor and the player ends up stable.

use webrace_core::bsp::Bsp;

fn load(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../maps").join(name);
    std::fs::read(&p).unwrap()
}

#[test]
fn spawn_and_drop_on_real_map() {
    let data = load("hoppin.bsp");
    let bsp = Bsp::parse("hoppin", &data).expect("parse");
    println!("spawns: {:?}", bsp.spawns);
    println!("brushes: {}", bsp.brush_plane_offsets.len());
    println!("planes: {}", bsp.planes.len());

    if let Some(sp) = bsp.spawns.first() {
        println!("spawn[0] render origin = {:?}, yaw={}", sp.origin, sp.yaw);
    }

    // Create a session and see the resulting position.
    let session = webrace_core::sim::Session::new(&bsp, 0).expect("session");
    println!(
        "after drop: origin={:?} on_ground={} speed={:.1}",
        session.origin(),
        session.on_ground(),
        session.speed()
    );
}

#[test]
fn holding_jump_stays_bounded() {
    let data = load("hoppin.bsp");
    let bsp = Bsp::parse("hoppin", &data).expect("parse");
    let mut session = webrace_core::sim::Session::new(&bsp, 0).expect("session");
    let spawn_z = session.origin()[2];

    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    // Hold jump + forward for 2 seconds (500 ticks).
    for _ in 0..500 {
        session.set_keys(false, false, false, false, true, false, false, false);
        session.step();
        let o = session.origin();
        min_z = min_z.min(o[2]);
        max_z = max_z.max(o[2]);
    }
    println!("holding jump 2s: z range [{min_z:.1}, {max_z:.1}] spawn_z={spawn_z:.1} on_ground={}", session.on_ground());
    // The player should never fly upward unboundedly nor fall through:
    // z stays within a reasonable band of the spawn height.
    assert!(max_z < spawn_z + 400.0, "player flew up too high: {max_z}");
    assert!(min_z > spawn_z - 300.0, "player fell through floor: {min_z}");
}

