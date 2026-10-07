/**
 * The machine's load average and Design note 27's quiet-machine rule, on every platform (plan R05,
 * T20; decision-cross-platform-server.md, items 6 and 7).
 *
 * @remarks
 * Node's `os.loadavg()` gives the kernel's averages on Linux and macOS, and `[0, 0, 0]` on Windows,
 * which keeps none (Node's `os` documentation). A Windows reading is therefore no reading: a run
 * there is always provisional, since whether its machine was quiet is unchecked. Nothing here reads
 * `/proc`, so the descent's demand record (`scripts/descentDemand.mjs`, which loads this module)
 * runs on every platform. Since results version 6 the results file records the load average as a
 * `Measured`, none with {@link NO_WINDOWS_LOAD_AVERAGE} on Windows; the demand record writes an
 * empty list.
 */

import { type Measured, measured, missing } from "./measured";

/** The 1-, 5- and 15-minute load averages. */
export type LoadAverage = readonly [number, number, number];

/** Why a Windows machine has no load average. */
export const NO_WINDOWS_LOAD_AVERAGE = "Windows keeps no load average";

/** The quiet note of a run on a platform that keeps no load average, which is always provisional. */
export const QUIET_RULE_UNCHECKED = `${NO_WINDOWS_LOAD_AVERAGE}: the quiet-machine rule (Design note 27) is unchecked`;

/** What reads the load average, injected so that a test runs each platform's path on any. */
export interface LoadSources {
  /** `process.platform` outside a test. */
  readonly platform: NodeJS.Platform;
  /** `os.loadavg` outside a test. */
  loadavg(): ReadonlyArray<number>;
}

/**
 * Whether `platform` keeps a load average: every platform but Windows, under Node's name (`win32`)
 * or Rust's (`windows`, a native replay's `run.platform`).
 */
export function keepsLoadAverage(platform: string): boolean {
  return platform !== "win32" && platform !== "windows";
}

/**
 * The machine's load average now, or why it has none.
 *
 * @throws Error when `loadavg` gives fewer than three averages, which Node's never does.
 */
export function readLoadAverage(sources: LoadSources): Measured<LoadAverage> {
  if (!keepsLoadAverage(sources.platform)) {
    return missing(NO_WINDOWS_LOAD_AVERAGE);
  }
  const averages = sources.loadavg();
  const [one, five, fifteen] = averages;
  if (one === undefined || five === undefined || fifteen === undefined) {
    throw new Error(`os.loadavg() gave ${averages.length} of its three averages`);
  }
  return measured([one, five, fifteen]);
}

/**
 * The load average a demand record's cell carries: Node's, or an empty list where the platform
 * keeps none, which the record's summary states.
 */
export function recordedLoadAverage(sources: LoadSources): ReadonlyArray<number> {
  return readLoadAverage(sources).value ?? [];
}

/** Whether a run counts towards the verdict under Design note 27, and why not. */
export interface Quiet {
  readonly provisional: boolean;
  readonly note: string | null;
}

/**
 * Design note 27's rule for a run started on `platform` at `loadAverage`: provisional at a
 * 1-minute load of 1 or more, and always on a platform that keeps no load average.
 */
export function quietOf(platform: string, loadAverage: LoadAverage): Quiet {
  if (!keepsLoadAverage(platform)) {
    return { provisional: true, note: QUIET_RULE_UNCHECKED };
  }
  const [load] = loadAverage;
  return load >= 1
    ? {
        provisional: true,
        note: `load average ${load.toFixed(2)} at the start (Design note 27 asks under 1): provisional`,
      }
    : { provisional: false, note: null };
}
