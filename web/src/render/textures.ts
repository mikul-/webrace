// Texture loading + TGA decoding for map surfaces.
//
// Shader names like "textures/base_wall/basewall01_owfx" resolve to image
// files in the pk3 archives; the map-server serves them at /tex/<name>.
// Browser can decode jpg/png natively; TGA needs a small manual decoder.

import { api } from "../base";

export interface DecodedImage {
  width: number;
  height: number;
  /** RGBA8 pixel data, row-major top-to-bottom. */
  data: Uint8ClampedArray | Uint8Array;
}

/** Load a shader name's texture image from the map-server and decode it. */
export async function loadTexture(shaderName: string): Promise<DecodedImage | null> {
  const url = api("/tex/" + encodeURIComponent(shaderName));
  try {
    const res = await fetch(url);
    if (!res.ok) {
      if (res.status === 404) return null;
      throw new Error(`texture ${shaderName}: ${res.status}`);
    }
    const bytes = new Uint8Array(await res.arrayBuffer());
    // Try native decode first (jpg/png/webp).
    try {
      const bmp = await createImageBitmap(new Blob([bytes]));
      const w = bmp.width;
      const h = bmp.height;
      const canvas = document.createElement("canvas");
      canvas.width = w;
      canvas.height = h;
      const ctx = canvas.getContext("2d")!;
      ctx.drawImage(bmp, 0, 0);
      const img = ctx.getImageData(0, 0, w, h);
      bmp.close();
      return { width: w, height: h, data: img.data };
    } catch {
      // Not a browser-decodable format; try TGA.
      const tga = decodeTga(bytes);
      if (tga) return tga;
      return null;
    }
  } catch (e) {
    console.warn("texture load failed:", shaderName, e);
    return null;
  }
}

/** Decode a TGA image (uncompressed or RLE, 24/32-bit). */
function decodeTga(bytes: Uint8Array): DecodedImage | null {
  if (bytes.length < 18) return null;
  const idLen = bytes[0];
  const colorMapType = bytes[1];
  const imageType = bytes[2];
  const width = bytes[12] | (bytes[13] << 8);
  const height = bytes[14] | (bytes[15] << 8);
  const bpp = bytes[16];
  const descriptor = bytes[17];

  if (colorMapType !== 0) return null; // no color maps supported
  const type = imageType; // 2 = uncompressed truecolor, 10 = RLE truecolor
  const bpc = bpp / 8; // bytes per pixel (3 or 4)
  if (bpc !== 3 && bpc !== 4) return null;

  let p = 18 + idLen;
  const dataSize = width * height * bpc;
  const out = new Uint8Array(width * height * 4);

  const flip = (descriptor & 0x20) === 0; // bit 5 clear => bottom-up (flip to top-down)

  if (type === 2) {
    // Uncompressed.
    for (let i = 0; i < dataSize; i += bpc) {
      const b = bytes[p++], g = bytes[p++], r = bytes[p++];
      let a = 255;
      if (bpc === 4) a = bytes[p++];
      const idx = i / bpc;
      writePixel(out, idx, width, height, flip, r, g, b, a);
    }
    return { width, height, data: out };
  }

  if (type === 10) {
    // RLE.
    let pixel = 0;
    while (pixel < width * height) {
      const header = bytes[p++];
      const count = (header & 0x7f) + 1;
      if (header & 0x80) {
        // Run-length packet.
        const b = bytes[p++], g = bytes[p++], r = bytes[p++];
        let a = 255;
        if (bpc === 4) a = bytes[p++];
        for (let i = 0; i < count; i++) {
          writePixel(out, pixel++, width, height, flip, r, g, b, a);
        }
      } else {
        // Raw packet.
        for (let i = 0; i < count; i++) {
          const b = bytes[p++], g = bytes[p++], r = bytes[p++];
          let a = 255;
          if (bpc === 4) a = bytes[p++];
          writePixel(out, pixel++, width, height, flip, r, g, b, a);
        }
      }
    }
    return { width, height, data: out };
  }

  return null;
}

function writePixel(
  out: Uint8Array,
  pixel: number,
  width: number,
  height: number,
  flip: boolean,
  r: number,
  g: number,
  b: number,
  a: number,
) {
  const x = pixel % width;
  const y = flip ? height - 1 - Math.floor(pixel / width) : Math.floor(pixel / width);
  const o = (y * width + x) * 4;
  out[o] = r;
  out[o + 1] = g;
  out[o + 2] = b;
  out[o + 3] = a;
}
