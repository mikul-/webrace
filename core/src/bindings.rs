//! WebAssembly bindings exposed to the browser.
//!
//! The JS side calls `bsp_parse` with the raw `.bsp` bytes (after extracting
//! from a `.pk3`), then pulls back render + collision data. Map handles are
//! stored in a thread-local (WASM is single-threaded), so no `unsafe` statics.

use std::cell::RefCell;
use wasm_bindgen::prelude::*;

use crate::bsp::Bsp;

thread_local! {
    static MAPS: RefCell<Vec<Bsp>> = RefCell::new(Vec::new());
}

#[wasm_bindgen]
pub fn bsp_parse(name: &str, data: &[u8]) -> Result<usize, JsValue> {
    let bsp = Bsp::parse(name, data).map_err(|e| JsValue::from_str(&e))?;
    MAPS.with(|m| {
        let mut m = m.borrow_mut();
        m.push(bsp);
        Ok(m.len() - 1)
    })
}

#[wasm_bindgen]
pub fn bsp_vertex_count(id: usize) -> usize {
    MAPS.with(|m| m.borrow().get(id).map(|b| b.positions.len()).unwrap_or(0))
}

#[wasm_bindgen]
pub fn bsp_index_count(id: usize) -> usize {
    MAPS.with(|m| m.borrow().get(id).map(|b| b.indices.len()).unwrap_or(0))
}

/// Byte offset into WASM memory of the render vertices (14 f32 per vertex).
#[wasm_bindgen]
pub fn bsp_vertices_ptr(id: usize) -> usize {
    MAPS.with(|m| {
        m.borrow()
            .get(id)
            .map(|b| b.positions.as_ptr() as usize)
            .unwrap_or(0)
    })
}

#[wasm_bindgen]
pub fn bsp_indices_ptr(id: usize) -> usize {
    MAPS.with(|m| {
        m.borrow()
            .get(id)
            .map(|b| b.indices.as_ptr() as usize)
            .unwrap_or(0)
    })
}

#[wasm_bindgen]
pub fn bsp_triangle_count(id: usize) -> usize {
    MAPS.with(|m| m.borrow().get(id).map(|b| b.triangle_count()).unwrap_or(0))
}

#[wasm_bindgen]
pub fn bsp_brush_count(id: usize) -> usize {
    MAPS.with(|m| {
        m.borrow()
            .get(id)
            .map(|b| b.brush_plane_offsets.len())
            .unwrap_or(0)
    })
}

/// Completely drop all loaded maps and reset the thread-local store.
#[wasm_bindgen]
pub fn bsp_release_all() {
    MAPS.with(|m| m.borrow_mut().clear());
}
