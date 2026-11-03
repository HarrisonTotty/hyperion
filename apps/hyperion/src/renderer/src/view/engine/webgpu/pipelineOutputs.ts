/**
 * The colour formats, with or without depth, a material's pipelines are made for, kept apart from
 * the engine's module so that tests outside the adapter may check every material against them
 * (plan R14, R14.T2.b's finding; `engineBoundary.test.ts`).
 */

import type { PreparedTarget } from "../types";
import type { PipelineColour } from "./materials";
import { srgbViewFormat } from "./view";

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
