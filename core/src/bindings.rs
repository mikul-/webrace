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

/// Internal helper: run a closure against a parsed map, returning its result
/// or an error if the handle is out of range. Used by `session.rs`.
pub fn with_map<T>(id: usize, f: impl FnOnce(&Bsp) -> T) -> Result<T, JsValue> {
    MAPS.with(|m| {
        let m = m.borrow();
        let bsp = m.get(id).ok_or_else(|| JsValue::from_str("invalid map id"))?;
        Ok(f(bsp))
    })
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

/// Byte offset into WASM memory of the packed RGB lightmap atlas.
#[wasm_bindgen]
pub fn bsp_lightmap_ptr(id: usize) -> usize {
    MAPS.with(|m| {
        m.borrow()
            .get(id)
            .map(|b| b.lightmap_atlas.as_ptr() as usize)
            .unwrap_or(0)
    })
}

#[wasm_bindgen]
pub fn bsp_lightmap_len(id: usize) -> usize {
    MAPS.with(|m| m.borrow().get(id).map(|b| b.lightmap_atlas.len()).unwrap_or(0))
}

#[wasm_bindgen]
pub fn bsp_lightmap_w(id: usize) -> u32 {
    MAPS.with(|m| m.borrow().get(id).map(|b| b.lightmap_atlas_w).unwrap_or(0))
}

#[wasm_bindgen]
pub fn bsp_lightmap_h(id: usize) -> u32 {
    MAPS.with(|m| m.borrow().get(id).map(|b| b.lightmap_atlas_h).unwrap_or(0))
}

#[wasm_bindgen]
pub fn bsp_chunk_count(id: usize) -> usize {
    MAPS.with(|m| m.borrow().get(id).map(|b| b.chunks.len()).unwrap_or(0))
}

/// Write chunk data into JS-provided arrays: (shader_index, first_index,
/// index_count) triplets. `shader_idx`, `first`, `count` are pre-allocated
/// `Uint32Array`s of length chunk_count.
#[wasm_bindgen]
pub fn bsp_chunks(
    id: usize,
    shader_idx: &mut [u32],
    first: &mut [u32],
    count: &mut [u32],
) {
    MAPS.with(|m| {
        let b = m.borrow();
        if let Some(bsp) = b.get(id) {
            for (i, (shader, f, c)) in bsp.chunks.iter().enumerate() {
                if i >= shader_idx.len() {
                    break;
                }
                shader_idx[i] = *shader as u32;
                first[i] = *f;
                count[i] = *c;
            }
        }
    });
}

/// Return the shader name for a given shader index (empty if out of range).
#[wasm_bindgen]
pub fn bsp_shader_name(id: usize, shader_index: usize) -> String {
    MAPS.with(|m| {
        m.borrow()
            .get(id)
            .and_then(|b| b.shaders.get(shader_index))
            .cloned()
            .unwrap_or_default()
    })
}

#[wasm_bindgen]
pub fn bsp_shader_count(id: usize) -> usize {
    MAPS.with(|m| m.borrow().get(id).map(|b| b.shaders.len()).unwrap_or(0))
}

/// Copy each shader's surface flags into `out` (parallel to `bsp_shader_name`).
#[wasm_bindgen]
pub fn bsp_shader_flags(id: usize, out: &mut [i32]) {
    MAPS.with(|m| {
        let b = m.borrow();
        if let Some(bsp) = b.get(id) {
            for (i, f) in bsp.shader_flags.iter().enumerate() {
                if i >= out.len() {
                    break;
                }
                out[i] = *f;
            }
        }
    });
}

/// Number of jumppad (`trigger_push`) volumes parsed from the map.
#[wasm_bindgen]
pub fn bsp_jumppad_count(id: usize) -> usize {
    MAPS.with(|m| m.borrow().get(id).map(|b| b.jumppads.len()).unwrap_or(0))
}

/// Number of teleporter (`trigger_teleport`) volumes parsed from the map.
#[wasm_bindgen]
pub fn bsp_teleporter_count(id: usize) -> usize {
    MAPS.with(|m| m.borrow().get(id).map(|b| b.teleporters.len()).unwrap_or(0))
}

/// Completely drop all loaded maps and reset the thread-local store.
#[wasm_bindgen]
pub fn bsp_release_all() {
    MAPS.with(|m| m.borrow_mut().clear());
}
