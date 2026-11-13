/**
 * The smoke harness's decoration twin (plan R11, R11.T2.a): `decoration.wgsl` and
 * `detailTextures.wgsl` on the real device against `decorationLattice.ts` and `detailTextures.ts`,
 * their references.
 *
 * @remarks
 * The lattice's hashes of 10⁴ points and each texel case's places on three lattices, one coarser
 * than its patch, must equal the reference's to the bit. Each texel's decoration (its albedo
 * factor, its slopes and its cells' width), its perturbed normal and the projection's tile
 * coordinates and weights at it must lie within {@link TEXEL_TOLERANCE} of the reference's, which
 * the reference forms in `f64` from the same cases: `f32`'s rounding. A texel near a scar's rim or
 * a fade's ends differs by no more, since each field is continuous there.
 */

import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { BufferHandle, RenderEngine } from "../view/engine/types";
import {
  decorateTexel,
  latticeHash,
  latticePlace,
  perturbNormal,
} from "../view/surface/decoration/decorationLattice";
import {
  DECORATION_TWIN_HASHES,
  DECORATION_TWIN_KERNEL,
  DECORATION_TWIN_WORKGROUP,
  decorationHashCases,
  decorationTexelCases,
  packHashCases,
  packTexelCases,
  type TexelCase,
  texelCellM,
  texelMix,
  twinFrame,
  twinProjection,
} from "../view/surface/decoration/decorationTwin";
import {
  biplanarWeights,
  detailFirst,
  detailSecond,
  detailSidePlace,
  triplanarWeights,
} from "../view/surface/decoration/detailTextures";
import type { Checks } from "./harness";

/** The texel cases the twin decorates. */
const TEXEL_CASES = 2_048;

/**
 * The largest difference allowed between the shader's and the reference's values, relative to
 * 1 + |the reference's|: some tens of `f32` roundings of each field's sums and `log2`'s, `cos`'s
 * and `sqrt`'s errors on SwiftShader.
 */
const TEXEL_TOLERANCE = 2e-4;

/** A storage buffer of `bytes`, at least 16, holding `data` where given, which the CPU may read. */
function storage(
  engine: RenderEngine,
  name: string,
  bytes: number,
  data: ArrayBufferView | null,
): BufferHandle {
  const buffer = engine.createBuffer({
    name,
    bytes: Math.max(16, bytes),
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST | BUFFER_USAGE.COPY_SRC,
    category: "other",
  });
  if (data !== null) {
    engine.writeBuffer(buffer, 0, data);
  }
  return buffer;
}

/** The largest relative difference of a value from the reference's, and where, over many. */
class Worst {
  value = 0;
  where = "";

  /** Notes one comparison; a NaN counts as infinite. */
  note(got: number | undefined, wanted: number, where: string): void {
    const error = Math.abs((got ?? Number.NaN) - wanted) / (1 + Math.abs(wanted));
    if (!(error <= this.value)) {
      this.value = Number.isNaN(error) ? Infinity : error;
      this.where = `${where}: ${String(got)} against ${wanted}`;
    }
  }
}

/** The dot product of two 3-vectors. */
function dot(a: ReadonlyArray<number>, b: ReadonlyArray<number>): number {
  return (a[0] ?? 0) * (b[0] ?? 0) + (a[1] ?? 0) * (b[1] ?? 0) + (a[2] ?? 0) * (b[2] ?? 0);
}

/** A texel case's reference values, in the kernel's outputs' order. */
interface TexelExpected {
  /** The albedo factor, the two slopes and the cells' width along x. */
  readonly texel: ReadonlyArray<number>;
  /** The places on the lattices two levels coarser and five finer than the body's: x and y each. */
  readonly places: ReadonlyArray<number>;
  /** The place two levels coarser than the patch's. */
  readonly below: {
    readonly x: number;
    readonly y: number;
    readonly fx: number;
    readonly fy: number;
  };
  readonly normal: ReadonlyArray<number>;
  /** Both scales' coordinates, the triplanar weights, a side place's fraction, the biplanar's and its cell. */
  readonly detail: ReadonlyArray<number>;
}

/** The reference's values of a texel case. */
function expectedOf(c: TexelCase): TexelExpected {
  const cellM = texelCellM(c);
  const decorated = decorateTexel({ ...c, cellM, mix: texelMix(c) });
  const coarse = latticePlace(c.key, c.u, c.v, c.level - 2);
  const fine = latticePlace(c.key, c.u, c.v, c.level + 5);
  const frame = twinFrame();
  const n = perturbNormal(frame.normal, frame.tangentX, frame.tangentY, decorated.slope);
  const along = [dot(n, frame.tangentX), dot(n, frame.tangentY), dot(n, frame.normal)] as const;
  const projection = twinProjection(c.seed, c.distanceM);
  const place = latticePlace(c.key, c.u, c.v, c.level);
  const side = detailSidePlace(place.x, place.fx, projection.heightCells, projection.period);
  return {
    texel: [decorated.albedo, decorated.slope[0], decorated.slope[1], cellM[0]],
    places: [coarse.x, coarse.y, fine.x, fine.y],
    below: latticePlace(c.key, c.u, c.v, c.key.level - 2),
    normal: n,
    detail: [
      ...detailFirst(place, projection),
      ...detailSecond(place, projection),
      ...triplanarWeights(along),
      side.fy,
      ...biplanarWeights(along),
      side.y,
    ],
  };
}

/** R11.T2.a: the decoration library's twin, its hashes, places and fields against the reference. */
export async function checkDecorationTwin(engine: RenderEngine, checks: Checks): Promise<void> {
  const hashCases = decorationHashCases(DECORATION_TWIN_HASHES);
  const texelCases = decorationTexelCases(TEXEL_CASES);
  const hashWords = packHashCases(hashCases);
  const texelBytes = new Uint8Array(packTexelCases(texelCases));
  const kernel = await engine.createComputeAsync(DECORATION_TWIN_KERNEL);
  const buffers = {
    hash_cases: storage(engine, "decoration twin hash cases", hashWords.byteLength, hashWords),
    hash_results: storage(engine, "decoration twin hash results", hashCases.length * 4, null),
    texel_cases: storage(engine, "decoration twin texel cases", texelBytes.byteLength, texelBytes),
    texel_results: storage(engine, "decoration twin texel results", texelCases.length * 16, null),
    place_results: storage(engine, "decoration twin place results", texelCases.length * 32, null),
    normal_results: storage(engine, "decoration twin normal results", texelCases.length * 16, null),
    detail_results: storage(engine, "decoration twin detail results", texelCases.length * 48, null),
  };
  try {
    const invocations = hashCases.length + texelCases.length;
    engine.dispatch(
      kernel,
      {
        uniforms: { params: new Uint32Array([hashCases.length, texelCases.length, 0, 0]) },
        buffers,
        sampled: {},
        storage: {},
      },
      [Math.ceil(invocations / DECORATION_TWIN_WORKGROUP), 1, 1],
      DECORATION_TWIN_KERNEL.name,
    );
    const hashes = new Uint32Array(await engine.readBuffer(buffers.hash_results));
    const texels = new Float32Array(await engine.readBuffer(buffers.texel_results));
    const places = new Uint32Array(await engine.readBuffer(buffers.place_results));
    const normals = new Float32Array(await engine.readBuffer(buffers.normal_results));
    const details = new Float32Array(await engine.readBuffer(buffers.detail_results));

    const hashMisses = hashCases.flatMap((c, k) => {
      const expected = latticeHash(c.seed, c.salt, c.face, c.level, c.x, c.y);
      return hashes[k] === expected
        ? []
        : [`${JSON.stringify(c)}: ${String(hashes[k])} ≠ ${expected}`];
    });
    checks.check(
      "R11.T2.a the lattice hash equals its TypeScript reference at 10⁴ lattice points",
      hashMisses.length === 0,
      hashMisses.length === 0
        ? `${hashCases.length} hashes equal`
        : `${hashMisses.length} differ, first ${hashMisses[0] ?? ""}`,
    );

    const placeMisses: string[] = [];
    const fields = new Worst();
    const normal = new Worst();
    const detail = new Worst();
    // A fraction's bits, as the kernel's `bitcast` writes them.
    const fractionBits = new Float32Array(2);
    const fractionWords = new Uint32Array(fractionBits.buffer);
    texelCases.forEach((c, k) => {
      const expected = expectedOf(c);
      fractionBits.set([expected.below.fx, expected.below.fy]);
      const wantedPlaces = [
        ...expected.places,
        expected.below.x,
        expected.below.y,
        fractionWords[0] ?? 0,
        fractionWords[1] ?? 0,
      ];
      const got = Array.from(places.subarray(8 * k, 8 * k + 8));
      if (got.some((value, i) => value !== wantedPlaces[i])) {
        placeMisses.push(`case ${k}: ${got.join(",")} ≠ ${wantedPlaces.join(",")}`);
      }
      expected.texel.forEach((value, i) => {
        fields.note(texels[4 * k + i], value, `case ${k} field ${i}`);
      });
      expected.normal.forEach((value, i) => {
        normal.note(normals[4 * k + i], value, `case ${k} component ${i}`);
      });
      expected.detail.forEach((value, i) => {
        detail.note(details[12 * k + i], value, `case ${k} value ${i}`);
      });
    });
    checks.check(
      "R11.T2.a each texel's places on three lattices equal the reference's, one below its patch's",
      placeMisses.length === 0,
      placeMisses.length === 0
        ? `${texelCases.length} texels, three places each`
        : `${placeMisses.length} differ, first ${placeMisses[0] ?? ""}`,
    );
    for (const [what, worst] of [
      ["decoration", fields],
      ["perturbed normal", normal],
      ["detail projection, its coordinates and weights,", detail],
    ] as const) {
      checks.check(
        `R11.T2.a each texel's ${what} equals the reference's to f32's rounding`,
        worst.value <= TEXEL_TOLERANCE,
        `largest relative difference ${worst.value.toExponential(2)} (${worst.where}) against ${TEXEL_TOLERANCE}`,
      );
    }
  } finally {
    for (const buffer of Object.values(buffers)) {
      engine.releaseBuffer(buffer);
    }
    engine.releaseCompute(kernel);
  }
}
