/**
 * The baked star cube's draw, in its two variants (plan R06, T13.g; decision record item 1 of
 * 2026-10-02): the wireframe's display variant, toned per pixel by R02's `agxSprite` straight to
 * the canvas as its sprites are, and the HDR variant, linear, into R07.T7's scene target.
 *
 * @remarks
 * Both sample the `rgb9e5ufloat` cube trilinearly through a linear sampler and divide out the
 * bake's power of two, read from the bake's peak buffer as `bakeCommon.wgsl`'s `scaleExponent`.
 * The cube is drawn at infinity before the lines and the sprites, depth-tested so that a body's
 * occluder hides it.
 */

import type {
  DrawItem,
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  WgslMaterialSpec,
} from "../engine/types";
import frameWgsl from "../shaders/frame.wgsl?raw";
import toneCurveWgsl from "../shaders/toneCurve.wgsl?raw";
import type { BakedCube } from "./bake";
import bakeCommonWgsl from "./shaders/bakeCommon.wgsl?raw";
import cubeWgsl from "./shaders/cube.wgsl?raw";

/** The display variant's tone: R02's per-sprite curve. */
const DISPLAY_TONE = `
fn cubeTone(rgb : vec3f) -> vec3f {
  return agxSprite(rgb);
}
`;

/** The HDR variant's tone: none, the scene target holds linear light. */
const HDR_TONE = `
fn cubeTone(rgb : vec3f) -> vec3f {
  return rgb;
}
`;

/** A cube material composed with its tone step. */
function cubeMaterial(name: string, displayName: string, prefix: string): WgslMaterialSpec {
  const source = frameWgsl + bakeCommonWgsl + prefix + cubeWgsl;
  return {
    name,
    displayName,
    vertexWgsl: source,
    fragmentWgsl: source,
    uniforms: [{ name: "exposure", type: "vec4f" }],
    samplers: [{ name: "starsSampler", filter: "linear", address: "clamp-to-edge", binding: 1 }],
    textures: [{ name: "stars", binding: 0, viewDimension: "cube" }],
    storageBuffers: [{ name: "peak", binding: 2 }],
    cullMode: "none",
    depthWrite: false,
    colourWrites: true,
    blend: "additive",
  };
}

/** The cube toned per pixel onto the wireframe's canvas. */
export const CUBE_DISPLAY_MATERIAL = cubeMaterial(
  "sky:cubeDisplay",
  "BAKED STARS",
  toneCurveWgsl + DISPLAY_TONE,
);

/** The cube, linear, into the HDR scene target. */
export const CUBE_HDR_MATERIAL = cubeMaterial("sky:cubeHdr", "BAKED STARS HDR", HDR_TONE);

/** The layer's GPU objects on one device. */
interface CubeLayerResources {
  readonly display: MaterialHandle;
  readonly hdr: MaterialHandle;
  readonly triangle: MeshHandle;
}

/** A baked cube's draws on one engine, made again after a device loss. */
export class SkyCubeLayer {
  readonly #engine: RenderEngine;
  #resources: CubeLayerResources;
  readonly #unsubscribe: () => void;

  constructor(engine: RenderEngine) {
    this.#engine = engine;
    this.#resources = this.#make();
    // The handles died with a lost device: the restore makes them again (R01 Design note 9).
    this.#unsubscribe = engine.onRestored(() => {
      this.#resources = this.#make();
    });
  }

  #make(): CubeLayerResources {
    return {
      display: this.#engine.createMaterial(CUBE_DISPLAY_MATERIAL),
      hdr: this.#engine.createMaterial(CUBE_HDR_MATERIAL),
      triangle: this.#engine.createMesh({
        name: "sky cube triangle",
        positions: new Float32Array([-1, -1, 0, 3, -1, 0, -1, 3, 0]),
        indices: null,
        topology: "triangle-list",
        attributes: {},
      }),
    };
  }

  /** Stops following the engine's restores. */
  dispose(): void {
    this.#unsubscribe();
  }

  /**
   * The cube's draw for a frame.
   *
   * @param variant - `display` for the wireframe's canvas, `hdr` for the scene target.
   * @param exposureScale - The frame's pre-exposure scale (R02's `exposureScale`).
   */
  draw(baked: BakedCube, variant: "display" | "hdr", exposureScale: number): DrawItem {
    return {
      mesh: this.#resources.triangle,
      material: variant === "display" ? this.#resources.display : this.#resources.hdr,
      offsetFromCameraM: new Float32Array(3),
      uniforms: { exposure: new Float32Array([exposureScale, 0, 0, 0]) },
      textures: { stars: baked.cube },
      storageBuffers: { peak: baked.peak },
    };
  }
}
