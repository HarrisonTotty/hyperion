/**
 * A ring's local particle field close to (plan R11, R11.T9.d; Design note 16): the cells of the
 * ring plane its particles are placed in, which turn at the local Keplerian rate so that the field
 * shears as a ring does, the parameters `ringParticles.wgsl` places them by, and that placement's
 * CPU twin.
 *
 * @remarks
 * **The sheet.** About the camera, the ring is a flat sheet: x radial from a reference radius r₀,
 * y along the orbit as the arc r₀ ψ, ψ the azimuth in the frame turning at Ω₀ = √(GM ÷ r₀³) from a
 * reference time, z along the plane's normal. A particle at radial offset x drifts along the orbit
 * at −(3/2) Ω₀ x against that frame (Hill's linearised shear), so its cell's row is offset by S x
 * with S = (3/2) Ω₀ t at time t from the reference: each row of cells turns rigidly at its own rate.
 * r₀ and the reference time are the field's own, held while the camera stays within
 * {@link RING_FIELD_PATCH_M} of r₀ radially; past that the field is made again about it, a pop of
 * the near field the slab's share hides in part. Rows run straight, not on arcs: about 1.4–1.8 m
 * off at 18 km across Saturn's B ring (y² ÷ 2r), within the plane. The turning frame is taken
 * about the camera as a sliding one, so seen from a camera at rest a particle at x moves at
 * v₀ − (3/2) Ω₀ x rather than Kepler's v₀ − ½ Ω₀ x, yet stands within |x y| ÷ r₀ of its place on
 * the arc, some centimetres at the field's 2 km, each frame placing it afresh.
 *
 * **The octaves.** Particle radii run in {@link RING_FIELD_LEVELS} octaves from s_max = 5 m down, the
 * last to s_min = 1 cm, octave k holding radii (s_k ÷ 2, s_k] with s_k = s_max ÷ 2^k. Its cells are
 * λ s_k a side, λ chosen so that a cell holds {@link RING_FIELD_PER_CELL} particles on average
 * (an octave holds 1.5 C ÷ s_k² a square metre, C = τ ÷ (π ln(s_max ÷ s_min)), `ringSplit.ts`), and
 * it is placed within D_k = 2 s_k ÷ θ of the camera, the farthest its largest particle fills a pixel
 * of angle θ: so each octave places about the same number of cells. A particle is placed only where
 * its diameter exceeds its own distance's pixel footprint, 2s ≥ d θ, the instanced radii of
 * `instancedShare`, so that the slab's share removed and the particles placed agree at every
 * distance; and only where its bounds meet the view.
 *
 * **The bands.** The field follows the ring's profile across it: up to {@link RING_FIELD_STEPS}
 * radial steps about the camera, each its band's τ over the field's reference τ (the densest
 * within reach, which sets λ) and its particles' albedo, 0 beyond the annulus's edges, so that the
 * field holds a gap's thinner population and stops where the ring does. A cell draws its count at
 * the greatest ratio its width meets, and keeps each particle at its own step's ratio over that.
 *
 * **The draws.** A cell's count is ⌊n + u⌋ for its mean n and a uniform u of its hash, at most
 * {@link RING_FIELD_MAX_PER_CELL}; each particle's place in the cell, height (Gaussian, the layer's
 * σ = H_eff ÷ √(2π), R11.T9.b's), radius (the octave's s⁻³ law), turn (Shoemake 1992, "Uniform
 * random rotations", Graphics Gems III) and shade come from PCG hashes of its cell and index alone
 * (Jarzynski and Olano 2020, JCGT 9(3)), so that the field is the same whatever the dispatch's
 * order. Presentation, not authoritative.
 *
 * **The light on a particle.** The star's beam reaches a particle through the layer above it on the
 * star's side: its colour's alpha holds exp(−τ Q(z_s ÷ σ) ÷ μ₀), Q(x) = ½ erfc(x ÷ √2) the share
 * of the Gaussian layer's column beyond its height towards the star z_s, τ its band's (the
 * mean-field direct beam, as Design note 14's single-scattering faces attenuate it at depth; the
 * shadow-hiding correlation is the shadowing table's alone).
 */
import { add, cross, dot, norm, normalise, scale, sub, type Vec3 } from "../../geometry/vec3";
import type { CullPlane } from "../clusters/cullPass";
import { type ParticleInstance, sphereParticleTemplates } from "../clusters/particles";
import type { Rgb } from "../photometry/toneCurve";
import type { RingProfile } from "./ringProfile";
import { planeAxes } from "./ringSlab";
import { RING_PARTICLE_RADII_M } from "./ringSplit";
import { SHADOWING_HEADER } from "./shadowingTable";

/** The octaves of radius the field places: 9, 5 m down to 1 cm. */
export const RING_FIELD_LEVELS = 9;

/** The particles a cell holds on average: 64. */
export const RING_FIELD_PER_CELL = 64;

/** The most a cell holds. */
export const RING_FIELD_MAX_PER_CELL = 255;

/** The radial distance from its reference radius at which the field is made again about the camera, m. */
export const RING_FIELD_PATCH_M = 50_000;

/**
 * The layer's Gaussian σ, m: H_eff ÷ √(2π) of R11.T9.b's shadowing layer (H_eff 11.17 m at D₀ 0.35,
 * Salo and French 2010's τ_dyn 1.5; `SHADOWING_HEADER.acceptance`).
 */
export const RING_LAYER_SIGMA_M = SHADOWING_HEADER.acceptance.thicknessM / Math.sqrt(2 * Math.PI);

/** The most radial steps of the ring's profile the field follows about the camera. */
export const RING_FIELD_STEPS = 4;

/** An edge past every step: f32's largest. */
const NO_EDGE_M = 3.4e38;

/** The most cells a side of an octave's window spans: a field never asks more of a dispatch. */
const MAX_HALF_WINDOW = 400;

/** λ, a cell's side over its octave's largest radius, for the field's reference τ. */
function cellsPerRadius(tauReference: number): number {
  const [sMin, sMax] = RING_PARTICLE_RADII_M;
  const c = tauReference / (Math.PI * Math.log(sMax / sMin));
  return Math.sqrt(RING_FIELD_PER_CELL / (1.5 * c));
}

/**
 * The least pixel angle, rad, at which a field of reference τ `tauReference` keeps every octave's
 * window within its {@link MAX_HALF_WINDOW} cells: an octave's reach over its cell is 2 ÷ (θ λ),
 * whatever the octave, so that a denser ring, whose cells are smaller, asks a wider pixel.
 */
export function fieldWindowPixelRad(tauReference: number): number {
  return 2 / ((MAX_HALF_WINDOW - 2) * cellsPerRadius(tauReference));
}

/** One octave of the field as the placement reads it. */
export interface RingFieldLevel {
  /** Its cells' side, m. */
  readonly cellM: number;
  /** Its radii, (low, high], m. */
  readonly radiusLowM: number;
  readonly radiusHighM: number;
  /** The particles a cell holds on average. */
  readonly perCell: number;
  /** The camera's cell row, a whole number, and its x within it, in [0, 1). */
  readonly row: number;
  readonly rowFraction: number;
  /** The camera's sheared column at its own row, mod 2³², and its y within it, in [0, 1). */
  readonly column: number;
  readonly columnFraction: number;
  /** The window's half-width in cells: rows and columns from −it to it about the camera's. */
  readonly halfWindow: number;
  /** Its first cell's index among the field's. */
  readonly firstCell: number;
}

/** A field as `ringParticles.wgsl` places it, every vector in the passes' frame. */
export interface RingField {
  /** The sheet's radial, prograde and normal axes at the camera's azimuth. */
  readonly axes: readonly [Vec3, Vec3, Vec3];
  /** The camera's height over the plane along its normal, m. */
  readonly heightM: number;
  readonly sigmaM: number;
  /** The pixel's angle θ the particles are instanced at, rad. */
  readonly pixelRad: number;
  /** The shear's rows' offset S = (3/2) Ω₀ t, its whole part mod 2³² and its fraction. */
  readonly shearWhole: number;
  readonly shearFraction: number;
  /** The field's reference τ, its steps' ratios' unit. */
  readonly tauReference: number;
  /** The star's direction's sine over the plane, signed along its normal: μ₀ with its side. */
  readonly starSine: number;
  /**
   * The radial steps' edges, m from the camera's radius, ascending, at most
   * {@link RING_FIELD_STEPS}; and each interval's band, one more than the edges: its τ over the
   * field's reference τ (0 off the annulus) and its particles' albedo.
   */
  readonly edgesM: ReadonlyArray<number>;
  readonly steps: ReadonlyArray<{ readonly tauRatio: number; readonly albedo: Rgb }>;
  readonly seed: number;
  readonly levels: ReadonlyArray<RingFieldLevel>;
  /** Its cells, every octave's window. */
  readonly cells: number;
  /** The view's planes, inward normals, the camera at the origin. */
  readonly planes: ReadonlyArray<CullPlane>;
}

/** What a field is made from. */
export interface RingFieldInput {
  /** The ringed planet's centre from the camera, m, in the passes' frame. */
  readonly centreM: Vec3;
  /** The ring plane's unit normal. */
  readonly normal: Vec3;
  /** The ring's radial profile, which the field follows. */
  readonly profile: RingProfile;
  /** The planet's GM, m³ s⁻², or `null` where not known, when the field does not turn or shear. */
  readonly gravitationalParameterM3S2: number | null;
  /** The field's reference radius r₀, m, and the time since its reference time, s. */
  readonly referenceRadiusM: number;
  readonly sinceS: number;
  /** The pixel's angle θ the particles are instanced at, rad. */
  readonly pixelRad: number;
  readonly planes: ReadonlyArray<CullPlane>;
  readonly seed: number;
  /** The unit direction towards the ring's brightest star, or `null` for none, when it lights none. */
  readonly towardsStar: Vec3 | null;
}

/** The camera's place on the ring's sheet: its radius, azimuth and height, m and rad. */
export interface SheetPlace {
  readonly radiusM: number;
  readonly azimuthRad: number;
  readonly heightM: number;
  /** The radial and prograde unit axes at its azimuth. */
  readonly radial: Vec3;
  readonly prograde: Vec3;
}

/** The camera's place on the plane of `normal` about a planet at `centreM` from it. */
export function sheetPlaceOf(centreM: Vec3, normal: Vec3): SheetPlace {
  const n = normalise(normal);
  const p = scale(centreM, -1);
  const heightM = dot(p, n);
  const inPlane = sub(p, scale(n, heightM));
  const radiusM = norm(inPlane);
  const [e1, e2] = planeAxes(n);
  const radial = radiusM > 0 ? scale(inPlane, 1 / radiusM) : e1;
  return {
    radiusM,
    azimuthRad: Math.atan2(dot(inPlane, e2), dot(inPlane, e1)),
    heightM,
    radial,
    prograde: cross(n, radial),
  };
}

/** `x` mod 2³², as a whole number in [0, 2³²). */
function wrapU32(x: number): number {
  const m = x % 4_294_967_296;
  return m < 0 ? m + 4_294_967_296 : m;
}

/** `x` in [0, 2π). */
function wrapTurn(x: number): number {
  const t = 2 * Math.PI;
  const m = x % t;
  return m < 0 ? m + t : m;
}

/**
 * The ring's profile within `reachM` of the radius `radiusM`, as radial steps from it: the edges
 * of its bands and of the annulus there, m from that radius, ascending, at most
 * {@link RING_FIELD_STEPS} (the nearest), and each interval's τ and albedo, 0 off the annulus.
 */
export function profileSteps(
  profile: RingProfile,
  radiusM: number,
  reachM: number,
): {
  readonly edgesM: ReadonlyArray<number>;
  readonly steps: ReadonlyArray<{ readonly tau: number; readonly albedo: Rgb }>;
} {
  const none = { tau: 0, albedo: [0, 0, 0] as const };
  const edges = [...new Set([profile.innerM, ...profile.bands.map((band) => band.outerM)])]
    .map((edgeM) => edgeM - radiusM)
    .filter((offsetM) => Math.abs(offsetM) < reachM)
    .toSorted((a, b) => Math.abs(a) - Math.abs(b))
    .slice(0, RING_FIELD_STEPS)
    .toSorted((a, b) => a - b);
  // Each interval's band at its middle, the outermost ones a metre past their one edge.
  const steps: Array<{ readonly tau: number; readonly albedo: Rgb }> = [];
  for (let k = 0; k <= edges.length; k += 1) {
    const low = edges[k - 1];
    const high = edges[k];
    let probe = 0;
    if (low !== undefined && high !== undefined) {
      probe = (low + high) / 2;
    } else if (high !== undefined) {
      probe = high - 1;
    } else if (low !== undefined) {
      probe = low + 1;
    }
    const at = radiusM + probe;
    const band = profile.bands.find((each) => at >= each.innerM && at < each.outerM);
    steps.push(band === undefined ? none : { tau: band.tau, albedo: band.albedo });
  }
  return { edgesM: edges, steps };
}

/**
 * The field about the camera (see the module's remarks), or `null` where no octave reaches the
 * plane: the camera farther from it than the largest particle's hand-over, or no optical depth.
 */
export function ringFieldOf(input: RingFieldInput): RingField | null {
  const [sMin, sMax] = RING_PARTICLE_RADII_M;
  const place = sheetPlaceOf(input.centreM, input.normal);
  const reachM = (2 * sMax) / input.pixelRad + 3 * RING_LAYER_SIGMA_M;
  const bands = profileSteps(input.profile, place.radiusM, reachM);
  const tauReference = Math.max(0, ...bands.steps.map((step) => step.tau));
  const c = tauReference / (Math.PI * Math.log(sMax / sMin));
  if (!(c > 0) || !(input.pixelRad > 0)) {
    return null;
  }
  const r0 = input.referenceRadiusM;
  const omega =
    input.gravitationalParameterM3S2 === null
      ? 0
      : Math.sqrt(input.gravitationalParameterM3S2 / r0 ** 3);
  const shear = 1.5 * omega * input.sinceS;
  const xc = place.radiusM - r0;
  const yc = r0 * wrapTurn(place.azimuthRad - omega * input.sinceS);
  const levels: RingFieldLevel[] = [];
  let cells = 0;
  for (let k = 0; k < RING_FIELD_LEVELS; k += 1) {
    const high = sMax / 2 ** k;
    const low = Math.max(high / 2, sMin);
    const octaveReachM = (2 * high) / input.pixelRad;
    if (!(octaveReachM > Math.abs(place.heightM) - 3 * RING_LAYER_SIGMA_M) || !(high > sMin)) {
      continue;
    }
    // A cell of λ s_k holds 1.5 C λ² on a full octave; the last, cut at s_min, holds fewer.
    const lambda = cellsPerRadius(tauReference);
    const cellM = lambda * high;
    const perCell = (c / 2) * (low ** -2 - high ** -2) * cellM * cellM;
    const halfWindow = Math.min(Math.ceil(octaveReachM / cellM) + 1, MAX_HALF_WINDOW);
    const x = xc / cellM;
    const row = Math.floor(x);
    const y = yc / cellM + shear * (row + 0.5);
    const column = Math.floor(y);
    levels.push({
      cellM,
      radiusLowM: low,
      radiusHighM: high,
      perCell,
      row,
      rowFraction: x - row,
      column: wrapU32(column),
      columnFraction: y - column,
      halfWindow,
      firstCell: cells,
    });
    cells += (2 * halfWindow + 1) ** 2;
  }
  if (levels.length === 0) {
    return null;
  }
  const n = normalise(input.normal);
  return {
    axes: [place.radial, place.prograde, n],
    heightM: place.heightM,
    sigmaM: RING_LAYER_SIGMA_M,
    pixelRad: input.pixelRad,
    shearWhole: wrapU32(Math.floor(shear)),
    shearFraction: shear - Math.floor(shear),
    tauReference,
    starSine: input.towardsStar === null ? 0 : dot(normalise(input.towardsStar), n),
    edgesM: bands.edgesM,
    steps: bands.steps.map((step) => ({ tauRatio: step.tau / tauReference, albedo: step.albedo })),
    seed: input.seed >>> 0,
    levels,
    cells,
    planes: input.planes,
  };
}

/** The most frustum planes the placement reads. */
export const RING_FIELD_PLANES = 6;

/**
 * Bytes of `ringParticles.wgsl`'s `FieldParams`: its seven header rows, its planes and its steps,
 * then its octaves.
 */
export const RING_FIELD_PARAMS_BYTES =
  16 * (7 + RING_FIELD_PLANES + RING_FIELD_STEPS + 1) + 48 * RING_FIELD_LEVELS;

/** Where the steps' rows begin, and the octaves'. */
const STEPS_AT = 16 * (7 + RING_FIELD_PLANES);
const LEVELS_AT = STEPS_AT + 16 * (RING_FIELD_STEPS + 1);

/** The field as `ringParticles.wgsl`'s `FieldParams` lays it out. */
export function packRingFieldParams(field: RingField, capacity: number): ArrayBuffer {
  const out = new ArrayBuffer(RING_FIELD_PARAMS_BYTES);
  const view = new DataView(out);
  const f32 = (at: number, v: number): void => {
    view.setFloat32(at, v, true);
  };
  const u32 = (at: number, v: number): void => {
    view.setUint32(at, v >>> 0, true);
  };
  const [ex, ey, ez] = field.axes;
  const rows: ReadonlyArray<readonly [Vec3, number]> = [
    [ex, field.heightM],
    [ey, field.pixelRad],
    [ez, field.sigmaM],
  ];
  rows.forEach(([axis, w], k) => {
    f32(16 * k, axis.x);
    f32(16 * k + 4, axis.y);
    f32(16 * k + 8, axis.z);
    f32(16 * k + 12, w);
  });
  for (let k = 0; k < RING_FIELD_STEPS; k += 1) {
    f32(48 + 4 * k, field.edgesM[k] ?? NO_EDGE_M);
  }
  u32(64, field.levels.length);
  u32(68, capacity);
  u32(72, field.cells);
  u32(76, field.seed);
  u32(80, field.shearWhole);
  u32(84, Math.min(field.planes.length, RING_FIELD_PLANES));
  f32(96, field.shearFraction);
  f32(100, field.tauReference);
  f32(104, field.starSine);
  field.planes.slice(0, RING_FIELD_PLANES).forEach((plane, k) => {
    const at = 112 + 16 * k;
    f32(at, plane.normal.x);
    f32(at + 4, plane.normal.y);
    f32(at + 8, plane.normal.z);
    f32(at + 12, plane.offsetM);
  });
  field.steps.slice(0, RING_FIELD_STEPS + 1).forEach((step, k) => {
    const at = STEPS_AT + 16 * k;
    f32(at, step.albedo[0]);
    f32(at + 4, step.albedo[1]);
    f32(at + 8, step.albedo[2]);
    f32(at + 12, step.tauRatio);
  });
  field.levels.forEach((level, k) => {
    const at = LEVELS_AT + 48 * k;
    f32(at, level.cellM);
    f32(at + 4, level.radiusLowM);
    f32(at + 8, level.radiusHighM);
    f32(at + 12, level.perCell);
    view.setInt32(at + 16, level.row, true);
    u32(at + 20, level.column);
    u32(at + 24, level.halfWindow);
    u32(at + 28, level.firstCell);
    f32(at + 32, level.rowFraction);
    f32(at + 36, level.columnFraction);
  });
  return out;
}

/** The PCG hash's output of `v`, a `u32` (Jarzynski and Olano 2020's `pcg`). */
export function pcg(v: number): number {
  const state = (Math.imul(v >>> 0, 747_796_405) + 2_891_336_453) >>> 0;
  const shift = ((state >>> 28) + 4) >>> 0;
  const word = Math.imul(((state >>> shift) ^ state) >>> 0, 277_803_737) >>> 0;
  return ((word >>> 22) ^ word) >>> 0;
}

/** A number in [0, 1) from a hash's top 24 bits. */
function unitOf(h: number): number {
  return (h >>> 8) / 16_777_216;
}

/** One particle of the field as its twin places it, with the cell it is placed in. */
export interface PlacedParticle extends ParticleInstance {
  readonly level: number;
}

/** The step of `field` at radial offset `xM` from the camera: the edges at or below it. */
function stepAt(field: RingField, xM: number): number {
  return field.edgesM.filter((edge) => edge <= xM).length;
}

/** The field's optical depth at radial offset `xM` from the camera, as its steps hold it. */
export function fieldTauAt(field: RingField, xM: number): number {
  return field.tauReference * (field.steps[stepAt(field, xM)]?.tauRatio ?? 0);
}

/** The greatest τ ratio of the steps the radial span [a, b] meets. */
function mostRatio(field: RingField, aM: number, bM: number): number {
  let most = 0;
  for (let k = stepAt(field, aM); k <= stepAt(field, bM); k += 1) {
    most = Math.max(most, field.steps[k]?.tauRatio ?? 0);
  }
  return most;
}

/** Whether a sphere of `radiusM` about `centreM` meets every plane's inside. */
function inView(planes: ReadonlyArray<CullPlane>, centreM: Vec3, radiusM: number): boolean {
  return planes.every((plane) => dot(plane.normal, centreM) + plane.offsetM >= -radiusM);
}

/** The bounds scale of the templates the particles are drawn by, the instance cull's own. */
const BOUNDS_SCALE = sphereParticleTemplates().boundsScale;

/**
 * The particles of one cell of octave `level`, `di` rows and `dj` columns from the camera's, as
 * `ringParticles.wgsl`'s thread for it places them, in `f64`: those instanced at their distance and
 * in view.
 */
export function cellParticles(
  field: RingField,
  level: number,
  di: number,
  dj: number,
): PlacedParticle[] {
  const octave = field.levels[level];
  if (octave === undefined) {
    return [];
  }
  const [ex, ey, ez] = field.axes;
  const l = octave.cellM;
  // The camera's sheared column at this row: its own row's and the shear's S per row since.
  const along = octave.columnFraction + field.shearFraction * di;
  const columnHere = wrapU32(
    octave.column + Math.floor(along) + Math.imul(field.shearWhole, di | 0),
  );
  const fractionHere = along - Math.floor(along);
  const rowHere = (octave.row + di) | 0;
  const columnCell = wrapU32(columnHere + dj);
  const cellSeed = pcg((level ^ pcg((rowHere >>> 0) ^ pcg(columnCell ^ pcg(field.seed)))) >>> 0);
  const xCell = (di - octave.rowFraction) * l;
  const yCell = (dj - fractionHere) * l;
  // The cell's own bounds against the view and the octave's reach.
  const centreCell = add(
    add(scale(ex, xCell + l / 2), scale(ey, yCell + l / 2)),
    scale(ez, -field.heightM),
  );
  const cellRadius = l * Math.SQRT1_2 + 3 * field.sigmaM + octave.radiusHighM;
  const reach = (2 * octave.radiusHighM) / field.pixelRad;
  if (norm(centreCell) - cellRadius > reach || !inView(field.planes, centreCell, cellRadius)) {
    return [];
  }
  // Drawn at the greatest ratio the cell's width meets, each kept at its own step's over that.
  const most = mostRatio(field, xCell, xCell + l);
  const count = Math.min(
    Math.floor(octave.perCell * most + unitOf(cellSeed)),
    RING_FIELD_MAX_PER_CELL,
  );
  const out: PlacedParticle[] = [];
  for (let m = 0; m < count; m += 1) {
    let h = pcg((cellSeed ^ pcg(m + 1)) >>> 0);
    const u: number[] = [];
    for (let k = 0; k < 10; k += 1) {
      h = pcg(h);
      u.push(unitOf(h));
    }
    const [ux = 0, uy = 0, z1 = 0, z2 = 0, us = 0, q1 = 0, q2 = 0, q3 = 0, shade = 0, keep = 0] = u;
    const step = field.steps[stepAt(field, xCell + ux * l)];
    if (step === undefined || !(keep * most < step.tauRatio)) {
      continue;
    }
    const z =
      field.sigmaM * Math.sqrt(-2 * Math.log(Math.max(z1, 1e-7))) * Math.cos(2 * Math.PI * z2);
    const low = octave.radiusLowM ** -2;
    const radiusM = (low - us * (low - octave.radiusHighM ** -2)) ** -0.5;
    const centreM = add(
      add(scale(ex, xCell + ux * l), scale(ey, yCell + uy * l)),
      scale(ez, z - field.heightM),
    );
    if (2 * radiusM < norm(centreM) * field.pixelRad) {
      continue;
    }
    if (!inView(field.planes, centreM, BOUNDS_SCALE * radiusM)) {
      continue;
    }
    const s = Math.sqrt(1 - q1);
    const t = Math.sqrt(q1);
    const grey = 0.85 + 0.3 * shade;
    const lit = beamThrough(field, step.tauRatio, z);
    out.push({
      level,
      centreM,
      radiusM,
      rotation: {
        x: s * Math.sin(2 * Math.PI * q2),
        y: s * Math.cos(2 * Math.PI * q2),
        z: t * Math.sin(2 * Math.PI * q3),
        w: t * Math.cos(2 * Math.PI * q3),
      },
      colour: packColour(
        [step.albedo[0] * grey, step.albedo[1] * grey, step.albedo[2] * grey],
        lit,
      ),
      tag: (cellSeed ^ (m + 1)) >>> 0,
    });
  }
  return out;
}

/** A share in [0, 1] as an `unorm8` byte. */
function unorm8(v: number): number {
  return Math.round(Math.min(Math.max(v, 0), 1) * 255);
}

/** An albedo per channel and the beam's share as `rgba8unorm`, red in the low byte. */
function packColour(rgb: Rgb, alpha: number): number {
  const byte = unorm8;
  return (byte(rgb[0]) | (byte(rgb[1]) << 8) | (byte(rgb[2]) << 16) | (byte(alpha) << 24)) >>> 0;
}

/**
 * erfc(x) by Abramowitz and Stegun 1964's 7.1.26 (|error| ≤ 1.5 × 10⁻⁷ in erf), as
 * `ringParticles.wgsl`'s `field_erfc` takes it.
 */
export function erfcApprox(x: number): number {
  const a = Math.abs(x);
  const t = 1 / (1 + 0.3275911 * a);
  const poly =
    t *
    (0.254829592 + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
  const tail = poly * Math.exp(-a * a);
  return x >= 0 ? tail : 2 - tail;
}

/**
 * The share of the star's beam that reaches a particle at height `zM` over the mid-plane: exp(−τ
 * Q(z_s ÷ σ) ÷ μ₀) with z_s its height towards the star (see the module's remarks); 0 with the star
 * in the plane, and with none (a `starSine` of 0), where no beam lights the particles.
 */
export function beamThrough(field: RingField, tauRatio: number, zM: number): number {
  const mu0 = Math.abs(field.starSine);
  if (field.starSine === 0) {
    return 0;
  }
  const towards = field.starSine > 0 ? zM : -zM;
  const beyond = 0.5 * erfcApprox(towards / (Math.SQRT2 * field.sigmaM));
  return Math.exp((-field.tauReference * tauRatio * beyond) / Math.max(mu0, 1e-3));
}

/** Every particle of the field, as its twin places it: for tests, at small fields. */
export function fieldParticles(field: RingField): PlacedParticle[] {
  const out: PlacedParticle[] = [];
  field.levels.forEach((octave, level) => {
    for (let di = -octave.halfWindow; di <= octave.halfWindow; di += 1) {
      for (let dj = -octave.halfWindow; dj <= octave.halfWindow; dj += 1) {
        out.push(...cellParticles(field, level, di, dj));
      }
    }
  });
  return out;
}
