import { describe, expect, it } from "vitest";

import { add, cross, dot, norm, normalise, type Vec3, vec3 } from "../../geometry/vec3";
import { PROVISIONAL_PHOTOMETRY } from "../appearance/fromWire";
import type { LitBodyInput } from "../bodies/draw";
import type { ViewRing } from "../scene/model";
import {
  beamThrough,
  cellParticles,
  erfcApprox,
  fieldWindowPixelRad,
  fieldParticles,
  packRingFieldParams,
  pcg,
  RING_FIELD_LEVELS,
  RING_FIELD_PARAMS_BYTES,
  RING_LAYER_SIGMA_M,
  type RingField,
  ringFieldOf,
} from "./ringField";
import { ringProfile } from "./ringProfile";
import { profileTauScale, ringProfileTexels, ringSlabFragment, ringSlabRecordOf } from "./ringSlab";
import { instancedShare, RING_PARTICLE_RADII_M } from "./ringSplit";

const A_M = 6e7;
const R0_M = 1e8;
const PLANET = "0300080020000000.0201";
const NORMAL = vec3(0, 0, 1);
/** Saturn's GM, m³ s⁻² (from memory; any figure serves the shear's test). */
const GM = 3.7931e16;

/** A point's radial and prograde place on a field's sheet about the camera, m. */
function sheet(field: RingField, p: Vec3): readonly [number, number] {
  return [dot(p, field.axes[0]), dot(p, field.axes[1])];
}

/** A uniform massive ring about the reference radius, 2,000 km wide, of optical depth `tau`. */
function ringOf(tau: number, gapRadiiM: ReadonlyArray<number> = []): ViewRing {
  return {
    body: PLANET,
    ringKind: "massive",
    material: "porous_ice",
    innerRadiusM: R0_M - 1e6,
    outerRadiusM: R0_M + 1e6,
    opticalDepth: tau,
    gapRadiiM,
    normal: NORMAL,
  };
}

/** The planet's centre from a camera `heightM` over the plane at the reference radius plus `xM`. */
function centreFrom(heightM: number, xM = 0): Vec3 {
  return vec3(-(R0_M + xM), 0, -heightM);
}

/** A field of `ring` seen from `heightM` over it, at the pixel's angle `pixelRad`, all cells kept. */
function fieldOf(
  ring: ViewRing,
  heightM: number,
  pixelRad: number,
  options: { readonly sinceS?: number; readonly xM?: number; readonly gm?: number | null } = {},
): RingField | null {
  return ringFieldOf({
    centreM: centreFrom(heightM, options.xM ?? 0),
    normal: NORMAL,
    profile: ringProfile(ring),
    gravitationalParameterM3S2: options.gm ?? null,
    referenceRadiusM: R0_M,
    sinceS: options.sinceS ?? 0,
    pixelRad,
    planes: [],
    seed: 7,
    towardsStar: vec3(0.3, 0.2, 0.93),
  });
}

/** The cells of octave `level` whose squares meet a box of half-width `halfM` about (x, y), sheet m. */
function cellsAbout(
  field: RingField,
  level: number,
  xM: number,
  yM: number,
  halfM: number,
): Array<readonly [number, number]> {
  const octave = field.levels[level];
  if (octave === undefined) {
    return [];
  }
  const l = octave.cellM;
  const cells: Array<readonly [number, number]> = [];
  const first = (v: number, frac: number): number => Math.floor((v - halfM) / l + frac) - 1;
  const last = (v: number, frac: number): number => Math.floor((v + halfM) / l + frac) + 1;
  for (let di = first(xM, octave.rowFraction); di <= last(xM, octave.rowFraction); di += 1) {
    // With no shear, every row's fraction is the camera's own.
    for (
      let dj = first(yM, octave.columnFraction);
      dj <= last(yM, octave.columnFraction);
      dj += 1
    ) {
      cells.push([di, dj]);
    }
  }
  return cells;
}

describe("a ring's particle field close to (R11.T9.d)", () => {
  it("places nothing beyond its largest particle's hand-over", () => {
    const pixelRad = Math.PI / 3 / 1920;
    const handOverM = (2 * RING_PARTICLE_RADII_M[1]) / pixelRad;
    expect(fieldOf(ringOf(1), handOverM + 4 * RING_LAYER_SIGMA_M, pixelRad)).toBeNull();
    expect(fieldOf(ringOf(1), handOverM / 2, pixelRad)).not.toBeNull();
  });

  it("is the same field whatever the order its cells are placed in, each cell its own", () => {
    const field = fieldOf(ringOf(1), 200, 0.01);
    if (field === null) {
      throw new Error("no field");
    }
    const octave = field.levels[0];
    if (octave === undefined) {
      throw new Error("no octave");
    }
    const span = Array.from({ length: 2 * octave.halfWindow + 1 }, (_, k) => k - octave.halfWindow);
    const tags = (cells: ReadonlyArray<readonly [number, number]>): number[] =>
      cells.flatMap(([di, dj]) => cellParticles(field, 0, di, dj).map((p) => p.tag));
    const inOrder = span.flatMap((di) => span.map((dj): readonly [number, number] => [di, dj]));
    // A fixed shuffle: the cells sorted by a hash of their place in the window.
    const shuffled = inOrder
      .map((cell, k) => ({ cell, key: pcg(k) }))
      .toSorted((a, b) => a.key - b.key)
      .map(({ cell }) => cell);
    expect(new Set(tags(shuffled))).toEqual(new Set(tags(inOrder)));
    // A neighbour's tags are its own: another seed changes them all.
    const reseeded = { ...field, seed: field.seed + 1 };
    const own = cellParticles(field, 0, 0, 1).map((p) => p.tag);
    const other = new Set(cellParticles(reseeded, 0, 0, 1).map((p) => p.tag));
    expect(own.length).toBeGreaterThan(0);
    expect(own.some((tag) => other.has(tag))).toBe(false);
  });

  it("places each particle only where its diameter fills its own distance's pixel", () => {
    const pixelRad = 0.02;
    const field = fieldOf(ringOf(1), 300, pixelRad);
    if (field === null) {
      throw new Error("no field");
    }
    const particles = fieldParticles(field);
    expect(particles.length).toBeGreaterThan(100);
    expect(particles.every((p) => 2 * p.radiusM >= norm(p.centreM) * pixelRad)).toBe(true);
  });

  it("stops at the annulus's edge and holds a gap's thinner population", () => {
    const pixelRad = 0.02;
    const ring = ringOf(1, [R0_M + 2e3]);
    // From over the gap's inner edge: the ring's τ inside it, a tenth of it across the gap.
    const field = fieldOf(ring, 200, pixelRad, { xM: 2e3 });
    const edgeField = fieldOf(ring, 200, pixelRad, { xM: -1e6 + 300 });
    if (field === null || edgeField === null) {
      throw new Error("no field");
    }
    const inside = fieldParticles(edgeField).filter((p) => p.centreM.x < -300);
    expect(inside).toEqual([]);
    const all = fieldParticles(field);
    const before = all.filter((p) => p.centreM.x > -600 && p.centreM.x < -100).length;
    const across = all.filter((p) => p.centreM.x > 100 && p.centreM.x < 600).length;
    expect(across / before).toBeGreaterThan(0.05);
    expect(across / before).toBeLessThan(0.2);
  });

  it("shears as Hill's flow does: against the frame turning at Ω₀, each row drifts at −(3/2) Ω₀ x at its middle", () => {
    const pixelRad = 0.02;
    const ring = ringOf(1);
    const omega = Math.sqrt(GM / R0_M ** 3);
    // A camera 200 m over r₀ that orbits with the frame, so that the field drifts past it at Hill's
    // shear alone, some 0.13 m/s at 460 m out: over 100 s, metres.
    const dt = 100;
    const at = (sinceS: number): RingField | null => {
      const phi = omega * sinceS;
      return ringFieldOf({
        centreM: vec3(-R0_M * Math.cos(phi), -R0_M * Math.sin(phi), -200),
        normal: NORMAL,
        profile: ringProfile(ring),
        gravitationalParameterM3S2: GM,
        referenceRadiusM: R0_M,
        sinceS,
        pixelRad,
        planes: [],
        seed: 7,
        towardsStar: vec3(0.3, 0.2, 0.93),
      });
    };
    const before = at(1_000);
    const after = at(1_000 + dt);
    if (before === null || after === null) {
      throw new Error("no field");
    }
    const ends = new Map(fieldParticles(after).map((p) => [p.tag, sheet(after, p.centreM)]));
    const drifts: Array<readonly [number, number]> = [];
    let worstRow = 0;
    let worstOwn = 0;
    for (const p of fieldParticles(before)) {
      const end = ends.get(p.tag);
      const cellM = before.levels[p.level]?.cellM;
      if (end === undefined || cellM === undefined) {
        continue;
      }
      const [x, y] = sheet(before, p.centreM);
      const drift = (end[1] - y) / dt;
      // The camera stands at r₀, so x is the particle's own offset from it; its row's middle.
      const middle = (Math.floor(x / cellM) + 0.5) * cellM;
      worstRow = Math.max(worstRow, Math.abs(drift + 1.5 * omega * middle));
      worstOwn = Math.max(worstOwn, Math.abs(drift + 1.5 * omega * x) / (1.5 * omega * cellM));
      drifts.push([x, drift]);
    }
    // Its row's drift to 10⁻⁶ m/s, its own within its row's half-width of Hill's at its own x.
    expect(worstRow).toBeLessThan(1e-6);
    expect(worstOwn).toBeLessThanOrEqual(0.5 + 1e-6);
    // The slope dv/dx over the field, Hill's −(3/2) Ω₀ to 5%, rows several cells out on both sides.
    const n = drifts.length;
    const mx = drifts.reduce((sum, [x]) => sum + x, 0) / n;
    const mv = drifts.reduce((sum, [, v]) => sum + v, 0) / n;
    const sxx = drifts.reduce((sum, [x]) => sum + (x - mx) ** 2, 0);
    const sxv = drifts.reduce((sum, [x, v]) => sum + (x - mx) * (v - mv), 0);
    expect(n).toBeGreaterThan(100);
    expect(Math.max(...drifts.map(([x]) => x))).toBeGreaterThan(300);
    expect(Math.min(...drifts.map(([x]) => x))).toBeLessThan(-300);
    expect(Math.abs(sxv / sxx / (-1.5 * omega) - 1)).toBeLessThan(0.05);
  });

  it("keeps every octave's window within its cap at the least pixel the cap allows, a dense ring's", () => {
    const tau = 10;
    const pixelRad = fieldWindowPixelRad(tau);
    expect(pixelRad).toBeGreaterThan(Math.PI / 3 / 1920);
    const field = fieldOf(ringOf(tau), 200, pixelRad);
    if (field === null) {
      throw new Error("no field");
    }
    for (const octave of field.levels) {
      const reach = (2 * octave.radiusHighM) / pixelRad;
      expect(Math.ceil(reach / octave.cellM) + 1).toBeLessThanOrEqual(octave.halfWindow);
    }
  });

  it("lays its parameters out as ringParticles.wgsl reads them", () => {
    const field = fieldOf(ringOf(1), 200, 0.02);
    if (field === null) {
      throw new Error("no field");
    }
    const params = packRingFieldParams(field, 1024);
    const view = new DataView(params);
    expect(params.byteLength).toBe(RING_FIELD_PARAMS_BYTES);
    expect([view.getUint32(64, true), view.getUint32(68, true), view.getUint32(72, true)]).toEqual([
      field.levels.length,
      1024,
      field.cells,
    ]);
    expect(field.levels.length).toBeLessThanOrEqual(RING_FIELD_LEVELS);
    expect(view.getFloat32(12, true)).toBeCloseTo(200, 3);
  });
});

/** The planet as the slab's record reads it. */
const PLANET_BODY: LitBodyInput = {
  id: PLANET,
  centreM: vec3(0, 0, 0),
  figure: { equatorialRadiusM: A_M, polarRadiusM: A_M, pole: NORMAL },
  photometry: PROVISIONAL_PHOTOMETRY,
  lighting: undefined,
};

/** A particle as the rays test it. */
interface Sphere {
  readonly centreM: Vec3;
  readonly radiusM: number;
}

/** The bins' side, m, by which the rays find the particles near their path. */
const BIN_M = 4;

/** A bin's key. */
function binKey(i: number, j: number): number {
  return (i + 50_000) * 100_000 + (j + 50_000);
}

/** The particles binned by their place on the sheet. */
function binned(particles: ReadonlyArray<Sphere>): Map<number, Sphere[]> {
  const bins = new Map<number, Sphere[]>();
  for (const p of particles) {
    const key = binKey(Math.floor(p.centreM.x / BIN_M), Math.floor(p.centreM.y / BIN_M));
    const bin = bins.get(key) ?? [];
    bin.push(p);
    bins.set(key, bin);
  }
  return bins;
}

/**
 * Whether the unit ray from the camera meets any particle's sphere ahead: those of the bins about
 * its track through the layer, ±4σ about the plane, where every centre lies, the bins reaching past
 * the largest radius.
 */
function blocked(ray: Vec3, heightM: number, bins: Map<number, Sphere[]>): boolean {
  const depthM = 4 * RING_LAYER_SIGMA_M;
  const tIn = (heightM - depthM) / -ray.z;
  const tOut = (heightM + depthM) / -ray.z;
  const keys = new Set<number>();
  const reach = Math.ceil(RING_PARTICLE_RADII_M[1] / BIN_M);
  for (let t = tIn; t <= tOut + BIN_M; t += BIN_M / 2) {
    const i0 = Math.floor((ray.x * t) / BIN_M);
    const j0 = Math.floor((ray.y * t) / BIN_M);
    for (let i = i0 - reach; i <= i0 + reach; i += 1) {
      for (let j = j0 - reach; j <= j0 + reach; j += 1) {
        keys.add(binKey(i, j));
      }
    }
  }
  for (const key of keys) {
    for (const p of bins.get(key) ?? []) {
      if (dot(p.centreM, ray) > 0 && norm(cross(p.centreM, ray)) < p.radiusM) {
        return true;
      }
    }
  }
  return false;
}

describe("the star's beam through the layer to a particle (R11.T9.d)", () => {
  it("is erfc's, by Abramowitz and Stegun's 7.1.26, to 2 × 10⁻⁷", () => {
    for (const [x, expected] of [
      [0, 1],
      [0.5, 0.479500122],
      [1, 0.157299207],
      [2, 0.004677735],
      [-1, 1.842700793],
    ] as const) {
      expect(Math.abs(erfcApprox(x) - expected)).toBeLessThan(2e-7);
    }
  });

  it("is whole above the layer on the star's side, exp(−τ ÷ 2μ₀) at its mid-plane and exp(−τ ÷ μ₀) below it", () => {
    const field = fieldOf(ringOf(1.5), 200, 0.01);
    if (field === null) {
      throw new Error("no field");
    }
    const mu0 = Math.abs(field.starSine);
    const sigma = RING_LAYER_SIGMA_M;
    expect(beamThrough(field, 1, 6 * sigma)).toBeCloseTo(1, 6);
    expect(beamThrough(field, 1, 0)).toBeCloseTo(Math.exp(-1.5 / (2 * mu0)), 6);
    expect(beamThrough(field, 1, -6 * sigma)).toBeCloseTo(Math.exp(-1.5 / mu0), 6);
  });
});

describe("a split ring's light let through (R11.T9.d)", () => {
  it("is the unsplit slab's to 1% along 4 × 10⁴ random rays, the plan's 10⁴ and more: slab and particles together", () => {
    const tau = 0.5;
    const heightM = 300;
    const pixelRad = 1e-3;
    const ring = ringOf(tau);
    const field = fieldOf(ring, heightM, pixelRad);
    if (field === null) {
      throw new Error("no field");
    }
    // The rays aim at a 400 m square of the plane 520 m from the camera's foot, at μ = 0.5 at its
    // middle: wide enough to hold some 20,000 particles, whose shadows' cover then averages out.
    const target = vec3(-520, 0, -heightM);
    const halfM = 200;
    const particles = field.levels.flatMap((_, level) =>
      cellsAbout(field, level, target.x, target.y, halfM + 80).flatMap(([di, dj]) =>
        cellParticles(field, level, di, dj),
      ),
    );
    const bins = binned(particles);
    const centreM = centreFrom(heightM);
    const record = ringSlabRecordOf(ring, { ...PLANET_BODY, centreM }, [], [], {
      annuli: 4,
      exposureScale: 1,
      instancedPixelRad: pixelRad,
    });
    const unsplit = ringSlabRecordOf(ring, { ...PLANET_BODY, centreM }, [], [], {
      annuli: 4,
      exposureScale: 1,
    });
    const texels = ringProfileTexels(ringProfile(ring));
    expect(profileTauScale(ringProfile(ring))).toBe(tau);
    let split = 0;
    let whole = 0;
    let state = 12_345;
    const next = (): number => {
      state = pcg(state);
      return state / 4_294_967_296;
    };
    // 4 × 10⁴ rather than 10⁴: their mean's own noise, 0.3% of it, then stands well inside the 1%.
    const rays = 40_000;
    for (let k = 0; k < rays; k += 1) {
      const ray = normalise(
        add(target, vec3(2 * halfM * (next() - 0.5), 2 * halfM * (next() - 0.5), 0)),
      );
      const slab = ringSlabFragment(record, texels, ray, 1e-9);
      const full = ringSlabFragment(unsplit, texels, ray, 1e-9);
      if (slab === null || full === null) {
        throw new Error("a ray missed the slab");
      }
      split += blocked(ray, heightM, bins) ? 0 : 1 - slab.alpha;
      whole += 1 - full.alpha;
    }
    // The share the particles took there, for the record of what the test holds.
    const share = instancedShare(norm(target) * pixelRad, ...RING_PARTICLE_RADII_M);
    expect(particles.length).toBeGreaterThan(10_000);
    expect(share).toBeGreaterThan(0.3);
    expect(Math.abs(split / whole - 1)).toBeLessThan(0.01);
  });
});
