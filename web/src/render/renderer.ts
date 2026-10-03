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

export class Renderer {
  gl: WebGL2RenderingContext;
  private program: WebGLProgram;
  private vao: WebGLVertexArrayObject;
  private vbo: WebGLBuffer;
  private ibo: WebGLBuffer;
  private indexCount = 0;
  private uProjView: WebGLUniformLocation;
  private uHasTexture: WebGLUniformLocation;
  private uLit: WebGLUniformLocation;
  private uScroll: WebGLUniformLocation;
  private uTime: WebGLUniformLocation;
  // Overlay (FX) program.
  private fxProgram: WebGLProgram;
  private fxProjView: WebGLUniformLocation;
  private fxScroll: WebGLUniformLocation;
  private fxTime: WebGLUniformLocation;
  private lightmapTex: WebGLTexture;
  private whiteTex: WebGLTexture;
  private textures: Map<number, WebGLTexture>;
  private texCount: number;
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

    const vs = `#version 300 es
      layout(location=0) in vec3 a_pos;
      layout(location=1) in vec2 a_uv;
      layout(location=2) in vec2 a_lm;
      layout(location=3) in vec3 a_norm;
      layout(location=4) in vec4 a_color;
      uniform mat4 u_proj_view;
      out vec2 v_uv; out vec2 v_lm; out vec3 v_norm; out vec4 v_color;
      void main() {
        gl_Position = u_proj_view * vec4(a_pos, 1.0);
        v_uv = a_uv; v_lm = a_lm; v_norm = a_norm; v_color = a_color;
      }`;
    const fs = `#version 300 es
      precision highp float;
      in vec2 v_uv; in vec2 v_lm; in vec3 v_norm; in vec4 v_color;
      uniform sampler2D u_texture;
      uniform sampler2D u_lightmap;
      uniform float u_has_texture;
      uniform float u_lit;
      uniform vec2 u_scroll;
      uniform float u_time;
      out vec4 outColor;
      void main() {
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
    this.uHasTexture = gl.getUniformLocation(this.program, "u_has_texture")!;
    this.uLit = gl.getUniformLocation(this.program, "u_lit")!;
    this.uScroll = gl.getUniformLocation(this.program, "u_scroll")!;
    this.uTime = gl.getUniformLocation(this.program, "u_time")!;
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
      out vec2 v_uv; out vec4 v_color;
      void main() {
        gl_Position = u_proj_view * vec4(a_pos, 1.0);
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
    this.fxScroll = gl.getUniformLocation(this.fxProgram, "u_scroll")!;
    this.fxTime = gl.getUniformLocation(this.fxProgram, "u_time")!;
    gl.useProgram(this.fxProgram);
    const fxTex = gl.getUniformLocation(this.fxProgram, "u_texture");
    if (fxTex) gl.uniform1i(fxTex, 1);
    gl.useProgram(this.program);

    // Lightmap atlas texture (unit 0).
    this.lightmapTex = gl.createTexture()!;
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.lightmapTex);
    setTexParams(gl);

    // Texture cache (unit 1): shader-index -> WebGLTexture.
    this.textures = new Map();
    this.texCount = 0;
    // Default 1x1 white texture for faces whose image failed to load.
    this.whiteTex = gl.createTexture()!;
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, this.whiteTex);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([158, 158, 158, 255]));

    this.vao = gl.createVertexArray()!;
    this.vbo = gl.createBuffer()!;
    this.ibo = gl.createBuffer()!;

    gl.bindVertexArray(this.vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.vbo);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.ibo);

    // interleaved: 14 floats = pos(3) uv(2) lm(2) normal(3) color(4)
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

    gl.enable(gl.DEPTH_TEST);
    // Culling disabled for now: the q2t reflection may flip winding, and
    // Quake BSP front-face convention differs from WebGL's default. Re-enable
    // once winding is verified.
    // gl.enable(gl.CULL_FACE);
    gl.clearColor(0.04, 0.05, 0.07, 1);
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
    this.skyFaceTextures = faces.map((img) => {
      if (!img) return null;
      const tex = gl.createTexture()!;
      gl.bindTexture(gl.TEXTURE_2D, tex);
      setTexParams(gl);
      gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, img.width, img.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, img.data);
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
    setTexParams(gl);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, img.width, img.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, img.data);
    this.textures.set(id, tex);
    return id;
  }

  draw(projView: Float32Array, eye: [number, number, number] = [0, 0, 0], time = 0) {
    const gl = this.gl;
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    // Skybox first: it writes depth far away, so the world draws over it.
    this.drawSkybox(projView, eye);
    if (this.indexCount === 0) return;
    gl.bindVertexArray(this.vao);

    if (this.chunks.length === 0) {
      gl.useProgram(this.program);
      gl.uniformMatrix4fv(this.uProjView, false, projView);
      gl.uniform1f(this.uHasTexture, 0);
      gl.uniform1f(this.uLit, 1);
      gl.uniform2f(this.uScroll, 0, 0);
      gl.uniform1f(this.uTime, time);
      this.setBlend("opaque");
      gl.drawElements(gl.TRIANGLES, this.indexCount, gl.UNSIGNED_INT, 0);
      gl.bindVertexArray(null);
      return;
    }

    for (const c of this.chunks) {
      // Base pass.
      gl.useProgram(this.program);
      gl.uniformMatrix4fv(this.uProjView, false, projView);
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
      this.setBlend(c.blend);
      gl.drawElements(gl.TRIANGLES, c.count, gl.UNSIGNED_INT, c.first * 4);

      // Overlay passes (additive / blended / multiply).
      for (const o of c.overlays) {
        const otex = this.textures.get(o.tex);
        if (!otex) continue;
        gl.useProgram(this.fxProgram);
        gl.uniformMatrix4fv(this.fxProjView, false, projView);
        gl.activeTexture(gl.TEXTURE1);
        gl.bindTexture(gl.TEXTURE_2D, otex);
        gl.uniform2f(this.fxScroll, o.scroll[0], o.scroll[1]);
        gl.uniform1f(this.fxTime, time);
        this.setBlend(o.blend);
        gl.drawElements(gl.TRIANGLES, c.count, gl.UNSIGNED_INT, c.first * 4);
      }
    }
    gl.disable(gl.BLEND);
    gl.bindVertexArray(null);
  }
}

function setTexParams(gl: WebGL2RenderingContext) {
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.REPEAT);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.REPEAT);
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
