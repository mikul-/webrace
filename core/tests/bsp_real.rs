//! Integration test: validate the BSP parser against real Warfork maps.
//!
//! Run with: `cargo test --test bsp_real -- --nocapture`
//!
//! Expects `.bsp` files under `../maps/` (see the `tools/` sync script).

use std::path::Path;

use webrace_core::bsp::Bsp;

fn load(name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../maps").join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("read {p:?}: {e}"))
}

#[test]
fn parse_kool_simple2() {
    let data = load("kool_simple2-wjfix.bsp");
    let bsp = Bsp::parse("kool_simple2-wjfix", &data).expect("parse");
    assert!(bsp.triangle_count() > 0, "no triangles");
    assert!(bsp.brush_plane_offsets.len() > 0, "no collision brushes");
    assert!(bsp.positions.len() > 0);
    assert!(bsp.indices.len() % 3 == 0);
    println!(
        "kool_simple2-wjfix: {} tris, {} verts, {} brushes, {} shaders",
        bsp.triangle_count(),
        bsp.positions.len() / 14,
        bsp.brush_plane_offsets.len(),
        bsp.shaders.len()
    );
}

#[test]
fn parse_hoppin() {
    let data = load("hoppin.bsp");
    let bsp = Bsp::parse("hoppin", &data).expect("parse");
    assert!(bsp.triangle_count() > 0);
    assert!(bsp.brush_plane_offsets.len() > 0);
    println!(
        "hoppin: {} tris, {} verts, {} brushes",
        bsp.triangle_count(),
        bsp.positions.len() / 14,
        bsp.brush_plane_offsets.len()
    );
}

#[test]
fn parse_ktotam1() {
    let data = load("KTOTAM-1.bsp");
    let bsp = Bsp::parse("KTOTAM-1", &data).expect("parse");
    assert!(bsp.triangle_count() > 0);
    assert!(bsp.brush_plane_offsets.len() > 0);
    println!(
        "KTOTAM-1: {} tris, {} verts, {} brushes",
        bsp.triangle_count(),
        bsp.positions.len() / 14,
        bsp.brush_plane_offsets.len()
    );
}

#[test]
fn parse_rek_dire() {
    let data = load("rek-dire-wjfix-allslick.bsp");
    let bsp = Bsp::parse("rek-dire-wjfix-allslick", &data).expect("parse");
    assert!(bsp.triangle_count() > 0);
    assert!(bsp.brush_plane_offsets.len() > 0);
    println!(
        "rek-dire-wjfix-allslick: {} tris, {} verts, {} brushes",
        bsp.triangle_count(),
        bsp.positions.len() / 14,
        bsp.brush_plane_offsets.len()
    );
}
