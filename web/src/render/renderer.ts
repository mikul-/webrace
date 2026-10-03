// Thin WebGL2 renderer for the static world (lightmapped BSP) plus dynamic
// entities. Kept allocation-light for the 360fps target: buffers are created
// once per map and reused.

import type { BlendMode } from "./shader";

/** A draw chunk: a base pass plus additive/blended overlay passes. */
export interface DrawChunk {
  first: number;
  count: number;
  /** Base texture ids (animmap frames), or [] for untextured. */
  tex: number[];
  animFreq: number;
  blend: BlendMode;
  lit: boolean;
  scroll: [number, number];
  overlays: Array<{ tex: number; blend: BlendMode; scroll: [number, number] }>;
}

/** A draw chunk belonging to a moving brush entity (indexed by `model`). */
export interface MoverChunk extends DrawChunk {
  model: number;
}

const IDENTITY = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);

/** Texture filtering quality (maps to min/mag filters + mipmaps + anisotropy). */
export type TextureMode = "nearest" | "bilinear" | "trilinear" | "anisotropic";
export const TEXTURE_MODES: TextureMode[] = ["nearest", "bilinear", "trilinear", "anisotropic"];

export class Renderer {
  gl: WebGL2RenderingContext;
  private program: WebGLProgram;
  private vao: WebGLVertexArrayObject;
  private vbo: WebGLBuffer;
  private ibo: WebGLBuffer;
  private indexCount = 0;
  // Wireframe: a second VAO per mesh sharing the vertex buffer but using a
  // line-index buffer (WebGL2 has no polygon mode). Line indices are exactly
  // 2x the triangle indices in the same order, so chunk ranges scale by 2.
  private wireIbo: WebGLBuffer;
  private wireVao: WebGLVertexArrayObject;
  private wireIndexCount = 0;
  private wireframe = false;
  // Moving brush entities use a second vertex/index buffer so each mover can be
  // drawn with its own model matrix.
  private moverVao: WebGLVertexArrayObject | null = null;
  private moverVbo: WebGLBuffer | null = null;
  private moverIbo: WebGLBuffer | null = null;
  private moverWireIbo: WebGLBuffer | null = null;
  private moverWireVao: WebGLVertexArrayObject | null = null;
  private moverIndexCount = 0;
  private moverChunks: MoverChunk[] = [];
  private uProjView: WebGLUniformLocation;
  private uModel: WebGLUniformLocation;
  private uHasTexture: WebGLUniformLocation;
  private uLit: WebGLUniformLocation;
  private uScroll: WebGLUniformLocation;
  private uTime: WebGLUniformLocation;
  private uWire: WebGLUniformLocation;
  // Overlay (FX) program.
  private fxProgram: WebGLProgram;
  private fxProjView: WebGLUniformLocation;
  private fxModel: WebGLUniformLocation;
  private fxScroll: WebGLUniformLocation;
  private fxTime: WebGLUniformLocation;
  private lightmapTex: WebGLTexture;
  private whiteTex: WebGLTexture;
  private textures: Map<number, WebGLTexture>;
  private texCount: number;
  /** Texture filtering quality + anisotropic filtering capability. */
  private textureMode: TextureMode = "anisotropic";
  private anisoExt: {
    TEXTURE_MAX_ANISOTROPY_EXT: number;
    MAX_TEXTURE_MAX_ANISOTROPY_EXT: number;
  } | null = null;
  private anisoMax = 1;
  private anisoLevel = 8;
  /** Draw chunks: base pass + additive/blended overlays. */
  private chunks: DrawChunk[] = [];

  // Skybox.
  private skyProgram: WebGLProgram | null = null;
  private skyVao: WebGLVertexArrayObject | null = null;
  private skyVbo: WebGLBuffer | null = null;
  private skyIbo: WebGLBuffer | null = null;
  private skyFaceTextures: (WebGLTexture | null)[] = [];
  private skyRanges: Array<{ first: number; count: number; axis: number }> = [];

  /** Surface flags. */
  static readonly SURF_SKY = 0x4;

  constructor(canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl2", {
      antialias: false,
      depth: true,
      stencil: false,
      powerPreference: "high-performance",
    }) as WebGL2RenderingContext | null;
    if (!gl) throw new Error("WebGL2 not supported");
    this.gl = gl;

    // Anisotropic filtering (optional; falls back to trilinear if unsupported).
    const aniso = gl.getExtension("EXT_texture_filter_anisotropic") as {
      TEXTURE_MAX_ANISOTROPY_EXT: number;
      MAX_TEXTURE_MAX_ANISOTROPY_EXT: number;
    } | null;
    this.anisoExt = aniso;
    if (aniso) {
      this.anisoMax = gl.getParameter(aniso.MAX_TEXTURE_MAX_ANISOTROPY_EXT) as number;
    }

    const vs = `#version 300 es
      layout(location=0) in vec3 a_pos;
      layout(location=1) in vec2 a_uv;
      layout(location=2) in vec2 a_lm;
      layout(location=3) in vec3 a_norm;
      layout(location=4) in vec4 a_color;
      uniform mat4 u_proj_view;
      uniform mat4 u_model;
      out vec2 v_uv; out vec2 v_lm; out vec3 v_norm; out vec4 v_color;
      void main() {
        gl_Position = u_proj_view * u_model * vec4(a_pos, 1.0);
        v_uv = a_uv; v_lm = a_lm; v_norm = a_norm; v_color = a_color;
      }`;
    const fs = `#version 300 es
      precision highp float;
      in vec2 v_uv; in vec2 v_lm; in vec3 v_norm; in vec4 v_color;
      uniform sampler2D u_texture;
      uniform sampler2D u_lightmap;
      uniform float u_has_texture;
      uniform float u_lit;
      uniform float u_wire;
      uniform vec2 u_scroll;
      uniform float u_time;
      out vec4 outColor;
      void main() {
        // Wireframe debug: flat bright lines, ignore texture/lightmap.
        if (u_wire > 0.5) {
          outColor = vec4(0.20, 1.0, 0.40, 1.0);
          return;
        }
        vec4 tex = texture(u_texture, v_uv + u_scroll * u_time);
        // Surface albedo (from texture) or a neutral gray fallback.
        vec3 albedo = mix(vec3(0.62), tex.rgb, u_has_texture);
        float alpha = mix(1.0, tex.a, u_has_texture);
        if (u_lit > 0.5) {
          // Baked lightmap: stored dark, meant to multiply the albedo.
          vec3 lm = texture(u_lightmap, v_lm).rgb;
          // Overbright in the style of Q3 (r_overbrightBits ~2) + gamma lift.
          vec3 base = albedo * lm * 4.0;
          outColor = vec4(pow(clamp(base, 0.0, 1.0), vec3(0.72)), alpha);
        } else {
          outColor = vec4(albedo, alpha);
        }
      }`;

    this.program = this.link(vs, fs);
    gl.useProgram(this.program);

    this.uProjView = gl.getUniformLocation(this.program, "u_proj_view")!;
    this.uModel = gl.getUniformLocation(this.program, "u_model")!;
    this.uHasTexture = gl.getUniformLocation(this.program, "u_has_texture")!;
    this.uLit = gl.getUniformLocation(this.program, "u_lit")!;
    this.uScroll = gl.getUniformLocation(this.program, "u_scroll")!;
    this.uTime = gl.getUniformLocation(this.program, "u_time")!;
    this.uWire = gl.getUniformLocation(this.program, "u_wire")!;
    const uLightmap = gl.getUniformLocation(this.program, "u_lightmap");
    const uTexture = gl.getUniformLocation(this.program, "u_texture");
    if (uLightmap) gl.uniform1i(uLightmap, 0);
    if (uTexture) gl.uniform1i(uTexture, 1);

    // Overlay (FX) program for additive/blended shader stages.
    const fxVs = `#version 300 es
      layout(location=0) in vec3 a_pos;
      layout(location=1) in vec2 a_uv;
      layout(location=4) in vec4 a_color;
      uniform mat4 u_proj_view;
      uniform mat4 u_model;
      out vec2 v_uv; out vec4 v_color;
      void main() {
        gl_Position = u_proj_view * u_model * vec4(a_pos, 1.0);
        v_uv = a_uv; v_color = a_color;
      }`;
    const fxFs = `#version 300 es
      precision highp float;
      in vec2 v_uv; in vec4 v_color;
      uniform sampler2D u_texture;
      uniform vec2 u_scroll;
      uniform float u_time;
      out vec4 outColor;
      void main() {
        outColor = texture(u_texture, v_uv + u_scroll * u_time) * v_color;
      }`;
    this.fxProgram = this.link(fxVs, fxFs);
    this.fxProjView = gl.getUniformLocation(this.fxProgram, "u_proj_view")!;
    this.fxModel = gl.getUniformLocation(this.fxProgram, "u_model")!;
    this.fxScroll = gl.getUniformLocation(this.fxProgram, "u_scroll")!;
    this.fxTime = gl.getUniformLocation(this.fxProgram, "u_time")!;
    gl.useProgram(this.fxProgram);
    const fxTex = gl.getUniformLocation(this.fxProgram, "u_texture");
    if (fxTex) gl.uniform1i(fxTex, 1);
    gl.useProgram(this.program);

    // Lightmap atlas texture (unit 0). Kept unfiltered-by-mode and without
    // mipmaps: the atlas is a grid of independent cells, so mips would bleed
    // light between them. CLAMP avoids sampling past the atlas edge.
    this.lightmapTex = gl.createTexture()!;
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.lightmapTex);
    this.configureLightmap();

    // Texture cache (unit 1): shader-index -> WebGLTexture.
    this.textures = new Map();
    this.texCount = 0;
    // Default 1x1 white texture for faces whose image failed to load.
    this.whiteTex = gl.createTexture()!;
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, this.whiteTex);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([158, 158, 158, 255]));
    gl.generateMipmap(gl.TEXTURE_2D);
    this.configureTex();

    this.vao = gl.createVertexArray()!;
    this.vbo = gl.createBuffer()!;
    this.ibo = gl.createBuffer()!;
    this.wireIbo = gl.createBuffer()!;
    this.wireVao = gl.createVertexArray()!;

    gl.bindVertexArray(this.vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.vbo);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.ibo);
    this.setupAttribs();
    gl.bindVertexArray(null);

    gl.bindVertexArray(this.wireVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.vbo);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.wireIbo);
    this.setupAttribs();
    gl.bindVertexArray(null);

    gl.enable(gl.DEPTH_TEST);
    // Culling disabled for now: the q2t reflection may flip winding, and
    // Quake BSP front-face convention differs from WebGL's default. Re-enable
    // once winding is verified.
    // gl.enable(gl.CULL_FACE);
    gl.clearColor(0.04, 0.05, 0.07, 1);
  }

  /** Set the interleaved 14-float vertex attributes on the bound VAO. */
  private setupAttribs() {
    const gl = this.gl;
    const stride = 14 * 4;
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, stride, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 2, gl.FLOAT, false, stride, 12);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 2, gl.FLOAT, false, stride, 20);
    gl.enableVertexAttribArray(3);
    gl.vertexAttribPointer(3, 3, gl.FLOAT, false, stride, 28);
    gl.enableVertexAttribArray(4);
    gl.vertexAttribPointer(4, 4, gl.FLOAT, false, stride, 40);
  }

  /** Enable/disable wireframe (debug) rendering. */
  setWireframe(on: boolean) {
    this.wireframe = on;
  }
  isWireframe(): boolean {
    return this.wireframe;
  }

  private link(vs: string, fs: string): WebGLProgram {
    const gl = this.gl;
    const mk = (t: number, s: string) => {
      const sh = gl.createShader(t)!;
      gl.shaderSource(sh, s);
      gl.compileShader(sh);
      if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
        throw new Error("shader: " + gl.getShaderInfoLog(sh));
      }
      return sh;
    };
    const p = gl.createProgram()!;
    gl.attachShader(p, mk(gl.VERTEX_SHADER, vs));
    gl.attachShader(p, mk(gl.FRAGMENT_SHADER, fs));
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) {
      throw new Error("link: " + gl.getProgramInfoLog(p));
    }
    return p;
  }

  /** Upload a map's vertex + index data (from WASM memory via views). */
  uploadMap(
    memory: WebAssembly.Memory,
    vertsPtr: number,
    vertCount: number,
    idxPtr: number,
    idxCount: number,
  ) {
    const gl = this.gl;
    gl.bindVertexArray(this.vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.vbo);
    const v = new Float32Array(memory.buffer, vertsPtr, vertCount);
    gl.bufferData(gl.ARRAY_BUFFER, v, gl.STATIC_DRAW);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.ibo);
    const idx = new Uint32Array(memory.buffer, idxPtr, idxCount);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, idx, gl.STATIC_DRAW);
    this.indexCount = idxCount;

    // Wireframe line indices (2x the triangle indices, emitted in the same
    // order so every triangle range maps to `[first*2, count*2)`). Bind the
    // line VAO *before* touching the line IBO, otherwise the element-buffer
    // binding would be recorded into `this.vao` and normal draws would use the
    // line indices.
    const lines = buildLineIndices(idx);
    gl.bindVertexArray(this.wireVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.vbo);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.wireIbo);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, lines, gl.STATIC_DRAW);
    this.setupAttribs();
    this.wireIndexCount = lines.length;
    gl.bindVertexArray(null);
  }

  /**
   * Upload the moving brush entities' geometry into a second vertex/index
   * buffer. The same interleaved 14-float layout as `uploadMap`.
   */
  uploadMovers(
    memory: WebAssembly.Memory,
    vertsPtr: number,
    vertCount: number,
    idxPtr: number,
    idxCount: number,
  ) {
    const gl = this.gl;
    if (!this.moverVao) {
      this.moverVao = gl.createVertexArray();
      this.moverVbo = gl.createBuffer();
      this.moverIbo = gl.createBuffer();
      this.moverWireIbo = gl.createBuffer();
      this.moverWireVao = gl.createVertexArray();
    }
    if (idxCount === 0 || vertCount === 0) {
      this.moverIndexCount = 0;
      return;
    }
    gl.bindVertexArray(this.moverVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.moverVbo);
    const v = new Float32Array(memory.buffer, vertsPtr, vertCount);
    gl.bufferData(gl.ARRAY_BUFFER, v, gl.STATIC_DRAW);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.moverIbo);
    const idx = new Uint32Array(memory.buffer, idxPtr, idxCount);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, idx, gl.STATIC_DRAW);
    this.setupAttribs();
    this.moverIndexCount = idxCount;

    const lines = buildLineIndices(idx);
    // Bind the wire VAO before the wire IBO so `moverVao` keeps its triangle IBO.
    gl.bindVertexArray(this.moverWireVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.moverVbo);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.moverWireIbo);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, lines, gl.STATIC_DRAW);
    this.setupAttribs();
    gl.bindVertexArray(null);
  }

  /** Set the mover draw chunks (each references a mover index for its matrix). */
  setMoverChunks(chunks: MoverChunk[]) {
    this.moverChunks = chunks;
  }

  /** Upload the packed RGB lightmap atlas (from WASM memory). */
  uploadLightmap(memory: WebAssembly.Memory, ptr: number, len: number, w: number, h: number) {
    const gl = this.gl;
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.lightmapTex);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    const data = new Uint8Array(memory.buffer, ptr, len);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB, w, h, 0, gl.RGB, gl.UNSIGNED_BYTE, data);
  }

  resize(w: number, h: number) {
    this.gl.viewport(0, 0, w, h);
  }

  /** Set the draw chunks (base pass + overlays) for this map. */
  setChunks(chunks: DrawChunk[]) {
    this.chunks = chunks;
  }

  /** Pick the current animmap frame texture id for `time` seconds. */
  private pickAnim(ids: number[], freq: number, time: number): number | null {
    if (!ids.length) return null;
    if (ids.length === 1 || freq <= 0) return ids[0];
    const frame = Math.floor(time * freq) % ids.length;
    return ids[frame];
  }

  private setBlend(mode: BlendMode) {
    const gl = this.gl;
    if (mode === "opaque") {
      gl.disable(gl.BLEND);
      return;
    }
    gl.enable(gl.BLEND);
    if (mode === "add") gl.blendFunc(gl.ONE, gl.ONE);
    else if (mode === "blend") gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    else if (mode === "filter") gl.blendFunc(gl.DST_COLOR, gl.ZERO);
  }

  /**
   * Build the skybox from 6 face images in Q3 suffix order
   * `[rt, bk, lf, ft, up, dn]` (matching Q3's `suf[]` / `MakeSkyVec`).
   */
  setSkybox(faces: Array<{ width: number; height: number; data: Uint8ClampedArray | Uint8Array } | null>) {
    const gl = this.gl;
    if (!this.skyProgram) {
      const vs = `#version 300 es
        layout(location=0) in vec3 a_pos;
        layout(location=1) in vec2 a_uv;
        uniform mat4 u_proj_view;
        uniform vec3 u_eye;
        out vec2 v_uv;
        void main() {
          gl_Position = u_proj_view * vec4(a_pos + u_eye, 1.0);
          v_uv = a_uv;
        }`;
      const fs = `#version 300 es
        precision highp float;
        in vec2 v_uv;
        uniform sampler2D u_tex;
        out vec4 outColor;
        void main() { outColor = vec4(texture(u_tex, v_uv).rgb, 1.0); }`;
      this.skyProgram = this.link(vs, fs);
      this.skyVao = gl.createVertexArray();
      this.skyVbo = gl.createBuffer();
      this.skyIbo = gl.createBuffer();
    }

    // Q3 `st_to_vec` / suffix order.
    const stToVec = [
      [3, -1, 2],
      [-3, 1, 2],
      [1, 3, 2],
      [-1, -3, 2],
      [-2, -1, 3],
      [2, -1, -3],
    ];
    const SCALE = 80000;
    const corners = [
      [-1, -1],
      [1, -1],
      [1, 1],
      [-1, 1],
    ];
    const verts: number[] = [];
    const idx: number[] = [];
    this.skyRanges = [];
    for (let axis = 0; axis < 6; axis++) {
      const base = verts.length / 5;
      for (const [s, t] of corners) {
        const b = [s, t, 1];
        const out = [0, 0, 0];
        for (let j = 0; j < 3; j++) {
          const k = stToVec[axis][j];
          out[j] = (k < 0 ? -b[-k - 1] : b[k - 1]) * SCALE;
        }
        // The wall faces (rt/bk/lf/ft = axes 0..3) need their V inverted so the
        // sky image's top lands on the wall top (the up/dn faces are already
        // correct with the direct mapping).
        const v = axis < 4 ? 1 - (t + 1) * 0.5 : (t + 1) * 0.5;
        verts.push(out[0], out[1], out[2], (s + 1) * 0.5, v);
      }
      const first = idx.length;
      idx.push(base, base + 1, base + 2, base, base + 2, base + 3);
      this.skyRanges.push({ first, count: 6, axis });
    }

    gl.bindVertexArray(this.skyVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.skyVbo);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(verts), gl.STATIC_DRAW);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.skyIbo);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, new Uint32Array(idx), gl.STATIC_DRAW);
    const stride = 5 * 4;
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, stride, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 2, gl.FLOAT, false, stride, 12);
    gl.bindVertexArray(null);

    // Textures (one per face).
    gl.activeTexture(gl.TEXTURE2);
    this.skyFaceTextures = faces.map((img) => {
      if (!img) return null;
      const tex = gl.createTexture()!;
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, img.width, img.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, img.data);
      gl.generateMipmap(gl.TEXTURE_2D);
      this.configureTex();
      return tex;
    });
  }

  /** Remove any skybox (e.g. when loading a map that has none). */
  clearSkybox() {
    const gl = this.gl;
    for (const t of this.skyFaceTextures) {
      if (t) gl.deleteTexture(t);
    }
    this.skyFaceTextures = [];
    this.skyRanges = [];
  }

  private drawSkybox(projView: Float32Array, eye: [number, number, number]) {
    if (!this.skyProgram || this.skyRanges.length === 0) return;
    const gl = this.gl;
    gl.useProgram(this.skyProgram);
    gl.uniformMatrix4fv(gl.getUniformLocation(this.skyProgram, "u_proj_view"), false, projView);
    gl.uniform3fv(gl.getUniformLocation(this.skyProgram, "u_eye"), eye);
    gl.uniform1i(gl.getUniformLocation(this.skyProgram, "u_tex"), 2);
    gl.disable(gl.BLEND);
    gl.bindVertexArray(this.skyVao);
    for (const r of this.skyRanges) {
      const tex = this.skyFaceTextures[r.axis];
      if (!tex) continue;
      gl.activeTexture(gl.TEXTURE2);
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.drawElements(gl.TRIANGLES, r.count, gl.UNSIGNED_INT, r.first * 4);
    }
    gl.bindVertexArray(null);
  }

  /** Register a decoded image as a texture, returning a texture id. */
  registerTexture(img: { width: number; height: number; data: Uint8ClampedArray | Uint8Array }): number {
    const gl = this.gl;
    const tex = gl.createTexture()!;
    const id = this.texCount++;
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, img.width, img.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, img.data);
    gl.generateMipmap(gl.TEXTURE_2D);
    this.configureTex();
    this.textures.set(id, tex);
    return id;
  }

  /** Set the texture filtering mode and re-apply it to every loaded texture. */
  setTextureMode(mode: TextureMode) {
    this.textureMode = mode;
    this.applyTextureMode();
  }

  /** Whether anisotropic filtering is available on this GPU. */
  anisoAvailable(): boolean {
    return this.anisoExt !== null && this.anisoMax > 1;
  }

  /** Re-apply the current texture filter mode to every loaded world/sky texture. */
  private applyTextureMode() {
    const gl = this.gl;
    // Use unit 1 so we don't clobber the lightmap binding on unit 0.
    gl.activeTexture(gl.TEXTURE1);
    for (const tex of this.textures.values()) {
      gl.bindTexture(gl.TEXTURE_2D, tex);
      this.configureTex();
    }
    gl.bindTexture(gl.TEXTURE_2D, this.whiteTex);
    this.configureTex();
    for (const tex of this.skyFaceTextures) {
      if (!tex) continue;
      gl.bindTexture(gl.TEXTURE_2D, tex);
      this.configureTex();
    }
  }

  /** Apply the current filter mode + anisotropy to the bound texture. */
  private configureTex() {
    const gl = this.gl;
    const mode = this.textureMode;
    const mag = mode === "nearest" ? gl.NEAREST : gl.LINEAR;
    const min =
      mode === "nearest"
        ? gl.NEAREST
        : mode === "bilinear"
          ? gl.LINEAR
          : gl.LINEAR_MIPMAP_LINEAR; // trilinear / anisotropic
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, min);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, mag);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.REPEAT);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.REPEAT);
    if (this.anisoExt) {
      const level = mode === "anisotropic" ? Math.min(this.anisoLevel, this.anisoMax) : 1;
      gl.texParameterf(gl.TEXTURE_2D, this.anisoExt.TEXTURE_MAX_ANISOTROPY_EXT, level);
    }
  }

  /** Fixed filtering for the lightmap atlas (no mips, no mode/aniso). */
  private configureLightmap() {
    const gl = this.gl;
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  }

  draw(
    projView: Float32Array,
    eye: [number, number, number] = [0, 0, 0],
    time = 0,
    moverMatrices: Float32Array[] = [],
  ) {
    const gl = this.gl;
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    // Skybox first: it writes depth far away, so the world draws over it. Skip
    // it in wireframe so the lines stand out against the dark clear color.
    if (!this.wireframe) this.drawSkybox(projView, eye);
    if (this.indexCount === 0 && this.moverIndexCount === 0) return;
    const wire = this.wireframe;
    const primitive = wire ? gl.LINES : gl.TRIANGLES;
    gl.bindVertexArray(wire ? this.wireVao : this.vao);

    if (this.chunks.length === 0) {
      gl.useProgram(this.program);
      gl.uniformMatrix4fv(this.uProjView, false, projView);
      gl.uniformMatrix4fv(this.uModel, false, IDENTITY);
      gl.uniform1f(this.uHasTexture, 0);
      gl.uniform1f(this.uLit, 1);
      gl.uniform1f(this.uWire, wire ? 1 : 0);
      gl.uniform2f(this.uScroll, 0, 0);
      gl.uniform1f(this.uTime, time);
      this.setBlend("opaque");
      gl.drawElements(
        primitive,
        wire ? this.wireIndexCount : this.indexCount,
        gl.UNSIGNED_INT,
        0,
      );
    } else {
      for (const c of this.chunks) {
        this.drawChunkBody(c, projView, time, IDENTITY, primitive);
      }
    }

    // Moving brush entities, drawn from their own buffer with a per-mover
    // model matrix.
    if (this.moverVao && this.moverIndexCount > 0 && this.moverChunks.length > 0) {
      gl.bindVertexArray(wire ? this.moverWireVao : this.moverVao);
      for (const c of this.moverChunks) {
        const model = moverMatrices[c.model] ?? IDENTITY;
        this.drawChunkBody(c, projView, time, model, primitive);
      }
    }

    gl.disable(gl.BLEND);
    gl.bindVertexArray(null);
  }

  /** Draw one chunk's base pass plus overlay passes with the given model matrix. */
  private drawChunkBody(
    c: DrawChunk,
    projView: Float32Array,
    time: number,
    model: Float32Array,
    primitive: number,
  ) {
    const gl = this.gl;
    const wire = this.wireframe;
    // Base pass.
    gl.useProgram(this.program);
    gl.uniformMatrix4fv(this.uProjView, false, projView);
    gl.uniformMatrix4fv(this.uModel, false, model);
    gl.uniform1f(this.uWire, wire ? 1 : 0);
    const baseId = this.pickAnim(c.tex, c.animFreq, time);
    const baseTex = baseId !== null ? this.textures.get(baseId) : undefined;
    if (baseTex) {
      gl.activeTexture(gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, baseTex);
      gl.uniform1f(this.uHasTexture, 1);
    } else {
      gl.uniform1f(this.uHasTexture, 0);
    }
    gl.uniform1f(this.uLit, c.lit ? 1 : 0);
    gl.uniform2f(this.uScroll, c.scroll[0], c.scroll[1]);
    gl.uniform1f(this.uTime, time);
    this.setBlend(wire ? "opaque" : c.blend);
    // In wireframe the bound IBO is the line buffer, whose range is 2x the
    // triangle range in the same order.
    const first = wire ? c.first * 2 : c.first;
    const count = wire ? c.count * 2 : c.count;
    gl.drawElements(primitive, count, gl.UNSIGNED_INT, first * 4);

    // Overlay passes (additive / blended / multiply). Skipped in wireframe.
    if (wire) return;
    for (const o of c.overlays) {
      const otex = this.textures.get(o.tex);
      if (!otex) continue;
      gl.useProgram(this.fxProgram);
      gl.uniformMatrix4fv(this.fxProjView, false, projView);
      gl.uniformMatrix4fv(this.fxModel, false, model);
      gl.activeTexture(gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, otex);
      gl.uniform2f(this.fxScroll, o.scroll[0], o.scroll[1]);
      gl.uniform1f(this.fxTime, time);
      this.setBlend(o.blend);
      gl.drawElements(gl.TRIANGLES, c.count, gl.UNSIGNED_INT, c.first * 4);
    }
  }
}

/** Build a `gl.LINES` index buffer (3 edges per triangle) from triangle indices. */
function buildLineIndices(tris: Uint32Array): Uint32Array {
  const lines = new Uint32Array((tris.length / 3) * 6);
  let o = 0;
  for (let i = 0; i + 2 < tris.length; i += 3) {
    const a = tris[i];
    const b = tris[i + 1];
    const c = tris[i + 2];
    lines[o++] = a;
    lines[o++] = b;
    lines[o++] = b;
    lines[o++] = c;
    lines[o++] = c;
    lines[o++] = a;
  }
  return lines;
}

// 4x4 matrix helpers (column-major floats). Kept minimal and inlined.
export function perspective(fovYRad: number, aspect: number, near: number, far: number): Float32Array {
  const f = 1 / Math.tan(fovYRad / 2);
  const out = new Float32Array(16);
  out[0] = f / aspect; out[5] = f;
  out[10] = (far + near) / (near - far);
  out[11] = -1;
  out[14] = (2 * far * near) / (near - far);
  return out;
}

export function lookAt(eye: [number, number, number], yaw: number, pitch: number): Float32Array {
  // Build a view matrix from eye + euler (yaw around Z-up in our space).
  const cy = Math.cos(yaw), sy = Math.sin(yaw);
  const cp = Math.cos(pitch), sp = Math.sin(pitch);
  // forward
  const fx = cy * cp, fy = sy * cp, fz = sp;
  // right
  const rx = sy, ry = -cy, rz = 0;
  // up = right x forward
  const ux = ry * fz - rz * fy;
  const uy = rz * fx - rx * fz;
  const uz = rx * fy - ry * fx;
  const m = new Float32Array(16);
  m[0] = rx; m[1] = ux; m[2] = -fx; m[3] = 0;
  m[4] = ry; m[5] = uy; m[6] = -fy; m[7] = 0;
  m[8] = rz; m[9] = uz; m[10] = -fz; m[11] = 0;
  m[12] = -(rx * eye[0] + ry * eye[1] + rz * eye[2]);
  m[13] = -(ux * eye[0] + uy * eye[1] + uz * eye[2]);
  m[14] = -(-fx * eye[0] - fy * eye[1] - fz * eye[2]);
  m[15] = 1;
  return m;
}

export function multiply(a: Float32Array, b: Float32Array): Float32Array {
  const o = new Float32Array(16);
  for (let c = 0; c < 4; c++)
    for (let r = 0; r < 4; r++) {
      let s = 0;
      for (let k = 0; k < 4; k++) s += a[k * 4 + r] * b[c * 4 + k];
      o[c * 4 + r] = s;
    }
  return o;
}
