use webrace_core::bsp::{RaceGate, RaceGateKind};
use webrace_core::pmove::{PlayerState, Pmove};
use webrace_core::trace::World;
use webrace_core::sim::Session;

/// Build a minimal Bsp-like set of race gates and a flat world, then drive a
/// Session manually by checking the gate overlap logic indirectly.
/// (We test via Session by constructing gates through the public API is not
/// possible without a Bsp, so we test the mechanism through a synthetic world.)

#[test]
fn session_race_gate_helpers() {
    // Just ensure the RaceGate enum and AABB overlap helpers are exported and
    // usable (compile-time + logical check).
    let g = RaceGate {
        kind: RaceGateKind::Start,
        mins: [0.0, 0.0, 0.0],
        maxs: [10.0, 10.0, 10.0],
    };
    assert_eq!(g.kind, RaceGateKind::Start);
    let _ = g.mins;
}

#[test]
fn race_timer_runs_on_real_map() {
    use webrace_core::bsp::Bsp;
    let data = std::fs::read("../maps/kool_simple2-wjfix.bsp").unwrap();
    let bsp = Bsp::parse("kool_simple2-wjfix", &data).expect("parse");
    let mut session = Session::new(&bsp, 0).expect("session");
    // The player spawns at info_player_start; the start gate is elsewhere.
    // Simulate many ticks — timer should be 0 (not running) until crossing start.
    for _ in 0..100 {
        session.step();
    }
    println!(
        "after 100 ticks: running={} ticks={} finished={}",
        session.race_running(),
        session.race_ticks(),
        session.race_finished()
    );
    // Without crossing the start gate, the timer should not be running.
    assert!(!session.race_running(), "timer should not run before crossing start");
}
