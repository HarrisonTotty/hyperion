/**
 * Decorative scatter's placement kernel and its records (plan R11, R11.T6.a): `pebbles.wgsl` for
 * each vertex path, the records a frame writes for it, which patches and octaves it dispatches over,
 * and the CPU twin of what it places ({@link placePebblesCpu}), which the smoke harness holds the
 * kernel to.
 *
 * @remarks
 * - **A frame's work.** Each drawn patch of the body's finest levels, nearest the camera first, at
 *   most {@link MAX_SCATTER_PATCHES} ({@link scatterEntries}): for each octave whose largest pebble
 *   spans more than `PEBBLE_FADE_PX` at the patch's nearest point, the octave's lattice cells over
 *   the patch in blocks of {@link PEBBLE_BLOCK_SIDE}², a workgroup a block, at most
 *   {@link MAX_PEBBLE_ENTRIES}, whole patches only.
 * - **Its budget.** The patches are taken, nearest first, only while a bound of the pebbles they
 *   keep on average ({@link expectedPebblesBound}) stays within {@link PEBBLE_BUDGET_SHARE} of the
 *   instances' capacity: so the kernel's count all but never meets the capacity, past which which
 *   pebbles stay would be the device's order of its atomic adds, and the nearest ground, not a
 *   frame's luck, keeps its pebbles when the budget binds.
 * - **The patches' records** ({@link SCATTER_PATCH_BYTES}) are the terrain instance's own terms of
 *   the patch: its origin less the camera, in the passes' frame, narrowed once from `f64`, its slot,
 *   morph bands, parent and class sets, and its key, so that a pebble stands on the triangle of the
 *   patch's mesh that holds it, its morph by its distance as the terrain's is (but the holds at a
 *   coarse–fine edge, which move it by a centimetre or two at most), and its draw shades it as that
 *   ground (`pebbleDraw.wgsl`, which reads the record by the instance's tag).
 * - **The instance** is the particle path's record (`clusters/particles.ts`): its centre, its
 *   radius D ÷ 2, its rotation, its colour word (its place in the patch, two 12-bit fractions, and
 *   its tint's byte above them) and its tag ({@link pebbleTag}): the patch record's index, and its
 *   fade's byte above, by which its draw takes it over to its ground's shading (`pebbleLaw.ts`'
 *   falloff). A pebble whose fade's byte is 0 is not placed.
 * - **Arithmetic.** The kernel's `f32` is the device's (`presentation-only`): the twin takes the
 *   slot record's `FaceDifferences` terms in `f32` (`vertexEmulation.ts`) and the law in `f64`, so
 *   that a proposal at its keep's edge or a cell at its count's may fall either way; the smoke check
 *   holds the two to a stated share.
 */

import { cross, dot, normalise, type Vec3 } from "../../../geometry/vec3";
import type { Quaternion } from "../../camera/pose";
import { FOLDED_GROUP_WGSL, foldedWorkgroups } from "../../clusters/fold";
import type { KernelPair } from "../../engine/kernels";
import type { TerrainVertexPath } from "../../quality/qualitySetting";
import { PATH_SOURCES } from "../../terrain/gpu/material";
import {
  directionDifferenceF32,
  normalDifferenceF32,
  type SlotTermsF32,
} from "../../terrain/gpu/vertexEmulation";
import { type PatchKey, unreachable } from "../../terrain/patchKey";
import patchVertexWgsl from "../../terrain/shaders/patchVertex.wgsl?raw";
import { terrainShares, texelWeights } from "../../terrain/terrainMaterial";
import decorationWgsl from "../decoration/decoration.wgsl?raw";
import {
  PEBBLE_AXES,
  PEBBLE_FADE_PX,
  PEBBLE_OCTAVES,
  PEBBLE_REPOSE_DEG,
  cellProposals,
  pebbleAcceptance,
  pebbleCellHash,
  pebbleFade,
  proposalsPerM2,
  scatterDensity,
  slopeFilter,
} from "./pebbleLaw";
import pebblesWgsl from "./pebbles.wgsl?raw";
import { intersectsBody, type ScatterBox, SCATTER_CULL_WGSL } from "./scatterCull";

/** The cells along a block's side: a workgroup's block of an octave's cells, 16 × 16. */
export const PEBBLE_BLOCK_SIDE = 16;

/** The most patches a frame scatters over, nearest first. */
export const MAX_SCATTER_PATCHES = 1024;

/** The most entries, blocks of cells, a frame dispatches. */
export const MAX_PEBBLE_ENTRIES = 32_768;

/** Bytes of `pebbles.wgsl`'s `PebbleParams`. */
export const PEBBLE_PARAMS_BYTES = 80;

/** Bytes of the octaves' uniform: five `vec4f`, low, high and proposals per m². */
export const PEBBLE_OCTAVES_BYTES = 16 * PEBBLE_OCTAVES.length;

/** Bytes of one `PebbleEntry`: the patch, the octave, its cells' levels and its block. */
export const PEBBLE_ENTRY_BYTES = 16;

/** Bytes of one `ScatterPatch` (`pebbles.wgsl`, `pebbleDraw.wgsl`). */
export const SCATTER_PATCH_BYTES = 64;

/** The placement's pass label, R12's `scatter` row's (Design note 18). */
export const SCATTER_PASS_LABEL = "scatter";

/**
 * The share of the instances' capacity a frame's patches may fill on average, by
 * {@link expectedPebblesBound} (the module's remarks): a tenth held back, many times the count's
 * Poisson spread at any capacity the pass holds.
 */
export const PEBBLE_BUDGET_SHARE = 0.9;

/** The bits of an instance's tag that hold its patch record's index; its fade's byte is above. */
export const PEBBLE_TAG_PATCH_BITS = 24;

/** A fade in [0, 1] as the byte its tag carries: `pebbles.wgsl`'s, ⌊255 fade + ½⌋. */
export function pebbleFadeByte(fade: number): number {
  return Math.floor(Math.fround(Math.fround(Math.min(Math.max(fade, 0), 1)) * 255) + 0.5);
}

/**
 * An instance's tag: its patch record's index below {@link PEBBLE_TAG_PATCH_BITS} and its fade's
 * byte ({@link pebbleFadeByte}) above.
 */
export function pebbleTag(patch: number, fadeByte: number): number {
  return (patch | (fadeByte << PEBBLE_TAG_PATCH_BITS)) >>> 0;
}

/** The `f32` words of `pebbles.wgsl`'s patch arithmetic every path's prelude defines. */
const PATCH_CONSTANTS =
  "const VERTICES_PER_SIDE : u32 = 65u;\nconst PATCH_VERTICES : u32 = 4225u;\n";

/** The binding `terrainBakedOffsets.wgsl` declares its offsets at, a material's group 2. */
const MATERIAL_OFFSETS_BINDING = "@group(2) @binding(5)";

/**
 * Each vertex path's prelude of `pebbles.wgsl` (the module's remarks): the patch constants, the
 * path's one buffer at group 0's binding 7, and its `ownOffset` and `morphOffset`, the materials'
 * own sources (`terrain/gpu/material.ts`' `PATH_SOURCES`), its offsets' binding moved to the
 * kernel's group.
 *
 * @throws Error unless the baked path's source declares its offsets once.
 */
function pathPrelude(path: TerrainVertexPath): string {
  switch (path) {
    case "face-differences":
      return `${PATCH_CONSTANTS}
@group(0) @binding(7) var<storage, read> heights : array<f32>;

// A vertex's own (0) or morph (1) height (\`terrainPatch.wgsl\`'s \`heightAt\`).
fn heightAt(slot : u32, x : u32, y : u32, which : u32) -> f32 {
  return heights[(slot * PATCH_VERTICES + y * VERTICES_PER_SIDE + x) * 2u + which];
}
${PATH_SOURCES[path]}`;
    case "baked-offsets": {
      const parts = PATH_SOURCES[path].split(MATERIAL_OFFSETS_BINDING);
      if (parts.length !== 2) {
        throw new Error("the baked path's source must declare its offsets once");
      }
      return `${PATCH_CONSTANTS}${parts.join("@group(0) @binding(7)")}`;
    }
  }
  return unreachable(path);
}

/** The name of the buffer each path's prelude binds at 7: its heights or its offsets. */
export const PEBBLE_PATH_BUFFER: Readonly<Record<TerrainVertexPath, "heights" | "offsets">> = {
  "face-differences": "heights",
  "baked-offsets": "offsets",
};

/** The placement kernel of a vertex path (see the module's remarks): `presentation-only`. */
export function pebbleKernel(path: TerrainVertexPath): KernelPair {
  return {
    name: `scatter pebbles ${path}`,
    reference:
      FOLDED_GROUP_WGSL +
      decorationWgsl +
      patchVertexWgsl +
      pathPrelude(path) +
      SCATTER_CULL_WGSL +
      pebblesWgsl,
    subgroup: null,
    readback: "presentation-only",
  };
}

/** Both paths' placement kernels, as the catalogue holds them. */
export const PEBBLE_KERNELS: Readonly<Record<TerrainVertexPath, KernelPair>> = {
  "face-differences": pebbleKernel("face-differences"),
  "baked-offsets": pebbleKernel("baked-offsets"),
};

/** A patch a frame scatters over, as its record holds it (see the module's remarks). */
export interface ScatterPatchRecord {
  /** Its origin less the camera, m, in the passes' frame. */
  readonly originM: Vec3;
  readonly slot: number;
  readonly morphStartM: number;
  readonly morphEndM: number;
  /** Its parent's slot, `NO_PARENT_SLOT` for none, and its quadrant there. */
  readonly parentSlot: number;
  readonly parentQuadrant: number;
  readonly key: PatchKey;
  /** The terrain instance's class sets (`InstanceRecords.setCells`'s four words). */
  readonly cells: readonly [number, number, number, number];
}

/** Packs `patches` into `out` ({@link SCATTER_PATCH_BYTES} each), narrowing each value once. */
export function packScatterPatches(
  patches: ReadonlyArray<ScatterPatchRecord>,
  out: ArrayBuffer,
): void {
  const view = new DataView(out);
  patches.forEach((p, n) => {
    const at = n * SCATTER_PATCH_BYTES;
    view.setFloat32(at, p.originM.x, true);
    view.setFloat32(at + 4, p.originM.y, true);
    view.setFloat32(at + 8, p.originM.z, true);
    view.setUint32(at + 12, p.slot, true);
    view.setFloat32(at + 16, p.morphStartM, true);
    view.setFloat32(at + 20, p.morphEndM, true);
    view.setUint32(at + 24, p.parentSlot >>> 0, true);
    view.setUint32(at + 28, p.parentQuadrant >>> 0, true);
    [p.key.face, p.key.level, p.key.i, p.key.j, ...p.cells].forEach((word, k) => {
      view.setUint32(at + 32 + 4 * k, word >>> 0, true);
    });
  });
}

/** One block of an octave's cells over a patch, as `PebbleEntry` holds it. */
export interface PebbleEntry {
  /** The patch record's index. */
  readonly patch: number;
  readonly octave: number;
  /** Its cells' levels below the patch: 2^levels cells along its side. */
  readonly cellLevels: number;
  /** The block, row by row of ⌈2^levels ÷ 16⌉ a row. */
  readonly block: number;
}

/** What a frame knows of a patch it may scatter over, beside its record, for its entries. */
export interface ScatterPatchReach {
  /** Its nearest point's distance from the camera, m. */
  readonly nearestM: number;
  /** A bound above its ground's area, m²: π r² of its bounding sphere's radius r. */
  readonly areaM2: number;
  /** The largest rock abundance k of its palette's slots (`TerrainLayers.rockAbundances`). */
  readonly abundance: number;
}

/** A patch's place for {@link scatterEntries}: its key and its reach. */
export interface ScatterPatchPlace extends ScatterPatchReach {
  readonly key: PatchKey;
}

/** What a frame's pebbles may fill: the pass's density and its instances' capacity. */
export interface ScatterBudget {
  readonly density: number;
  readonly capacity: number;
}

/**
 * A bound above the pebbles octave `octave` keeps on average over a patch of `place` at
 * `density`: its proposals over the patch's area bound, each kept at most as at its octave's least
 * diameter on a ground wholly of the patch's largest k (the law's share rises with k and falls
 * with D), the slope and the fade taken as keeping all.
 */
export function expectedPebblesBound(
  place: ScatterPatchReach,
  octave: number,
  density: number,
): number {
  const range = PEBBLE_OCTAVES[octave];
  if (range === undefined || !(place.areaM2 > 0)) {
    return 0;
  }
  const share = pebbleAcceptance([0, 1], [place.abundance], range[0]);
  return scatterDensity(density) * proposalsPerM2(octave) * place.areaM2 * share;
}

/**
 * The entries a frame dispatches over `patches` (in their order, nearest first; see the module's
 * remarks): for each octave whose largest pebble spans more than `PEBBLE_FADE_PX` at the patch's
 * nearest point, a block of its lattice's cells at a time, at most {@link MAX_PEBBLE_ENTRIES},
 * whole patches only, and only while the patches' {@link expectedPebblesBound} stays within
 * {@link PEBBLE_BUDGET_SHARE} of `budget`'s capacity.
 *
 * @param levels - Each octave's lattice level (`pebbleLatticeLevels`).
 * @param pxPerRad - The view's px per rad (`decorationPxPerRad`).
 */
export function scatterEntries(
  patches: ReadonlyArray<ScatterPatchPlace>,
  levels: ReadonlyArray<number>,
  pxPerRad: number,
  budget: ScatterBudget,
): PebbleEntry[] {
  const out: PebbleEntry[] = [];
  const most = PEBBLE_BUDGET_SHARE * budget.capacity;
  let expected = 0;
  for (let p = 0; p < patches.length && p < MAX_SCATTER_PATCHES; p += 1) {
    const place = patches[p];
    if (place === undefined) {
      continue;
    }
    const own: PebbleEntry[] = [];
    let ownExpected = 0;
    PEBBLE_OCTAVES.forEach(([, high], octave) => {
      const level = levels[octave];
      const cellLevels = level === undefined ? -1 : level - place.key.level;
      const spanPx = (high * pxPerRad) / Math.max(place.nearestM, 1e-6);
      if (cellLevels < 0 || !(spanPx > PEBBLE_FADE_PX)) {
        return;
      }
      ownExpected += expectedPebblesBound(place, octave, budget.density);
      const blocks = Math.max(1, 2 ** cellLevels / PEBBLE_BLOCK_SIDE) ** 2;
      for (let block = 0; block < blocks; block += 1) {
        own.push({ patch: p, octave, cellLevels, block });
      }
    });
    if (out.length + own.length > MAX_PEBBLE_ENTRIES || expected + ownExpected > most) {
      break;
    }
    expected += ownExpected;
    out.push(...own);
  }
  return out;
}

/** Packs `entries` into `out` ({@link PEBBLE_ENTRY_BYTES} each). */
export function packPebbleEntries(entries: ReadonlyArray<PebbleEntry>, out: Uint32Array): void {
  entries.forEach((e, n) => {
    out.set([e.patch, e.octave, e.cellLevels, e.block], 4 * n);
  });
}

/** The octaves' uniform: each octave's low and high diameters, m, and its proposals per m². */
export function packPebbleOctaves(out: Float32Array): void {
  PEBBLE_OCTAVES.forEach(([low, high], octave) => {
    out.set([low, high, proposalsPerM2(octave), 0], 4 * octave);
  });
}

/** What a frame's placement takes beside its records. */
export interface PebbleFrameParams {
  /** The rows of the body's rotation, body-fixed axes into the passes' frame. */
  readonly rotationRows: readonly [Vec3, Vec3, Vec3];
  /** The view's px per rad (`decorationPxPerRad`). */
  readonly pxPerRad: number;
  /** The setting's `scatterDensity`, [0, 1]. */
  readonly density: number;
  readonly entries: number;
  /** The particle path's capacity. */
  readonly capacity: number;
  /** The body's decoration seed (`decorationSeed`). */
  readonly seed: number;
  /** The bodies' boxes packed (`packScatterBoxes`). */
  readonly bodies: number;
}

/** Packs `params` into `out` ({@link PEBBLE_PARAMS_BYTES} at least) as `PebbleParams`. */
export function packPebbleParams(params: PebbleFrameParams, out: ArrayBuffer): void {
  const view = new DataView(out);
  params.rotationRows.forEach((row, k) => {
    view.setFloat32(16 * k, row.x, true);
    view.setFloat32(16 * k + 4, row.y, true);
    view.setFloat32(16 * k + 8, row.z, true);
    view.setFloat32(16 * k + 12, 0, true);
  });
  view.setFloat32(48, params.pxPerRad, true);
  view.setFloat32(52, scatterDensity(params.density), true);
  view.setFloat32(56, Math.cos((PEBBLE_REPOSE_DEG[0] * Math.PI) / 180), true);
  view.setFloat32(60, Math.cos((PEBBLE_REPOSE_DEG[1] * Math.PI) / 180), true);
  view.setUint32(64, params.entries, true);
  view.setUint32(68, params.capacity, true);
  view.setUint32(72, params.seed >>> 0, true);
  view.setUint32(76, params.bodies, true);
}

/** The dispatch's workgroups for `entries` entries, a workgroup each, folded. */
export function pebbleWorkgroups(entries: number): readonly [number, number, 1] {
  return foldedWorkgroups(entries);
}

/**
 * The unit quaternion turning a pebble's axes onto the passes' frame: +z onto the unit `up` by the
 * shortest arc, after a turn of `yawRad` about +z (half a turn about x where `up` is −z);
 * `pebble_rotation`.
 */
export function pebbleRotation(up: Vec3, yawRad: number): Quaternion {
  let arc: Quaternion = { x: 1, y: 0, z: 0, w: 0 };
  if (1 + up.z > 1e-6) {
    const n = Math.hypot(up.y, up.x, 1 + up.z);
    arc = { x: -up.y / n, y: up.x / n, z: 0, w: (1 + up.z) / n };
  }
  const tz = Math.sin(0.5 * yawRad);
  const tw = Math.cos(0.5 * yawRad);
  // arc ⊗ turn, turn = (0, 0, tz, tw): a.w b + b.w a + a × b, and a.w b.w − a · b.
  return {
    x: tw * arc.x + arc.y * tz,
    y: tw * arc.y - arc.x * tz,
    z: arc.w * tz + tw * arc.z,
    w: arc.w * tw - arc.z * tz,
  };
}

/** A slot's patch as the twin reads it: its terms, its vertices' offsets and its class layers. */
export interface PebbleTwinSlot {
  readonly terms: SlotTermsF32;
  /** Vertex (x, y)'s own offset from the patch's origin, body-fixed m. */
  readonly ownOffset: (x: number, y: number) => Vec3;
  /** Vertex (x, y)'s morph target's offset. */
  readonly morphOffset: (x: number, y: number) => Vec3;
  /** Its 65 × 65 vertices' eight class-weight bytes (`TerrainLayers.weights`). */
  readonly weights: Uint8Array;
  /** Whether each palette slot has a law. */
  readonly named: ReadonlyArray<boolean>;
  /** Each palette slot's rock abundance k (`TerrainLayers.rockAbundances`). */
  readonly abundances: ReadonlyArray<number>;
}

/** What the CPU twin places from: the frame's records and its slots. */
export interface PebbleTwinInputs {
  readonly params: PebbleFrameParams;
  readonly entries: ReadonlyArray<PebbleEntry>;
  readonly patches: ReadonlyArray<ScatterPatchRecord>;
  readonly bodies: ReadonlyArray<ScatterBox>;
  readonly slot: (slot: number) => PebbleTwinSlot;
}

/** One pebble the twin places: the instance record's values, unpacked. */
export interface PebbleTwinInstance {
  readonly centreM: Vec3;
  readonly radiusM: number;
  readonly diameterM: number;
  readonly rotation: Quaternion;
  readonly colour: number;
  readonly tag: number;
}

const f = Math.fround;

/** `row · v` of the rotation's rows. */
function rotated(rows: readonly [Vec3, Vec3, Vec3], v: Vec3): Vec3 {
  return { x: dot(rows[0], v), y: dot(rows[1], v), z: dot(rows[2], v) };
}

function lerp3(a: Vec3, b: Vec3, t: number): Vec3 {
  return { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t, z: a.z + (b.z - a.z) * t };
}

function weighted(
  w: readonly [number, number, number],
  [a, b, c]: readonly [Vec3, Vec3, Vec3],
): Vec3 {
  return {
    x: w[0] * a.x + w[1] * b.x + w[2] * c.x,
    y: w[0] * a.y + w[1] * b.y + w[2] * c.y,
    z: w[0] * a.z + w[1] * b.z + w[2] * c.z,
  };
}

/**
 * The datum's area, m², of the quad of `terms`' patch holding the place `at` (quads): the
 * parallelogram of its corner's edges, M (d − d₀) at each; `pebble_quad_area`.
 */
export function quadAreaM2(terms: SlotTermsF32, at: readonly [number, number]): number {
  const qx = Math.min(Math.floor(Math.max(at[0], 0)), 63);
  const qy = Math.min(Math.floor(Math.max(at[1], 0)), 63);
  const m = (x: number, y: number): Vec3 => {
    const dd = directionDifferenceF32(terms, x, y);
    return { x: terms.scale.x * dd.x, y: terms.scale.y * dd.y, z: terms.scale.z * dd.z };
  };
  const m00 = m(qx, qy);
  const m10 = m(qx + 1, qy);
  const m01 = m(qx, qy + 1);
  const e1 = { x: m10.x - m00.x, y: m10.y - m00.y, z: m10.z - m00.z };
  const e2 = { x: m01.x - m00.x, y: m01.y - m00.y, z: m01.z - m00.z };
  const c = cross(e1, e2);
  return Math.hypot(c.x, c.y, c.z);
}

/** The datum's unit normal at the place `at` (quads) of `terms`' patch: ν₀ + (ν − ν₀). */
function datumNormalAt(terms: SlotTermsF32, at: readonly [number, number]): Vec3 {
  const dnu = normalDifferenceF32(terms, directionDifferenceF32(terms, at[0], at[1]));
  return normalise({
    x: terms.nu0.x + dnu.x,
    y: terms.nu0.y + dnu.y,
    z: terms.nu0.z + dnu.z,
  });
}

/**
 * The pebbles the kernel places for `inputs` (see the module's remarks), in the order of the
 * entries, their cells and their proposals, at most the capacity: `pebbles.wgsl`'s, operation for
 * operation, but its device's `f32`.
 */
export function placePebblesCpu(inputs: PebbleTwinInputs): PebbleTwinInstance[] {
  const { params, entries, patches, bodies } = inputs;
  const out: PebbleTwinInstance[] = [];
  const density = scatterDensity(params.density);
  for (const entry of entries) {
    const patch = patches[entry.patch];
    const range = PEBBLE_OCTAVES[entry.octave];
    if (patch === undefined || range === undefined) {
      continue;
    }
    const slot = inputs.slot(patch.slot);
    const side = 2 ** entry.cellLevels;
    const blocks = Math.max(side / PEBBLE_BLOCK_SIDE, 1);
    const corner = [
      (entry.block % blocks) * PEBBLE_BLOCK_SIDE,
      Math.floor(entry.block / blocks) * PEBBLE_BLOCK_SIDE,
    ];
    const quadsEach = 64 / side;
    const level = patch.key.level + entry.cellLevels;
    for (let c = 0; c < PEBBLE_BLOCK_SIDE * PEBBLE_BLOCK_SIDE; c += 1) {
      const cx = (corner[0] ?? 0) + (c % PEBBLE_BLOCK_SIDE);
      const cy = (corner[1] ?? 0) + Math.floor(c / PEBBLE_BLOCK_SIDE);
      if (cx >= side || cy >= side) {
        continue;
      }
      const hash = pebbleCellHash(
        params.seed,
        entry.octave,
        patch.key.face,
        level,
        (patch.key.i * side + cx) >>> 0,
        (patch.key.j * side + cy) >>> 0,
      );
      const centreAt: [number, number] = [(cx + 0.5) * quadsEach, (cy + 0.5) * quadsEach];
      const mean = f(
        f(f(proposalsPerM2(entry.octave)) * f(quadAreaM2(slot.terms, centreAt))) *
          f(quadsEach * quadsEach),
      );
      for (const p of cellProposals(hash, entry.octave, mean)) {
        const at: [number, number] = [(cx + p.at[0]) * quadsEach, (cy + p.at[1]) * quadsEach];
        const share =
          pebbleAcceptance(
            terrainShares(texelWeights(slot.weights, at[0] / 64, at[1] / 64), slot.named),
            slot.abundances,
            p.diameterM,
          ) * density;
        if (!(p.keep < share)) {
          continue;
        }
        const qx = Math.min(Math.floor(at[0]), 63);
        const qy = Math.min(Math.floor(at[1]), 63);
        const fx = at[0] - qx;
        const fy = at[1] - qy;
        const mid: [number, number] = fx >= fy ? [qx + 1, qy] : [qx, qy + 1];
        const weights: [number, number, number] = [
          1 - Math.max(fx, fy),
          Math.abs(fx - fy),
          Math.min(fx, fy),
        ];
        const far: [number, number] = [qx + 1, qy + 1];
        const owns: readonly [Vec3, Vec3, Vec3] = [
          slot.ownOffset(qx, qy),
          slot.ownOffset(mid[0], mid[1]),
          slot.ownOffset(far[0], far[1]),
        ];
        const morphs: readonly [Vec3, Vec3, Vec3] = [
          slot.morphOffset(qx, qy),
          slot.morphOffset(mid[0], mid[1]),
          slot.morphOffset(far[0], far[1]),
        ];
        const own = weighted(weights, owns);
        const morphed = weighted(weights, morphs);
        const ownFrom = rotated(params.rotationRows, own);
        const distanceOwn = Math.hypot(
          patch.originM.x + ownFrom.x,
          patch.originM.y + ownFrom.y,
          patch.originM.z + ownFrom.z,
        );
        const k =
          patch.morphEndM > patch.morphStartM
            ? Math.min(
                Math.max(
                  (distanceOwn - patch.morphStartM) / (patch.morphEndM - patch.morphStartM),
                  0,
                ),
                1,
              )
            : 0;
        const ground = rotated(params.rotationRows, lerp3(own, morphed, k));
        const centreM = {
          x: patch.originM.x + ground.x,
          y: patch.originM.y + ground.y,
          z: patch.originM.z + ground.z,
        };
        const fade = pebbleFade(
          p.diameterM,
          Math.hypot(centreM.x, centreM.y, centreM.z),
          params.pxPerRad,
        );
        const fadeByte = pebbleFadeByte(fade);
        if (fadeByte === 0) {
          continue;
        }
        const datum = datumNormalAt(slot.terms, at);
        const [a0, a1, a2] = owns;
        let normal = normalise(
          cross(
            { x: a1.x - a0.x, y: a1.y - a0.y, z: a1.z - a0.z },
            { x: a2.x - a0.x, y: a2.y - a0.y, z: a2.z - a0.z },
          ),
        );
        if (dot(normal, datum) < 0) {
          normal = { x: -normal.x, y: -normal.y, z: -normal.z };
        }
        if (!(p.keep < share * slopeFilter(dot(normal, datum)))) {
          continue;
        }
        const radiusM = 0.5 * p.diameterM;
        if (intersectsBody({ centreM, radiusM: radiusM * PEBBLE_AXES[0] }, bodies)) {
          continue;
        }
        const u = Math.min(Math.max(at[0] / 64, 0), 1);
        const v = Math.min(Math.max(at[1] / 64, 0), 1);
        const colour =
          (Math.round(u * 4095) | (Math.round(v * 4095) << 12) | (p.tintByte << 24)) >>> 0;
        out.push({
          centreM,
          radiusM,
          diameterM: p.diameterM,
          rotation: pebbleRotation(rotated(params.rotationRows, normal), p.yawRad),
          colour,
          tag: pebbleTag(entry.patch, fadeByte),
        });
        if (out.length >= params.capacity) {
          return out;
        }
      }
    }
  }
  return out;
}
