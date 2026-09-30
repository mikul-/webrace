//! webrace core — Warfork-style movement + QFusion BSP in Rust → WASM.

pub mod bsp;
pub mod bindings;
pub mod input;
pub mod pmove;
pub mod session;
pub mod sim;
pub mod trace;

use wasm_bindgen::prelude::*;

/// Maximum tick rate the simulation runs at (250 Hz).
pub const TICK_RATE: u32 = 250;

/// Warfork movement constants (verified against `gs_pmove.cpp`).
pub const GRAVITY: f32 = 850.0;
pub const BASEGRAVITY: f32 = 800.0;
pub const GRAVITY_COMPENSATE: f32 = GRAVITY / BASEGRAVITY;

pub const PM_ACCELERATE: f32 = 12.0;
pub const PM_AIRACCELERATE: f32 = 1.0;
pub const PM_AIRDECELERATE: f32 = 2.0;
pub const PM_STRANGE_BUNNY_ACCEL: f32 = 70.0;
pub const PM_AIRCONTROL: f32 = 150.0;
pub const PM_FRICTION: f32 = 8.0;
pub const PM_WISHSPEED: f32 = 30.0;

pub const WALK_SPEED: f32 = 160.0;
/// Crouch move speed (Warfork: crouch shares the 160 walk speed).
pub const CROUCH_SPEED: f32 = 160.0;

/// Warfork DEFAULT_JUMPSPEED (280; gs_public.h) and dash/walljump speeds.
pub const JUMP_SPEED: f32 = 280.0;
pub const DASH_SPEED: f32 = 450.0;

pub const PM_DASH_UPSPEED: f32 = 174.0 * GRAVITY_COMPENSATE;
pub const PM_WJ_UPSPEED: f32 = 330.0 * GRAVITY_COMPENSATE;
pub const PM_WJ_BOUNCE: f32 = 0.3;
pub const PM_OVERBOUNCE: f32 = 1.01;

pub const DASHJUMP_TIMEDELAY: u32 = 1000;
pub const WALLJUMP_TIMEDELAY: u32 = 1300;

/// Built without a `#[wasm_bindgen(start)]` entrypoint; the first public
/// exported function call is the entry. A panic hook is installed lazily in
/// JS by calling `install_panic_hook` (below) in `main.ts`.
#[wasm_bindgen]
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("{info}");
        log(&msg);
    }));
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

fn log(s: &str) {
    error(s);
}
