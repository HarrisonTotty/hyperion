/**
 * The colour formats, with or without depth, a material's pipelines are made for, kept apart from
 * the engine's module so that tests outside the adapter may check every material against them
 * (plan R14, R14.T2.b's finding; `engineBoundary.test.ts`).
 */

import type { PreparedTarget, WgslMaterialSpec } from "../types";
import type { PipelineColour } from "./materials";
import { srgbViewFormat } from "./view";

/** A fragment output of the depth it writes, `@builtin(frag_depth)`, spaces allowed. */
const FRAG_DEPTH = /@builtin\s*\(\s*frag_depth\s*\)/u;

/**
 * Whether `spec`'s fragment source writes `frag_depth`: comments aside, any `@builtin(frag_depth)`
 * in it, whichever entry point's it is.
 */
export function writesFragDepth(spec: Pick<WgslMaterialSpec, "fragmentWgsl">): boolean {
  const code = spec.fragmentWgsl.replace(/\/\*[\s\S]*?\*\//gu, " ").replace(/\/\/[^\n]*/gu, " ");
  return FRAG_DEPTH.test(code);
}

/**
 * The colour formats, with or without depth, `spec`'s pipelines are made for, for each of
 * `targets` ({@link pipelineOutputs}): but none without depth for a material that writes
 * `frag_depth`, which WebGPU refuses a pipeline with no depth attachment ("Depth stencil state is
 * not present when fragment stage … is writing to frag_depth", Dawn; W3C WebGPU, "validating
 * GPUFragmentState"). A bare colour format then takes its form with depth alone, the one such a
 * material can draw into (R11.T8.b's and T8.c's seas, which plan R12, R12.T7.a's ladder found
 * refused whole for it).
 *
 * @throws Error naming the material and the format for a target without depth alone (`{ colour,
 *   depth: false }`) asked of a material that writes `frag_depth`.
 */
export function materialOutputs(
  spec: Pick<WgslMaterialSpec, "name" | "fragmentWgsl">,
  targets: ReadonlyArray<PreparedTarget>,
  preferredCanvas: GPUTextureFormat,
): Array<readonly [PipelineColour, boolean]> {
  if (!writesFragDepth(spec)) {
    return targets.flatMap((target) => pipelineOutputs(target, preferredCanvas));
  }
  return targets.flatMap((target) => {
    if (typeof target === "object" && !target.depth) {
      throw new Error(
        `material ${spec.name} writes frag_depth, which a target of ${target.colour} without depth cannot take`,
      );
    }
    return pipelineOutputs(target, preferredCanvas).filter(([, hasDepth]) => hasDepth);
  });
}

/**
 * The colour formats, with or without depth, a material's pipelines are made for ahead of its
 * draws, for one of `createMaterialAsync`'s targets or `prepareMaterial`'s (a colour format with
 * its depth or without it alone, R10.T14.d; `depth-only`, a depth-layer target's passes, with
 * depth and no colour, R10.T9.b).
 */
export function pipelineOutputs(
  target: PreparedTarget,
  preferredCanvas: GPUTextureFormat,
): Array<readonly [PipelineColour, boolean]> {
  if (typeof target === "object") {
    return [[target.colour, target.depth]];
  }
  if (target === "canvas") {
    return [[srgbViewFormat(preferredCanvas), true]];
  }
  if (target === "canvas-in-pass") {
    return [[preferredCanvas, true]];
  }
  if (target === "depth-only") {
    return [["depth-only", true]];
  }
  return [
    [target, true],
    [target, false],
  ];
}
