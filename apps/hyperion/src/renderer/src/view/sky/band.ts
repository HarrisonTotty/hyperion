/**
 * The sky's band layer (plan R06, Design notes 15 and 20, T13.d): the band map as a small
 * `rgba16float` cube, the light of the stars a view culled added in, drawn first into the HDR
 * scene target by a full-screen draw.
 *
 * @remarks
 * The server's band holds the light of every star it did not list; a view adds the light of the
 * listed stars it culled, each star's illuminance over its texel's solid angle, so that the view's
 * total light is kept. A texel's colour is its chromaticity at unit luminance times its luminance.
 * The band is drawn only in the photorealistic style (decision record item 1): its HDR draw is
 * the one variant, checked by the harness on a target the test makes until R07.T7 makes the
 * views' scene target. Values above a half's 65,504 are clamped at upload; a band never nears it
 * (the Milky Way is about 10⁻⁴ cd/m²).
 */

import type { SkyBand } from "@hyperion/protocol";

import { TEXTURE_USAGE } from "../engine/gpuFlags";
import type {
  DrawItem,
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  TextureHandle,
  WgslMaterialSpec,
} from "../engine/types";
import frameWgsl from "../shaders/frame.wgsl?raw";
import { texelSolidAnglesSr } from "./cube";
import { HALF_MAX, toHalfArray } from "./half";
import { unitLuminanceRgb } from "./photometry";
import bandWgsl from "./shaders/band.wgsl?raw";

/** The band's full-screen draw into the HDR scene target. */
export const BAND_MATERIAL: WgslMaterialSpec = {
  name: "sky:band",
  displayName: "STAR BAND",
  vertexWgsl: frameWgsl + bandWgsl,
  fragmentWgsl: frameWgsl + bandWgsl,
  uniforms: [{ name: "exposure", type: "vec4f" }],
  samplers: [{ name: "bandSampler", filter: "linear", address: "clamp-to-edge", binding: 1 }],
  textures: [{ name: "band", binding: 0, viewDimension: "cube" }],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "additive",
};

/**
 * The band's texels as `rgba` luminances, cd/m², face after face, rows from the top: the server's
 * band in its colour, and the culled stars' light over each texel's solid angle.
 *
 * @param faceTexels - The band's face side (the response's `face_texels`).
 * @param culledIlluminanceLx - Three floats a texel, the cull's `bandIlluminanceLx`.
 * @throws Error when the band or the culled light do not cover six faces of `faceTexels`².
 */
export function bandTexels(
  band: SkyBand,
  faceTexels: number,
  culledIlluminanceLx: Float64Array,
): Float32Array {
  const perFace = faceTexels * faceTexels;
  if (band.count !== 6 * perFace || culledIlluminanceLx.length !== 6 * perFace * 3) {
    throw new Error(`a band of ${band.count} texels is not six faces of ${faceTexels}²`);
  }
  const omegas = texelSolidAnglesSr(faceTexels);
  const texels = new Float32Array(band.count * 4);
  for (let texel = 0; texel < band.count; texel += 1) {
    const luminance = band.luminanceCdM2[texel] ?? 0;
    const colour = unitLuminanceRgb(band.chroma[texel * 2] ?? 0, band.chroma[texel * 2 + 1] ?? 0);
    const omega = omegas[texel % perFace] ?? 1;
    for (let channel = 0; channel < 3; channel += 1) {
      const culled = (culledIlluminanceLx[texel * 3 + channel] ?? 0) / omega;
      texels[texel * 4 + channel] = Math.min(HALF_MAX, luminance * (colour[channel] ?? 0) + culled);
    }
    texels[texel * 4 + 3] = 1;
  }
  return texels;
}

/** The band on the GPU: its cube, its material and its full-screen triangle. */
export class BandLayer {
  readonly #engine: RenderEngine;
  readonly #material: MaterialHandle;
  readonly #triangle: MeshHandle;
  #cube: TextureHandle | null = null;
  #faceTexels = 0;

  constructor(engine: RenderEngine) {
    this.#engine = engine;
    this.#material = engine.createMaterial(BAND_MATERIAL);
    this.#triangle = engine.createMesh({
      name: "sky band triangle",
      positions: new Float32Array([-1, -1, 0, 3, -1, 0, -1, 3, 0]),
      indices: null,
      topology: "triangle-list",
      attributes: {},
    });
  }

  /**
   * Uploads a band: the server's map with the culled stars' light, as half floats, into a cube
   * made once per face size.
   */
  update(band: SkyBand, faceTexels: number, culledIlluminanceLx: Float64Array): void {
    if (this.#cube === null || this.#faceTexels !== faceTexels) {
      if (this.#cube !== null) {
        this.#engine.releaseTexture(this.#cube);
      }
      this.#cube = this.#engine.createTexture({
        name: "sky band",
        size: { width: faceTexels, height: faceTexels, depthOrArrayLayers: 6 },
        dimension: "cube",
        format: "rgba16float",
        mips: 1,
        usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
        category: "other",
      });
      this.#faceTexels = faceTexels;
    }
    const halves = toHalfArray(bandTexels(band, faceTexels, culledIlluminanceLx));
    this.#engine.writeTexture(this.#cube, [0, 0, 0], [faceTexels, faceTexels, 6], halves);
  }

  /**
   * The band's draw for a frame into the HDR scene target, or `null` before a band is uploaded.
   *
   * @param exposureScale - The frame's pre-exposure scale (R02's `exposureScale`).
   */
  draw(exposureScale: number): DrawItem | null {
    if (this.#cube === null) {
      return null;
    }
    return {
      mesh: this.#triangle,
      material: this.#material,
      offsetFromCameraM: new Float32Array(3),
      uniforms: { exposure: new Float32Array([exposureScale, 0, 0, 0]) },
      textures: { band: this.#cube },
    };
  }

  /** Releases the band's cube. */
  dispose(): void {
    if (this.#cube !== null) {
      this.#engine.releaseTexture(this.#cube);
      this.#cube = null;
    }
  }
}
