//! wasm-bindgen surface for the playable session (movement + camera).

use std::cell::RefCell;
use wasm_bindgen::prelude::*;

use crate::bsp::Bsp;
use crate::sim::Session;

thread_local! {
    static SESSIONS: RefCell<Vec<Option<Session>>> = RefCell::new(Vec::new());
}

/// Create a playable session bound to a previously-parsed map (by handle id)
/// at the given spawn index. Returns a new session handle.
#[wasm_bindgen]
pub fn session_new(map_id: usize, spawn_index: usize) -> Result<usize, JsValue> {
    let session = crate::bindings::with_map(map_id, |bsp: &Bsp| Session::new(bsp, spawn_index))
        .map_err(|e| JsValue::from_str(&format!("{e:?}")))?
        .map_err(|e| JsValue::from_str(&e))?;
    SESSIONS.with(|s| {
        let mut s = s.borrow_mut();
        s.push(Some(session));
        Ok(s.len() - 1)
    })
}

#[wasm_bindgen]
pub fn session_drop(id: usize) {
    SESSIONS.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(slot) = s.get_mut(id) {
            *slot = None;
        }
    });
}

#[wasm_bindgen]
pub fn session_add_mouse(id: usize, dx: f32, dy: f32) {
    SESSIONS.with(|s| {
        if let Some(Some(s)) = s.borrow_mut().get_mut(id) {
            s.add_mouse(dx, dy);
        }
    });
}

#[wasm_bindgen]
pub fn session_set_keys(
    id: usize,
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    jump: bool,
    crouch: bool,
    special: bool,
    attack: bool,
) {
    SESSIONS.with(|s| {
        if let Some(Some(s)) = s.borrow_mut().get_mut(id) {
            s.set_keys(forward, back, left, right, jump, crouch, special, attack);
        }
    });
}

#[wasm_bindgen]
pub fn session_set_sensitivity(id: usize, sens: f32) {
    SESSIONS.with(|s| {
        if let Some(Some(s)) = s.borrow_mut().get_mut(id) {
            s.set_sensitivity(sens);
        }
    });
}

#[wasm_bindgen]
pub fn session_step(id: usize) {
    SESSIONS.with(|s| {
        if let Some(Some(s)) = s.borrow_mut().get_mut(id) {
            s.step();
        }
    });
}

// Getters (copy small values out to avoid holding the borrow across JS).

#[wasm_bindgen]
pub fn session_eye(id: usize) -> Vec<f32> {
    SESSIONS.with(|s| {
        s.borrow()
            .get(id)
            .and_then(|o| o.as_ref())
            .map(|s| s.eye().to_vec())
            .unwrap_or_else(|| vec![0.0, 0.0, 0.0])
    })
}

#[wasm_bindgen]
pub fn session_angles(id: usize) -> Vec<f32> {
    SESSIONS.with(|s| {
        s.borrow()
            .get(id)
            .and_then(|o| o.as_ref())
            .map(|s| vec![s.yaw(), s.pitch()])
            .unwrap_or_else(|| vec![0.0, 0.0])
    })
}

#[wasm_bindgen]
pub fn session_speed(id: usize) -> f32 {
    SESSIONS.with(|s| {
        s.borrow()
            .get(id)
            .and_then(|o| o.as_ref())
            .map(|s| s.speed())
            .unwrap_or(0.0)
    })
}

/// Movement HUD data (8 floats): velocity[3], wishdir[3], speed, accel.
#[wasm_bindgen]
pub fn session_movement_hint(id: usize) -> Vec<f32> {
    SESSIONS.with(|s| {
        s.borrow()
            .get(id)
            .and_then(|o| o.as_ref())
            .map(|s| s.movement_hint())
            .unwrap_or_else(|| vec![0.0; 8])
    })
}

#[wasm_bindgen]
pub fn session_on_ground(id: usize) -> bool {
    SESSIONS.with(|s| {
        s.borrow()
            .get(id)
            .and_then(|o| o.as_ref())
            .map(|s| s.on_ground())
            .unwrap_or(false)
    })
}

/// Reset the player to the spawn point (race restart).
#[wasm_bindgen]
pub fn session_reset(id: usize) {
    SESSIONS.with(|s| {
        if let Some(Some(session)) = s.borrow_mut().get_mut(id) {
            session.reset();
        }
    });
}

/// Save the current position as the new spawn (only within the start zone).
/// Returns true if the save succeeded.
#[wasm_bindgen]
pub fn session_position_save(id: usize) -> bool {
    SESSIONS.with(|s| {
        s.borrow_mut()
            .get_mut(id)
            .and_then(|o| o.as_mut())
            .map(|session| session.position_save())
            .unwrap_or(false)
    })
}

// Race state getters.

#[wasm_bindgen]
pub fn session_race_running(id: usize) -> bool {
    SESSIONS.with(|s| {
        s.borrow()
            .get(id)
            .and_then(|o| o.as_ref())
            .map(|s| s.race_running())
            .unwrap_or(false)
    })
}

#[wasm_bindgen]
pub fn session_race_finished(id: usize) -> bool {
    SESSIONS.with(|s| {
        s.borrow()
            .get(id)
            .and_then(|o| o.as_ref())
            .map(|s| s.race_finished())
            .unwrap_or(false)
    })
}

#[wasm_bindgen]
pub fn session_race_ticks(id: usize) -> u32 {
    SESSIONS.with(|s| {
        s.borrow()
            .get(id)
            .and_then(|o| o.as_ref())
            .map(|s| s.race_ticks())
            .unwrap_or(0)
    })
}

#[wasm_bindgen]
pub fn session_race_finished_ticks(id: usize) -> u32 {
    SESSIONS.with(|s| {
        s.borrow()
            .get(id)
            .and_then(|o| o.as_ref())
            .and_then(|s| s.race_finished_ticks())
            .unwrap_or(0)
    })
}

#[wasm_bindgen]
pub fn session_race_splits(id: usize) -> Vec<u32> {
    SESSIONS.with(|s| {
        s.borrow()
            .get(id)
            .and_then(|o| o.as_ref())
            .map(|s| s.race_splits())
            .unwrap_or_default()
    })
}

#[wasm_bindgen]
pub fn session_race_total_checkpoints(id: usize) -> usize {
    SESSIONS.with(|s| {
        s.borrow()
            .get(id)
            .and_then(|o| o.as_ref())
            .map(|s| s.race_total_checkpoints())
            .unwrap_or(0)
    })
}
