/**
 * The smoke harness's twin check of the cluster vertex stages' decode (plan R14, R14.T2.a, Design
 * note 19's mask; `R14.T2.a the cluster decode against its twin`): `terrainCluster.wgsl`'s
 * `terrain_cluster_grid`, which every cluster pass's vertex stage calls, dispatched on the device
 * over every cluster of a terrain page an instance draws (a mask's cull drops the rest) at template
 * vertices of every kind, for a whole patch, a
 * sub-patch of its own grid and stand-ins' masks one to three levels down at several parts, its grid
 * places read back and held to its twins' (`clusters/terrain.ts`' `terrainClusterGrid`, and
 * `terrainFinGrid` for the fin and head runs) exactly.
 *
 * @remarks
 * The kernel is the stage's decode as the wireframe composes it (its source before its vertex entry
 * point, `terrainWire.ts`' `withWireClusterVertex`), less its list's binding, after the lit mesh's
 * codes (`terrainSurvey.wgsl`'s) and before a compute entry point of the check's own; the decode's
 * arithmetic is in whole numbers, so the two agree to the bit.
 */

import {
  TERRAIN_CLUSTERS_PER_PAGE,
  TERRAIN_PAGE_CLUSTERS,
  TERRAIN_SUB_PATCH_MASK,
  terrainClusterGrid,
  terrainFinGrid,
  terrainMaskCluster,
} from "../view/clusters/terrain";
import type { KernelPair } from "../view/engine/kernels";
import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { BufferHandle, RenderEngine } from "../view/engine/types";
import { subPatchWord } from "../view/terrain/gpu/uniforms";
import terrainClusterWgsl from "../view/terrain/shaders/terrainCluster.wgsl?raw";
import type { Checks } from "./harness";

/** The lit mesh's codes as `terrainSurvey.wgsl` holds them, which the decode reads. */
const LIT_CODES = `
const LIT_GRID : u32 = 0u;
const LIT_SKIRT_FOOT : u32 = 1u;
const LIT_SKIRT_HEAD : u32 = 2u;
const LIT_COLUMN_FIN : u32 = 4u;
const LIT_ROW_FIN : u32 = 5u;
`;

/** The check's entry point: one decode an input, (cluster, lx | ly << 16, sub, code). */
const ENTRY = `
@group(0) @binding(0) var<storage, read> grid_inputs : array<vec4u>;
@group(0) @binding(1) var<storage, read_write> grid_outputs : array<vec4f>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id : vec3u) {
  if (id.x >= arrayLength(&grid_inputs)) {
    return;
  }
  let input = grid_inputs[id.x];
  let local = vec3f(f32(input.y & 0xffffu), f32(input.y >> 16u), f32(input.w));
  grid_outputs[id.x] = vec4f(terrain_cluster_grid(input.x, local, input.z), 0.0);
}
`;

/** The decode as the cluster stages compose it, its list's binding left out. */
function decodeSource(): string {
  const [decode = ""] = terrainClusterWgsl.split("@vertex\n");
  return decode.replace(/@group\(2\) @binding\(22\)[^\n]*\n/u, "");
}

/** The check's kernel: the decode and its entry point. */
const CLUSTER_GRID_KERNEL: KernelPair = {
  name: "smoke cluster decode",
  reference: `${LIT_CODES}${decodeSource()}${ENTRY}`,
  subgroup: null,
  readback: "bit-exact",
};

/**
 * The template vertices tried, (lx, ly, code), of a block's or a strip's (8 quads, every code) and
 * of a fin or head run's (32 quads, `local.z` its kind).
 */
const LOCALS: ReadonlyArray<readonly [number, number, number]> = [
  [0, 0, 0],
  [3, 0, 0],
  [8, 0, 1],
  [5, 1, 2],
  [2, 7, 0],
  [8, 8, 0],
];
const RUN_LOCALS: ReadonlyArray<readonly [number, number, number]> = [
  [0, 0, 0],
  [17, 0, 1],
  [31, 0, 0],
  [32, 0, 1],
];

/** The instances' sub-patch words: a whole patch, a sub-patch, and masks at several parts. */
function subWords(): number[] {
  const words = [0, subPatchWord(2, 1, 3), subPatchWord(5, 17, 4)];
  for (const levels of [1, 2, 3]) {
    const side = 2 ** levels;
    for (const [i, j] of [
      [0, 0],
      [side - 1, 0],
      [Math.floor(side / 2), side - 1],
    ] as const) {
      words.push((TERRAIN_SUB_PATCH_MASK | subPatchWord(levels, i, j)) >>> 0);
    }
  }
  return words;
}

/** R14.T2.a's decode on the device against its twin: see the module's remarks. */
export async function checkClusterGrid(engine: RenderEngine, checks: Checks): Promise<void> {
  const inputs: Array<readonly [number, number, number, number]> = [];
  for (const sub of subWords()) {
    for (let cluster = 0; cluster < TERRAIN_PAGE_CLUSTERS; cluster += 1) {
      // Only the clusters the instance draws: a mask's cull drops the rest.
      if (terrainMaskCluster(sub, cluster) === "none") {
        continue;
      }
      for (const [lx, ly, code] of cluster < TERRAIN_CLUSTERS_PER_PAGE ? LOCALS : RUN_LOCALS) {
        inputs.push([cluster, lx | (ly << 16), sub, code]);
      }
    }
  }
  const made: BufferHandle[] = [];
  const buffer = (name: string, bytes: number, readable: boolean): BufferHandle => {
    const handle = engine.createBuffer({
      name,
      bytes,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST | (readable ? BUFFER_USAGE.COPY_SRC : 0),
      category: "geometry-clusters",
    });
    made.push(handle);
    return handle;
  };
  try {
    const kernel = await engine.createComputeAsync(CLUSTER_GRID_KERNEL);
    const packed = new Uint32Array(inputs.flat());
    const gridInputs = buffer("smoke cluster decode inputs", packed.byteLength, false);
    engine.writeBuffer(gridInputs, 0, packed);
    const gridOutputs = buffer("smoke cluster decode outputs", inputs.length * 16, true);
    engine.dispatch(
      kernel,
      {
        uniforms: {},
        buffers: { grid_inputs: gridInputs, grid_outputs: gridOutputs },
        sampled: {},
        storage: {},
      },
      [Math.ceil(inputs.length / 64), 1, 1],
      "smoke cluster decode",
    );
    const device = new Float32Array(await engine.readBuffer(gridOutputs));
    let differing = 0;
    let masked = 0;
    let first = "";
    inputs.forEach(([cluster, packedLocal, sub, code], n) => {
      const local = [packedLocal & 0xffff, packedLocal >>> 16, code] as const;
      const twin =
        cluster < TERRAIN_CLUSTERS_PER_PAGE
          ? terrainClusterGrid(cluster, local, sub)
          : terrainFinGrid(cluster, local);
      const got = [device[4 * n], device[4 * n + 1], device[4 * n + 2]];
      masked += (sub & TERRAIN_SUB_PATCH_MASK) !== 0 && cluster >= 64 && cluster < 96 ? 1 : 0;
      if (got.some((v, k) => v !== twin[k])) {
        differing += 1;
        if (first === "") {
          first = `; first at cluster ${String(cluster)}, sub ${sub.toString(16)}, local ${local.join(",")}: ${got.join(",")} against ${twin.join(",")}`;
        }
      }
    });
    checks.check(
      "R14.T2.a the cluster vertex stages' decode on the device is its twin's, a stand-in's mask's strips along its set's sides among them (Design note 19)",
      differing === 0 && masked > 0,
      `${String(inputs.length)} template vertices of ${String(TERRAIN_PAGE_CLUSTERS)} clusters under ${String(subWords().length)} sub-patch words, ${String(masked)} of them a mask's strips; ${String(differing)} differing${first}`,
    );
  } finally {
    for (const handle of made) {
      engine.releaseBuffer(handle);
    }
  }
}
