// Thin WebGL2 renderer for the static world (lightmapped BSP) plus dynamic
// entities. Kept allocation-light for the 360fps target: buffers are created
// once per map and reused.

export class Renderer {
  gl: WebGL2RenderingContext;
  private program: WebGLProgram;
  private vao: WebGLVertexArrayObject;
  private vbo: WebGLBuffer;
  private ibo: WebGLBuffer;
  private indexCount = 0;
  private uProjView: WebGLUniformLocation;
  private uHasTexture: WebGLUniformLocation;
  private lightmapTex: WebGLTexture;
  private whiteTex: WebGLTexture;
  private textures: Map<number, WebGLTexture>;
  private texCount: number;
  /** Draw chunks: (firstIndex, indexCount, textureId). */
  private chunks: Array<{ first: number; count: number; tex: number }> = [];

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
      out vec4 outColor;
      void main() {
        // Surface albedo (from texture) or a neutral gray fallback.
        vec3 albedo = mix(vec3(0.62), texture(u_texture, v_uv).rgb, u_has_texture);
        // Baked lightmap: stored dark, meant to multiply the albedo.
        vec3 lm = texture(u_lightmap, v_lm).rgb;
        // Overbright in the style of Q3 (r_overbrightBits ~2) + gamma lift.
        vec3 base = albedo * lm * 4.0;
        base = pow(clamp(base, 0.0, 1.0), vec3(0.72));
        outColor = vec4(base, 1.0);
      }`;

    this.program = this.link(vs, fs);
    gl.useProgram(this.program);

    this.uProjView = gl.getUniformLocation(this.program, "u_proj_view")!;
    this.uHasTexture = gl.getUniformLocation(this.program, "u_has_texture")!;
    const uLightmap = gl.getUniformLocation(this.program, "u_lightmap");
    const uTexture = gl.getUniformLocation(this.program, "u_texture");
    if (uLightmap) gl.uniform1i(uLightmap, 0);
    if (uTexture) gl.uniform1i(uTexture, 1);

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

  /** Set the draw chunks (first index, count, texture id) for this map. */
  setChunks(chunks: Array<{ first: number; count: number; tex: number }>) {
    this.chunks = chunks;
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

  draw(projView: Float32Array) {
    const gl = this.gl;
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    if (this.indexCount === 0) return;
    gl.useProgram(this.program);
    gl.uniformMatrix4fv(this.uProjView, false, projView);
    gl.bindVertexArray(this.vao);

    if (this.chunks.length === 0) {
      // Fallback: draw everything with no texture.
      gl.uniform1f(this.uHasTexture, 0);
      gl.drawElements(gl.TRIANGLES, this.indexCount, gl.UNSIGNED_INT, 0);
    } else {
      for (const c of this.chunks) {
        const tex = this.textures.get(c.tex);
        if (tex) {
          gl.activeTexture(gl.TEXTURE1);
          gl.bindTexture(gl.TEXTURE_2D, tex);
          gl.uniform1f(this.uHasTexture, 1);
        } else {
          gl.uniform1f(this.uHasTexture, 0);
        }
        gl.drawElements(gl.TRIANGLES, c.count, gl.UNSIGNED_INT, c.first * 4);
      }
    }
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
