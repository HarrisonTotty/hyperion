/**
 * The decoration library's GPU twin (plan R11, R11.T2.a): a kernel that runs `decoration.wgsl` and
 * `detailTextures.wgsl` over cases whose answers `decorationLattice.ts` and `detailTextures.ts`
 * give, so that the smoke harness holds the shaders to their references on SwiftShader: the
 * lattice's hashes and places to the bit, its fields, the perturbed normal and the projection's
 * coordinates and weights to `f32`'s rounding.
 *
 * @remarks
 * The projection's sampling is not run here, since a kernel binds no sampler; R11.T2.c's and
 * T2.b's smoke checks sample a world's sets through it. The cases are made here, deterministically,
 * so that the smoke page and the unit tests read the same ones.
 */

import type { KernelPair } from "../../engine/kernels";
import { FACES, type PatchKey } from "../../terrain/patchKey";
import { bandLimitM } from "../../terrain/planet";
import decorationWgsl from "./decoration.wgsl?raw";
import {
  DECORATION_CLASSES,
  DECORATION_SALT,
  type DecorationMix,
  decorationLevel,
  decorationPxPerRad,
  latticeCellM,
  lowbias32,
  MAX_LATTICE_LEVEL,
  mixOf,
  type RippleDirection,
  rippleOf,
  RIPPLE_LEVEL_OFFSET,
  RIPPLE_WAVELENGTH_M,
} from "./decorationLattice";
import type { DetailProjection } from "./detailTextures";
import detailTexturesWgsl from "./detailTextures.wgsl?raw";

/** The decoration library's source, which a material composes before calling `decorate`. */
export const DECORATION_WGSL: string = decorationWgsl;

/** The detail textures' projection's source, composed after {@link DECORATION_WGSL}. */
export const DETAIL_TEXTURES_WGSL: string = detailTexturesWgsl;

/** The twin's workgroup size. */
export const DECORATION_TWIN_WORKGROUP = 64;

/** The lattice points whose hashes the twin checks: the plan's 10⁴. */
export const DECORATION_TWIN_HASHES = 10_000;

/** The `u32` words of one hash case: seed, salt, face, level, x and y. */
export const HASH_CASE_WORDS = 6;

/** The 32-bit words of one texel case (`TwinTexel`, 112 bytes, its alignment 16). */
export const TEXEL_CASE_WORDS = 28;

/**
 * The shading normal the twin perturbs by each texel's slope, its x and y over a z of 1, with its
 * tangents along the face's x and y formed from it ({@link twinFrame}).
 */
export const TWIN_NORMAL: readonly [number, number] = [0.3, -0.2];

/** A 3-vector of the twin's frame. */
type Vec3Tuple = readonly [number, number, number];

/** The twin's unit normal and its tangents, as its kernel forms them. */
export function twinFrame(): {
  readonly normal: Vec3Tuple;
  readonly tangentX: Vec3Tuple;
  readonly tangentY: Vec3Tuple;
} {
  const [x, y] = TWIN_NORMAL;
  const length = Math.hypot(x, y, 1);
  const n: Vec3Tuple = [x / length, y / length, 1 / length];
  // cross((0, 1, 0), n) = (n_z, 0, −n_x), normalised, then n × that.
  const tl = Math.hypot(n[2], n[0]);
  const tx: Vec3Tuple = [n[2] / tl, 0, -n[0] / tl];
  const ty: Vec3Tuple = [
    n[1] * tx[2] - n[2] * tx[1],
    n[2] * tx[0] - n[0] * tx[2],
    n[0] * tx[1] - n[1] * tx[0],
  ];
  return { normal: n, tangentX: tx, tangentY: ty };
}

const TWIN_ENTRY = `
struct TwinParams {
  hashes : u32,
  texels : u32,
}

struct TwinTexel {
  key : vec4u,
  uv : vec2f,
  st : vec2f,
  seed : u32,
  level : u32,
  distance_m : f32,
  px_per_rad : f32,
  classes : vec2u,
  second_share : f32,
  pad0 : f32,
  ripple : vec4i,
  scar_shares : vec4f,
  radius_m : f32,
  // Three words, not a vec3f, whose alignment of 16 would grow the stride to 128.
  pad1 : f32,
  pad2 : f32,
  pad3 : f32,
}

@group(0) @binding(0) var<uniform> params : TwinParams;
@group(0) @binding(1) var<storage, read> hash_cases : array<u32>;
@group(0) @binding(2) var<storage, read_write> hash_results : array<u32>;
@group(0) @binding(3) var<storage, read> texel_cases : array<TwinTexel>;
@group(0) @binding(4) var<storage, read_write> texel_results : array<vec4f>;
@group(0) @binding(5) var<storage, read_write> place_results : array<vec4u>;
@group(0) @binding(6) var<storage, read_write> normal_results : array<vec4f>;
@group(0) @binding(7) var<storage, read_write> detail_results : array<vec4f>;

// The projection a texel case's detail coordinates take (\`twinProjection\`).
fn twin_projection(seed : u32, distance_m : f32) -> DetailProjection {
  return DetailProjection(
    1u + seed % 7u,
    vec4u(seed, seed >> 8u, seed >> 16u, seed ^ 0x00abcdefu),
    distance_m * 3.7 - 5.0,
    0.0,
  );
}

// The two classes' mix by their shares, as \`mixOf\` blends them.
fn twin_mix(classes : vec2u, second : f32) -> DecorationMix {
  let a = decoration_class(classes.x);
  let b = decoration_class(classes.y);
  return DecorationMix(
    (1.0 - second) * a.roughness + second * b.roughness,
    (1.0 - second) * a.albedo + second * b.albedo,
    (1.0 - second) * a.ripples + second * b.ripples,
    (1.0 - second) * a.scars + second * b.scars,
  );
}

@compute @workgroup_size(${String(DECORATION_TWIN_WORKGROUP)})
fn main(@builtin(global_invocation_id) id : vec3u) {
  let i = id.x;
  if (i < params.hashes) {
    let c = i * ${String(HASH_CASE_WORDS)}u;
    hash_results[i] = decoration_hash(
      hash_cases[c], hash_cases[c + 1u], hash_cases[c + 2u], hash_cases[c + 3u],
      hash_cases[c + 4u], hash_cases[c + 5u],
    );
    return;
  }
  let t = i - params.hashes;
  if (t >= params.texels) {
    return;
  }
  let k = texel_cases[t];
  let cell_m = decoration_cell_m(k.st, k.radius_m, k.level);
  let decorated = decorate(
    DecorationBody(k.seed, k.level, k.scar_shares),
    k.key,
    k.uv,
    cell_m,
    k.distance_m,
    k.px_per_rad,
    twin_mix(k.classes, k.second_share),
    k.ripple.xyz,
  );
  texel_results[t] = vec4f(decorated.albedo, decorated.slope, cell_m.x);
  let coarse = decoration_place(k.key, k.uv, k.level - 2u);
  let fine = decoration_place(k.key, k.uv, k.level + 5u);
  let below = decoration_place(k.key, k.uv, k.key.y - 2u);
  place_results[2u * t] = vec4u(coarse.cell, fine.cell);
  place_results[2u * t + 1u] = vec4u(below.cell, bitcast<vec2u>(below.frac));
  let n = normalize(vec3f(${String(TWIN_NORMAL[0])}, ${String(TWIN_NORMAL[1])}, 1.0));
  let tangent_x = normalize(cross(vec3f(0.0, 1.0, 0.0), n));
  let tangent_y = cross(n, tangent_x);
  let perturbed = decoration_normal(n, tangent_x, tangent_y, decorated.slope);
  normal_results[t] = vec4f(perturbed, 0.0);
  let projection = twin_projection(k.seed, k.distance_m);
  let place = decoration_place(k.key, k.uv, k.level);
  let along = vec3f(dot(perturbed, tangent_x), dot(perturbed, tangent_y), dot(perturbed, n));
  let side = detail_side_place(place.cell.x, place.frac.x, projection.height_cells, projection.period);
  detail_results[3u * t] = vec4f(detail_first(place, projection), detail_second(place, projection));
  detail_results[3u * t + 1u] = vec4f(detail_triplanar_weights(along), side.frac.y);
  detail_results[3u * t + 2u] = vec4f(detail_biplanar_weights(along), f32(side.cell.y));
}
`;

/**
 * The twin kernel: `decoration.wgsl` and `detailTextures.wgsl` with an entry that hashes each hash
 * case and decorates each texel case, its integers bit-exact, with the perturbed normal and the
 * projection's coordinates and weights at the texel; registered in R01's `WGSL_CATALOGUE`.
 */
export const DECORATION_TWIN_KERNEL: KernelPair = {
  name: "R11 decoration twin",
  reference: `${decorationWgsl}\n${detailTexturesWgsl}\n${TWIN_ENTRY}`,
  subgroup: null,
  readback: "bit-exact",
};

/** The projection a texel case's detail coordinates take: the twin's `twin_projection`. */
export function twinProjection(seed: number, distanceM: number): DetailProjection {
  return {
    period: 1 + (seed % 7),
    offsets: [seed >>> 0, seed >>> 8, seed >>> 16, (seed ^ 0x00ab_cdef) >>> 0],
    heightCells: Math.fround(Math.fround(distanceM) * Math.fround(3.7)) - 5,
  };
}

/** One lattice point to hash: `latticeHash`'s arguments. */
export interface HashCase {
  readonly seed: number;
  readonly salt: number;
  readonly face: number;
  readonly level: number;
  readonly x: number;
  readonly y: number;
}

/** A stream of `u32`s from `seed`, each the hash of the last: the cases' one source of numbers. */
function stream(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = lowbias32((state + 0x9e37_79b9) >>> 0);
    return state;
  };
}

/**
 * `count` hash cases: every salt, face and level the decoration reads and some it never does, at
 * points across a lattice's range, the corners 0 and 2³¹ − 1 among them.
 */
export function decorationHashCases(count: number): HashCase[] {
  const next = stream(0x0d3c_0a71);
  const salts = [DECORATION_SALT.noise, DECORATION_SALT.wobble, DECORATION_SALT.scar];
  return Array.from({ length: count }, (_, k): HashCase => {
    const level = k % (MAX_LATTICE_LEVEL + 1);
    const side = 2 ** level;
    const corner = k % 97 === 0;
    return {
      seed: next(),
      salt: k % 5 === 4 ? next() & 0xff : (salts[k % 3] ?? DECORATION_SALT.noise),
      face: k % 6,
      level,
      x: corner ? side - 1 : next() % side,
      y: corner ? 0 : next() % side,
    };
  });
}

/** The cases' words, {@link HASH_CASE_WORDS} each. */
export function packHashCases(cases: ReadonlyArray<HashCase>): Uint32Array {
  const words = new Uint32Array(cases.length * HASH_CASE_WORDS);
  cases.forEach((c, k) => {
    words.set([c.seed, c.salt, c.face, c.level, c.x, c.y], k * HASH_CASE_WORDS);
  });
  return words;
}

/** One texel to decorate, with the body's radius by which its cells' widths are formed. */
export interface TexelCase {
  readonly seed: number;
  readonly level: number;
  readonly key: PatchKey;
  /** [0, 1], a multiple of 2⁻¹⁰, exact in `f32`. */
  readonly u: number;
  readonly v: number;
  readonly radiusM: number;
  readonly distanceM: number;
  readonly pxPerRad: number;
  /** Two material classes' codes, `MaterialClass`'s, which the texel blends. */
  readonly classes: readonly [number, number];
  /** The second class's share of the texel, the first taking the rest. */
  readonly secondShare: number;
  readonly ripple: RippleDirection | null;
  readonly scarShares: readonly [number, number, number, number];
}

/** The face coordinates (s, t) of a texel case's place. */
export function texelSt(c: Pick<TexelCase, "key" | "u" | "v">): readonly [number, number] {
  const side = 2 ** c.key.level;
  return [(c.key.i + c.u) / side, (c.key.j + c.v) / side];
}

/** The widths of a texel case's cells at its level, m (`latticeCellM` at its place). */
export function texelCellM(c: TexelCase): readonly [number, number] {
  const [s, t] = texelSt(c);
  return latticeCellM(s, t, c.radiusM, c.level);
}

/** A body of the texel cases: its radius and the level of its finest patches. */
interface TwinBody {
  readonly radiusM: number;
  readonly patchLevel: number;
}

/** A Moon-like, an Earth-like and a small body, each at its finest patches' level. */
const TWIN_BODIES: ReadonlyArray<TwinBody> = [
  { radiusM: 1_737_400, patchLevel: 17 },
  { radiusM: 6_378_137, patchLevel: 19 },
  { radiusM: 200_000, patchLevel: 14 },
];

/** The distances the texel cases are seen from, m: every octave in, some faded and most out. */
const TWIN_DISTANCES_M: ReadonlyArray<number> = [0, 0.7, 3, 12, 60, 300];

/** The scar shares the cases take: saturated, half, a quarter and none, by octave. */
const TWIN_SCAR_SHARES: ReadonlyArray<readonly [number, number, number, number]> = [
  [1, 1, 1, 1],
  [0.5, 1, 0.25, 0],
  [0, 0, 0, 0],
];

/** A texel case's mix: its two classes' by their shares (`mixOf`). */
export function texelMix(c: Pick<TexelCase, "classes" | "secondShare">): DecorationMix {
  return mixOf(c.classes, [1 - c.secondShare, c.secondShare]);
}

/**
 * `count` texel cases on the {@link TWIN_BODIES}, seen from 1080p's pixel scale at a 60° field:
 * every class alone and blended with sand, ripples along each of the eight directions and none,
 * each distance and each scar share.
 */
export function decorationTexelCases(count: number): TexelCase[] {
  const next = stream(0x7e11_0c45);
  const pxPerRad = decorationPxPerRad({ viewport: { widthPx: 1920 }, fovXRad: Math.PI / 3 });
  return Array.from({ length: count }, (_, k): TexelCase => {
    const body = TWIN_BODIES[k % TWIN_BODIES.length] ?? { radiusM: 1_737_400, patchLevel: 17 };
    const side = 2 ** body.patchLevel;
    const level = decorationLevel(body.radiusM, bandLimitM());
    const own = k % DECORATION_CLASSES.length;
    const angle = ((k % 9) * Math.PI) / 4;
    const rippleCells = RIPPLE_WAVELENGTH_M / (body.radiusM / 2 ** (level + RIPPLE_LEVEL_OFFSET));
    return {
      seed: next(),
      level,
      key: { face: FACES[k % 6] ?? 0, level: body.patchLevel, i: next() % side, j: next() % side },
      u: (next() % 1025) / 1024,
      v: (next() % 1025) / 1024,
      radiusM: body.radiusM,
      distanceM: TWIN_DISTANCES_M[k % TWIN_DISTANCES_M.length] ?? 0,
      pxPerRad,
      // Every class alone, and with sand at half.
      classes: [own, k % 4 === 3 ? 3 : own],
      secondShare: k % 4 === 3 ? 0.5 : 0,
      ripple: k % 9 === 8 ? null : rippleOf(Math.cos(angle), Math.sin(angle), rippleCells),
      scarShares: TWIN_SCAR_SHARES[k % TWIN_SCAR_SHARES.length] ?? [0, 0, 0, 0],
    };
  });
}

/** The cases' bytes as the kernel's `TwinTexel` array reads them, {@link TEXEL_CASE_WORDS} each. */
export function packTexelCases(cases: ReadonlyArray<TexelCase>): ArrayBuffer {
  const buffer = new ArrayBuffer(cases.length * TEXEL_CASE_WORDS * 4);
  const u32 = new Uint32Array(buffer);
  const f32 = new Float32Array(buffer);
  const i32 = new Int32Array(buffer);
  cases.forEach((c, k) => {
    const at = k * TEXEL_CASE_WORDS;
    const [s, t] = texelSt(c);
    u32.set([c.key.face, c.key.level, c.key.i, c.key.j], at);
    f32.set([c.u, c.v, s, t], at + 4);
    u32.set([c.seed, c.level], at + 8);
    f32.set([c.distanceM, c.pxPerRad], at + 10);
    u32.set(c.classes, at + 12);
    f32[at + 14] = c.secondShare;
    const ripple = c.ripple ?? { p: 0, q: 0, n: 0 };
    i32.set([ripple.p, ripple.q, ripple.n, 0], at + 16);
    f32.set(c.scarShares, at + 20);
    f32[at + 24] = c.radiusM;
  });
  return buffer;
}
