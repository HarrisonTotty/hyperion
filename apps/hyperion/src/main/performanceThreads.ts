/**
 * The machine's performance cores, where its OS tells them from its efficiency cores (plan R10,
 * R10.T6.a's worker count for Apple silicon).
 *
 * @remarks
 * `navigator.hardwareConcurrency` counts every logical processor, an Apple chip's efficiency cores
 * among them, which bake far slower than its performance cores. macOS names the performance
 * level's processors `hw.perflevel0.logicalcpu` (perflevel0 the fastest; Apple's `sysctl(3)` on
 * macOS 12 and later, the `hw.perflevel` nodes), and Apple silicon has no SMT, so they are its
 * performance cores. An Intel Mac has no `hw.perflevel0` node, and no other platform is read: both
 * give `null`, and the renderer's height-worker count falls back to the thread count alone.
 */

import { execFileSync } from "node:child_process";

/** The `sysctl` node of the performance level's logical processors on macOS. */
export const PERFORMANCE_THREADS_NODE = "hw.perflevel0.logicalcpu";

/** Reads one `sysctl` node's value, or throws where it has none. */
export type SysctlRead = (node: string) => string;

/**
 * The performance cores' hardware threads on `platform`, from `sysctl`'s
 * {@link PERFORMANCE_THREADS_NODE}: a whole number from 1, or `null` off macOS, where the node is
 * absent or its read fails, and where it holds no such number.
 */
export function performanceThreadsOf(platform: NodeJS.Platform, sysctl: SysctlRead): number | null {
  if (platform !== "darwin") {
    return null;
  }
  let text: string;
  try {
    text = sysctl(PERFORMANCE_THREADS_NODE);
  } catch {
    // No such node, an Intel Mac's, or no sysctl to run: the thread count alone decides.
    return null;
  }
  const threads = Number(text.trim());
  return Number.isInteger(threads) && threads >= 1 ? threads : null;
}

/** This machine's performance cores' threads ({@link performanceThreadsOf}), read once at launch. */
export function machinePerformanceThreads(): number | null {
  return performanceThreadsOf(process.platform, (node) =>
    execFileSync("/usr/sbin/sysctl", ["-n", node], { encoding: "utf8", timeout: 2_000 }),
  );
}
