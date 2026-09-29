/* tslint:disable */
/* eslint-disable */

export function bsp_brush_count(id: number): number;

export function bsp_index_count(id: number): number;

export function bsp_indices_ptr(id: number): number;

export function bsp_parse(name: string, data: Uint8Array): number;

/**
 * Completely drop all loaded maps and reset the thread-local store.
 */
export function bsp_release_all(): void;

export function bsp_triangle_count(id: number): number;

export function bsp_vertex_count(id: number): number;

/**
 * Byte offset into WASM memory of the render vertices (14 f32 per vertex).
 */
export function bsp_vertices_ptr(id: number): number;

/**
 * Built without a `#[wasm_bindgen(start)]` entrypoint; the first public
 * exported function call is the entry. A panic hook is installed lazily in
 * JS by calling `install_panic_hook` (below) in `main.ts`.
 */
export function install_panic_hook(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly bsp_brush_count: (a: number) => number;
    readonly bsp_index_count: (a: number) => number;
    readonly bsp_indices_ptr: (a: number) => number;
    readonly bsp_parse: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly bsp_release_all: () => void;
    readonly bsp_triangle_count: (a: number) => number;
    readonly bsp_vertex_count: (a: number) => number;
    readonly bsp_vertices_ptr: (a: number) => number;
    readonly install_panic_hook: () => void;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
