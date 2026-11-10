/**
 * The solid-angles worker's one job (lane perf-long-tasks-2): a face size's texel solid angles,
 * `cube.ts`'s {@link texelSolidAnglesLowerF32}, for `solidAngles.worker.ts` to post back.
 */

import { texelSolidAnglesLowerF32 } from "./cube";

/** The table for the face size a message asks, or `null` for a message that is not a face size. */
export function solidAnglesReply(sizePx: unknown): Float32Array | null {
  return typeof sizePx === "number" && Number.isInteger(sizePx) && sizePx >= 1
    ? texelSolidAnglesLowerF32(sizePx)
    : null;
}
