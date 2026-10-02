/**
 * R01.T10's subgroup twins (Design note 16): toy kernel pairs, each with a reference and a
 * `subgroupAdd` variant, that the smoke harness runs on both capability paths, with their inputs
 * and CPU references.
 *
 * @remarks
 * - `sum u32` and `sum u32 ragged` are `bit-exact`: a wrapping `u32` sum is associative, so both
 *   paths' bytes equal each other and the CPU's `wrapping_add` total. The inputs make the sum wrap
 *   past 2³² many times. `ragged`'s workgroups are 37 invocations, which no subgroup size (a power
 *   of two from 4 to 128, WGSL §15.5) divides, so the last subgroup of each is partly inactive
 *   and must add the identity. The subgroup variant reads `subgroup_size` and records the largest
 *   it saw; it hard-codes none.
 * - `sum f32` is `presentation-only`: the two paths sum in different orders, so its result may only
 *   be read with the harness's tolerance access, within `highamBound` of an `f64` sum of finite,
 *   normal inputs (WGSL may flush subnormals).
 *
 * Every uniform-control-flow rule holds: each `subgroupAdd` follows a bounds `if`, never sits in a
 * loop whose trip count varies by invocation.
 */

import type { KernelPair } from "./kernels";

/** The u32 inputs' count: 2¹⁶. */
export const SUM_U32_COUNT = 65_536;

/** The ragged sum's count, which no workgroup of 37 divides. */
export const SUM_U32_RAGGED_COUNT = 65_533;

/** The f32 inputs' count. */
export const SUM_F32_COUNT = 65_536;

/** The u32 sum's workgroup size. */
export const SUM_U32_WORKGROUP = 64;

/** The ragged u32 sum's workgroup size, which no subgroup size divides. */
export const SUM_U32_RAGGED_WORKGROUP = 37;

/** The u32 inputs: 24-bit values by a multiplicative hash, so that the sum wraps many times. */
export function sumU32Inputs(count: number): Uint32Array {
  return Uint32Array.from(
    { length: count },
    (_, index) => Math.imul(index + 1, 2_654_435_761) >>> 8,
  );
}

/** The CPU's wrapping sum, as Rust's `wrapping_add` folds it. */
export function wrappingSumU32(values: ArrayLike<number>): number {
  let sum = 0;
  for (let index = 0; index < values.length; index += 1) {
    sum = (sum + (values[index] ?? 0)) >>> 0;
  }
  return sum;
}

/**
 * The f32 inputs: finite and normal, in [1, 2), each exact in f32 with 20 fraction bits, so that the
 * partial sums round and the two paths may differ.
 */
export function sumF32Inputs(count: number): Float32Array {
  return Float32Array.from(
    { length: count },
    (_, index) => 1 + ((index * 7919) % 1_048_576) / 1_048_576,
  );
}

/** The `f64` sum and Σ|xᵢ| of `values`. */
export function sumF64(values: ArrayLike<number>): {
  readonly sum: number;
  readonly sumAbs: number;
} {
  let sum = 0;
  let sumAbs = 0;
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index] ?? 0;
    sum += value;
    sumAbs += Math.abs(value);
  }
  return { sum, sumAbs };
}

/** The bindings every u32 twin declares: the count, the inputs, the total and the subgroup size. */
const U32_BINDINGS = `
struct Params { count : u32 }
@group(0) @binding(0) var<uniform> params : Params;
@group(0) @binding(1) var<storage, read> values : array<u32>;
@group(0) @binding(2) var<storage, read_write> result : array<atomic<u32>, 2>;
`;

/** A u32 sum's reference: each workgroup's partial sums serially, then one `atomicAdd`. */
function u32Reference(workgroup: number): string {
  return `${U32_BINDINGS}
var<workgroup> partial : array<u32, ${workgroup}>;

@compute @workgroup_size(${workgroup})
fn main(@builtin(global_invocation_id) id : vec3u, @builtin(local_invocation_index) local : u32) {
  var value = 0u;
  if (id.x < params.count) {
    value = values[id.x];
  }
  partial[local] = value;
  workgroupBarrier();
  if (local == 0u) {
    var sum = 0u;
    for (var index = 0u; index < ${workgroup}u; index += 1u) {
      sum += partial[index];
    }
    atomicAdd(&result[0], sum);
  }
}
`;
}

/** A u32 sum's subgroup variant: `subgroupAdd`, then one `atomicAdd` a subgroup. */
function u32Subgroup(workgroup: number): string {
  return `enable subgroups;
${U32_BINDINGS}
@compute @workgroup_size(${workgroup})
fn main(
  @builtin(global_invocation_id) id : vec3u,
  @builtin(subgroup_invocation_id) lane : u32,
  @builtin(subgroup_size) size : u32,
) {
  var value = 0u;
  if (id.x < params.count) {
    value = values[id.x];
  }
  let sum = subgroupAdd(value);
  if (lane == 0u) {
    atomicAdd(&result[0], sum);
    atomicMax(&result[1], size);
  }
}
`;
}

/** The bit-exact u32 sum, workgroups of 64. */
export const SUM_U32: KernelPair = {
  name: "sum u32",
  reference: u32Reference(SUM_U32_WORKGROUP),
  subgroup: u32Subgroup(SUM_U32_WORKGROUP),
  readback: "bit-exact",
};

/** The bit-exact u32 sum over workgroups of 37, which leave subgroups partly inactive. */
export const SUM_U32_RAGGED: KernelPair = {
  name: "sum u32 ragged",
  reference: u32Reference(SUM_U32_RAGGED_WORKGROUP),
  subgroup: u32Subgroup(SUM_U32_RAGGED_WORKGROUP),
  readback: "bit-exact",
};

/** The bindings of the f32 twins. */
const F32_BINDINGS = `
struct Params { count : u32 }
@group(0) @binding(0) var<uniform> params : Params;
@group(0) @binding(1) var<storage, read> values : array<f32>;
@group(0) @binding(2) var<storage, read_write> result : array<f32, 1>;
var<workgroup> partial : array<f32, 256>;
`;

/** One workgroup's strided sum: a uniform loop, the bounds test inside it. */
const F32_STRIDED = `
  var sum = 0.0;
  for (var base = 0u; base < params.count; base += 256u) {
    if (base + local < params.count) {
      sum += values[base + local];
    }
  }
`;

/** The tree reduction of `partial` and the write of the total. */
const F32_TREE = `
  workgroupBarrier();
  for (var stride = 128u; stride > 0u; stride >>= 1u) {
    if (local < stride) {
      partial[local] += partial[local + stride];
    }
    workgroupBarrier();
  }
  if (local == 0u) {
    result[0] = partial[0];
  }
`;

/** The presentation-only f32 sum: one workgroup of 256, strided sums, then a tree. */
export const SUM_F32: KernelPair = {
  name: "sum f32",
  reference: `${F32_BINDINGS}
@compute @workgroup_size(256)
fn main(@builtin(local_invocation_index) local : u32) {
${F32_STRIDED}
  partial[local] = sum;
${F32_TREE}
}
`,
  subgroup: `enable subgroups;
${F32_BINDINGS}
@compute @workgroup_size(256)
fn main(@builtin(local_invocation_index) local : u32, @builtin(subgroup_invocation_id) lane : u32) {
${F32_STRIDED}
  let subgroupSum = subgroupAdd(sum);
  partial[local] = select(0.0, subgroupSum, lane == 0u);
${F32_TREE}
}
`,
  readback: "presentation-only",
};

/** Every twin, for the catalogue. */
export const SUBGROUP_TWINS: ReadonlyArray<KernelPair> = [SUM_U32, SUM_U32_RAGGED, SUM_F32];
