/**
 * Bloom's mip chain on the GPU: the downsample and upsample materials, the targets of one view's
 * chain, and the glare sources' buffer (plan R07, T14.b, Design note 12).
 *
 * @remarks
 * Each pass is a material drawn on a full-screen triangle into the next level's own
 * `rgba16float` target (one target per level, R01's `createRenderTarget`): down from the view's
 * HDR colour, which is an argument (decision 2026-10-02, item 1), to the coarsest level, then up
 * to level 1. The last step, w₀ excess(L) + up(U₁), and every glare source's veil run in the
 * tone-mapping pass (`glare.wgsl`). `rgba16float` throughout: R01's probe reports toward-zero
 * rounding for `rg11b10ufloat` on both the UHD 620 and the RTX 3080, which the CPU twin models.
 * Every pass is timed under {@link BLOOM_PASS}.
 */

import FRAME_WGSL from "../shaders/frame.wgsl?raw";
import { BUFFER_USAGE } from "../engine/gpuFlags";
import type {
  BufferHandle,
  DrawItem,
  MeshHandle,
  RenderEngine,
  RenderTarget,
  TextureHandle,
  ViewSize,
  WgslMaterialSpec,
} from "../engine/types";
import { HDR_COLOUR_FORMAT } from "../photometry/toneCurve";
import { type BloomKernel, levelWeight, mipSize } from "./bloom";
import BLOOM_DOWN_WGSL from "./bloomDown.wgsl?raw";
import BLOOM_UP_WGSL from "./bloomUp.wgsl?raw";
import {
  type GlareSource,
  type GlareSpreadTerms,
  GLARE_POISSON_TERMS,
  glareSourceSolidAngleSr,
  POINT_SOURCE_FRACTION,
  rectangleInsideLevel,
} from "./glare";

/** The pass label every bloom pass is timed under (R07's `PHOTOREAL_PASS_LABELS.bloom`). */
export const BLOOM_PASS = "bloom";

const DOWN_SOURCE = `${FRAME_WGSL}\n${BLOOM_DOWN_WGSL}`;
const UP_SOURCE = `${FRAME_WGSL}\n${BLOOM_UP_WGSL}`;

/** The downsample, Jimenez's 13 taps, thresholding on its first pass. */
export const BLOOM_DOWN_MATERIAL: WgslMaterialSpec = {
  name: "bloom down",
  displayName: "GLARE DOWNSAMPLE",
  vertexWgsl: DOWN_SOURCE,
  fragmentWgsl: DOWN_SOURCE,
  uniforms: [{ name: "threshold", type: "f32" }],
  samplers: [],
  textures: [{ name: "source", binding: 0 }],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** The upsample, the 3 × 3 tent, adding each level at its weight. */
export const BLOOM_UP_MATERIAL: WgslMaterialSpec = {
  name: "bloom up",
  displayName: "GLARE UPSAMPLE",
  vertexWgsl: UP_SOURCE,
  fragmentWgsl: UP_SOURCE,
  uniforms: [
    { name: "levelWeight", type: "f32" },
    { name: "coarseWeight", type: "f32" },
  ],
  samplers: [],
  textures: [
    { name: "level", binding: 0 },
    { name: "coarse", binding: 1 },
  ],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/** A full-screen triangle in clip space, which the bloom materials pass through. */
export function fullScreenTriangle(
  engine: Pick<RenderEngine, "createMesh">,
  name: string,
): MeshHandle {
  return engine.createMesh({
    name,
    positions: new Float32Array([-1, -1, 0, 3, -1, 0, -1, 3, 0]),
    indices: null,
    topology: "triangle-list",
    attributes: {},
  });
}

/** The identity, column-major: the bloom passes read no camera. */
const IDENTITY = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
const NO_OFFSET = new Float32Array(3);

/** The size of mip level `level` of an image of `size`. */
export function levelSize(size: ViewSize, level: number): ViewSize {
  let { widthPx, heightPx } = size;
  for (let m = 0; m < level; m += 1) {
    widthPx = mipSize(widthPx);
    heightPx = mipSize(heightPx);
  }
  return { widthPx, heightPx };
}

/**
 * One view's bloom chain: its down targets for levels 1 to the coarsest, and its up targets for
 * levels 1 to the one below the coarsest.
 */
export class BloomChain {
  readonly #engine: RenderEngine;
  readonly #name: string;
  readonly #down: RenderTarget[] = [];
  readonly #up: RenderTarget[] = [];
  readonly #mesh: MeshHandle;
  readonly #downMaterial;
  readonly #upMaterial;
  #kernel: BloomKernel;
  #size: ViewSize;

  private constructor(
    engine: RenderEngine,
    name: string,
    size: ViewSize,
    kernel: BloomKernel,
    materials: { readonly down: DrawItem["material"]; readonly up: DrawItem["material"] },
    mesh: MeshHandle,
  ) {
    this.#engine = engine;
    this.#name = name;
    this.#size = size;
    this.#kernel = kernel;
    this.#downMaterial = materials.down;
    this.#upMaterial = materials.up;
    this.#mesh = mesh;
    this.#makeTargets();
  }

  /** Makes a view's chain with its pipelines compiled, so that no frame waits on a compile. */
  static async create(
    engine: RenderEngine,
    name: string,
    size: ViewSize,
    kernel: BloomKernel,
  ): Promise<BloomChain> {
    const mesh = fullScreenTriangle(engine, `${name} bloom triangle`);
    const [down, up] = await Promise.all([
      engine.createMaterialAsync(BLOOM_DOWN_MATERIAL, [HDR_COLOUR_FORMAT], [mesh]),
      engine.createMaterialAsync(BLOOM_UP_MATERIAL, [HDR_COLOUR_FORMAT], [mesh]),
    ]);
    return new BloomChain(engine, name, size, kernel, { down, up }, mesh);
  }

  /** The coarsest level the kernel weights. */
  get lastLevel(): number {
    return this.#kernel.firstLevel + this.#kernel.levels - 1;
  }

  /** Level 1's up target, U₁, which the tone-mapping pass tent-upsamples. */
  get levelOne(): TextureHandle {
    const level = this.lastLevel === 1 ? this.#down[0] : this.#up[0];
    if (level === undefined) {
      throw new Error(`bloom chain ${this.#name} has no level 1`);
    }
    return level.colour;
  }

  /** U₁'s weight in the last step: 1, or the coarsest level's weight when that is level 1. */
  get levelOneWeight(): number {
    return this.lastLevel === 1 ? levelWeight(this.#kernel, 1) : 1;
  }

  /** The kernel the chain runs. */
  get kernel(): BloomKernel {
    return this.#kernel;
  }

  /** Follows the view's internal resolution. */
  resize(size: ViewSize): void {
    if (size.widthPx === this.#size.widthPx && size.heightPx === this.#size.heightPx) {
      return;
    }
    this.#size = size;
    for (const [index, target] of this.#down.entries()) {
      target.resize(levelSize(size, index + 1));
    }
    for (const [index, target] of this.#up.entries()) {
      target.resize(levelSize(size, index + 1));
    }
  }

  /** Takes a new kernel; a change of levels remakes the targets. */
  setKernel(kernel: BloomKernel): void {
    const levelsChanged =
      kernel.firstLevel + kernel.levels !== this.#kernel.firstLevel + this.#kernel.levels;
    this.#kernel = kernel;
    if (levelsChanged) {
      this.#disposeTargets();
      this.#makeTargets();
    }
  }

  /**
   * Runs the chain over `hdrColour`, the view's HDR colour at the chain's size.
   *
   * @param thresholdPreExposed - The threshold in the target's units (`bloomThreshold`).
   */
  run(hdrColour: TextureHandle, thresholdPreExposed: number): void {
    const last = this.lastLevel;
    for (let m = 1; m <= last; m += 1) {
      const target = this.#down[m - 1];
      const source = m === 1 ? hdrColour : this.#down[m - 2]?.colour;
      if (target === undefined || source === undefined) {
        throw new Error(`bloom chain ${this.#name} has no level ${m}`);
      }
      target.render(
        this.#frame({
          material: this.#downMaterial,
          uniforms: { threshold: new Float32Array([m === 1 ? thresholdPreExposed : 0]) },
          textures: { source },
        }),
      );
    }
    for (let m = last - 1; m >= 1; m -= 1) {
      const target = this.#up[m - 1];
      const level = this.#down[m - 1]?.colour;
      const coarse = m === last - 1 ? this.#down[last - 1]?.colour : this.#up[m]?.colour;
      if (target === undefined || level === undefined || coarse === undefined) {
        throw new Error(`bloom chain ${this.#name} has no level ${m}`);
      }
      target.render(
        this.#frame({
          material: this.#upMaterial,
          uniforms: {
            levelWeight: new Float32Array([levelWeight(this.#kernel, m)]),
            coarseWeight: new Float32Array([m === last - 1 ? levelWeight(this.#kernel, last) : 1]),
          },
          textures: { level, coarse },
        }),
      );
    }
  }

  /** Releases the chain's targets. */
  dispose(): void {
    this.#disposeTargets();
  }

  #frame(
    draw: Pick<DrawItem, "material" | "uniforms" | "textures">,
  ): Parameters<RenderTarget["render"]>[0] {
    return {
      label: BLOOM_PASS,
      viewRotation: IDENTITY,
      projection: IDENTITY,
      draws: [{ mesh: this.#mesh, offsetFromCameraM: NO_OFFSET, ...draw }],
      postProcesses: [],
    };
  }

  #makeTargets(): void {
    const last = this.lastLevel;
    for (let m = 1; m <= last; m += 1) {
      this.#down.push(this.#target(`${this.#name}:bloom down ${m}`, m));
    }
    for (let m = 1; m < last; m += 1) {
      this.#up.push(this.#target(`${this.#name}:bloom up ${m}`, m));
    }
  }

  #target(name: string, level: number): RenderTarget {
    return this.#engine.createRenderTarget({
      name,
      size: levelSize(this.#size, level),
      format: HDR_COLOUR_FORMAT,
      mips: 1,
      depth: false,
      category: "render-targets",
    });
  }

  #disposeTargets(): void {
    for (const target of [...this.#down, ...this.#up]) {
      target.dispose();
    }
    this.#down.length = 0;
    this.#up.length = 0;
  }
}

/** Bytes of one source in {@link packGlareSources}'s layout: three `vec4f`. */
export const GLARE_SOURCE_BYTES = 48;

/** Floats of one source in {@link packGlareSources}'s layout. */
const GLARE_SOURCE_FLOATS = GLARE_SOURCE_BYTES / 4;

/**
 * The glare sources as the tone-mapping pass's storage buffer holds them (`GlareSourceGpu` in
 * `glare.wgsl`): direction and angular radius, then the excess in the target's units (cd/m² × the
 * pre-exposure) and the solid angle, then each narrow term's level inside the disc.
 *
 * @param preExposure - The target's pre-exposure scale, 1 ÷ (cd/m²).
 * @param terms - The view's spread function, whose narrow terms set the levels inside each disc.
 */
export function packGlareSources(
  sources: ReadonlyArray<GlareSource>,
  preExposure: number,
  terms: GlareSpreadTerms,
): Float32Array<ArrayBuffer> {
  const out = new Float32Array(Math.max(1, sources.length) * GLARE_SOURCE_FLOATS);
  sources.forEach((source, i) => {
    const { direction: d, excessLuminance: e } = source;
    const rho = source.angularRadiusRad;
    const inside = (k: number): number => {
      const term = terms.poisson[k];
      return term === undefined || rho < POINT_SOURCE_FRACTION * term.scaleRad
        ? 0
        : rectangleInsideLevel(rho, term.scaleRad);
    };
    out.set(
      [
        d.x,
        d.y,
        d.z,
        rho,
        e[0] * preExposure,
        e[1] * preExposure,
        e[2] * preExposure,
        glareSourceSolidAngleSr(source),
        inside(0),
        inside(1),
        inside(2),
        0,
      ],
      GLARE_SOURCE_FLOATS * i,
    );
  });
  return out;
}

/**
 * A view's spread function as the `GlareTerms` uniforms of `glare.wgsl`: three narrow terms, the
 * broad terms, and the rest. Absent terms take amplitude 0 and scale 1, so that no division is by 0.
 */
export function packGlareTerms(terms: GlareSpreadTerms): Readonly<Record<string, Float32Array>> {
  const poisson = (i: number): Float32Array => {
    const term = terms.poisson[i];
    return new Float32Array([term?.amplitude ?? 0, term?.scaleRad ?? 1, 0, 0]);
  };
  if (terms.poisson.length > GLARE_POISSON_TERMS) {
    throw new Error(`a spread function has at most ${GLARE_POISSON_TERMS} narrow terms`);
  }
  const lorentz = terms.lorentz[0];
  const root = terms.root[0];
  return {
    glarePoisson0: poisson(0),
    glarePoisson1: poisson(1),
    glarePoisson2: poisson(2),
    glareBroad: new Float32Array([
      lorentz?.amplitude ?? 0,
      lorentz?.scaleRad ?? 1,
      root?.amplitude ?? 0,
      root?.scaleRad ?? 1,
    ]),
    glareMisc: new Float32Array([
      terms.quadratic,
      terms.constant,
      terms.gaussian.amplitude,
      terms.gaussian.sigmaRad,
    ]),
  };
}

/** A storage buffer for up to `capacity` glare sources. */
export function createGlareSourceBuffer(
  engine: Pick<RenderEngine, "createBuffer">,
  name: string,
  capacity: number,
): BufferHandle {
  return engine.createBuffer({
    name: `${name} glare sources`,
    bytes: Math.max(1, capacity) * GLARE_SOURCE_BYTES,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
}
