/**
 * The inspection profile's probes of the ray march from orbit (plan R08, R08.T8.d; the owner's Mac,
 * 2026-11-01: from 22 Mm the march took 26% of the M5's GPU at 17.6 µs a workgroup against 5.1 µs
 * at 390 km). Both are check-only: an inspection profile's `--march-probe` and its census, never a
 * player's frame.
 *
 * @remarks
 * - {@link marchProbedKernel}: the march with no far-limb footprint, every texel its centre ray
 *   alone, so that one build's profiles tell the footprint's node rays apart from the rest of the
 *   march.
 * - {@link MARCH_CENSUS_KERNEL}: the footprint's rays counted over a march's area as its entry point
 *   marches them (`marchCensus.wgsl`), texel by texel and tile by tile, so that a profile names how
 *   many texels take how many node rays and how much of the area's work waits on them.
 */

import { BUFFER_USAGE } from "../engine/gpuFlags";
import type { KernelPair } from "../engine/kernels";
import type { BufferHandle, ComputeBindings } from "../engine/types";
import preciseTrigWgsl from "../shaders/preciseTrig.wgsl?raw";
import commonWgsl from "./shaders/common.wgsl?raw";
import farLimbWgsl from "./shaders/farLimb.wgsl?raw";
import marchAreaWgsl from "./shaders/marchArea.wgsl?raw";
import marchCensusWgsl from "./shaders/marchCensus.wgsl?raw";
import oblateWgsl from "./shaders/oblate.wgsl?raw";
import viewWgsl from "./shaders/view.wgsl?raw";

/**
 * A probe of the march (`--march-probe`): `centre`, every texel's centre ray alone, as a texel of
 * w_L 0 is marched, with no footprint formed or marched anywhere.
 */
export type MarchProbe = "centre";

/**
 * The line of the march's entry points (`rayMarchFootprint.wgsl`, `rayMarchCentre.wgsl`) that a
 * probe replaces: the camera above the top, which every footprint is formed under.
 */
const MARCH_ABOVE_LINE = "let above = view.camera.w * view.curvature.z >= medium.topHeightM;";

/** What each probe puts in {@link MARCH_ABOVE_LINE}'s place: `centre`, the camera never above. */
const MARCH_PROBE_LINES: Readonly<Record<MarchProbe, string>> = { centre: "let above = false;" };

/** The names of the march's kernels, each of which begins with the general march's. */
const MARCH_KERNEL_PREFIX = "atmosphere ray march";

/**
 * `pair` under `probe` where it is one of the march's kernels: `centre` holds the camera under the
 * top in the entry point's footprint test (`let above = false;`), so that no texel forms or marches
 * a footprint and the tile cull's centre ray classes no tile as the footprint's. Any other kernel,
 * and a march's that holds no such test (its fill), is `pair` itself. The name is kept, so that a
 * profile's kernels read as they do unprobed.
 *
 * @throws Error naming the kernel where its WGSL holds the test more than once.
 */
export function marchProbedKernel(pair: KernelPair, probe: MarchProbe): KernelPair {
  if (!pair.name.startsWith(MARCH_KERNEL_PREFIX)) {
    return pair;
  }
  const rewrite = (wgsl: string): string => {
    const at = wgsl.indexOf(MARCH_ABOVE_LINE);
    if (at < 0) {
      return wgsl;
    }
    if (wgsl.indexOf(MARCH_ABOVE_LINE, at + 1) >= 0) {
      throw new Error(`kernel ${pair.name} holds more than one place for: ${MARCH_ABOVE_LINE}`);
    }
    return wgsl.replace(MARCH_ABOVE_LINE, MARCH_PROBE_LINES[probe]);
  };
  const reference = rewrite(pair.reference);
  return reference === pair.reference
    ? pair
    : {
        ...pair,
        reference,
        subgroup: pair.subgroup === null ? null : rewrite(pair.subgroup),
      };
}

/**
 * The far limb's census over a march's area (`marchCensus.wgsl`): dispatched over the march's own
 * tiles with its uniforms and depth, its counts integers, so read back exactly.
 */
export const MARCH_CENSUS_KERNEL: KernelPair = {
  name: "atmosphere march census",
  reference: `${preciseTrigWgsl}\n${commonWgsl}\n${viewWgsl}\n${oblateWgsl}\n${farLimbWgsl}\n${marchAreaWgsl}\n${marchCensusWgsl}`,
  subgroup: null,
  readback: "bit-exact",
};

/** The most nodes a footprint takes (`marchCensus.wgsl`'s MARCH_CENSUS_NODES): 2 and 8. */
export const MARCH_CENSUS_NODES = 10;

/** `marchCensus.wgsl`'s `MarchCensus`, in words: the texels by nodes, then six counts. */
const MARCH_CENSUS_WORDS = MARCH_CENSUS_NODES + 1 + 6;

/** The census buffer's size, bytes, and its usage: written by the kernel, cleared and read back. */
export const MARCH_CENSUS_BUFFER = {
  bytes: MARCH_CENSUS_WORDS * 4,
  usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC | BUFFER_USAGE.COPY_DST,
} as const;

/**
 * A march's far-limb census: what each texel of its area marches under the `area` cover, the
 * footprint's entry point over every tile, whatever cover drew the frame.
 */
export interface MarchCensus {
  /** The march's kernel the census was taken beside. */
  readonly kernel: string;
  /** The texels on the target by their footprints' node rays, 0 to {@link MARCH_CENSUS_NODES}. */
  readonly texelsByNodes: ReadonlyArray<number>;
  /** The texels whose footprint's blend is 1, which march their nodes and no centre ray (E4). */
  readonly wholeMeans: number;
  /** The texels whose centre ray meets the top's shell ahead of the camera. */
  readonly centresInAir: number;
  /** The area's tiles of 8 × 8 texels, the march's workgroups. */
  readonly tiles: number;
  /** The tiles that hold a texel with a footprint's node ray. */
  readonly limbTiles: number;
  /** The rays marched that meet the shell: each texel's centre ray where it does, and its nodes. */
  readonly rays: number;
  /**
   * The rays as each tile's lanes wait on its slowest texel: its texels on the target times that
   * texel's rays, summed over the tiles. Over {@link MarchCensus.rays}, the share of the march's
   * lanes' time spent waiting.
   */
  readonly laneRays: number;
}

/** The census of the words `marchCensus.wgsl` wrote, taken beside `kernel`. */
export function marchCensusOf(kernel: string, words: Uint32Array): MarchCensus {
  if (words.length < MARCH_CENSUS_WORDS) {
    throw new RangeError(
      `a march census holds ${String(MARCH_CENSUS_WORDS)} words, not ${String(words.length)}`,
    );
  }
  const word = (i: number): number => words[i] ?? 0;
  const first = MARCH_CENSUS_NODES + 1;
  return {
    kernel,
    texelsByNodes: Array.from(words.subarray(0, first)),
    wholeMeans: word(first),
    centresInAir: word(first + 1),
    tiles: word(first + 2),
    limbTiles: word(first + 3),
    rays: word(first + 4),
    laneRays: word(first + 5),
  };
}

/**
 * The census's bindings beside a dispatch of `kernel` with `bindings`, given the buffer it counts
 * into, or `null` where it is not a march's footprint over an area: one of the march's kernels but
 * its tile cull's centre ray's, dispatched with the medium, the view and the area, the scene's depth
 * and the body's table, as every footprint's entry point is, thin or thick, deck or thermal.
 */
export function marchCensusBindingsOf(
  kernel: string,
  bindings: ComputeBindings,
): ((census: BufferHandle) => ComputeBindings) | null {
  const { medium, view, marchArea } = bindings.uniforms;
  const { sceneDepth, oblateBody } = bindings.sampled;
  if (
    !kernel.startsWith(MARCH_KERNEL_PREFIX) ||
    kernel.includes("at the centre ray") ||
    medium === undefined ||
    view === undefined ||
    marchArea === undefined ||
    sceneDepth === undefined ||
    oblateBody === undefined
  ) {
    return null;
  }
  return (census) => ({
    uniforms: { medium, view, marchArea },
    buffers: { census },
    sampled: { sceneDepth, oblateBody },
    storage: {},
  });
}
