/**
 * The terrain draw's records on the GPU (plan R05, R05.T11.a, Design notes 4 to 6): one record per
 * slot of the patch's static terms, one per drawn patch each frame, and the grounded contacts.
 *
 * @remarks
 * A per-draw uniform cannot carry per-instance data, so the one instanced draw reads its instances
 * from a storage buffer by `instance_index`, and the `Draw` uniform's `offsetFromCameraM` is zero.
 * Every layout here is WGSL's storage layout (W3C WGSL §14.4, "Memory Layout"): a `vec3f` aligns
 * to 16 bytes and takes 12, so a scalar packs into its fourth word. The WGSL structs that read them
 * (T11.b's `terrain.wgsl`, and `patchVertex.wgsl` for the slot record) are quoted in each
 * constant's documentation.
 *
 * Every `f64` value is narrowed to `f32` exactly once, by the store into a `Float32Array`
 * (round to nearest, ties to even, as `Math.fround`).
 */

import type { Vec3 } from "../../../geometry/vec3";
import type { SpheroidFigure } from "../../atmosphere/hillaire";
import {
  EDGE_BANDS,
  edgeBandHalf,
  INSTANCE_FINEST,
  NO_STRIDE_EDGE_BANDS,
  NO_STRIDE_SCALE,
  type StrideEdgeBandsM,
  type StrideScale,
} from "../../clusters/strides";
import { MAX_MORPH_HOLDS } from "../coarseFineHolds";
import type { Face, PatchKey } from "../patchKey";
import type { GraticulePatch } from "../terrainWire";

/**
 * Bytes of one slot record:
 *
 * ```wgsl
 * struct SlotRecord {
 *   axisA : vec3f, s0 : f32,        // the face's centre axis a; the patch centre's s₀
 *   axisE1 : vec3f, t0 : f32,       // the face's u axis e₁; t₀
 *   axisE2 : vec3f, u0 : f32,       // the face's v axis e₂; u₀
 *   scale : vec3f, v0 : f32,        // M's diagonal (a, a, c), metres; v₀
 *   m0 : vec3f, step : f32,         // m₀ = M⁻¹ d₀, per metre; the vertex step in s, 2^−(n + 6)
 *   nu0 : vec3f, h0M : f32,         // ν₀; h₀, the origin's height, metres
 *   skirtDepthM : f32,              // how far the skirts hang along ν, metres
 *   straddles : u32,                // 1 for a level-0 patch, which straddles s = ½
 *   _pad : vec2u,
 *   blockErrors : array<vec4u, 24>, // each block's e₂, e₄ and e₈, metres, half floats in pairs,
 *                                   // half 3 b + l of block b towards stride 2^(l + 1), +∞ where
 *                                   // unmeasured (plan R14, R14.T3.c's shared bands)
 * }
 * ```
 *
 * The fields are `hyperion_surface::patch::vertex::PatchTermsF32`'s, plus the skirt depth and the
 * blocks' errors, which the vertex stage's stride morph reads (`terrainStrides.wgsl`).
 */
export const SLOT_RECORD_BYTES = 496;

/**
 * Bytes of one instance record, one per drawn patch a frame:
 *
 * ```wgsl
 * struct Instance {
 *   originM : vec3f,        // the patch origin less the camera, metres, narrowed once from f64
 *   slot : u32,             // the cache slot whose heights, offsets, normals and record it reads
 *   morphStartM : f32,      // the distance at which the morph to the parent begins, metres
 *   morphEndM : f32,        // the distance at which it is complete, metres
 *   parentSlot : u32,       // the parent's slot, whose horizon map and normals the patch's blend
 *                           // to, or NO_PARENT_SLOT (R10.T8.b, T8.c; decision-r10-t8a §1)
 *   parentQuadrant : u32,   // bits 0 and 1 the patch's quadrant in its parent (i & 1, j & 1);
 *                           // bits 2 to 17 its morph holds at its coarse–fine edges and corners
 *                           // (`coarseFineHolds.ts`; R10.T8.c); bit 18 `INSTANCE_FINEST`
 *   sub : vec4u,            // x the sub-patch it draws of its slot's patch, or 0 for the whole
 *                           // patch ({@link subPatchWord}); y its edges' fades and z its corners',
 *                           // a byte each; w its own fade, its rebake's and the slot it fades
 *                           // from ({@link fadeWord}; R10.T16.i)
 *   strides : vec4f,        // its stride scale at its view's τ′ (plan R14, R14.T3.c; `clusters/
 *                           // strides.ts`' `StrideScale`): E_L ÷ τ′_rad, m, then 1 ÷ τ′_rad, at or
 *                           // below 0 for none, then its holds' onsets' shares (a u32's bits; a
 *                           // byte an edge, `strideFactor`) and 0
 *   cells : vec4u,          // its class sets (lane fix-coast-class-map; {@link InstanceRecords.setCells}):
 *                           // x its ancestor of the field's cells' level, L − 6, or NO_PARENT_SLOT;
 *                           // y its place there ({@link cellAncestorPlace}); z the fade to it, an
 *                           // f32's bits; w its flags ({@link CLASS_MORPH})
 *   edgeBands : array<vec4u, 12>, // its edges' shared bands (`strideEdges.ts`), half floats of
 *                           // kilometres rounded up in pairs, half `edgeBandIndex(e, t, l)`,
 *                           // negative for none (words 20 to 67)
 * }
 * ```
 */
export const INSTANCE_RECORD_BYTES = 272;

/**
 * `Instance.cells.w`'s flag that the patch's class weights morph to its parent's by its morph
 * factor, as its heights, normals and maturity do (lane fix-coast-class-map;
 * `terrainPatch.wgsl`'s `CLASS_MORPH`), so that a coast does not pop where it splits.
 */
export const CLASS_MORPH = 1;

/** The most levels a patch lies below the ancestor of its field's cells that `cells.y` places it in. */
export const MAX_CELL_ANCESTOR_LEVELS = 13;

/**
 * `Instance.cells.y`: the place of a patch of (`i`, `j`) in its ancestor `levels` up
 * (`terrainPatch.wgsl`'s `ancestorPatchUv`): bits 0 to 4 the levels d, bits 5 to 17 i mod 2^d,
 * bits 18 to 30 j mod 2^d.
 *
 * @throws RangeError for levels that are not a whole number 0 to {@link MAX_CELL_ANCESTOR_LEVELS}.
 */
export function cellAncestorPlace(levels: number, i: number, j: number): number {
  if (!Number.isInteger(levels) || levels < 0 || levels > MAX_CELL_ANCESTOR_LEVELS) {
    throw new RangeError(
      `a cell ancestor ${levels} levels up is not 0 to ${MAX_CELL_ANCESTOR_LEVELS}`,
    );
  }
  const mask = 2 ** levels - 1;
  return (levels | ((i & mask) << 5) | ((j & mask) << 18)) >>> 0;
}

/** The instance record's 32-bit words a reader takes by index (the struct above): its `slot`. */
export const INSTANCE_SLOT_WORD = 3;

/** The first of the instance record's words of its edges' shared stride bands (`edgeBands`). */
export const INSTANCE_EDGE_BANDS_WORD = 20;
/** Its `parentSlot`. */
export const INSTANCE_PARENT_SLOT_WORD = 6;
/** Its `sub.x`, the sub-patch it draws of its slot's patch ({@link subPatchWord}), 0 for the whole. */
export const INSTANCE_SUB_PATCH_WORD = 8;
/** Its `sub.w`, the fades' word ({@link fadeWord}). */
export const INSTANCE_FADES_WORD = 11;

/**
 * The most levels a sub-patch instance lies below the patch whose slot it draws
 * ({@link subPatchWord}): its (i, j) take 13 bits each.
 */
export const MAX_SUB_PATCH_LEVELS = 13;

/**
 * The instance record's sub-patch word (the seamless lane, the owner's priority, 2026-11-01): an
 * instance drawing the part of its slot's patch under a missing selected patch `levels` deeper, at
 * (`i`, `j`) of that patch's 2^levels × 2^levels parts (the missing patch's i and j less the slot
 * patch's, shifted up by `levels`), on the slot patch's own surface; 0 for the whole patch. Bits 0
 * to 4 hold `levels`, 5 to 17 `i` and 18 to 30 `j` (`shaders/terrainPatch.wgsl`'s
 * `sourceGridOf`).
 *
 * @throws RangeError for `levels` outside 0 to {@link MAX_SUB_PATCH_LEVELS}, or an `i` or `j` that
 *   is not a whole number below 2^levels.
 */
export function subPatchWord(levels: number, i: number, j: number): number {
  const side = 2 ** levels;
  if (!Number.isInteger(levels) || levels < 0 || levels > MAX_SUB_PATCH_LEVELS) {
    throw new RangeError(`a sub-patch ${levels} levels down is not 0 to ${MAX_SUB_PATCH_LEVELS}`);
  }
  if (!Number.isInteger(i) || !Number.isInteger(j) || i < 0 || j < 0 || i >= side || j >= side) {
    throw new RangeError(`sub-patch (${i}, ${j}) is not one of ${side} × ${side}`);
  }
  return (levels | (i << 5) | (j << 18)) >>> 0;
}

/** The most slots a rebake's fade can name its old bake's in ({@link fadeWord}): 12 bits. */
export const MAX_FADE_SLOT = 0xfff;

/**
 * The steps of a heights mix's fade ({@link fadeWord}): 12 bits, so that a mix over a change of a
 * thousand pixels on screen (newly surveyed relief near the ground) steps a quarter of one.
 */
export const MIX_STEPS = 0xfff;

/** A heights mix's fade of 0 to 1 as its step of {@link MIX_STEPS}, rounded to the nearest. */
export function mixStep(fade: number): number {
  return Math.round(Math.min(1, Math.max(0, fade)) * MIX_STEPS);
}

/**
 * The slot a rise from the reference surface names as its older bake ({@link fadeWord}; plan R10,
 * R10.T16.i): no slot, the datum, height 0 everywhere, its normal the spheroid's
 * (`shaders/terrainPatch.wgsl`'s `DATUM_SLOT`).
 */
export const DATUM_SLOT = MAX_FADE_SLOT;

/**
 * The slot a refine's or a coarsen's fade names as the surface it mixes from ({@link fadeWord};
 * plan R10, R10.T16.i): the instance's parent (its `parentSlot`), at its place there by its
 * quadrant, the datum where the parent held it unsurveyed (`shaders/terrainPatch.wgsl`'s
 * `PARENT_MIX_SLOT`). A rebake's older slot is always below it.
 */
export const PARENT_MIX_SLOT = MAX_FADE_SLOT - 1;

/**
 * The instance record's word of its own fade and its heights' mix (plan R10, R10.T16.i): bits 0 to
 * 7 its own fade's byte (`coarseFineHolds.ts`' `fadeByte`), 8 to 19 the mix's fade in steps of
 * {@link MIX_STEPS} ({@link mixStep}), from the surface `oldSlot` names (bits 20 to 31: an older
 * bake's slot, {@link DATUM_SLOT} or {@link PARENT_MIX_SLOT}) to the slot's own, 0 with none. A
 * fading rebake's instance carries both slots: the new in its `slot`, the old here.
 *
 * @throws RangeError for an own fade that is not a whole number of 0 to 255, a mix's of 0 to
 *   {@link MIX_STEPS}, or a slot past {@link MAX_FADE_SLOT}.
 */
export function fadeWord(own: number, mix: number, oldSlot: number): number {
  if (!Number.isInteger(own) || own < 0 || own > 0xff) {
    throw new RangeError(`a fade byte ${own} is not 0 to 255`);
  }
  if (!Number.isInteger(mix) || mix < 0 || mix > MIX_STEPS) {
    throw new RangeError(`a mix's fade ${mix} is not 0 to ${MIX_STEPS}`);
  }
  if (!Number.isInteger(oldSlot) || oldSlot < 0 || oldSlot > MAX_FADE_SLOT) {
    throw new RangeError(`a rebake's old slot ${oldSlot} is not 0 to ${MAX_FADE_SLOT}`);
  }
  return (own | (mix << 8) | (oldSlot << 20)) >>> 0;
}

/**
 * The sub-patch word ({@link subPatchWord}) of the part of `patch` under `within`, a descendant of
 * it on its face.
 *
 * @throws RangeError for a `within` that is not a descendant of `patch` within
 *   {@link MAX_SUB_PATCH_LEVELS} levels.
 */
export function subPatchOf(patch: PatchKey, within: PatchKey): number {
  const levels = within.level - patch.level;
  const side = 2 ** levels;
  if (within.face !== patch.face) {
    throw new RangeError(`a sub-patch on face ${within.face} of a patch on face ${patch.face}`);
  }
  return subPatchWord(levels, within.i - patch.i * side, within.j - patch.j * side);
}

/**
 * The instance record's parent slot where the patch has no parent's horizon map and normals to
 * blend to: a root, a parent not resident, or one whose slot holds no map (R10.T8.b, T8.c;
 * `shadows/horizon.wgsl`'s `HORIZON_NO_PARENT`, `shaders/terrainPatch.wgsl`'s `NO_PARENT_SLOT`).
 * On the lit layout every bake holds both its normals and its map.
 */
export const NO_PARENT_SLOT = 0xffff_ffff;

/** The quadrant of `key` in its parent, as the instance record carries it: bit 0 i & 1, bit 1 j & 1. */
export function parentQuadrant(key: PatchKey): number {
  return (key.i & 1) | ((key.j & 1) << 1);
}

/** The bits of the instance record's quadrant word below its morph holds. */
const QUADRANT_BITS = 2;

/**
 * Bytes of the contacts buffer's header, `count : u32` and three words of padding:
 *
 * ```wgsl
 * struct Contacts { count : u32, _pad0 : u32, _pad1 : u32, _pad2 : u32, items : array<Contact> }
 * ```
 */
export const CONTACTS_HEADER_BYTES = 16;

/**
 * Bytes of one grounded contact (Design note 6's morph hold):
 *
 * ```wgsl
 * struct Contact {
 *   centreM : vec3f,      // the contact's centre less the camera, metres, in the instance
 *                         // origins' frame (the body's rotated axes, not body-fixed); narrowed once
 *   heldRadiusM : f32,    // r_g: the morph is held at zero within it, metres
 *   rampM : f32,          // the width over which the hold rises to 1 beyond r_g, metres
 *   _pad0 : f32, _pad1 : f32, _pad2 : f32,
 * }
 * ```
 */
export const CONTACT_RECORD_BYTES = 32;

/**
 * The most contacts a frame may carry.
 *
 * @remarks
 * A swept path is passed as a chain of contacts one radius apart (Design note 9); 1,024 covers a
 * 20 m craft's path over 20 km, and the buffer is 32 KiB. More is a caller's error and throws.
 */
export const MAX_CONTACTS = 1024;

const F32_WORDS_PER_SLOT = SLOT_RECORD_BYTES / 4;
const F32_WORDS_PER_INSTANCE = INSTANCE_RECORD_BYTES / 4;

/**
 * A patch's terms for the `FaceDifferences` path, in `f64`: the client's twin of
 * `hyperion_surface::patch::vertex::PatchTerms`, formed in the same operations in the same order
 * so that its `f32` narrowing is the Rust `narrow()`'s, bit for bit (T4.b's golden checks it).
 */
export interface PatchTerms {
  /** The face's centre axis a, and its u and v axes e₁ and e₂, each a signed unit axis. */
  readonly axes: readonly [Vec3, Vec3, Vec3];
  /** The patch centre's s₀ and t₀. */
  readonly st0: readonly [number, number];
  /** The patch centre's u₀ and v₀. */
  readonly uv0: readonly [number, number];
  /** The step in s of one vertex, 2^−(level + 6). */
  readonly step: number;
  /** M's diagonal, (a, a, c), metres. */
  readonly scale: Vec3;
  /** m₀ = M⁻¹ d₀, per metre. */
  readonly m0: Vec3;
  /** ν₀, the spheroid's normal at the centre. */
  readonly nu0: Vec3;
  /** h₀, the origin's height above the datum, metres. */
  readonly h0M: number;
  /** Whether the patch straddles s = ½ or t = ½: level 0 only. */
  readonly straddles: boolean;
}

/** The face's axes a, e₁ and e₂ by S2's convention, as `hyperion_surface::patch::vertex` has them. */
const FACE_AXES: Readonly<Record<Face, readonly [Vec3, Vec3, Vec3]>> = {
  0: [
    { x: 1, y: 0, z: 0 },
    { x: 0, y: 1, z: 0 },
    { x: 0, y: 0, z: 1 },
  ],
  1: [
    { x: 0, y: 1, z: 0 },
    { x: -1, y: 0, z: 0 },
    { x: 0, y: 0, z: 1 },
  ],
  2: [
    { x: 0, y: 0, z: 1 },
    { x: -1, y: 0, z: 0 },
    { x: 0, y: -1, z: 0 },
  ],
  3: [
    { x: -1, y: 0, z: 0 },
    { x: 0, y: 0, z: -1 },
    { x: 0, y: -1, z: 0 },
  ],
  4: [
    { x: 0, y: -1, z: 0 },
    { x: 0, y: 0, z: -1 },
    { x: 1, y: 0, z: 0 },
  ],
  5: [
    { x: 0, y: 0, z: -1 },
    { x: 0, y: 1, z: 0 },
    { x: 1, y: 0, z: 0 },
  ],
};

/** The cube sphere's warp from s ∈ [0, 1] to u ∈ [−1, 1], in `hyperion_surface::cube::st_to_uv`'s operations. */
function stToUv(s: number): number {
  if (s >= 0.5) {
    return (4 * s * s - 1) / 3;
  }
  const r = 1 - s;
  return (1 - 4 * r * r) / 3;
}

/**
 * The terms of `key` on `figure` for an origin at `originHeightM` metres above its centre vertex,
 * in `PatchTerms::new`'s operations.
 */
export function patchTerms(
  key: PatchKey,
  figure: SpheroidFigure,
  originHeightM: number,
): PatchTerms {
  const cells = 2 ** key.level;
  const st0: [number, number] = [(key.i + 0.5) / cells, (key.j + 0.5) / cells];
  const uv0: [number, number] = [stToUv(st0[0]), stToUv(st0[1])];
  const axes = FACE_AXES[key.face];
  const [a, e1, e2] = axes;
  const n0 = {
    x: a.x + uv0[0] * e1.x + uv0[1] * e2.x,
    y: a.y + uv0[0] * e1.y + uv0[1] * e2.y,
    z: a.z + uv0[0] * e1.z + uv0[1] * e2.z,
  };
  const len = Math.sqrt(n0.x * n0.x + n0.y * n0.y + n0.z * n0.z);
  const d0 = { x: n0.x / len, y: n0.y / len, z: n0.z / len };
  const scale = {
    x: figure.equatorialRadiusM,
    y: figure.equatorialRadiusM,
    z: figure.polarRadiusM,
  };
  const m0 = { x: d0.x / scale.x, y: d0.y / scale.y, z: d0.z / scale.z };
  // `Spheroid::normal`: M⁻¹ d₀ normalised, its own division by the radii repeated.
  const mLen = Math.sqrt(m0.x * m0.x + m0.y * m0.y + m0.z * m0.z);
  const nu0 = { x: m0.x / mLen, y: m0.y / mLen, z: m0.z / mLen };
  return {
    axes,
    st0,
    uv0,
    step: 2 ** -(key.level + 6),
    scale,
    m0,
    nu0,
    h0M: originHeightM,
    straddles: key.level === 0,
  };
}

/**
 * Writes `terms`, the skirt depth and the blocks' measured errors into slot record `slot` of `out`,
 * narrowing each value once.
 *
 * @param out - The slot records, {@link SLOT_RECORD_BYTES} each; at least `slot + 1` of them.
 * @param clusterErrors - The bake's clusters' errors, m (`BakedPatch.clusterErrors`, four a
 * cluster, e₁ first), of which each block's e₂, e₄ and e₈ are written as half floats (plan R14,
 * R14.T3.c); `null`, or a NaN, writes a block unmeasured (+∞).
 * @throws RangeError if `slot` lies outside `out`.
 */
export function writeSlotRecord(
  out: ArrayBuffer,
  slot: number,
  terms: PatchTerms,
  skirtDepthM: number,
  clusterErrors: ArrayLike<number> | null = null,
): void {
  if (!Number.isInteger(slot) || slot < 0 || (slot + 1) * SLOT_RECORD_BYTES > out.byteLength) {
    throw new RangeError(`slot ${slot} lies outside ${out.byteLength} B of slot records`);
  }
  const f = new Float32Array(out, slot * SLOT_RECORD_BYTES, F32_WORDS_PER_SLOT);
  const u = new Uint32Array(out, slot * SLOT_RECORD_BYTES, F32_WORDS_PER_SLOT);
  const [a, e1, e2] = terms.axes;
  const rows: ReadonlyArray<readonly [Vec3, number]> = [
    [a, terms.st0[0]],
    [e1, terms.st0[1]],
    [e2, terms.uv0[0]],
    [terms.scale, terms.uv0[1]],
    [terms.m0, terms.step],
    [terms.nu0, terms.h0M],
  ];
  rows.forEach(([v, w], row) => {
    f[4 * row] = v.x;
    f[4 * row + 1] = v.y;
    f[4 * row + 2] = v.z;
    f[4 * row + 3] = w;
  });
  f[24] = skirtDepthM;
  u[25] = terms.straddles ? 1 : 0;
  u[26] = 0;
  u[27] = 0;
  // The blocks' e₂, e₄ and e₈ (plan R14, R14.T3.c), +∞ where unmeasured or a NaN.
  const halves = new Uint16Array(out, slot * SLOT_RECORD_BYTES + 112, BLOCK_ERROR_HALVES);
  for (let block = 0; block < 64; block += 1) {
    for (let level = 0; level < 3; level += 1) {
      const e = clusterErrors?.[4 * block + level + 1] ?? Number.POSITIVE_INFINITY;
      ERROR_HALF[0] = Number.isNaN(e) ? Number.POSITIVE_INFINITY : e;
      halves[3 * block + level] = ERROR_HALF_BITS[0] ?? 0x7c00;
    }
  }
}

/** The half floats of a slot's blocks' errors: 64 blocks × e₂, e₄ and e₈. */
const BLOCK_ERROR_HALVES = 192;

/** The `Float16Array` scratch a block's error is narrowed through. */
const ERROR_HALF = new Float16Array(1);
const ERROR_HALF_BITS = new Uint16Array(ERROR_HALF.buffer);

/** The first `bytes` of `buffer` as bytes, from `views` or made once into it. */
function viewOf(views: Map<number, Uint8Array>, buffer: ArrayBuffer, bytes: number): Uint8Array {
  let view = views.get(bytes);
  if (view === undefined) {
    view = new Uint8Array(buffer, 0, bytes);
    views.set(bytes, view);
  }
  return view;
}

/**
 * One frame's instance records, in a buffer made once at the cache's slot count and refilled each
 * frame.
 */
export class InstanceRecords {
  /** The most records it holds: the slot count, since a draw set never draws a slot twice. */
  readonly capacity: number;
  /** The records, {@link INSTANCE_RECORD_BYTES} each, the first {@link count} of them this frame's. */
  readonly buffer: ArrayBuffer;
  readonly #f32: Float32Array;
  readonly #u32: Uint32Array;
  /** One view a record count, made the first time a frame has that count. */
  readonly #views = new Map<number, Uint8Array>();
  #count = 0;

  constructor(capacity: number) {
    if (!Number.isInteger(capacity) || capacity < 1) {
      throw new RangeError(`an instance buffer needs a whole, positive capacity, not ${capacity}`);
    }
    this.capacity = capacity;
    this.buffer = new ArrayBuffer(capacity * INSTANCE_RECORD_BYTES);
    this.#f32 = new Float32Array(this.buffer);
    this.#u32 = new Uint32Array(this.buffer);
  }

  /** Records this frame. */
  get count(): number {
    return this.#count;
  }

  /**
   * Its records' 32-bit words, read only: the whole capacity's, of which the first
   * {@link InstanceRecords.count} × 12 are this frame's (plan R14, R14.T6.a).
   */
  get words(): Uint32Array {
    return this.#u32;
  }

  /** Starts a frame with no records. */
  clear(): void {
    this.#count = 0;
  }

  /**
   * Appends a drawn patch.
   *
   * @param originMinusCameraM - The patch origin less the camera, metres, differenced in `f64`
   * (R02's `originMinusCamera`, the body-fixed origin rotated into the body's frame first); it is
   * narrowed here, once.
   * @param parentSlot - The slot of the parent whose horizon map and normals the patch's blend
   * to, or {@link NO_PARENT_SLOT} (R10.T8.b, T8.c).
   * @param quadrant - The patch's quadrant in that parent ({@link parentQuadrant}), 0 to 3.
   * @param holds - The patch's morph holds at its coarse–fine edges and corners
   * (`CoarseFineHolds`; R10.T8.c), 0 to {@link MAX_MORPH_HOLDS}, packed above the quadrant.
   * @param sub - The sub-patch of the slot's patch it draws ({@link subPatchWord}), with bit 31 a
   * mask of that patch's own blocks (`clusters/terrain.ts`' `TERRAIN_SUB_PATCH_MASK`), or 0 for the
   * whole patch.
   * @param edgeFades - Its edges' fades, a byte each in `EDGES`' order (R10.T16.i), 0 for none.
   * @param cornerFades - Its corners' fades, a byte each in `cornerNeighbours`' order, 0 for none.
   * @param fades - Its own fade and its rebake's ({@link fadeWord}), 0 for none.
   * @param strideScale - Its stride scale at its view's τ′ (plan R14, R14.T3.c), none by default.
   * @param finest - Whether every camera pass draws it at stride 1 (`clusters/strides.ts`'
   * `INSTANCE_FINEST`).
   * @param edgeBandsM - Its edges' shared stride bands, metres (`strideEdges.ts`), none by default.
   * @param strideHoldFades - Its holds' onsets' shares of the stride morph a byte an edge, `EDGES`'
   * order (`clusters/strides.ts`' `strideFactor`), 0 for none.
   * @throws RangeError past {@link capacity}, or for a slot, parent slot, quadrant, holds,
   * sub-patch word or fades word that is not a whole number in its range.
   */
  push(
    slot: number,
    originMinusCameraM: Vec3,
    morphStartM: number,
    morphEndM: number,
    parentSlot = NO_PARENT_SLOT,
    quadrant = 0,
    holds = 0,
    sub = 0,
    edgeFades = 0,
    cornerFades = 0,
    fades = 0,
    strideScale: StrideScale = NO_STRIDE_SCALE,
    finest = false,
    edgeBandsM: StrideEdgeBandsM = NO_STRIDE_EDGE_BANDS,
    strideHoldFades = 0,
  ): void {
    this.pushXyz(
      slot,
      originMinusCameraM.x,
      originMinusCameraM.y,
      originMinusCameraM.z,
      morphStartM,
      morphEndM,
      parentSlot,
      quadrant,
      holds,
      sub,
      edgeFades,
      cornerFades,
      fades,
      strideScale,
      finest,
      edgeBandsM,
      strideHoldFades,
    );
  }

  /** {@link push} from the origin's components, so that a frame makes no vector per patch. */
  pushXyz(
    slot: number,
    xM: number,
    yM: number,
    zM: number,
    morphStartM: number,
    morphEndM: number,
    parentSlot = NO_PARENT_SLOT,
    quadrant = 0,
    holds = 0,
    sub = 0,
    edgeFades = 0,
    cornerFades = 0,
    fades = 0,
    strideScale: StrideScale = NO_STRIDE_SCALE,
    finest = false,
    edgeBandsM: StrideEdgeBandsM = NO_STRIDE_EDGE_BANDS,
    strideHoldFades = 0,
  ): void {
    if (this.#count >= this.capacity) {
      throw new RangeError(`more than ${this.capacity} instances in a frame`);
    }
    if (!Number.isInteger(slot) || slot < 0) {
      throw new RangeError(`slot ${slot} is not a whole number`);
    }
    if (!Number.isInteger(parentSlot) || parentSlot < 0 || parentSlot > NO_PARENT_SLOT) {
      throw new RangeError(`parent slot ${parentSlot} is not a u32`);
    }
    if (!Number.isInteger(quadrant) || quadrant < 0 || quadrant > 3) {
      throw new RangeError(`quadrant ${quadrant} is not 0 to 3`);
    }
    if (!Number.isInteger(holds) || holds < 0 || holds > MAX_MORPH_HOLDS) {
      throw new RangeError(`morph holds ${holds} are not 0 to ${MAX_MORPH_HOLDS}`);
    }
    if (!Number.isInteger(sub) || sub < 0 || sub > 0xffff_ffff) {
      throw new RangeError(`sub-patch word ${sub} is not a u32`);
    }
    for (const word of [edgeFades, cornerFades, fades, strideHoldFades]) {
      if (!Number.isInteger(word) || word < 0 || word > 0xffff_ffff) {
        throw new RangeError(`fades word ${word} is not a u32`);
      }
    }
    const base = this.#count * F32_WORDS_PER_INSTANCE;
    this.#f32[base] = xM;
    this.#f32[base + 1] = yM;
    this.#f32[base + 2] = zM;
    this.#u32[base + 3] = slot;
    this.#f32[base + 4] = morphStartM;
    this.#f32[base + 5] = morphEndM;
    this.#u32[base + 6] = parentSlot;
    this.#u32[base + 7] =
      (quadrant | (holds << QUADRANT_BITS) | (finest ? INSTANCE_FINEST : 0)) >>> 0;
    this.#u32[base + 8] = sub;
    this.#u32[base + 9] = edgeFades;
    this.#u32[base + 10] = cornerFades;
    this.#u32[base + 11] = fades;
    this.#f32[base + 12] = strideScale.statedM;
    this.#f32[base + 13] = strideScale.perErrorM;
    this.#u32[base + 14] = strideHoldFades;
    this.#f32[base + 15] = 0;
    this.#u32[base + 16] = NO_PARENT_SLOT;
    this.#u32[base + 17] = 0;
    this.#u32[base + 18] = 0;
    this.#u32[base + 19] = 0;
    for (let h = 0; h < EDGE_BANDS; h += 2) {
      const lo = edgeBandHalf(edgeBandsM[h] ?? -1);
      const hi = edgeBandHalf(edgeBandsM[h + 1] ?? -1);
      this.#u32[base + INSTANCE_EDGE_BANDS_WORD + h / 2] = (lo | (hi << 16)) >>> 0;
    }
    this.#count += 1;
  }

  /**
   * Sets the last record's class sets (lane fix-coast-class-map; `terrainLit.wgsl`'s
   * `class_sets`): the ancestor of its field's cells' level whose class weights it fades to, the
   * fade and its flags. A record takes none until this is called: no ancestor, no fade, no flag.
   *
   * @param ancestorSlot - The ancestor's slot, or {@link NO_PARENT_SLOT} for none.
   * @param place - The patch's place in it ({@link cellAncestorPlace}).
   * @param fade - The fade to it, 0 to 1.
   * @param flags - {@link CLASS_MORPH}, or 0.
   * @throws RangeError with no record this frame, or for a slot, place, fade or flags out of range.
   */
  setCells(ancestorSlot: number, place: number, fade: number, flags: number): void {
    if (this.#count === 0) {
      throw new RangeError("no record to set the class sets of");
    }
    if (!Number.isInteger(ancestorSlot) || ancestorSlot < 0 || ancestorSlot > NO_PARENT_SLOT) {
      throw new RangeError(`ancestor slot ${ancestorSlot} is not a u32`);
    }
    if (!Number.isInteger(place) || place < 0 || place > 0x7fff_ffff) {
      throw new RangeError(`cell ancestor place ${place} is not 31 bits`);
    }
    if (!(fade >= 0 && fade <= 1)) {
      throw new RangeError(`class fade ${fade} is not 0 to 1`);
    }
    if (flags !== 0 && flags !== CLASS_MORPH) {
      throw new RangeError(`class flags ${flags} are not 0 or CLASS_MORPH`);
    }
    const base = (this.#count - 1) * F32_WORDS_PER_INSTANCE;
    this.#u32[base + 16] = ancestorSlot;
    this.#u32[base + 17] = place;
    this.#f32[base + 18] = fade;
    this.#u32[base + 19] = flags;
  }

  /**
   * Appends record `index` of `source` as it stands, word for word, so that its bytes are the
   * source's (plan R10, R10.T9.b: a shadow cascade's casters are a subset of a view's own records,
   * with its origins, morph bands, parents and holds, decision-r10-t9b-design D4).
   *
   * @throws RangeError past {@link capacity}, or for an index that is not one of `source`'s
   * records this frame.
   */
  pushCopy(source: InstanceRecords, index: number): void {
    if (this.#count >= this.capacity) {
      throw new RangeError(`more than ${this.capacity} instances in a frame`);
    }
    if (!Number.isInteger(index) || index < 0 || index >= source.#count) {
      throw new RangeError(`record ${index} is not one of the ${source.#count} records`);
    }
    // Word by word, as bits, so that no float is rounded and a frame allocates no view a record.
    const from = index * F32_WORDS_PER_INSTANCE;
    const to = this.#count * F32_WORDS_PER_INSTANCE;
    for (let word = 0; word < F32_WORDS_PER_INSTANCE; word += 1) {
      this.#u32[to + word] = source.#u32[from + word] ?? 0;
    }
    this.#count += 1;
  }

  /**
   * This frame's records as bytes, for one `writeBuffer`; a view made once per record count, so
   * that a frame allocates nothing once its counts have been seen.
   */
  bytes(): Uint8Array {
    return viewOf(this.#views, this.buffer, this.#count * INSTANCE_RECORD_BYTES);
  }
}

/**
 * Bytes of one wireframe phase record (plan R10, R10.T11), one per drawn patch a frame, in the
 * instances' order:
 *
 * ```wgsl
 * struct WirePhase {
 *   contour : vec4f,  // φ = h₀ mod 5 CI, m; h₀ less the sea level, m; |P₀|, m; 0
 *   latLon : vec4f,   // φ₀ and λ₀ less their nearest lines', rad; those lines' indices
 *   frame : vec4f,    // sin φ₀, cos φ₀, sin λ₀, cos λ₀
 * }
 * ```
 *
 * Each value is formed in `f64` (`terrainWire.ts`' `contourPhase` and `graticulePatch`) and
 * narrowed once by the store.
 */
export const WIRE_PHASE_RECORD_BYTES = 48;

const F32_WORDS_PER_WIRE_PHASE = WIRE_PHASE_RECORD_BYTES / 4;

/**
 * One wireframe view's phase records a frame (plan R10, R10.T11), in a buffer made once at the
 * cache's slot count and refilled each frame, one a drawn patch in the instances' order.
 */
export class WirePhaseRecords {
  /** The most records it holds: the slot count, as {@link InstanceRecords.capacity}. */
  readonly capacity: number;
  /** The records, {@link WIRE_PHASE_RECORD_BYTES} each, the first {@link count} this frame's. */
  readonly buffer: ArrayBuffer;
  readonly #f32: Float32Array;
  readonly #views = new Map<number, Uint8Array>();
  #count = 0;

  constructor(capacity: number) {
    if (!Number.isInteger(capacity) || capacity < 1) {
      throw new RangeError(`a phase buffer needs a whole, positive capacity, not ${capacity}`);
    }
    this.capacity = capacity;
    this.buffer = new ArrayBuffer(capacity * WIRE_PHASE_RECORD_BYTES);
    this.#f32 = new Float32Array(this.buffer);
  }

  /** Records this frame. */
  get count(): number {
    return this.#count;
  }

  /** Starts a frame with no records. */
  clear(): void {
    this.#count = 0;
  }

  /**
   * Appends a drawn patch's phases, narrowed here, once.
   *
   * @param contourPhaseM - φ = h₀ mod 5 CI, m (`terrainWire.ts`' `contourPhase`).
   * @param aboveSeaM - The patch origin's height h₀ less the field's sea level, m.
   * @param graticule - The patch's place for the graticule (`graticulePatch`).
   * @throws RangeError past {@link capacity}.
   */
  push(contourPhaseM: number, aboveSeaM: number, graticule: GraticulePatch): void {
    if (this.#count >= this.capacity) {
      throw new RangeError(`more than ${this.capacity} phase records in a frame`);
    }
    const base = this.#count * F32_WORDS_PER_WIRE_PHASE;
    const f = this.#f32;
    f[base] = contourPhaseM;
    f[base + 1] = aboveSeaM;
    f[base + 2] = graticule.radiusM;
    f[base + 3] = 0;
    f[base + 4] = graticule.latPhaseRad;
    f[base + 5] = graticule.lonPhaseRad;
    f[base + 6] = graticule.latIndex;
    f[base + 7] = graticule.lonIndex;
    f[base + 8] = graticule.sinLat;
    f[base + 9] = graticule.cosLat;
    f[base + 10] = graticule.sinLon;
    f[base + 11] = graticule.cosLon;
    this.#count += 1;
  }

  /** This frame's records as bytes, for one `writeBuffer`, as {@link InstanceRecords.bytes}. */
  bytes(): Uint8Array {
    return viewOf(this.#views, this.buffer, this.#count * WIRE_PHASE_RECORD_BYTES);
  }
}

/** One frame's grounded contacts, with their header, in a buffer of {@link MAX_CONTACTS}. */
export class ContactRecords {
  /** The header and {@link MAX_CONTACTS} records. */
  readonly buffer = new ArrayBuffer(CONTACTS_HEADER_BYTES + MAX_CONTACTS * CONTACT_RECORD_BYTES);
  readonly #f32 = new Float32Array(this.buffer);
  readonly #u32 = new Uint32Array(this.buffer);
  readonly #views = new Map<number, Uint8Array>();
  #count = 0;

  /** Contacts this frame. */
  get count(): number {
    return this.#count;
  }

  /** Starts a frame with no contacts. */
  clear(): void {
    this.#count = 0;
    this.#u32[0] = 0;
  }

  /**
   * Appends a contact.
   *
   * @param centreMinusCameraM - The contact's centre less the camera, metres, differenced in `f64`
   * in the frame the instances' origins are in; narrowed here, once.
   * @param heldRadiusM - r_g, within which the morph is held at zero (Design note 6).
   * @param rampM - The width beyond r_g over which the hold rises to 1, one finest patch.
   * @throws RangeError past {@link MAX_CONTACTS}, or for a radius or ramp that is negative or not
   * finite.
   */
  push(centreMinusCameraM: Vec3, heldRadiusM: number, rampM: number): void {
    this.pushXyz(
      centreMinusCameraM.x,
      centreMinusCameraM.y,
      centreMinusCameraM.z,
      heldRadiusM,
      rampM,
    );
  }

  /** {@link push} from the centre's components, so that a frame makes no vector per contact. */
  pushXyz(xM: number, yM: number, zM: number, heldRadiusM: number, rampM: number): void {
    if (this.#count >= MAX_CONTACTS) {
      throw new RangeError(`more than ${MAX_CONTACTS} grounded contacts in a frame`);
    }
    if (!(
      heldRadiusM >= 0 &&
      Number.isFinite(heldRadiusM) &&
      rampM >= 0 &&
      Number.isFinite(rampM)
    )) {
      throw new RangeError(
        `a contact's radius ${heldRadiusM} m and ramp ${rampM} m must be finite and non-negative`,
      );
    }
    const base = (CONTACTS_HEADER_BYTES + this.#count * CONTACT_RECORD_BYTES) / 4;
    this.#f32[base] = xM;
    this.#f32[base + 1] = yM;
    this.#f32[base + 2] = zM;
    this.#f32[base + 3] = heldRadiusM;
    this.#f32[base + 4] = rampM;
    this.#f32[base + 5] = 0;
    this.#f32[base + 6] = 0;
    this.#f32[base + 7] = 0;
    this.#count += 1;
    this.#u32[0] = this.#count;
  }

  /** The header and this frame's contacts as bytes, for one `writeBuffer`, as {@link InstanceRecords.bytes}. */
  bytes(): Uint8Array {
    return viewOf(
      this.#views,
      this.buffer,
      CONTACTS_HEADER_BYTES + this.#count * CONTACT_RECORD_BYTES,
    );
  }
}
