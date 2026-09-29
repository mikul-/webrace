use webrace_core::bsp::{Bsp, RaceGateKind};

fn load(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../maps").join(name);
    std::fs::read(&p).unwrap()
}

#[test]
fn race_gates_parsed() {
    let data = load("kool_simple2-wjfix.bsp");
    let bsp = Bsp::parse("kool_simple2-wjfix", &data).expect("parse");
    let gates = &bsp.race_gates;
    println!("race gates: {}", gates.len());
    for g in gates {
        let kind = match g.kind {
            RaceGateKind::Start => "start",
            RaceGateKind::Checkpoint => "checkpoint",
            RaceGateKind::Finish => "finish",
        };
        println!(
            "  {kind:10} mins=({:.0},{:.0},{:.0}) maxs=({:.0},{:.0},{:.0})",
            g.mins[0], g.mins[1], g.mins[2], g.maxs[0], g.maxs[1], g.maxs[2]
        );
    }
    // kool_simple2 has 1 start, 1 finish, 6 checkpoints.
    assert!(gates.iter().any(|g| g.kind == RaceGateKind::Start));
    assert!(gates.iter().any(|g| g.kind == RaceGateKind::Finish));
    assert!(gates.iter().filter(|g| g.kind == RaceGateKind::Checkpoint).count() >= 1);
    // Start should be first.
    assert_eq!(gates[0].kind, RaceGateKind::Start);
}

#[test]
fn hoppin_start_finish() {
    let data = load("hoppin.bsp");
    let bsp = Bsp::parse("hoppin", &data).expect("parse");
    println!("hoppin gates:");
    for g in &bsp.race_gates {
        println!(
            "  {:?} mins=({:.0},{:.0},{:.0}) maxs=({:.0},{:.0},{:.0})",
            g.kind, g.mins[0], g.mins[1], g.mins[2], g.maxs[0], g.maxs[1], g.maxs[2]
        );
    }
}
