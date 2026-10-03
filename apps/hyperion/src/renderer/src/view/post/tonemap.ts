/**
 * The photorealistic style's last pass: upscale, bloom's last step, the glare sources' veil, the
 * exposure, R02's AgX, the encoding and the dither, in one full-screen draw onto the canvas (plan
 * R07, T15, Design notes 9, 12 and 13).
 *
 * @remarks
 * The pass writes the canvas through its own non-sRGB view (`FrameSubmission.encoding`
 * `"in-pass"`, decision 2026-10-02, item 6) so that the dither, a static blue-noise TPDF of ±1 LSB,
 * is applied in the encoded domain. The curve is R02's AgX less its floor (`agxSprite`, Filament's
 * port with its `pow(v, 2.2)`), encoded here exactly as the sRGB store encodes the wireframe's
 * sprites: an isolated star on black is identical in both styles before the dither and within one
 * code after it. Cased symbology follows in a second canvas pass that loads this one's colour
 * (`FrameSubmission.colourLoad` `"load"`, T16).
 */

import FRAME_WGSL from "../shaders/frame.wgsl?raw";
import TONE_CURVE_WGSL from "../shaders/toneCurve.wgsl?raw";
import type {
  BufferHandle,
  DrawItem,
  MeshHandle,
  TextureHandle,
  WgslMaterialSpec,
} from "../engine/types";
import { type Rgb, spriteToneCurve } from "../photometry/toneCurve";
import GLARE_WGSL from "./glare.wgsl?raw";
import TONEMAP_WGSL from "./tonemap.wgsl?raw";

/** The pass label the tone-mapping pass is timed under (R07's `PHOTOREAL_PASS_LABELS.tonemap`). */
export const TONEMAP_PASS = "tonemap";

const SOURCE = `${FRAME_WGSL}\n${TONE_CURVE_WGSL}\n${GLARE_WGSL}\n${TONEMAP_WGSL}`;

/** The tone-mapping pass, as a material drawn on a full-screen triangle onto the canvas. */
export const TONEMAP_MATERIAL: WgslMaterialSpec = {
  name: "tone mapping",
  displayName: "IMAGE",
  vertexWgsl: SOURCE,
  fragmentWgsl: SOURCE,
  uniforms: [
    { name: "exposure", type: "f32" },
    { name: "threshold", type: "f32" },
    { name: "levelZeroWeight", type: "f32" },
    { name: "levelOneWeight", type: "f32" },
    { name: "sourceCount", type: "u32" },
    { name: "glarePoisson0", type: "vec4f" },
    { name: "glarePoisson1", type: "vec4f" },
    { name: "glarePoisson2", type: "vec4f" },
    { name: "glareBroad", type: "vec4f" },
    { name: "glareMisc", type: "vec4f" },
    { name: "dither", type: "f32" },
  ],
  samplers: [{ name: "hdrSampler", filter: "linear", address: "clamp-to-edge", binding: 1 }],
  textures: [
    { name: "hdrColour", binding: 0 },
    { name: "bloomUp", binding: 2 },
    { name: "blueNoise", binding: 3 },
  ],
  storageBuffers: [{ name: "glareSources", binding: 4 }],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** The sRGB encoding of a display-linear value (IEC 61966-2-1:1999), clamped to [0, 1]. */
export function srgbEncode(linear: number): number {
  const v = Math.min(1, Math.max(0, linear));
  return v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055;
}

/**
 * A triangular value in [−1, 1] from a uniform one in [0, 1], by the inverse of the triangular
 * distribution's cumulative function, which keeps a blue-noise tile's spectrum.
 */
export function tpdf(uniform: number): number {
  const v = Math.min(1, Math.max(0, uniform));
  return v < 0.5 ? Math.sqrt(2 * v) - 1 : 1 - Math.sqrt(2 * (1 - v));
}

/**
 * The pass's output for one texel's light, the CPU twin of `fragmentMain`'s last steps: AgX less
 * its floor of the exposed light, encoded, plus the dither in LSB.
 *
 * @param light - The texel's light in the target's units, after bloom and glare.
 * @param exposure - The exposure scale over the target's pre-exposure.
 * @param noise - The texel's TPDF value per channel, in [−1, 1], or `null` for no dither.
 * @returns The stored value per channel in [0, 1], before the format's rounding.
 */
export function tonemapTexel(light: Rgb, exposure: number, noise: Rgb | null): Rgb {
  const display = spriteToneCurve([light[0] * exposure, light[1] * exposure, light[2] * exposure]);
  const encode = (channel: 0 | 1 | 2): number =>
    Math.min(1, Math.max(0, srgbEncode(display[channel]) + (noise?.[channel] ?? 0) / 255));
  return [encode(0), encode(1), encode(2)];
}

/** What one frame's tone-mapping draw needs. */
export interface TonemapInputs {
  readonly mesh: MeshHandle;
  readonly material: DrawItem["material"];
  /** The view's HDR colour, at its internal resolution (R07.T7's scene target). */
  readonly hdrColour: TextureHandle;
  /** The bloom chain's level 1 (`BloomChain.levelOne`). */
  readonly bloomUp: TextureHandle;
  readonly blueNoise: TextureHandle;
  readonly glareSources: BufferHandle;
  readonly uniforms: Readonly<Record<string, Float32Array>>;
}

/** The tone-mapping pass's draw. */
export function tonemapDraw(inputs: TonemapInputs): DrawItem {
  return {
    mesh: inputs.mesh,
    material: inputs.material,
    offsetFromCameraM: new Float32Array(3),
    uniforms: inputs.uniforms,
    textures: {
      hdrColour: inputs.hdrColour,
      bloomUp: inputs.bloomUp,
      blueNoise: inputs.blueNoise,
    },
    storageBuffers: { glareSources: inputs.glareSources },
  };
}
