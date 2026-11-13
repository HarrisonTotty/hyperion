/**
 * The machine's performance cores, where its OS tells them from its efficiency cores (plan R10,
 * R10.T6.a's worker count for Apple silicon), with its efficiency cores counted at a third.
 *
 * @remarks
 * `navigator.hardwareConcurrency` counts every logical processor, an Apple chip's efficiency cores
 * among them, which bake far slower than its performance cores. macOS names the performance
 * level's processors `hw.perflevel0.logicalcpu` (perflevel0 the fastest; Apple's `sysctl(3)` on
 * macOS 12 and later, the `hw.perflevel` nodes) and the efficiency level's
 * `hw.perflevel1.logicalcpu`, and Apple silicon has no SMT, so they are its cores. Three efficiency
 * cores count as one performance core's thread (a lean, 2026-11-01, plan R05's Risks): an M5's four
 * and six give six, an M1's four and four five, and a chip with two or fewer efficiency cores its
 * performance cores alone. An Intel Mac has no `hw.perflevel0` node, and no other platform is read:
 * both give `null`, and the renderer's height-worker count falls back to the thread count alone.
 */

import { execFileSync } from "node:child_process";

/** The `sysctl` node of the performance level's logical processors on macOS. */
export const PERFORMANCE_THREADS_NODE = "hw.perflevel0.logicalcpu";

/** The `sysctl` node of the efficiency level's logical processors on macOS. */
export const EFFICIENCY_THREADS_NODE = "hw.perflevel1.logicalcpu";

/** The efficiency cores' threads counted as one performance core's thread: three (a lean). */
export const EFFICIENCY_THREADS_A_PERFORMANCE_THREAD = 3;

/** Reads one `sysctl` node's value, or throws where it has none. */
export type SysctlRead = (node: string) => string;

/**
 * The performance cores' hardware threads on `platform`, from `sysctl`'s
 * {@link PERFORMANCE_THREADS_NODE}, with a third of its efficiency cores' from
 * {@link EFFICIENCY_THREADS_NODE}, rounded down: P + ⌊E ÷ 3⌋, a whole number from 1; or `null` off
 * macOS, where the performance node is absent or its read fails, and where it holds no such number.
 * Where the efficiency node is absent or holds no whole number, E is 0.
 */
export function performanceThreadsOf(platform: NodeJS.Platform, sysctl: SysctlRead): number | null {
  if (platform !== "darwin") {
    return null;
  }
  const performance = wholeNumberAt(sysctl, PERFORMANCE_THREADS_NODE);
  if (performance === null || performance < 1) {
    return null;
  }
  const efficiency = wholeNumberAt(sysctl, EFFICIENCY_THREADS_NODE) ?? 0;
  return performance + Math.floor(efficiency / EFFICIENCY_THREADS_A_PERFORMANCE_THREAD);
}

/** The whole number from 0 that `sysctl` holds at `node`, or `null` where it holds none. */
function wholeNumberAt(sysctl: SysctlRead, node: string): number | null {
  let text: string;
  try {
    text = sysctl(node);
  } catch {
    // No such node, an Intel Mac's, or no sysctl to run.
    return null;
  }
  const value = Number(text.trim());
  return text.trim().length > 0 && Number.isInteger(value) && value >= 0 ? value : null;
}

/** This machine's performance cores' threads ({@link performanceThreadsOf}), read once at launch. */
export function machinePerformanceThreads(): number | null {
  return performanceThreadsOf(process.platform, (node) =>
    execFileSync("/usr/sbin/sysctl", ["-n", node], { encoding: "utf8", timeout: 2_000 }),
  );
}
