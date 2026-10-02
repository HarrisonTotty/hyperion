/**
 * The smoke page's subgroup-twin checks (R01.T10, Design note 16), run on whichever path the run's
 * capabilities select: the bit-exact u32 sums equal the CPU's wrapping total, and the
 * presentation-only f32 sum is refused by the ordinary readback and lies within the Higham bound
 * of an `f64` sum when read with the harness's tolerance access.
 */

import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import { highamBound, type KernelPair } from "../view/engine/kernels";
import {
  SUM_F32,
  SUM_F32_COUNT,
  SUM_U32,
  SUM_U32_COUNT,
  SUM_U32_RAGGED,
  SUM_U32_RAGGED_COUNT,
  SUM_U32_RAGGED_WORKGROUP,
  SUM_U32_WORKGROUP,
  sumF32Inputs,
  sumF64,
  sumU32Inputs,
  wrappingSumU32,
} from "../view/engine/twins";
import {
  PresentationOnlyReadback,
  type BufferHandle,
  type RenderEngine,
} from "../view/engine/types";
import type { Checks } from "./harness";

/** A storage buffer holding `data`. */
function inputBuffer(
  engine: RenderEngine,
  name: string,
  data: Uint32Array | Float32Array,
): BufferHandle {
  const buffer = engine.createBuffer({
    name,
    bytes: data.byteLength,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeBuffer(buffer, 0, data);
  return buffer;
}

/** Runs one u32 twin over `count` inputs and checks its total against the CPU's. */
async function checkU32(
  engine: RenderEngine,
  checks: Checks,
  pair: KernelPair,
  count: number,
  workgroup: number,
): Promise<void> {
  const inputs = sumU32Inputs(count);
  const kernel = await engine.createComputeAsync(pair);
  const result = engine.createBuffer({
    name: `${pair.name} result`,
    bytes: 8,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
    category: "other",
  });
  engine.dispatch(
    kernel,
    {
      uniforms: { params: new Uint32Array([count]) },
      buffers: { values: inputBuffer(engine, `${pair.name} values`, inputs), result },
      sampled: {},
      storage: {},
    },
    [Math.ceil(count / workgroup), 1, 1],
    pair.name,
  );
  const [total = Number.NaN, subgroupSize = 0] = new Uint32Array(await engine.readBuffer(result));
  const expected = wrappingSumU32(inputs);
  const wraps = Math.floor(inputs.reduce((sum, value) => sum + value, 0) / 2 ** 32);
  checkPath(engine, checks, pair.name, kernel.path);
  checks.check(
    `T10 ${pair.name} on the ${kernel.path} path equals the CPU's wrapping sum`,
    total === expected && wraps >= 1,
    `${total} against ${expected}, ${wraps} wraps, ${count} inputs in workgroups of ${workgroup}` +
      (kernel.path === "subgroup" ? `, subgroup_size ${subgroupSize}` : ""),
  );
  if (kernel.path === "subgroup") {
    const minimum = engine.capabilities.subgroupMinSize ?? 4;
    checks.check(
      `T10 ${pair.name}'s subgroup_size is a power of two in [4, 128], at least the minimum`,
      Number.isInteger(Math.log2(subgroupSize)) &&
        subgroupSize >= Math.max(4, minimum) &&
        subgroupSize <= 128,
      `subgroup_size ${subgroupSize}, the adapter's minimum ${minimum}`,
    );
  }
}

/** Checks that a twin ran on the path the device's capabilities select (T9.c's two paths). */
function checkPath(
  engine: RenderEngine,
  checks: Checks,
  name: string,
  path: "reference" | "subgroup",
): void {
  const expected = engine.capabilities.subgroups ? "subgroup" : "reference";
  checks.check(
    `T10 ${name} runs on the ${expected} path`,
    path === expected,
    `ran on the ${path} path; subgroups ${engine.capabilities.subgroups ? "yes" : "no"}`,
  );
}

/** T10: both u32 twins and the f32 twin, on the run's path. */
export async function checkTwins(engine: RenderEngine, checks: Checks): Promise<void> {
  await checkU32(engine, checks, SUM_U32, SUM_U32_COUNT, SUM_U32_WORKGROUP);
  await checkU32(engine, checks, SUM_U32_RAGGED, SUM_U32_RAGGED_COUNT, SUM_U32_RAGGED_WORKGROUP);

  const inputs = sumF32Inputs(SUM_F32_COUNT);
  const kernel = await engine.createComputeAsync(SUM_F32);
  const result = engine.createBuffer({
    name: "sum f32 result",
    bytes: 4,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
    category: "other",
  });
  engine.dispatch(
    kernel,
    {
      uniforms: { params: new Uint32Array([SUM_F32_COUNT]) },
      buffers: { values: inputBuffer(engine, "sum f32 values", inputs), result },
      sampled: {},
      storage: {},
    },
    [1, 1, 1],
    SUM_F32.name,
  );
  let refused = "nothing thrown";
  try {
    await engine.readBuffer(result);
  } catch (error: unknown) {
    refused =
      error instanceof PresentationOnlyReadback ? "PresentationOnlyReadback" : String(error);
  }
  checks.check(
    "T10 sum f32's ordinary readback throws PresentationOnlyReadback",
    refused === "PresentationOnlyReadback",
    refused,
  );
  checkPath(engine, checks, SUM_F32.name, kernel.path);
  const gpu = new Float32Array(await engine.readBuffer(result, "tolerance"))[0] ?? Number.NaN;
  const { sum, sumAbs } = sumF64(inputs);
  const bound = highamBound(SUM_F32_COUNT, sumAbs);
  checks.check(
    `T10 sum f32 on the ${kernel.path} path lies within the Higham bound of the f64 sum`,
    Math.abs(gpu - sum) <= bound,
    `${gpu} against ${sum}, off by ${Math.abs(gpu - sum)}, bound ${bound.toPrecision(4)} (whether the paths differ is logged across runs, never asserted)`,
  );
}
