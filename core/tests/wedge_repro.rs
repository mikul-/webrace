use webrace_core::bsp::Bsp;
use webrace_core::sim::Session;

/// Regression: the player must not get permanently wedged in the beveled
/// corner on rek-dire at ~(-14876, 2333.6, -4428). This spot used to trap the
/// player (zero motion in all directions) because the start_solid recovery
/// pushed "up" (the deepest-normal) instead of the horizontal direction that
/// actually clears the box.
#[test]
fn does_not_wedge_at_bevel_corner() {
    let data = std::fs::read("../maps/rek-dire-wjfix-allslick.bsp").unwrap();
    let bsp = Bsp::parse("rek", &data).expect("parse");
    let mut session = Session::new(&bsp, 0).expect("session");

    session.teleport_raw(-14876.0, 2333.6, -4428.0, 0.0, 0.0);
    let start = session.origin();

    // Hold forward (+x) for 60 ticks; the player must actually move.
    for _ in 0..60 {
        session.set_keys(true, false, false, false, false, false, false, false);
        session.step();
    }
    let end = session.origin();
    let moved = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt();
    println!("wedge repro: moved {moved:.1} units, {start:?} -> {end:?}");
    assert!(moved > 10.0, "player wedged at the bevel corner (moved {moved})");
}
