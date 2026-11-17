/**
 * How the main process hands a perf run's launch (plan R12, R12.T4.b) to the preload.
 *
 * @remarks
 * As `spikeLaunch.ts` does for the descent spike: the launch rides in the renderer's own `argv` as
 * one extra switch, `--hyperion-perf-run=<launch>`, present only on a `--perf-run` launch, and the
 * preload gives `HyperionApi` its `perf` member only then. Both sides of the hand-off use this
 * module; it imports nothing of Node or Electron, so it is safe in either bundle.
 */

import { isPerfSceneId } from "@hyperion/perf";

import type { PerfLaunch } from "./api";

export type { PerfLaunch } from "./api";

/** The switch carrying the launch. */
const PERF_SWITCH = "--hyperion-perf-run=";

/** The extra renderer argument that carries `launch`, for `webPreferences.additionalArguments`. */
export function perfSwitch(launch: PerfLaunch): string {
  return `${PERF_SWITCH}${encodeURIComponent(JSON.stringify(launch))}`;
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * `value` as a perf launch, or `null` if it is not one: a scene of the catalogue, a setting, the
 * record's file or `null`, a whole number of repetitions from 1, and the two switches.
 */
export function readPerfLaunch(value: unknown): PerfLaunch | null {
  if (!isRecord(value)) {
    return null;
  }
  const { scene, setting, out, repetitions, control, force } = value;
  if (
    typeof scene !== "string" ||
    !isPerfSceneId(scene) ||
    (setting !== "high" && setting !== "low") ||
    !(out === null || (typeof out === "string" && out.length > 0)) ||
    typeof repetitions !== "number" ||
    !Number.isInteger(repetitions) ||
    repetitions < 1 ||
    typeof control !== "boolean" ||
    typeof force !== "boolean"
  ) {
    return null;
  }
  return { scene, setting, out, repetitions, control, force };
}

/**
 * The perf launch the renderer's `argv` carries, or `null` on any other launch.
 *
 * @throws Error if the switch is there but does not hold a launch.
 */
export function perfLaunchFromArgv(argv: readonly string[]): PerfLaunch | null {
  const carrier = argv.find((argument) => argument.startsWith(PERF_SWITCH));
  if (carrier === undefined) {
    return null;
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(decodeURIComponent(carrier.slice(PERF_SWITCH.length)));
  } catch (error: unknown) {
    throw new Error(`the renderer's ${PERF_SWITCH} is not JSON`, { cause: error });
  }
  const launch = readPerfLaunch(parsed);
  if (launch === null) {
    throw new Error(`the renderer's ${PERF_SWITCH} does not hold a perf run's launch`);
  }
  return launch;
}
