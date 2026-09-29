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
