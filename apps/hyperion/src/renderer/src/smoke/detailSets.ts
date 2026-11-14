/**
 * The smoke harness's detail sets (plan R11, R11.T2.c; Design note 21): a body's generated sets on
 * the real device.
 *
 * @remarks
 * A lunar-like airless body's materials, one of each form that takes a set, are generated through
 * `DetailSetBake` as the terrain makes them, then read back:
 * - every texel of every layer is finite, its normal unit length and facing up, and each layer is
 *   zero-mean, its slopes about the baked normal and its albedo and hue about the class albedo, to
 *   10⁻³, at level 0 and at its last mip;
 * - the same body gives byte-equal sets in a second run;
 * - a body that differs only in its seed, its pressure class, one entry's albedo, its `FeO_T` or
 *   its maturity gives a different layer.
 * The time to generate each layer, the sets' memory, held to 16 MB, and the longest layer are
 * stated (pod, SwiftShader); the frame timings that bound an arrival are the owner's hardware's.
 */

import type { RenderEngine } from "../view/engine/types";
import { perturbNormal } from "../view/surface/decoration/decorationLattice";
import {
  DETAIL_BODY_BYTES_MAX,
  DETAIL_LAYERS,
  DETAIL_MIPS,
  DETAIL_TEXELS,
  type DetailBody,
  DetailKernelSet,
  type DetailKernelSource,
  type DetailMaterial,
  DetailSetBake,
} from "../view/surface/decoration/detailTextureBake";
import { type Checks, bytesDigest, halfTexels } from "./harness";

/** An airless, lunar-like body: mature, old, its patches' level past the sets' start. */
const MOON: DetailBody = {
  seed: 0x5eed_0001,
  radiusM: 1_737_400,
  level: 22,
  startLevel: 17,
  surfacePressurePa: 0,
  surfaceAgeGyr: 3.8,
  maturity: 0.7,
  feoByEntry: [0.16, 0.06, 0, 0],
};

/** A mare basalt's B, V and R albedos, fresh to mature (a lean of R10's law's shape). */
const BASALT: DetailMaterial = {
  classCode: 1,
  entry: 0,
  albedoBvr: [0.08, 0.1, 0.12],
  weathering: { fresh: [0.12, 0.14, 0.16], mature: [0.07, 0.09, 0.11] },
};

/** One material of each form that takes a set, on the body's four entries. */
const MATERIALS: ReadonlyArray<DetailMaterial> = [
  BASALT,
  ...[0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 14].map((classCode, k): DetailMaterial => ({
    classCode,
    entry: k % 4,
    albedoBvr: classCode >= 7 && classCode <= 10 ? [0.8, 0.85, 0.85] : [0.15, 0.2, 0.25],
    weathering: null,
  })),
  // A bright volcanic rock, flow-banded.
  { classCode: 5, entry: 3, albedoBvr: [0.3, 0.35, 0.4], weathering: null },
];

/** A flat ground's frame: its normal and its face's grid's tangents. */
const UP = [0, 0, 1] as const;
const EAST = [1, 0, 0] as const;
const NORTH = [0, 1, 0] as const;

/** The level the bake is told its patches land at: the body's finest. */
const PATCH_LEVEL = 17;

/** A bake of `body`'s sets for `materials` by `kernels`, every layer generated, each timed. */
async function generate(
  engine: RenderEngine,
  kernels: DetailKernelSource,
  body: DetailBody,
  materials: ReadonlyArray<DetailMaterial>,
  name: string,
): Promise<{ readonly bake: DetailSetBake; readonly layerMs: ReadonlyArray<number> }> {
  const bake = new DetailSetBake(engine, kernels, body, name);
  try {
    bake.noteMaterials(materials, PATCH_LEVEL);
    const layerMs: number[] = [];
    for (;;) {
      const start = performance.now();
      if (!bake.step()) {
        break;
      }
      // The queue's work done before the next layer's, so that each layer is timed alone.
      // oxlint-disable-next-line no-await-in-loop
      await engine.readTexture(bake.texture ?? fail(name), DETAIL_MIPS - 1);
      layerMs.push(performance.now() - start);
    }
    return { bake, layerMs };
  } catch (error: unknown) {
    bake.dispose();
    throw error;
  }
}

/** Every level's texels of every layer of `bake`, digested in order, level 0 first. */
async function setsDigest(engine: RenderEngine, bake: DetailSetBake): Promise<string> {
  const levels: string[] = [];
  for (let level = 0; level < DETAIL_MIPS; level += 1) {
    // One level read at a time, in order, so that the digest's order is the levels'.
    // oxlint-disable-next-line no-await-in-loop
    levels.push(bytesDigest(await levelOf(engine, bake, level)));
  }
  return levels.join("/");
}

/** Throws for a bake with no texture after a layer. */
function fail(name: string): never {
  throw new Error(`the detail bake of ${name} made no texture`);
}

/** A level's texels of every layer, as floats: four a texel, layer by layer. */
async function levelOf(
  engine: RenderEngine,
  bake: DetailSetBake,
  level: number,
): Promise<Float32Array> {
  const texture = bake.texture ?? fail("the read");
  return halfTexels(await engine.readTexture(texture, level, undefined, "cpu"));
}

/** One layer's texels of a level of side `side`. */
function layerTexels(level: Float32Array, layer: number, side: number): Float32Array {
  const size = side * side * 4;
  return level.subarray(layer * size, (layer + 1) * size);
}

/** R11.T2.c: a body's generated sets, zero-mean, deterministic and its own. */
export async function checkDetailSets(engine: RenderEngine, checks: Checks): Promise<void> {
  const kernels = DetailKernelSet.forSetting({ decoration: true }, engine);
  if (kernels === null || !(await kernels.ready)) {
    kernels?.dispose();
    checks.check("R11.T2.c the detail sets' kernels compile", false, "no kernels");
    return;
  }
  // Every bake made, let go at the end whatever fails.
  const bakes: DetailSetBake[] = [];
  const made = async (
    body: DetailBody,
    materials: ReadonlyArray<DetailMaterial>,
    name: string,
  ): Promise<{ readonly bake: DetailSetBake; readonly layerMs: ReadonlyArray<number> }> => {
    const out = await generate(engine, kernels, body, materials, name);
    bakes.push(out.bake);
    return out;
  };
  try {
    const first = await made(MOON, MATERIALS, "smoke moon");
    const second = await made(MOON, MATERIALS, "smoke moon again");
    const layers = first.bake.layers.length;
    checks.check(
      "R11.T2.c a layer for each material, at most the sets' layers",
      layers === MATERIALS.length && layers <= DETAIL_LAYERS,
      `${layers} layers of ${MATERIALS.length} materials`,
    );
    const top = await levelOf(engine, first.bake, 0);
    const last = await levelOf(engine, first.bake, DETAIL_MIPS - 1);
    let worstMean = 0;
    let worstLast = 0;
    let finite = true;
    let flat = 0;
    let worstUnit = 0;
    let leastUp = 1;
    for (let layer = 0; layer < layers; layer += 1) {
      const texels = layerTexels(top, layer, DETAIL_TEXELS);
      const sums = [0, 0, 0, 0];
      const squares = [0, 0, 0, 0];
      for (let i = 0; i < texels.length; i += 1) {
        const value = texels[i] ?? Number.NaN;
        finite &&= Number.isFinite(value);
        sums[i % 4] = (sums[i % 4] ?? 0) + value;
        squares[i % 4] = (squares[i % 4] ?? 0) + value * value;
      }
      // Each texel's normal, its slopes on a flat ground's: unit length, and facing up.
      for (let i = 0; i < texels.length; i += 4) {
        const n = perturbNormal(UP, EAST, NORTH, [texels[i] ?? 0, texels[i + 1] ?? 0]);
        worstUnit = Math.max(worstUnit, Math.abs(Math.hypot(...n) - 1));
        leastUp = Math.min(leastUp, n[2]);
      }
      const count = texels.length / 4;
      for (const sum of sums) {
        worstMean = Math.max(worstMean, Math.abs(sum / count));
      }
      // A layer of no variation would be no detail at all.
      flat += (squares[2] ?? 0) / count > 1e-6 ? 0 : 1;
      for (const value of layerTexels(last, layer, 1)) {
        worstLast = Math.max(worstLast, Math.abs(value));
      }
    }
    checks.check(
      "R11.T2.c every texel of every layer is finite",
      finite,
      `${layers} layers of ${DETAIL_TEXELS}² texels`,
    );
    checks.check(
      "R11.T2.c every texel's normal is unit length and faces up",
      finite && worstUnit <= 1e-6 && leastUp > 0,
      `largest |‖n‖ − 1| ${worstUnit.toExponential(2)}; least n · up ${leastUp.toFixed(3)}, a crack's wall`,
    );
    checks.check(
      "R11.T2.c each layer is zero-mean about its class albedo and normal to 10⁻³, and varies",
      worstMean <= 1e-3 && worstLast <= 1e-3 && flat === 0,
      `largest mean ${worstMean.toExponential(2)} at level 0, ${worstLast.toExponential(2)} at the last mip; ${flat} flat layers`,
    );
    const once = await setsDigest(engine, first.bake);
    const twice = await setsDigest(engine, second.bake);
    checks.check(
      "R11.T2.c the same body gives byte-equal sets in two runs, every mip level",
      once === twice,
      `${once} and ${twice}`,
    );

    const base = bytesDigest(layerTexels(top, 0, DETAIL_TEXELS));
    const variants: ReadonlyArray<readonly [string, DetailBody, DetailMaterial]> = [
      ["seed", { ...MOON, seed: MOON.seed + 1 }, BASALT],
      ["pressure class", { ...MOON, surfacePressurePa: 600 }, BASALT],
      ["entry's albedo", MOON, { ...BASALT, albedoBvr: [0.1, 0.12, 0.15] }],
      ["FeO_T", { ...MOON, feoByEntry: [0.08, 0.06, 0, 0] }, { ...BASALT, weathering: null }],
      ["maturity", { ...MOON, maturity: 0.3 }, BASALT],
    ];
    // The FeO_T variant is held to its own base, the basalt without nodes, which FeO_T then drives.
    const unweathered = await made(MOON, [{ ...BASALT, weathering: null }], "smoke basalt");
    const unweatheredDigest = bytesDigest(
      layerTexels(await levelOf(engine, unweathered.bake, 0), 0, DETAIL_TEXELS),
    );
    unweathered.bake.dispose();
    const same: string[] = [];
    for (const [what, body, material] of variants) {
      // Each variant's bake made, read and let go before the next, so that one bake's scratch is
      // held at a time.
      // oxlint-disable-next-line no-await-in-loop
      const variant = await made(body, [material], `smoke ${what}`);
      // oxlint-disable-next-line no-await-in-loop
      const level = await levelOf(engine, variant.bake, 0);
      const digest = bytesDigest(layerTexels(level, 0, DETAIL_TEXELS));
      variant.bake.dispose();
      if (digest === (what === "FeO_T" ? unweatheredDigest : base)) {
        same.push(what);
      }
    }
    checks.check(
      "R11.T2.c a body differing only in seed, pressure class, an entry's albedo, FeO_T or maturity gives a different set",
      same.length === 0,
      same.length === 0
        ? "every variant differs"
        : `the same with only its ${same.join(", ")} changed`,
    );

    const longestMs = Math.max(...first.layerMs);
    const meanMs = first.layerMs.reduce((sum, ms) => sum + ms, 0) / first.layerMs.length;
    checks.check(
      "R11.T2.c the sets' generation, timed (pod, SwiftShader)",
      Number.isFinite(longestMs),
      `${first.layerMs.length} layers, ${meanMs.toFixed(1)} ms a layer on average, the longest ${longestMs.toFixed(1)} ms; ${(first.bake.memoryBytes / 1e6).toFixed(1)} MB held`,
    );
    checks.check(
      "R11.T2.c a body's sets hold at most 16 MB",
      first.bake.memoryBytes <= DETAIL_BODY_BYTES_MAX,
      `${first.bake.memoryBytes} B against ${DETAIL_BODY_BYTES_MAX} B`,
    );
  } finally {
    for (const bake of bakes) {
      bake.dispose();
    }
    kernels.dispose();
  }
}
