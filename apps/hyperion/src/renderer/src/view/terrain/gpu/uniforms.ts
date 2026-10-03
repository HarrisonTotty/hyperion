/**
 * The terrain draw's records on the GPU (plan R05, R05.T11.a, Design notes 4 to 6): one record per
 * slot of the patch's static terms, one per drawn patch each frame, and the grounded contacts.
 *
 * @remarks
 * A per-draw uniform cannot carry per-instance data, so the one instanced draw reads its instances
 * from a storage buffer by `instance_index`, and the `Draw` uniform's `offsetFromCameraM` is zero.
 * Every layout here is WGSL's storage layout (W3C WGSL §14.4, "Memory Layout"): a `vec3f` aligns
 * to 16 bytes and takes 12, so a scalar packs into its fourth word. The WGSL structs that read them
 * (T11.b's `terrain.wgsl`) are quoted in each constant's documentation.
 *
 * Every `f64` value is narrowed to `f32` exactly once, by the store into a `Float32Array`
 * (round to nearest, ties to even, as `Math.fround`).
 */

import type { Vec3 } from "../../../geometry/vec3";
import type { SpheroidFigure } from "../../atmosphere/hillaire";
import type { Face, PatchKey } from "../patchKey";

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
 * }
 * ```
 *
 * The fields are `hyperion_surface::patch::vertex::PatchTermsF32`'s, plus the skirt depth.
 */
export const SLOT_RECORD_BYTES = 112;

/**
 * Bytes of one instance record, one per drawn patch a frame:
 *
 * ```wgsl
 * struct Instance {
 *   originM : vec3f,      // the patch origin less the camera, metres, narrowed once from f64
 *   slot : u32,           // the cache slot whose heights, offsets, normals and record it reads
 *   morphStartM : f32,    // the distance at which the morph to the parent begins, metres
 *   morphEndM : f32,      // the distance at which it is complete, metres
 *   _pad : vec2f,
 * }
 * ```
 */
export const INSTANCE_RECORD_BYTES = 32;

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
 *   centreM : vec3f,      // the contact's centre less the camera, metres, narrowed once
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
 * Writes `terms` and the skirt depth into slot record `slot` of `out`, narrowing each value once.
 *
 * @param out - The slot records, {@link SLOT_RECORD_BYTES} each; at least `slot + 1` of them.
 * @throws RangeError if `slot` lies outside `out`.
 */
export function writeSlotRecord(
  out: ArrayBuffer,
  slot: number,
  terms: PatchTerms,
  skirtDepthM: number,
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
}

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
   * @throws RangeError past {@link capacity}, or for a slot that is not a whole number.
   */
  push(slot: number, originMinusCameraM: Vec3, morphStartM: number, morphEndM: number): void {
    if (this.#count >= this.capacity) {
      throw new RangeError(`more than ${this.capacity} instances in a frame`);
    }
    if (!Number.isInteger(slot) || slot < 0) {
      throw new RangeError(`slot ${slot} is not a whole number`);
    }
    const base = this.#count * F32_WORDS_PER_INSTANCE;
    this.#f32[base] = originMinusCameraM.x;
    this.#f32[base + 1] = originMinusCameraM.y;
    this.#f32[base + 2] = originMinusCameraM.z;
    this.#u32[base + 3] = slot;
    this.#f32[base + 4] = morphStartM;
    this.#f32[base + 5] = morphEndM;
    this.#f32[base + 6] = 0;
    this.#f32[base + 7] = 0;
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
    this.#f32[base] = centreMinusCameraM.x;
    this.#f32[base + 1] = centreMinusCameraM.y;
    this.#f32[base + 2] = centreMinusCameraM.z;
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
