use webrace_core::bsp::Bsp;
use webrace_core::sim::Session;

#[test]
fn position_save_saves_view_direction() {
    let data = std::fs::read("../maps/rek-dire-wjfix-allslick.bsp").unwrap();
    let bsp = Bsp::parse("rek", &data).expect("parse");
    let mut session = Session::new(&bsp, 0).expect("session");

    // Turn the view (yaw + pitch) via mouse, then save.
    session.add_mouse(-500.0, 300.0); // turn right, look up
    session.step();
    let yaw_before = session.yaw();
    let pitch_before = session.pitch();
    println!("before save: yaw={yaw_before:.2} pitch={pitch_before:.2}");

    let saved = session.position_save();
    println!("saved={saved}");

    // Look away, then reset and confirm view returns.
    session.add_mouse(2000.0, -1000.0);
    session.step();
    session.reset();
    let yaw_after = session.yaw();
    let pitch_after = session.pitch();
    println!("after reset: yaw={yaw_after:.2} pitch={pitch_after:.2}");

    // Yaw/pitch should round-trip close to the saved values (modulo 2pi).
    let yaw_diff = (yaw_after - yaw_before + std::f32::consts::PI).rem_euclid(2.0 * std::f32::consts::PI) - std::f32::consts::PI;
    let pitch_diff = (pitch_after - pitch_before).abs();
    println!("yaw_diff={yaw_diff:.3} pitch_diff={pitch_diff:.3}");
    assert!(yaw_diff.abs() < 0.01, "yaw not restored: {yaw_after} vs {yaw_before}");
    assert!(pitch_diff < 0.01, "pitch not restored: {pitch_after} vs {pitch_before}");
}
