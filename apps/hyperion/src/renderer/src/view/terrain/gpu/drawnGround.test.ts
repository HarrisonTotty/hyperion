/**
 * Collision agrees with what is drawn (plan R10, R10.T15.a; decision-r10-t15a-scope): the finest
 * patch's triangles, formed as the GPU forms them on both vertex paths, against `ground_at`, the
 * authoritative ground collision reads, on the bakes of real patches that
 * `crates/hyperion-surface/src/bake/drawn_ground.rs` writes (its module documentation gives the
 * golden's format).
 *
 * @remarks
 * For each sample the golden gives the ground's height h_g at the point's foot d on the datum, its
 * triangle, the point's weights (fa, fb) in that triangle's quad, and the foot's datum point M·d
 * and normal ν(d). The test picks the triangle by R05's diagonal rule, forms its three vertices
 * relative to the patch's origin on each path, and intersects the line M·d + t ν(d), taken
 * relative to the same origin, with the plane through them: the vertices `f32`, the arithmetic
 * `f64`. The drawn height t is held to h_g within {@link BOUND_M}.
 *
 * - `FaceDifferences` (the low setting): the client's own terms, `patchTerms` of the patch's key,
 *   the figure and the bake's origin height, narrowed once by `writeSlotRecord` into a slot record
 *   and read back, then `faceDifferencePositionF32` over the bake's `f32` own heights.
 * - `BakedOffsets` (the high setting): the bake's `f32` own offset q₀, as
 *   `terrainBakedOffsets.wgsl` reads it.
 *
 * Both at morph 0: the authoritative ground is the finest level's own vertices.
 */

import { describe, expect, it } from "vitest";

import earthLikeGolden from "../../../../../../../../crates/hyperion-surface/tests/golden/drawn_ground/earth_like.golden?raw";
import moonLikeGolden from "../../../../../../../../crates/hyperion-surface/tests/golden/drawn_ground/moon_like.golden?raw";
import { add, cross, dot, norm, scale, sub, vec3, type Vec3 } from "../../../geometry/vec3";
import type { SpheroidFigure } from "../../atmosphere/hillaire";
import type { TerrainVertexPath } from "../../quality/qualitySetting";
import type { Face, PatchKey } from "../patchKey";
import { PATCH_VERTICES_PER_SIDE } from "../slotLayout";
import { type PatchTerms, patchTerms, SLOT_RECORD_BYTES, writeSlotRecord } from "./uniforms";
import { faceDifferencePositionF32, type SlotTermsF32 } from "./vertexEmulation";

/**
 * The bound on the drawn height's distance from the ground's, metres: 2 mm for the height's `f32`
 * narrowing, the step of an `f32` height at ±20 km being 2⁻⁹ m, 1.95 mm (plan R05, Design note 5;
 * the rendering brainstorm's "about 2 mm" under The geometry), plus R05.T4.b's stated formation
 * error of either vertex path, under 1 mm (plan R05, Design note 4).
 */
const BOUND_M = 0.003;

/**
 * How far one vertex is moved for the control, metres: far enough past {@link BOUND_M}, with the
 * formation's own error, that the comparison must fail.
 */
const CONTROL_SHIFT_M = 0.005;

/**
 * How closely the `FaceDifferences` formula's origin P₀ = M d₀ + h₀ ν₀ agrees with the bake's
 * origin, metres. The two are the same point formed in the same `f64` operations: the bake's
 * origin is the surface point of vertex (32, 32), the patch centre, at its own height h₀
 * (`hyperion_surface::patch`), and `patchTerms` forms d₀ from the same face coordinates in the
 * same order. On every golden patch they agree bit for bit; the bound allows an ulp of a
 * coordinate at an Earth's radius, 2⁻³⁰ m, about 0.93 nm.
 */
const FRAME_BOUND_M = 1e-9;

/** The two vertex paths (plan R05, Design note 4): the low setting's and the high setting's. */
const PATHS: ReadonlyArray<TerrainVertexPath> = ["face-differences", "baked-offsets"];

/** The kinds of sample point, as the golden names them. */
const SAMPLE_KINDS = [
  "random",
  "vertex",
  "diagonal",
  "quad_edge",
  "patch_edge",
  "far_quad",
] as const;

/** A sample point's kind. */
type SampleKind = (typeof SAMPLE_KINDS)[number];

/** The golden's three patches, in its order. */
const PATCH_NAMES = ["inside", "edge", "corner"] as const;

/** A `vertex` line: a vertex's own height and own offset from the patch's origin, `f32`. */
interface GoldenVertex {
  readonly x: number;
  readonly y: number;
  readonly heightM: number;
  readonly offsetM: Vec3;
}

/** A `sample` line. */
interface GoldenSample {
  readonly kind: SampleKind;
  /** The ground's quad across and up the patch, 0 to 63. */
  readonly x: number;
  readonly y: number;
  /** Whether the ground's triangle is the quad's upper half. */
  readonly upper: boolean;
  /** The point's weights across and up the quad, each 0 to 1. */
  readonly fa: number;
  readonly fb: number;
  /** `ground_at`'s height at the foot, metres above the datum along ν(d). */
  readonly groundM: number;
  /** The foot's datum point M·d, body-fixed metres. */
  readonly datumPointM: Vec3;
  /** The datum's unit normal ν(d) at the foot. */
  readonly normal: Vec3;
}

/** A `patch` block. */
interface GoldenPatch {
  readonly name: string;
  readonly key: PatchKey;
  /** The bake's origin, body-fixed metres. */
  readonly originM: Vec3;
  /** The origin's height, metres: the `FaceDifferences` path's h₀. */
  readonly originHeightM: number;
  /** The bake's lowest and highest heights over both channels, metres. */
  readonly rangeM: readonly [number, number];
  /** The vertices by index, y × 65 + x ({@link vertexIndex}). */
  readonly vertices: ReadonlyMap<number, GoldenVertex>;
  readonly samples: ReadonlyArray<GoldenSample>;
}

/** A world's golden. */
interface GoldenWorld {
  readonly figure: SpheroidFigure;
  readonly patches: ReadonlyArray<GoldenPatch>;
}

const f64Scratch = new DataView(new ArrayBuffer(8));
const f32Scratch = new DataView(new ArrayBuffer(4));

/** The index of vertex (`x`, `y`) in a patch, row by row, as the bake lays them out. */
function vertexIndex(x: number, y: number): number {
  return PATCH_VERTICES_PER_SIDE * y + x;
}

/** The word at `at` of a golden line, which must be there. */
function wordAt(words: ReadonlyArray<string>, at: number): string {
  const word = words[at];
  if (word === undefined) {
    throw new Error(`a golden line has no word ${at}: ${words.join(" ")}`);
  }
  return word;
}

/** Word `at` of a golden line, which must be `0x` and `digits` hexadecimal digits. */
function hexAt(words: ReadonlyArray<string>, at: number, digits: number): string {
  const word = wordAt(words, at);
  if (word.length !== digits + 2 || !/^0x[0-9a-f]+$/u.test(word)) {
    throw new Error(`word ${at} of a golden line is not ${digits} hexadecimal digits: ${word}`);
  }
  return word;
}

/** The `f64` whose IEEE 754 bits word `at` holds. */
function f64At(words: ReadonlyArray<string>, at: number): number {
  f64Scratch.setBigUint64(0, BigInt(hexAt(words, at, 16)));
  return f64Scratch.getFloat64(0);
}

/** The `f32` whose IEEE 754 bits word `at` holds. */
function f32At(words: ReadonlyArray<string>, at: number): number {
  f32Scratch.setUint32(0, Number.parseInt(hexAt(words, at, 8), 16));
  return f32Scratch.getFloat32(0);
}

function f64VecAt(words: ReadonlyArray<string>, at: number): Vec3 {
  return vec3(f64At(words, at), f64At(words, at + 1), f64At(words, at + 2));
}

function f32VecAt(words: ReadonlyArray<string>, at: number): Vec3 {
  return vec3(f32At(words, at), f32At(words, at + 1), f32At(words, at + 2));
}

function intAt(words: ReadonlyArray<string>, at: number): number {
  const n = Number(wordAt(words, at));
  if (!Number.isInteger(n)) {
    throw new Error(`word ${at} of a golden line is not an integer: ${words.join(" ")}`);
  }
  return n;
}

function isFace(n: number): n is Face {
  return Number.isInteger(n) && n >= 0 && n <= 5;
}

function isSampleKind(word: string): word is SampleKind {
  return SAMPLE_KINDS.some((kind) => kind === word);
}

/** A patch block's lines, gathered before the block is closed. */
interface OpenPatch {
  readonly name: string;
  readonly key: PatchKey;
  originM: Vec3;
  originHeightM: number;
  rangeM: readonly [number, number];
  readonly vertices: Map<number, GoldenVertex>;
  readonly samples: GoldenSample[];
}

/** The golden `text` of one world (`drawn_ground.rs`'s format). */
function parseWorld(text: string): GoldenWorld {
  let figure: SpheroidFigure | null = null;
  const patches: OpenPatch[] = [];
  const current = (): OpenPatch => {
    const patch = patches.at(-1);
    if (patch === undefined) {
      throw new Error("a patch's line before any patch");
    }
    return patch;
  };
  for (const line of text.split("\n")) {
    const words = line.trim().split(/\s+/u);
    switch (words[0] ?? "") {
      case "figure":
        figure = { equatorialRadiusM: f64At(words, 1), polarRadiusM: f64At(words, 2) };
        break;
      case "patch": {
        const face = intAt(words, 2);
        if (!isFace(face)) {
          throw new Error(`a patch on no face: ${line}`);
        }
        patches.push({
          name: wordAt(words, 1),
          key: { face, level: intAt(words, 3), i: intAt(words, 4), j: intAt(words, 5) },
          originM: vec3(0, 0, 0),
          originHeightM: 0,
          rangeM: [0, 0],
          vertices: new Map(),
          samples: [],
        });
        break;
      }
      case "origin":
        current().originM = f64VecAt(words, 1);
        current().originHeightM = f64At(words, 4);
        break;
      case "range":
        current().rangeM = [f32At(words, 1), f32At(words, 2)];
        break;
      case "vertex": {
        const x = intAt(words, 1);
        const y = intAt(words, 2);
        current().vertices.set(vertexIndex(x, y), {
          x,
          y,
          heightM: f32At(words, 3),
          offsetM: f32VecAt(words, 4),
        });
        break;
      }
      case "sample": {
        const kind = wordAt(words, 1);
        if (!isSampleKind(kind)) {
          throw new Error(`a sample of no kind: ${line}`);
        }
        current().samples.push({
          kind,
          x: intAt(words, 2),
          y: intAt(words, 3),
          upper: wordAt(words, 4) === "upper",
          fa: f64At(words, 5),
          fb: f64At(words, 6),
          groundM: f64At(words, 7),
          datumPointM: f64VecAt(words, 8),
          normal: f64VecAt(words, 11),
        });
        break;
      }
      case "#":
      case "":
        break;
      default:
        throw new Error(`a golden line of no kind: ${line}`);
    }
  }
  if (figure === null) {
    throw new Error("a golden with no figure");
  }
  return { figure, patches };
}

/** Word `at` of a slot record, which must be there. */
function recordWord(words: Float32Array, at: number): number {
  const word = words[at];
  if (word === undefined) {
    throw new Error(`a slot record has no word ${at}`);
  }
  return word;
}

/**
 * The `f32` terms the GPU reads for `terms`: one slot record written by `writeSlotRecord`, which
 * narrows each value once, and its words read back in `patchVertex.wgsl`'s `SlotRecord` layout.
 */
function slotTermsOf(terms: PatchTerms): SlotTermsF32 {
  const record = new ArrayBuffer(SLOT_RECORD_BYTES);
  writeSlotRecord(record, 0, terms, 0);
  const f = new Float32Array(record);
  const word = (at: number): number => recordWord(f, at);
  const v = (at: number): Vec3 => vec3(word(at), word(at + 1), word(at + 2));
  return {
    axisA: v(0),
    s0: word(3),
    axisE1: v(4),
    t0: word(7),
    axisE2: v(8),
    u0: word(11),
    scale: v(12),
    v0: word(15),
    m0: v(16),
    step: word(19),
    nu0: v(20),
    h0M: word(23),
    straddles: new Uint32Array(record)[25] === 1,
  };
}

/**
 * The `FaceDifferences` formula's origin P₀ = M d₀ + h₀ ν₀, body-fixed metres, from the client's
 * own `f64` terms: d₀ = n₀ ÷ |n₀| with n₀ = a + u₀ e₁ + v₀ e₂, in `patchTerms`' operations.
 *
 * @remarks
 * The GPU draws a vertex at the instance's origin, the bake's origin less the camera, plus the
 * formula's P − P₀, so P₀ must be the bake's origin for the positions to be the ones the formula
 * means.
 */
function faceDifferenceOriginM(terms: PatchTerms): Vec3 {
  const [a, e1, e2] = terms.axes;
  const [u0, v0] = terms.uv0;
  const n0 = vec3(
    a.x + u0 * e1.x + v0 * e2.x,
    a.y + u0 * e1.y + v0 * e2.y,
    a.z + u0 * e1.z + v0 * e2.z,
  );
  const len = Math.sqrt(n0.x * n0.x + n0.y * n0.y + n0.z * n0.z);
  const d0 = vec3(n0.x / len, n0.y / len, n0.z / len);
  return vec3(
    terms.scale.x * d0.x + terms.h0M * terms.nu0.x,
    terms.scale.y * d0.y + terms.h0M * terms.nu0.y,
    terms.scale.z * d0.z + terms.h0M * terms.nu0.z,
  );
}

/**
 * The indices (x, y) in the patch of the three vertices of quad (`x`, `y`)'s half, in
 * `hyperion_surface::patch::collision::MeshTriangle::vertices`' order.
 */
function triangleVertices(
  x: number,
  y: number,
  upper: boolean,
): readonly [readonly [number, number], readonly [number, number], readonly [number, number]] {
  return upper
    ? [
        [x, y],
        [x + 1, y + 1],
        [x, y + 1],
      ]
    : [
        [x, y],
        [x + 1, y],
        [x + 1, y + 1],
      ];
}

/**
 * Whether a point with weights (`fa`, `fb`) in its quad lies in the quad's upper half by R05's
 * rule: quads are split from (0, 0) to (1, 1), and a point on the diagonal belongs to the lower
 * half (`hyperion_surface::patch::collision`).
 */
function inUpperHalf(fa: number, fb: number): boolean {
  return fa < fb;
}

/** A vertex's height and offset, or the golden's own when no change is asked. */
type VertexChange = (vertex: GoldenVertex) => GoldenVertex;

const UNCHANGED: VertexChange = (vertex) => vertex;

/** One patch's vertices as each path forms them, relative to the bake's origin. */
class DrawnPatch {
  readonly patch: GoldenPatch;
  readonly terms: PatchTerms;
  readonly slot: SlotTermsF32;

  constructor(figure: SpheroidFigure, patch: GoldenPatch) {
    this.patch = patch;
    this.terms = patchTerms(patch.key, figure, patch.originHeightM);
    this.slot = slotTermsOf(this.terms);
  }

  /** Vertex (`x`, `y`) on `path`, metres from the bake's origin, after `change`. */
  vertexM(path: TerrainVertexPath, x: number, y: number, change: VertexChange): Vec3 {
    const golden = this.patch.vertices.get(vertexIndex(x, y));
    if (golden === undefined) {
      throw new Error(`${this.patch.name}: the golden has no vertex (${x}, ${y})`);
    }
    const vertex = change(golden);
    let position: Vec3;
    switch (path) {
      case "face-differences":
        position = faceDifferencePositionF32(this.slot, x, y, vertex.heightM);
        break;
      case "baked-offsets":
        position = vertex.offsetM;
        break;
    }
    return position;
  }

  /**
   * The drawn height t at `sample`'s foot on `path`, metres: where the plane through its
   * triangle's three vertices crosses the line M·d + t ν(d), all relative to the bake's origin, in
   * `f64`.
   */
  drawnHeightM(path: TerrainVertexPath, sample: GoldenSample, change: VertexChange): number {
    const upper = inUpperHalf(sample.fa, sample.fb);
    const [p0, p1, p2] = triangleVertices(sample.x, sample.y, upper).map(([x, y]) =>
      this.vertexM(path, x, y, change),
    );
    if (p0 === undefined || p1 === undefined || p2 === undefined) {
      throw new Error("a triangle has three vertices");
    }
    const n = cross(sub(p1, p0), sub(p2, p0));
    const footM = sub(sample.datumPointM, this.patch.originM);
    return dot(n, sub(p0, footM)) / dot(n, sample.normal);
  }

  /** The largest |t − h_g| over the patch's samples on `path`, metres, and the sample's kind and index. */
  worst(
    path: TerrainVertexPath,
    change: VertexChange,
  ): { readonly gapM: number; readonly at: string } {
    let gapM = 0;
    let at = "none";
    this.patch.samples.forEach((sample, n) => {
      const sampleGapM = Math.abs(this.drawnHeightM(path, sample, change) - sample.groundM);
      if (Number.isNaN(sampleGapM) || sampleGapM > gapM) {
        gapM = sampleGapM;
        at = `${sample.kind} sample ${n}`;
      }
    });
    return { gapM, at };
  }
}

/** `v` narrowed to `f32`, as the bake narrows an offset. */
function narrowed(v: Vec3): Vec3 {
  return vec3(Math.fround(v.x), Math.fround(v.y), Math.fround(v.z));
}

/** The patch's `vertex` sample, the control's. */
function vertexSample(patch: GoldenPatch): GoldenSample {
  const sample = patch.samples.find((s) => s.kind === "vertex");
  if (sample === undefined) {
    throw new Error(`${patch.name}: no vertex sample`);
  }
  return sample;
}

const WORLDS: ReadonlyArray<readonly [string, GoldenWorld]> = [
  ["Earth-like", parseWorld(earthLikeGolden)],
  ["Moon-like", parseWorld(moonLikeGolden)],
];

/** Each patch on each path: the cases of the tests that hold one patch on one path. */
const PATCH_PATHS = PATCH_NAMES.flatMap((patchName) =>
  PATHS.map((path) => [patchName, path] as const),
);

describe.each(WORLDS)("the drawn finest mesh of %s", (name, world) => {
  const drawn = world.patches.map((patch) => new DrawnPatch(world.figure, patch));
  const drawnPatch = (patchName: string): DrawnPatch => {
    const patch = drawn.find((d) => d.patch.name === patchName);
    if (patch === undefined) {
      throw new Error(`${name}'s golden has no ${patchName} patch`);
    }
    return patch;
  };

  it("reads an inside, an edge and a corner patch of 69 samples each from its golden", () => {
    expect(world.patches.map((p) => [p.name, p.samples.length])).toEqual(
      PATCH_NAMES.map((patchName) => [patchName, 69]),
    );
    for (const patch of world.patches) {
      expect(new Set(patch.samples.map((s) => s.kind))).toEqual(new Set(SAMPLE_KINDS));
    }
  });

  it("reads every vertex's height inside its patch's height range", () => {
    const outside = world.patches.flatMap((patch) =>
      [...patch.vertices.values()]
        .filter((v) => !(v.heightM >= patch.rangeM[0] && v.heightM <= patch.rangeM[1]))
        .map((v) => `${patch.name} (${v.x}, ${v.y}) at ${v.heightM} m`),
    );
    expect(outside).toEqual([]);
  });

  it.each(PATCH_PATHS)(
    `draws ${name}'s %s patch within 3 mm of the ground on the %s path`,
    (patchName, path) => {
      const { gapM, at } = drawnPatch(patchName).worst(path, UNCHANGED);
      expect(gapM, `${gapM} m at the ${at}`).toBeLessThanOrEqual(BOUND_M);
    },
  );

  it("picks the ground's own triangle half by R05's diagonal rule at every sample", () => {
    const wrong = world.patches.flatMap((patch) =>
      patch.samples
        .filter((s) => inUpperHalf(s.fa, s.fb) !== s.upper)
        .map((s) => `${patch.name} ${s.kind} (${s.fa}, ${s.fb})`),
    );
    expect(wrong).toEqual([]);
  });

  it("puts the FaceDifferences formula's origin at the bake's origin", () => {
    for (const { patch, terms } of drawn) {
      const gapM = norm(sub(faceDifferenceOriginM(terms), patch.originM));
      expect(gapM, `${patch.name}: ${gapM} m`).toBeLessThanOrEqual(FRAME_BOUND_M);
    }
  });

  it.each(PATCH_PATHS)(
    "fails the bound at the %s patch's vertex sample on the %s path with that vertex moved 5 mm",
    (patchName, path) => {
      const patch = drawnPatch(patchName);
      const sample = vertexSample(patch.patch);
      // The sample is on the vertex at (x + fa, y + fb) of its quad, fa and fb each 0 or 1.
      const vx = sample.x + Math.round(sample.fa);
      const vy = sample.y + Math.round(sample.fb);
      const moved: VertexChange = (vertex) =>
        vertex.x === vx && vertex.y === vy
          ? {
              ...vertex,
              heightM: Math.fround(vertex.heightM + CONTROL_SHIFT_M),
              offsetM: narrowed(add(vertex.offsetM, scale(sample.normal, CONTROL_SHIFT_M))),
            }
          : vertex;
      const gapM = Math.abs(patch.drawnHeightM(path, sample, moved) - sample.groundM);
      expect(gapM).toBeGreaterThan(BOUND_M);
    },
  );
});
