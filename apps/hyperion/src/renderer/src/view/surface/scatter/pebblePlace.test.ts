import { describe, expect, it } from "vitest";

import { dot, norm, type Vec3 } from "../../../geometry/vec3";
import { rotate } from "../../camera/quaternion";
import { faceDifferencePositionF32 } from "../../terrain/gpu/vertexEmulation";
import { NO_PARENT_SLOT, patchTerms } from "../../terrain/gpu/uniforms";
import type { PatchKey } from "../../terrain/patchKey";
import {
  PEBBLE_LARGEST_DIAMETER_M,
  PEBBLE_OCTAVES,
  pebbleFade,
  pebbleLatticeLevels,
  proposalsPerM2,
} from "./pebbleLaw";
import {
  expectedPebblesBound,
  MAX_PEBBLE_ENTRIES,
  PEBBLE_BUDGET_SHARE,
  PEBBLE_TAG_PATCH_BITS,
  packPebbleEntries,
  packPebbleParams,
  packScatterPatches,
  PEBBLE_BLOCK_SIDE,
  PEBBLE_KERNELS,
  PEBBLE_PARAMS_BYTES,
  type PebbleTwinSlot,
  pebbleRotation,
  placePebblesCpu,
  pebbleFadeByte,
  pebbleTag,
  quadAreaM2,
  SCATTER_PATCH_BYTES,
  type ScatterBudget,
  scatterEntries,
  type ScatterPatchPlace,
} from "./pebblePlace";
import type { ScatterBox } from "./scatterCull";

/** A sphere of 1,000 km and its finest level's patch near the +x face's centre. */
const FIGURE = { equatorialRadiusM: 1e6, polarRadiusM: 1e6 } as const;
const LEVEL = 17;
const KEY: PatchKey = { face: 0, level: LEVEL, i: 2 ** (LEVEL - 1), j: 2 ** (LEVEL - 1) };

/** The patch's twin slot: its datum's own terms, flat at height 0, its first slot named whole. */
function flatSlot(): PebbleTwinSlot {
  const t = patchTerms(KEY, FIGURE, 0);
  const f = Math.fround;
  const v = (a: Vec3): Vec3 => ({ x: f(a.x), y: f(a.y), z: f(a.z) });
  const [a, e1, e2] = t.axes;
  const terms = {
    axisA: v(a),
    axisE1: v(e1),
    axisE2: v(e2),
    s0: f(t.st0[0]),
    t0: f(t.st0[1]),
    u0: f(t.uv0[0]),
    v0: f(t.uv0[1]),
    step: f(t.step),
    scale: v(t.scale),
    m0: v(t.m0),
    nu0: v(t.nu0),
    h0M: 0,
    straddles: t.straddles,
  };
  const weights = new Uint8Array(65 * 65 * 8);
  for (let n = 0; n < 65 * 65; n += 1) {
    weights[8 * n] = 255;
  }
  return {
    terms,
    ownOffset: (x, y) => faceDifferencePositionF32(terms, x, y, 0),
    morphOffset: (x, y) => faceDifferencePositionF32(terms, x, y, 0),
    weights,
    named: [true, false, false, false, false, false, false, false],
  };
}

/**
 * The patch's record, its origin `originM` from the camera, the body's axes the passes', its first
 * slot's rock abundance `k`.
 */
function record(originM: Vec3, k = 0.2) {
  return {
    originM,
    slot: 0,
    morphStartM: 0,
    morphEndM: 0,
    parentSlot: NO_PARENT_SLOT,
    parentQuadrant: 0,
    key: KEY,
    cells: [NO_PARENT_SLOT, 0, 0, 0] as const,
    abundances: [k, 0, 0, 0, 0, 0, 0, 0],
  };
}

const IDENTITY: readonly [Vec3, Vec3, Vec3] = [
  { x: 1, y: 0, z: 0 },
  { x: 0, y: 1, z: 0 },
  { x: 0, y: 0, z: 1 },
];

/** A budget that never binds. */
const UNBOUNDED: ScatterBudget = { density: 1, capacity: Number.POSITIVE_INFINITY };

/** The patch's place at `nearestM`, its area 64 m² and its largest k 0.2. */
function placeAt(nearestM: number): ScatterPatchPlace {
  return { key: KEY, nearestM, areaM2: 64, abundance: 0.2 };
}

/** The twin's pebbles over the flat patch seen from 1.5 m above its origin. */
function place(k: number, density: number, bodies: ReadonlyArray<ScatterBox> = []) {
  const patches = [record({ x: -1.5, y: 0, z: 0 }, k)];
  const patchM = (64 * 2 * Math.PI * 1e6) / 4 / 2 ** LEVEL;
  const levels = pebbleLatticeLevels(LEVEL, patchM);
  const pxPerRad = 1000;
  const entries = scatterEntries([placeAt(1.5)], levels, pxPerRad, UNBOUNDED);
  return placePebblesCpu({
    params: {
      rotationRows: IDENTITY,
      pxPerRad,
      density,
      entries: entries.length,
      capacity: 1_000_000,
      seed: 0x1234,
      bodies: bodies.length,
    },
    entries,
    patches,
    bodies,
    slot: () => flatSlot(),
  });
}

describe("the pebbles' placement", () => {
  it("dispatches each octave whose largest pebble shows at a patch's nearest point, in blocks of 16²", () => {
    const levels = pebbleLatticeLevels(LEVEL, 32);
    const near = scatterEntries([placeAt(1)], levels, 1000, UNBOUNDED);
    // Every octave near the camera, each its blocks.
    const octaves = new Set(near.map((e) => e.octave));
    expect(octaves.size).toBe(PEBBLE_OCTAVES.length);
    for (const e of near) {
      const side = 2 ** e.cellLevels;
      expect(e.block).toBeLessThan(Math.max(1, side / PEBBLE_BLOCK_SIDE) ** 2);
    }
    // At 90 m only the octaves whose largest spans more than 2 px: 0.4 m and 0.2 m at 1000 px/rad;
    // at 100 m the second's spans 2 px, where its fade is 0.
    const far = scatterEntries([placeAt(90)], levels, 1000, UNBOUNDED);
    expect(new Set(far.map((e) => e.octave))).toEqual(new Set([0, 1]));
    const farther = scatterEntries([placeAt(100)], levels, 1000, UNBOUNDED);
    expect(new Set(farther.map((e) => e.octave))).toEqual(new Set([0]));
    expect(scatterEntries([placeAt(1e4)], levels, 1000, UNBOUNDED)).toHaveLength(0);
    // Whole patches only, within the entries' cap.
    const many = scatterEntries(
      Array.from({ length: 2_000 }, () => placeAt(1)),
      levels,
      1000,
      UNBOUNDED,
    );
    expect(many.length).toBeLessThanOrEqual(MAX_PEBBLE_ENTRIES);
    expect(many.length % near.length).toBe(0);
  });

  it("bounds a patch's pebbles above by its area, its largest k and its octaves' least diameters", () => {
    // At k = 0.4 and the least diameter of the finest octave, almost every proposal is kept.
    const rocky = { nearestM: 1, areaM2: 10, abundance: 0.4 };
    const [low] = PEBBLE_OCTAVES[4] ?? [];
    expect(expectedPebblesBound(rocky, 4, 1)).toBeCloseTo(
      10 * proposalsPerM2(4) * Math.exp(-(1.79 + 0.152 / 0.4) * (low ?? 0)),
      9,
    );
    // Linear in the density and the area, rising with k, none on a bare ground.
    expect(expectedPebblesBound(rocky, 4, 0.5)).toBeCloseTo(expectedPebblesBound(rocky, 4, 1) / 2);
    expect(expectedPebblesBound({ ...rocky, areaM2: 20 }, 0, 1)).toBeCloseTo(
      2 * expectedPebblesBound(rocky, 0, 1),
    );
    expect(expectedPebblesBound({ ...rocky, abundance: 0.05 }, 0, 1)).toBeLessThan(
      expectedPebblesBound(rocky, 0, 1),
    );
    expect(expectedPebblesBound({ ...rocky, abundance: 0 }, 0, 1)).toBe(0);
    expect(expectedPebblesBound(rocky, 9, 1)).toBe(0);
  });

  it("takes the nearest patches whole while their bound stays within nine tenths of the capacity", () => {
    const levels = pebbleLatticeLevels(LEVEL, 32);
    const one = PEBBLE_OCTAVES.reduce(
      (sum, _, octave) => sum + expectedPebblesBound(placeAt(1), octave, 1),
      0,
    );
    const each = scatterEntries([placeAt(1)], levels, 1000, UNBOUNDED).length;
    // Room for three and a half patches' bound: three are taken, the nearest.
    const capacity = (3.5 * one) / PEBBLE_BUDGET_SHARE;
    const taken = scatterEntries(
      Array.from({ length: 10 }, (_, n) => placeAt(1 + n)),
      levels,
      1000,
      { density: 1, capacity },
    );
    expect(taken).toHaveLength(3 * each);
    expect(new Set(taken.map((e) => e.patch))).toEqual(new Set([0, 1, 2]));
    // At half the density, each patch's bound halves: seven are taken.
    expect(
      new Set(
        scatterEntries(
          Array.from({ length: 10 }, (_, n) => placeAt(1 + n)),
          levels,
          1000,
          { density: 0.5, capacity },
        ).map((e) => e.patch),
      ).size,
    ).toBe(7);
    // A patch that bears no pebbles costs nothing.
    expect(
      scatterEntries(
        Array.from({ length: 10 }, () => ({ ...placeAt(1), abundance: 0 })),
        levels,
        1000,
        { density: 1, capacity: 1 },
      ),
    ).toHaveLength(10 * each);
  });

  it("carries a pebble's fade as a byte above its patch's index in its tag", () => {
    expect([pebbleFadeByte(0), pebbleFadeByte(1), pebbleFadeByte(0.5), pebbleFadeByte(2)]).toEqual([
      0, 255, 128, 255,
    ]);
    expect(pebbleFadeByte(0.5 / 255 - 1e-6)).toBe(0);
    expect(pebbleTag(1023, 255)).toBe(1023 + 255 * 2 ** PEBBLE_TAG_PATCH_BITS);
    expect(pebbleTag(7, 0)).toBe(7);
  });

  it("packs its records at the kernel's sizes", () => {
    const patches = new ArrayBuffer(2 * SCATTER_PATCH_BYTES);
    packScatterPatches([record({ x: 1, y: 2, z: 3 }, 0.25), record({ x: 4, y: 5, z: 6 })], patches);
    const f = new Float32Array(patches);
    const u = new Uint32Array(patches);
    expect([f[0], f[1], f[2], u[3]]).toEqual([1, 2, 3, 0]);
    expect([u[6], u[8], u[9], u[10], u[11], u[12]]).toEqual([
      NO_PARENT_SLOT,
      KEY.face,
      KEY.level,
      KEY.i,
      KEY.j,
      NO_PARENT_SLOT,
    ]);
    // Its slots' rock abundances after its sixteen words of terms, two `vec4f`.
    expect(Array.from(f.subarray(16, 24))).toEqual([0.25, 0, 0, 0, 0, 0, 0, 0]);
    expect([f[24], f[40]]).toEqual([4, Math.fround(0.2)]);
    const entries = new Uint32Array(8);
    packPebbleEntries(
      [
        { patch: 3, octave: 2, cellLevels: 6, block: 9 },
        { patch: 4, octave: 0, cellLevels: 4, block: 0 },
      ],
      entries,
    );
    expect(Array.from(entries)).toEqual([3, 2, 6, 9, 4, 0, 4, 0]);
    const params = new ArrayBuffer(PEBBLE_PARAMS_BYTES);
    packPebbleParams(
      {
        rotationRows: IDENTITY,
        pxPerRad: 900,
        density: 2,
        entries: 5,
        capacity: 6,
        seed: 0xffff_ffff,
        bodies: 1,
      },
      params,
    );
    const pf = new Float32Array(params);
    const pu = new Uint32Array(params);
    expect([pf[0], pf[5], pf[10], pf[12], pf[13]]).toEqual([1, 1, 1, 900, 1]);
    expect([pu[16], pu[17], pu[18], pu[19]]).toEqual([5, 6, 0xffff_ffff, 1]);
  });

  it("turns a pebble's +z onto the ground's normal, about it by its yaw", () => {
    for (const up of [
      { x: 0, y: 0, z: 1 },
      { x: 0, y: 0, z: -1 },
      { x: 0.6, y: 0, z: 0.8 },
      { x: -0.48, y: 0.6, z: -0.64 },
    ]) {
      for (const yaw of [0, 1, 4]) {
        const q = pebbleRotation(up, yaw);
        expect(Math.hypot(q.x, q.y, q.z, q.w)).toBeCloseTo(1, 12);
        const z = rotate(q, { x: 0, y: 0, z: 1 });
        expect(dot(z, up)).toBeCloseTo(1, 9);
        // Its x lies in the ground's plane, turned by the yaw from the zero yaw's.
        const x = rotate(q, { x: 1, y: 0, z: 0 });
        const x0 = rotate(pebbleRotation(up, 0), { x: 1, y: 0, z: 0 });
        expect(dot(x, up)).toBeCloseTo(0, 9);
        expect(dot(x, x0)).toBeCloseTo(Math.cos(yaw), 9);
      }
    }
  });

  it("measures a quad's area on the datum", () => {
    const { terms } = flatSlot();
    const side = (2 * Math.PI * 1e6) / 4 / 2 ** (LEVEL + 6);
    // Near the face's centre a quad is about (π a ÷ 2) ÷ 2^(n+6) across, the warp's rate there 4/3
    // of u's (`st_to_uv`) against the arc's 1.
    const area = quadAreaM2(terms, [32, 32]);
    expect(area / side ** 2).toBeGreaterThan(0.5);
    expect(area / side ** 2).toBeLessThan(2);
  });

  it("places pebbles on the ground below 0.4 m, each its patch's, culled by a body's box", () => {
    const pebbles = place(0.2, 1);
    expect(pebbles.length).toBeGreaterThan(100);
    for (const p of pebbles) {
      expect(p.diameterM).toBeLessThan(PEBBLE_LARGEST_DIAMETER_M);
      // Drawn at its whole size, its fade in its tag above patch 0's index.
      expect(p.radiusM).toBe(p.diameterM / 2);
      const fadeByte = p.tag >>> PEBBLE_TAG_PATCH_BITS;
      expect(p.tag & (2 ** PEBBLE_TAG_PATCH_BITS - 1)).toBe(0);
      expect(fadeByte).toBeGreaterThan(0);
      expect(fadeByte).toBe(pebbleFadeByte(pebbleFade(p.diameterM, norm(p.centreM), 1000)));
      // On the datum, 1.5 m below the camera, to the patch's curvature (some 2 mm across it).
      const fromCentre = { x: p.centreM.x + 1e6 + 1.5, y: p.centreM.y, z: p.centreM.z };
      expect(Math.abs(norm(fromCentre) - 1e6)).toBeLessThan(5e-3);
      // Its place's word holds its fractions of the patch.
      const u = (p.colour & 0xfff) / 4095;
      const v = ((p.colour >>> 12) & 0xfff) / 4095;
      expect(u).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThanOrEqual(1);
    }
    // The twin keeps fewer than its bound on average over the patch, at every octave together, its
    // area its 64² quads, alike near the face's centre: some 4 % fewer, the finest octave's least
    // diameters, which the bound takes, holding most of its pebbles.
    const areaM2 = 64 * 64 * quadAreaM2(flatSlot().terms, [32, 32]);
    const bound = PEBBLE_OCTAVES.reduce(
      (sum, _, octave) =>
        sum + expectedPebblesBound({ nearestM: 1.5, areaM2, abundance: 0.2 }, octave, 1),
      0,
    );
    expect(pebbles.length).toBeLessThan(bound);
    // A box over part of the patch takes the pebbles under it.
    const box: ScatterBox = {
      centreM: { x: -1.5, y: 0, z: 0 },
      axes: [
        { x: 0, y: 1, z: 0 },
        { x: 0, y: 0, z: 1 },
        { x: 1, y: 0, z: 0 },
      ],
      halfExtentsM: [4, 4, 0.5],
    };
    const culled = place(0.2, 1, [box]);
    expect(culled.length).toBeLessThan(pebbles.length);
    expect(culled.every((p) => Math.abs(p.centreM.y) > 4 || Math.abs(p.centreM.z) > 4)).toBe(true);
    // The bare ground of no rocks, none; half the density, about half as many.
    expect(place(0, 1)).toHaveLength(0);
    const half = place(0.2, 0.5).length;
    expect(Math.abs(half - pebbles.length / 2)).toBeLessThan(
      4.5 * Math.sqrt(pebbles.length / 4) + 2,
    );
  });

  it("compiles a kernel for each vertex path, the path's buffer at binding 7 of group 0", () => {
    expect(PEBBLE_KERNELS["face-differences"].reference).toContain(
      "@group(0) @binding(7) var<storage, read> heights",
    );
    expect(PEBBLE_KERNELS["baked-offsets"].reference).toContain(
      "@group(0) @binding(7) var<storage, read> offsets",
    );
    for (const kernel of Object.values(PEBBLE_KERNELS)) {
      expect(kernel.reference).toContain(
        `const PEBBLE_TAG_PATCH_BITS : u32 = ${String(PEBBLE_TAG_PATCH_BITS)}u;`,
      );
      expect(kernel.reference).toContain("floor(clamp(fade, 0.0, 1.0) * 255.0 + 0.5)");
      expect(kernel.reference).not.toContain("@group(2)");
      expect(kernel.readback).toBe("presentation-only");
      expect(kernel.reference.match(/@compute/gu)).toHaveLength(1);
    }
  });
});
