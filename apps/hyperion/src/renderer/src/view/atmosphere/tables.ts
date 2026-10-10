/**
 * The atmosphere's per-planet tables (plan R05, R05.T12.b, Design note 16): transmittance and
 * multiple scattering, built by compute kernels through the engine when the medium changes and
 * never when the sun moves, since both are per unit illuminance and depend on the medium alone.
 *
 * @remarks
 * Hillaire 2020's first two tables, at his sizes (Design note 16): transmittance 256 × 64 and
 * multiple scattering 32 × 32, the same on both settings. Each is a `presentation-only` kernel
 * writing an `rgba16float` storage texture made under the `atmosphere-tables` memory category; only
 * the smoke harness reads them back. The tables are spherical, on a shell whose ground is
 * `bottomRadiusM` from the centre; T12.c's lookups take r = √(MN) + h on the spheroid.
 */

import { TEXTURE_USAGE } from "../engine/gpuFlags";
import type { KernelPair } from "../engine/kernels";
import type { ComputeHandle, RenderEngine, TextureHandle } from "../engine/types";
import type { AtmosphereMedium, DensityProfile, PhaseFunction } from "./medium";
import commonWgsl from "./shaders/common.wgsl?raw";
import mediumWgsl from "./shaders/medium.wgsl?raw";
import multiScatteringWgsl from "./shaders/multiScattering.wgsl?raw";
import transmittanceWgsl from "./shaders/transmittance.wgsl?raw";

/** The most terms a medium may have: `MAX_TERMS` in `common.wgsl`. */
export const MAX_TERMS = 8;

/** A table's size, texels. */
export interface TableSize {
  readonly widthTexels: number;
  readonly heightTexels: number;
}

/** The transmittance table's size: 256 in μ by 64 in r (Hillaire 2020's code, Design note 16). */
export const TRANSMITTANCE_SIZE: TableSize = { widthTexels: 256, heightTexels: 64 };

/** The multiple-scattering table's size: 32 in the sun's μ by 32 in r (Hillaire 2020). */
export const MULTI_SCATTERING_SIZE: TableSize = { widthTexels: 32, heightTexels: 32 };

/**
 * Midpoint steps along each transmittance ray: 256.
 *
 * @remarks
 * The midpoint rule's error on e^(−t/H) is about (Δh/H)² ÷ 24 of a term's depth, Δh the height a
 * step climbs, and the aerosol's 1.2 km scale height is the finest structure. At 128 steps, a
 * shallow ray from the ground (μ = 0.11, 5 km steps climbing 0.56 km each) left the blue
 * transmittance 1.2% above the oracle on SwiftShader; 256 quarters the error. Hillaire's 40, sampled
 * at 0.3 of each step, misses the plan's 1% by more. The table is built only when the medium
 * changes, so the cost is paid rarely.
 */
export const TRANSMITTANCE_SAMPLES = 256;

/** Midpoint steps along each of the 64 multiple-scattering rays: 32 (sebh's reference takes 20). */
export const MULTI_SCATTERING_SAMPLES = 32;

/** The transmittance kernel: `common.wgsl`, `medium.wgsl`, then `transmittance.wgsl`. */
export const TRANSMITTANCE_KERNEL: KernelPair = {
  name: "atmosphere transmittance",
  reference: `${commonWgsl}\n${mediumWgsl}\n${transmittanceWgsl}`,
  subgroup: null,
  readback: "presentation-only",
};

/** The multiple-scattering kernel: `common.wgsl`, `medium.wgsl`, then `multiScattering.wgsl`. */
export const MULTI_SCATTERING_KERNEL: KernelPair = {
  name: "atmosphere multiple scattering",
  reference: `${commonWgsl}\n${mediumWgsl}\n${multiScatteringWgsl}`,
  subgroup: null,
  readback: "presentation-only",
};

/** Floats in `Medium`: four scalars, the ground albedo, and three `vec4f` per term. */
const MEDIUM_FLOATS = 4 + 4 + MAX_TERMS * 12;

/**
 * A phase function as `source.wgsl`'s `phaseOf` reads it, in `Term.scattering.w` and
 * `Term.absorption.w`: 0 none, 1 Rayleigh, 2 Cornette–Shanks with its g.
 *
 * @throws Error for a `tabulated` phase, which the uniform cannot carry; R08.T6.b gives it a table
 *   and a code of its own. Also for a Rayleigh phase with ρ other than 0 in any channel: R05's
 *   kernels draw only the ρ = 0 form, so it would be drawn wrong; R08.T6.b's kernels are to read ρ.
 */
function phaseOf(phase: PhaseFunction): readonly [number, number] {
  let packed: readonly [number, number];
  switch (phase.kind) {
    case "none":
      packed = [0, 0];
      break;
    case "rayleigh":
      if (phase.depolarisation.some((rho) => rho !== 0)) {
        throw new Error(
          `a Rayleigh phase with ρ = ${phase.depolarisation.join(", ")} has no Medium-uniform form until R08.T6.b`,
        );
      }
      packed = [1, 0];
      break;
    case "cornette-shanks":
      packed = [2, phase.asymmetry];
      break;
    case "tabulated":
      throw new Error("a tabulated phase has no Medium-uniform form until R08.T6.b");
  }
  return packed;
}

/**
 * A density profile as `common.wgsl`'s `densityOf` reads it: 0 exponential with its scale height,
 * 1 tent with its three heights.
 *
 * @throws Error for a `tabulated` profile, which the uniform cannot carry and `densityOf` would read
 *   as a tent; R08.T6.b gives it a table and a code of its own.
 */
function profileOf(profile: DensityProfile): readonly [number, number, number, number] {
  let packed: readonly [number, number, number, number];
  switch (profile.kind) {
    case "exponential":
      packed = [0, profile.scaleHeightM, 0, 0];
      break;
    case "tent":
      packed = [1, profile.bottomM, profile.peakM, profile.topM];
      break;
    case "tabulated":
      throw new Error("a tabulated density has no Medium-uniform form until R08.T6.b");
  }
  return packed;
}

/**
 * The medium as `common.wgsl`'s `Medium` uniform.
 *
 * @param bottomRadiusM - The ground's radius, m.
 * @param samples - The kernel's steps along each ray.
 * @throws Error when the medium has more than {@link MAX_TERMS} terms, or a term with a
 *   `tabulated` density or phase, or a Rayleigh phase with ρ other than 0, which the uniform
 *   cannot carry until R08.T6.b.
 */
export function packMedium(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  samples: number,
): Float32Array {
  if (medium.terms.length > MAX_TERMS) {
    throw new Error(
      `atmosphere medium ${medium.name} has ${medium.terms.length} terms; at most ${MAX_TERMS} fit`,
    );
  }
  const packed = new Float32Array(MEDIUM_FLOATS);
  packed.set([bottomRadiusM, bottomRadiusM + medium.topHeightM, medium.terms.length, samples], 0);
  packed.set([...medium.groundAlbedo, 0], 4);
  for (const [i, term] of medium.terms.entries()) {
    const at = 8 + i * 12;
    const [phaseKind, asymmetry] = phaseOf(term.phase);
    packed.set([...term.scattering, phaseKind], at);
    packed.set([...term.absorption, asymmetry], at + 4);
    packed.set(profileOf(term.density), at + 8);
  }
  return packed;
}

/** Workgroups covering a table with 8 × 8 groups: the transmittance kernel's. */
function groupsOf(size: TableSize): readonly [number, number, number] {
  return [Math.ceil(size.widthTexels / 8), Math.ceil(size.heightTexels / 8), 1];
}

/** What fixes the tables' contents: the medium and the ground's radius. */
function keyOf(medium: AtmosphereMedium, bottomRadiusM: number): string {
  return JSON.stringify({ medium, bottomRadiusM });
}

/** The GPU objects of one device: the two tables and their kernels. */
interface TableResources {
  readonly transmittance: TextureHandle;
  readonly multiScattering: TextureHandle;
  readonly transmittanceKernel: ComputeHandle;
  readonly multiScatteringKernel: ComputeHandle;
}

function createResources(engine: RenderEngine): TableResources {
  const usage =
    TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_SRC;
  return {
    transmittance: engine.createTexture({
      name: "atmosphere transmittance",
      size: [TRANSMITTANCE_SIZE.widthTexels, TRANSMITTANCE_SIZE.heightTexels],
      dimension: "2d",
      format: "rgba16float",
      mips: 1,
      usage,
      category: "atmosphere-tables",
    }),
    multiScattering: engine.createTexture({
      name: "atmosphere multiple scattering",
      size: [MULTI_SCATTERING_SIZE.widthTexels, MULTI_SCATTERING_SIZE.heightTexels],
      dimension: "2d",
      format: "rgba16float",
      mips: 1,
      usage,
      category: "atmosphere-tables",
    }),
    transmittanceKernel: engine.createCompute(TRANSMITTANCE_KERNEL),
    multiScatteringKernel: engine.createCompute(MULTI_SCATTERING_KERNEL),
  };
}

/**
 * A planet's transmittance and multiple-scattering tables, made once and rebuilt in place when
 * the medium changes.
 *
 * @remarks
 * After a device loss the engine's restore makes every handle anew (`RenderEngine.onRestored`):
 * the tables then make their textures and kernels again and rebuild for the last medium, so a
 * caller reads {@link AtmosphereTables.transmittance} and {@link AtmosphereTables.multiScattering}
 * afresh each frame rather than keeping the handles.
 */
export class AtmosphereTables {
  readonly #engine: RenderEngine;
  readonly #offRestored: () => void;
  #resources: TableResources;
  #medium: AtmosphereMedium;
  #bottomRadiusM: number;
  #key: string | null = null;
  #builds = 0;

  /**
   * Makes the tables and builds them for `medium`.
   *
   * @param bottomRadiusM - The ground's radius on the tables' sphere, m.
   * @throws Error as {@link packMedium}.
   */
  constructor(engine: RenderEngine, medium: AtmosphereMedium, bottomRadiusM: number) {
    this.#engine = engine;
    this.#medium = medium;
    this.#bottomRadiusM = bottomRadiusM;
    this.#resources = createResources(engine);
    this.#offRestored = engine.onRestored(() => {
      this.#resources = createResources(engine);
      this.#key = null;
      this.setMedium(this.#medium, this.#bottomRadiusM);
    });
    this.setMedium(medium, bottomRadiusM);
  }

  /** Transmittance per (r, μ), `rgba16float`; a new handle after a restore. */
  get transmittance(): TextureHandle {
    return this.#resources.transmittance;
  }

  /** Multiple scattering per (r, μ_sun), per unit illuminance, `rgba16float`. */
  get multiScattering(): TextureHandle {
    return this.#resources.multiScattering;
  }

  /** How many times the tables have been built. */
  get builds(): number {
    return this.#builds;
  }

  /**
   * Rebuilds the tables if `medium` or `bottomRadiusM` differs from what they were built for.
   *
   * @returns Whether they were rebuilt.
   * @throws Error as {@link packMedium}; the tables keep what they held.
   */
  setMedium(medium: AtmosphereMedium, bottomRadiusM: number): boolean {
    const key = keyOf(medium, bottomRadiusM);
    if (key === this.#key) {
      return false;
    }
    const transmittanceMedium = packMedium(medium, bottomRadiusM, TRANSMITTANCE_SAMPLES);
    const multiScatteringMedium = packMedium(medium, bottomRadiusM, MULTI_SCATTERING_SAMPLES);
    const r = this.#resources;
    this.#engine.dispatch(
      r.transmittanceKernel,
      {
        uniforms: { medium: transmittanceMedium },
        buffers: {},
        sampled: {},
        storage: { transmittanceOut: { texture: r.transmittance, level: 0 } },
      },
      groupsOf(TRANSMITTANCE_SIZE),
      "atmosphere tables",
    );
    this.#engine.dispatch(
      r.multiScatteringKernel,
      {
        uniforms: { medium: multiScatteringMedium },
        buffers: {},
        sampled: { transmittance: r.transmittance },
        storage: { multiScatteringOut: { texture: r.multiScattering, level: 0 } },
      },
      [MULTI_SCATTERING_SIZE.widthTexels, MULTI_SCATTERING_SIZE.heightTexels, 1],
      "atmosphere tables",
    );
    this.#key = key;
    this.#medium = medium;
    this.#bottomRadiusM = bottomRadiusM;
    this.#builds += 1;
    return true;
  }

  /** Stops following the engine's restores. The engine owns the textures and frees them. */
  dispose(): void {
    this.#offRestored();
  }
}
