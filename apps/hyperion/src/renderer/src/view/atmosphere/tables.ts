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
 *
 * The medium reaches every kernel in three parts (plan R08, R08.T6.b): its shell, term count,
 * steps and ground albedo in the `Medium` uniform ({@link packMediumUniform}), its terms in a
 * storage buffer of {@link MAX_TERMS} terms ({@link packMedium}), and its tabulated densities and
 * phases in two 2D array textures, one layer a term ({@link packDensityTables},
 * {@link packPhaseTables}). A compute pass binds no sampler, so the kernels read the tables by
 * `textureLoad` and interpolate by hand; a table of one layer binds where a kernel declares
 * `texture_2d_array`, as a one-layer array (R08.T0). A medium with no tabulated term of a kind
 * binds a one-layer, one-texel placeholder, which the kernels never read. The tables hold the
 * medium resampled onto the kernels' own grids, the medium {@link kernelMedium} gives, which the
 * CPU twin (`tablesCpu.ts`) takes as the kernels' oracle.
 */

import { BUFFER_USAGE, TEXTURE_USAGE } from "../engine/gpuFlags";
import type { KernelPair } from "../engine/kernels";
import type { BufferHandle, ComputeHandle, RenderEngine, TextureHandle } from "../engine/types";
import {
  type AtmosphereMedium,
  checkNoAbsorbers,
  checkTabulatedTops,
  type DensityProfile,
  densityAt,
  type PhaseFunction,
  type PhaseMatrixElement,
  type PhaseTable,
  phaseTable,
  type TabulatedDensity,
  tabulatedDensity,
} from "./medium";
import commonWgsl from "./shaders/common.wgsl?raw";
import mediumWgsl from "./shaders/medium.wgsl?raw";
import multiScatteringWgsl from "./shaders/multiScattering.wgsl?raw";
import transmittanceWgsl from "./shaders/transmittance.wgsl?raw";

/**
 * The most terms a medium may have: `MAX_TERMS` in `common.wgsl`, the length of the kernels'
 * `terms` buffer. The CPU twin holds no limit; a medium of more terms is the kernels' refusal.
 */
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

/** Floats a term takes in the `terms` buffer: four `vec4f` (`common.wgsl`'s `Term`). */
const TERM_FLOATS = 16;

/** Floats in the `Medium` uniform: its radii, term count and steps, and the ground's albedo. */
const MEDIUM_UNIFORM_FLOATS = 8;

/** The bytes of the kernels' `terms` buffer: {@link MAX_TERMS} terms of 64 B, 512 B. */
export const TERMS_BUFFER_BYTES = MAX_TERMS * TERM_FLOATS * 4;

/**
 * The levels of a tabulated density's table: 1,024, at h_k = top × (k ÷ 1,023)², k = 0–1,023,
 * from the ground to the medium's top (Design note 2's square-root spacing).
 *
 * @remarks
 * The spacing is 2√(h × top) ÷ 1,023, finest at the ground where layers are thin. Read linear in
 * h between levels, an e^(−h ÷ H) errs by at most (Δh ÷ H)² ÷ 8 between them: at a 100 km top,
 * 6 × 10⁻⁶ at h = H = 8.4 km (R*T ÷ (M g₀) at the US Standard Atmosphere 1976's sea level, 8,435
 * m) and 4 × 10⁻⁵ at h = H = 1.2 km (Bruneton and Neyret 2008, §2's aerosol scale height
 * H_M ≃ 1.2 km). `tables.test.ts` holds R08.T3.a's
 * column, resampled, to its own optical depth. One layer is 4 KB in `r32float`.
 */
export const DENSITY_TABLE_LEVELS = 1_024;

/**
 * The entries of a tabulated phase's table: 256, even in u = √(θ ÷ π) from 0 to 1 (Design note 6's
 * grid). One layer is 4 KB in `rgba32float`, the three channels in rgb.
 */
export const PHASE_TABLE_ENTRIES = 256;

/** `Term.profile.x`'s code for each density profile (`medium.wgsl`'s `densityOf`). */
function densityCodeOf(
  profile: DensityProfile,
  topHeightM: number,
): readonly [number, number, number, number] {
  let packed: readonly [number, number, number, number];
  switch (profile.kind) {
    case "exponential":
      packed = [0, profile.scaleHeightM, 0, 0];
      break;
    case "tent":
      packed = [1, profile.bottomM, profile.peakM, profile.topM];
      break;
    case "tabulated":
      packed = [2, topHeightM, 0, 0];
      break;
  }
  return packed;
}

/**
 * `Term.phase` for each phase function (`source.wgsl`'s `phaseOf`): its parameters in xyz and its
 * code in w, 0 none, 1 Rayleigh with its depolarisation ratio ρ per channel, 2 Cornette–Shanks
 * with its g in x, 3 tabulated.
 */
function phaseCodeOf(phase: PhaseFunction): readonly [number, number, number, number] {
  let packed: readonly [number, number, number, number];
  switch (phase.kind) {
    case "none":
      packed = [0, 0, 0, 0];
      break;
    case "rayleigh":
      packed = [...phase.depolarisation, 1];
      break;
    case "cornette-shanks":
      packed = [phase.asymmetry, 0, 0, 2];
      break;
    case "tabulated":
      packed = [0, 0, 0, 3];
      break;
  }
  return packed;
}

/**
 * The medium's terms as the kernels' `terms` storage buffer reads them (`common.wgsl`'s `Term`):
 * per term its scattering, its absorption, its profile and its phase function, four `vec4f`, for
 * {@link MAX_TERMS} terms whatever the medium's count. A tabulated term's tables are its layer of
 * {@link packDensityTables} and {@link packPhaseTables}, its index here. A Rayleigh phase carries
 * its ρ per channel (R08.T3.c), which the kernels draw.
 *
 * @throws Error when the medium has more than {@link MAX_TERMS} terms; RangeError, as
 *   {@link checkTabulatedTops}, for a tabulated density held up to the top from a last level below
 *   it, and, as {@link checkNoAbsorbers}, for an absorber term, whose curves the kernels read only
 *   from R08.T6.c.
 */
export function packMedium(medium: AtmosphereMedium): Float32Array {
  if (medium.terms.length > MAX_TERMS) {
    throw new Error(
      `atmosphere medium ${medium.name} has ${medium.terms.length} terms; at most ${MAX_TERMS} fit`,
    );
  }
  checkTabulatedTops(medium);
  checkNoAbsorbers(medium);
  const packed = new Float32Array(MAX_TERMS * TERM_FLOATS);
  for (const [i, term] of medium.terms.entries()) {
    const at = i * TERM_FLOATS;
    packed.set([...term.scattering, 0], at);
    packed.set([...term.absorption, 0], at + 4);
    packed.set(densityCodeOf(term.density, medium.topHeightM), at + 8);
    packed.set(phaseCodeOf(term.phase), at + 12);
  }
  return packed;
}

/**
 * The medium's shell, term count and a kernel's steps as `common.wgsl`'s `Medium` uniform, with the
 * ground's albedo.
 *
 * @param bottomRadiusM - The ground's radius, m.
 * @param samples - The kernel's steps along each ray.
 */
export function packMediumUniform(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  samples: number,
): Float32Array {
  const packed = new Float32Array(MEDIUM_UNIFORM_FLOATS);
  packed.set([bottomRadiusM, bottomRadiusM + medium.topHeightM, medium.terms.length, samples], 0);
  packed.set([...medium.groundAlbedo, 0], 4);
  return packed;
}

/** The heights of a density table's {@link DENSITY_TABLE_LEVELS} levels under a top, m. */
export function densityTableLevelsM(topHeightM: number): Float64Array {
  const last = DENSITY_TABLE_LEVELS - 1;
  return Float64Array.from(
    { length: DENSITY_TABLE_LEVELS },
    (_, k) => topHeightM * (k / last) ** 2,
  );
}

/**
 * A tabulated density resampled onto its table's levels ({@link densityTableLevelsM}) under the
 * medium's top: its density at each level by the `tabulated` rule, as `medium.wgsl` reads the
 * table, linear in h between levels and constant beyond the last.
 */
export function resampledDensity(profile: TabulatedDensity, topHeightM: number): TabulatedDensity {
  const levels = densityTableLevelsM(topHeightM);
  return tabulatedDensity(
    levels,
    levels.map((h) => densityAt(profile, h)),
  );
}

/**
 * Simpson panels a phase table's interval is integrated over, per 1 ÷ 255 of u it spans, at least
 * one such share: 8 an interval of the kernels' grid, and more for a coarser source grid's.
 *
 * @remarks
 * Composite Simpson's error is (b − a) h⁴ max|f⁗| ÷ 180; with h ≤ 1 ÷ 2,040 it is under 2 × 10⁻⁹
 * for a Henyey–Greenstein table up to g = 0.99, where a fixed 8 panels an interval left 1.6 × 10⁻⁴
 * on a source grid of 5° in θ (R08.T6.b's science check).
 */
const PHASE_SIMPSON_PANELS_PER_ENTRY = 8;

/** The integrand of {@link phaseIntegral} at u = x, where the phase is p. */
function phaseIntegrand(x: number, p: number): number {
  return p * x * Math.sin(Math.PI * x * x);
}

/**
 * A phase channel's integral over the sphere, linear in u between its entries:
 * ∫ p dΩ = 4π² ∫₀¹ p(u) u sin(πu²) du, since θ = πu² and dθ = 2πu du, by Simpson's rule.
 */
function phaseIntegral(u: Float64Array, values: Float64Array): number {
  let sum = 0;
  for (let i = 0; i + 1 < u.length; i += 1) {
    const u0 = u[i] ?? Number.NaN;
    const u1 = u[i + 1] ?? Number.NaN;
    const p0 = values[i] ?? Number.NaN;
    const p1 = values[i + 1] ?? Number.NaN;
    const panels =
      PHASE_SIMPSON_PANELS_PER_ENTRY *
      Math.max(1, Math.ceil((u1 - u0) * (PHASE_TABLE_ENTRIES - 1)));
    const h = (u1 - u0) / panels;
    let interval = 0;
    for (let k = 0; k <= panels; k += 1) {
      const f = k / panels;
      let weight = k % 2 === 1 ? 4 : 2;
      if (k === 0 || k === panels) {
        weight = 1;
      }
      interval += weight * phaseIntegrand(u0 + f * (u1 - u0), p0 + f * (p1 - p0));
    }
    sum += (interval * h) / 3;
  }
  return 4 * Math.PI * Math.PI * sum;
}

/** A channel linear in u between `u`'s entries, at each of `at`'s ascending points. */
function resampledChannel(u: Float64Array, values: Float64Array, at: Float64Array): Float64Array {
  const out = new Float64Array(at.length);
  let i = 0;
  for (const [k, x] of at.entries()) {
    while (i + 2 < u.length && (u[i + 1] ?? Number.NaN) <= x) {
      i += 1;
    }
    const u0 = u[i] ?? Number.NaN;
    const f = Math.min(Math.max((x - u0) / ((u[i + 1] ?? Number.NaN) - u0), 0), 1);
    const v0 = values[i] ?? Number.NaN;
    out[k] = v0 + ((values[i + 1] ?? Number.NaN) - v0) * f;
  }
  return out;
}

/** Whether a table's entries are already the kernels' grid: {@link PHASE_TABLE_ENTRIES}, even in u. */
function onTableGrid(u: Float64Array): boolean {
  const last = PHASE_TABLE_ENTRIES - 1;
  return u.length === PHASE_TABLE_ENTRIES && u.every((x, i) => x === i / last);
}

/**
 * A phase table resampled onto the kernels' grid, {@link PHASE_TABLE_ENTRIES} entries even in u:
 * the table itself when it is on that grid already, otherwise each channel linear in u between its
 * entries, as `medium.ts`'s `phaseAt` reads it, and scaled so that its integral over the sphere is
 * the source's, so that a normalised table stays normalised (R08.T6.a's hand-off).
 *
 * @remarks
 * Each matrix element is resampled alike and scaled by its channel's factor, so that it stays
 * normalised as a₁ is. The integrals are taken by Simpson's rule, linear in u between entries.
 */
export function resampledPhase(table: PhaseTable): PhaseTable {
  if (onTableGrid(table.u)) {
    return table;
  }
  const last = PHASE_TABLE_ENTRIES - 1;
  const grid = Float64Array.from({ length: PHASE_TABLE_ENTRIES }, (_, i) => i / last);
  const factors = table.values.map((channel) => {
    const source = phaseIntegral(table.u, channel);
    const resampled = phaseIntegral(grid, resampledChannel(table.u, channel, grid));
    return resampled > 0 ? source / resampled : 1;
  });
  const element = (channels: PhaseMatrixElement): PhaseMatrixElement => {
    const [r, g, b] = channels.map((channel, c) =>
      resampledChannel(table.u, channel, grid).map((v) => v * (factors[c] ?? Number.NaN)),
    );
    if (r === undefined || g === undefined || b === undefined) {
      throw new Error("a phase table has three channels");
    }
    return [r, g, b];
  };
  const matrix = table.matrix;
  return phaseTable(
    grid,
    element(table.values),
    matrix === undefined
      ? undefined
      : {
          a2: element(matrix.a2),
          a3: element(matrix.a3),
          a4: element(matrix.a4),
          b1: element(matrix.b1),
          b2: element(matrix.b2),
        },
  );
}

/**
 * The medium as the kernels read it: each tabulated density resampled onto its table's levels
 * ({@link resampledDensity}) and each tabulated phase onto the kernels' grid
 * ({@link resampledPhase}); every other term as it is.
 *
 * @remarks
 * The CPU twin of this medium is the kernels' oracle (R08.T6.b's smoke); how far it lies from the
 * twin of `medium` itself is the resampling's error, which `tables.test.ts` bounds.
 */
export function kernelMedium(medium: AtmosphereMedium): AtmosphereMedium {
  return {
    ...medium,
    terms: medium.terms.map((term) => ({
      ...term,
      density:
        term.density.kind === "tabulated"
          ? resampledDensity(term.density, medium.topHeightM)
          : term.density,
      phase:
        term.phase.kind === "tabulated"
          ? { kind: "tabulated", table: resampledPhase(term.phase.table) }
          : term.phase,
    })),
  };
}

/**
 * A 2D array texture's contents, one row a layer: `widthTexels` texels a layer, layer after layer,
 * each texel's channels in turn.
 */
export interface LayeredTable {
  readonly widthTexels: number;
  readonly layers: number;
  readonly texels: Float32Array;
}

/** The one-layer, one-texel placeholder a medium with no table of a kind binds, of `channels`. */
function placeholder(channels: number): LayeredTable {
  return { widthTexels: 1, layers: 1, texels: new Float32Array(channels) };
}

/**
 * The medium's density tables as `medium.wgsl` reads them, `r32float`: one layer a term, term i's
 * at layer i, its {@link resampledDensity} on {@link DENSITY_TABLE_LEVELS} levels where its
 * density is tabulated and zeros where it is not; the placeholder where no term's is.
 */
export function packDensityTables(medium: AtmosphereMedium): LayeredTable {
  const terms = medium.terms;
  if (!terms.some((term) => term.density.kind === "tabulated")) {
    return placeholder(1);
  }
  const texels = new Float32Array(DENSITY_TABLE_LEVELS * terms.length);
  for (const [i, term] of terms.entries()) {
    if (term.density.kind === "tabulated") {
      texels.set(
        resampledDensity(term.density, medium.topHeightM).relative,
        i * DENSITY_TABLE_LEVELS,
      );
    }
  }
  return { widthTexels: DENSITY_TABLE_LEVELS, layers: terms.length, texels };
}

/**
 * The medium's phase tables as `source.wgsl` reads them, `rgba32float`, the channels in rgb and 0
 * in alpha: one layer a term, term i's at layer i, its {@link resampledPhase} on
 * {@link PHASE_TABLE_ENTRIES} entries where its phase is tabulated and zeros where it is not; the
 * placeholder where no term's is.
 */
export function packPhaseTables(medium: AtmosphereMedium): LayeredTable {
  const terms = medium.terms;
  if (!terms.some((term) => term.phase.kind === "tabulated")) {
    return placeholder(4);
  }
  const texels = new Float32Array(PHASE_TABLE_ENTRIES * 4 * terms.length);
  for (const [i, term] of terms.entries()) {
    if (term.phase.kind !== "tabulated") {
      continue;
    }
    const [r, g, b] = resampledPhase(term.phase.table).values;
    for (let k = 0; k < PHASE_TABLE_ENTRIES; k += 1) {
      texels.set(
        [r[k] ?? Number.NaN, g[k] ?? Number.NaN, b[k] ?? Number.NaN, 0],
        (i * PHASE_TABLE_ENTRIES + k) * 4,
      );
    }
  }
  return { widthTexels: PHASE_TABLE_ENTRIES, layers: terms.length, texels };
}

/** Workgroups covering a table with 8 × 8 groups: the transmittance kernel's. */
function groupsOf(size: TableSize): readonly [number, number, number] {
  return [Math.ceil(size.widthTexels / 8), Math.ceil(size.heightTexels / 8), 1];
}

/** What fixes the tables' contents: the medium and the ground's radius. */
function keyOf(medium: AtmosphereMedium, bottomRadiusM: number): string {
  return JSON.stringify({ medium, bottomRadiusM });
}

/** A 2D array texture the tables made, with the shape it was made at. */
interface LayeredTexture {
  readonly texture: TextureHandle;
  readonly widthTexels: number;
  readonly layers: number;
}

/** The GPU objects of one device: the two tables, the medium's buffer and tables, the kernels. */
interface TableResources {
  readonly transmittance: TextureHandle;
  readonly multiScattering: TextureHandle;
  readonly terms: BufferHandle;
  densityTables: LayeredTexture;
  phaseTables: LayeredTexture;
  readonly transmittanceKernel: ComputeHandle;
  readonly multiScatteringKernel: ComputeHandle;
}

/** The density tables' texture format: one float a level. */
const DENSITY_FORMAT: GPUTextureFormat = "r32float";
/** The phase tables' texture format: the three channels and an unused alpha. */
const PHASE_FORMAT: GPUTextureFormat = "rgba32float";

/** Makes a layered table's texture, uploaded with `TEXTURE_BINDING` and `COPY_DST`. */
function createLayered(
  engine: RenderEngine,
  name: string,
  format: GPUTextureFormat,
  table: Pick<LayeredTable, "widthTexels" | "layers">,
): LayeredTexture {
  const texture = engine.createTexture({
    name,
    size: [table.widthTexels, 1, table.layers],
    dimension: "2d",
    format,
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "atmosphere-tables",
  });
  return { texture, widthTexels: table.widthTexels, layers: table.layers };
}

/** Releases the buffer and textures of `resources`; the kernels have no release. */
function releaseResources(engine: RenderEngine, resources: TableResources): void {
  engine.releaseBuffer(resources.terms);
  for (const texture of [
    resources.transmittance,
    resources.multiScattering,
    resources.densityTables.texture,
    resources.phaseTables.texture,
  ]) {
    engine.releaseTexture(texture);
  }
}

function createResources(engine: RenderEngine): TableResources {
  const usage =
    TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_SRC;
  const one = { widthTexels: 1, layers: 1 };
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
    terms: engine.createBuffer({
      name: "atmosphere terms",
      bytes: TERMS_BUFFER_BYTES,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "atmosphere-tables",
    }),
    densityTables: createLayered(engine, "atmosphere density tables", DENSITY_FORMAT, one),
    phaseTables: createLayered(engine, "atmosphere phase tables", PHASE_FORMAT, one),
    transmittanceKernel: engine.createCompute(TRANSMITTANCE_KERNEL),
    multiScatteringKernel: engine.createCompute(MULTI_SCATTERING_KERNEL),
  };
}

/**
 * A planet's transmittance and multiple-scattering tables, made once and rebuilt in place when
 * the medium changes, with the medium's terms and tables, which the per-frame kernels read too.
 *
 * @remarks
 * After a device loss the engine's restore makes every handle anew (`RenderEngine.onRestored`):
 * the tables then make their textures, buffer and kernels again and rebuild for the last medium,
 * so a caller reads {@link AtmosphereTables.transmittance} and the rest afresh each frame rather
 * than keeping the handles. A medium whose tables have another shape than the last's makes them
 * anew and releases the old.
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
   * @throws Error or RangeError as {@link packMedium}, and `EngineUnavailable` while the engine has
   *   no device (a loss), having released the buffer and textures it made before.
   */
  constructor(engine: RenderEngine, medium: AtmosphereMedium, bottomRadiusM: number) {
    this.#engine = engine;
    this.#medium = medium;
    this.#bottomRadiusM = bottomRadiusM;
    this.#resources = createResources(engine);
    try {
      this.setMedium(medium, bottomRadiusM);
    } catch (error: unknown) {
      releaseResources(engine, this.#resources);
      throw error;
    }
    this.#offRestored = engine.onRestored(() => {
      this.#resources = createResources(engine);
      this.#key = null;
      this.setMedium(this.#medium, this.#bottomRadiusM);
    });
  }

  /** Transmittance per (r, μ), `rgba16float`; a new handle after a restore. */
  get transmittance(): TextureHandle {
    return this.#resources.transmittance;
  }

  /** Multiple scattering per (r, μ_sun), per unit illuminance, `rgba16float`. */
  get multiScattering(): TextureHandle {
    return this.#resources.multiScattering;
  }

  /** The medium's terms, {@link packMedium}'s, for the kernels' `terms` binding. */
  get terms(): BufferHandle {
    return this.#resources.terms;
  }

  /** The medium's density tables, {@link packDensityTables}'s, for `densityTables`. */
  get densityTables(): TextureHandle {
    return this.#resources.densityTables.texture;
  }

  /** The medium's phase tables, {@link packPhaseTables}'s, for `phaseTables`. */
  get phaseTables(): TextureHandle {
    return this.#resources.phaseTables.texture;
  }

  /**
   * The medium the tables follow: the last one {@link AtmosphereTables.setMedium} took, which a
   * restore builds and the per-frame kernels read beside them.
   */
  get medium(): AtmosphereMedium {
    return this.#medium;
  }

  /** How many times the tables have been built. */
  get builds(): number {
    return this.#builds;
  }

  /**
   * Rebuilds the tables if `medium` or `bottomRadiusM` differs from what they were built for.
   *
   * @returns Whether they were rebuilt.
   * @throws Error or RangeError as {@link packMedium}, and the tables keep what they held and
   *   follow; or `EngineUnavailable` while the engine has no device (a loss), and the tables follow
   *   `medium` from then on, so that the engine's restore builds it.
   */
  setMedium(medium: AtmosphereMedium, bottomRadiusM: number): boolean {
    const key = keyOf(medium, bottomRadiusM);
    if (key === this.#key) {
      return false;
    }
    const terms = packMedium(medium);
    const densityTables = packDensityTables(medium);
    const phaseTables = packPhaseTables(medium);
    this.#medium = medium;
    this.#bottomRadiusM = bottomRadiusM;
    this.#key = null;
    const engine = this.#engine;
    const r = this.#resources;
    engine.writeBuffer(r.terms, 0, terms);
    r.densityTables = this.#upload(r.densityTables, densityTables, DENSITY_FORMAT);
    r.phaseTables = this.#upload(r.phaseTables, phaseTables, PHASE_FORMAT);
    const sampled = { densityTables: r.densityTables.texture };
    engine.dispatch(
      r.transmittanceKernel,
      {
        uniforms: { medium: packMediumUniform(medium, bottomRadiusM, TRANSMITTANCE_SAMPLES) },
        buffers: { terms: r.terms },
        sampled,
        storage: { transmittanceOut: { texture: r.transmittance, level: 0 } },
      },
      groupsOf(TRANSMITTANCE_SIZE),
      "atmosphere tables",
    );
    engine.dispatch(
      r.multiScatteringKernel,
      {
        uniforms: { medium: packMediumUniform(medium, bottomRadiusM, MULTI_SCATTERING_SAMPLES) },
        buffers: { terms: r.terms },
        sampled: { ...sampled, transmittance: r.transmittance },
        storage: { multiScatteringOut: { texture: r.multiScattering, level: 0 } },
      },
      [MULTI_SCATTERING_SIZE.widthTexels, MULTI_SCATTERING_SIZE.heightTexels, 1],
      "atmosphere tables",
    );
    this.#key = key;
    this.#builds += 1;
    return true;
  }

  /**
   * Stops following the engine's restores. The engine owns the buffer and the textures it holds
   * then and frees them; the ones an earlier medium's shape left were released when it changed.
   */
  dispose(): void {
    this.#offRestored();
  }

  /**
   * Writes a layered table into `current`, or into a texture made at its shape, releasing
   * `current`, when its shape differs; a placeholder is left as it was made, zero.
   */
  #upload(current: LayeredTexture, table: LayeredTable, format: GPUTextureFormat): LayeredTexture {
    let target = current;
    if (current.widthTexels !== table.widthTexels || current.layers !== table.layers) {
      target = createLayered(this.#engine, current.texture.name, format, table);
      this.#engine.releaseTexture(current.texture);
    }
    if (table.widthTexels > 1) {
      this.#engine.writeTexture(
        target.texture,
        [0, 0, 0],
        [table.widthTexels, 1, table.layers],
        table.texels,
      );
    }
    return target;
  }
}
