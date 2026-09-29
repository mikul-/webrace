use webrace_core::input::{Angles, MouseConfig};

#[test]
fn sensitivity_is_quakelike() {
    let cfg = MouseConfig::default(); // 1.72 sens, 0.022 yaw/pitch
    let mut a = Angles::default();

    // A full mouse swipe of 1000 counts should turn a meaningful amount:
    // 1000 * 1.72 * 0.022° = 37.84°.
    a.add_mouse(1000.0, 0.0, &cfg);
    let expected_units = 1000.0 * cfg.sensitivity * cfg.m_yaw * (webrace_core::input::ANGLE_2_PI / 360.0);
    println!("raw delta (pre-wrap) = {:.1} angle units", expected_units);

    // yaw_frac is negative (yaw decreases with +dx), wound around 65536.
    let wrapped = a.yaw_frac;
    let magnitude = (65536.0 - wrapped).min(wrapped).abs();
    println!("wrapped yaw_frac = {wrapped}, magnitude = {magnitude:.1}");

    assert!(
        (magnitude - expected_units).abs() < 60.0,
        "magnitude {magnitude} should be ~{expected_units}"
    );
}
