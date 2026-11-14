/**
 * Folded dispatches (plan R14, R14.T5.d; Design note 6): a run of workgroups past WebGPU's 65,535 a
 * dimension laid over two dimensions, and the index each workgroup of the fold takes.
 *
 * @remarks
 * WebGPU's `maxComputeWorkgroupsPerDimension` is 65,535 by default, which every adapter meets and
 * the engine never raises (W3C WebGPU, "Limits"). The cluster cull takes a workgroup an instance,
 * so a consumer of more instances than that (R11's scatter, T5.c's decoration) folds them: `n`
 * workgroups as y = ⌈n ÷ 65,535⌉ rows of x = ⌈n ÷ y⌉, the workgroup (gx, gy) taking index
 * gy · x + gx, which the kernel reads from `@builtin(num_workgroups)` (WGSL's `folded_group`,
 * {@link FOLDED_GROUP_WGSL}), so that a direct and an indirect dispatch fold alike. The indices
 * below `n` are each taken once; the x · y − n left over, fewer than y, find no work. A run of at
 * most 65,535 is one row, its indices the unfolded dispatch's.
 *
 * The GPU forms the same fold from a count it wrote (`instanceFold.wgsl`'s `fold_of`), integer
 * arithmetic, so that an indirect dispatch's arguments are the CPU's for that count.
 */

/** WebGPU's default `maxComputeWorkgroupsPerDimension`, the most a fold's row or column holds. */
export const MAX_WORKGROUPS_PER_DIMENSION = 65_535;

/** The most workgroups a fold lays out: 65,535 rows of 65,535. */
export const MAX_FOLDED_WORKGROUPS = MAX_WORKGROUPS_PER_DIMENSION * MAX_WORKGROUPS_PER_DIMENSION;

/**
 * The fold of `n` workgroups (see the module's remarks): (⌈n ÷ y⌉, y, 1), y = ⌈n ÷ 65,535⌉, and
 * (0, 1, 1) for none.
 *
 * @throws RangeError for an `n` that is not a whole number from 0 to {@link MAX_FOLDED_WORKGROUPS}.
 */
export function foldedWorkgroups(n: number): readonly [number, number, 1] {
  if (!Number.isSafeInteger(n) || n < 0 || n > MAX_FOLDED_WORKGROUPS) {
    throw new RangeError(
      `${String(n)} workgroups do not fold: a whole number from 0 to ${String(MAX_FOLDED_WORKGROUPS)}`,
    );
  }
  const y = Math.max(1, Math.ceil(n / MAX_WORKGROUPS_PER_DIMENSION));
  return [Math.ceil(n / y), y, 1];
}

/** The index the workgroup (gx, gy) of a fold `x` wide takes: gy · x + gx. */
export function foldedIndex(gx: number, gy: number, x: number): number {
  return gy * x + gx;
}

/**
 * WGSL's `folded_group(group, groups)`, the index a workgroup of a fold takes from its
 * `@builtin(workgroup_id)` and `@builtin(num_workgroups)` ({@link foldedIndex}), composed into
 * every kernel a fold dispatches.
 */
export const FOLDED_GROUP_WGSL = `
// The index of workgroup \`group\` of a fold of \`groups\` (plan R14, R14.T5.d; \`clusters/fold.ts\`):
// its row's first index, the row's width times the row, plus its place in the row.
fn folded_group(group : vec3u, groups : vec3u) -> u32 {
  return group.y * groups.x + group.x;
}
`;
