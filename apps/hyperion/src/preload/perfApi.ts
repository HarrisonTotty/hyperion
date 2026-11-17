/**
 * The preload's perf-mode functions (plan R12, R12.T4.b; Design note 14): one narrow function an
 * operation, each a call on one fixed channel, which the main process checks
 * (`main/perf/handlers.ts`).
 *
 * @remarks
 * The channel names are repeated from `main/perf/handlers.ts`'s `PERF_CHANNELS`, since the preload
 * is bundled apart from the main process; `perfApi.test.ts` holds them equal. The answers are
 * narrowed here, at the bridge: the main process's are trusted no further than their shape.
 */

import type {
  PerfApi,
  PerfArrival,
  PerfConfig,
  PerfRepetitionStart,
  PerfWarmUpVerdict,
} from "./api";
import { perfLaunchFromArgv, readPerfLaunch } from "./perfLaunch";

/** Perf mode's channels (`main/perf/handlers.ts`'s `PERF_CHANNELS`). */
export const PERF_CHANNEL_NAMES = {
  config: "hyperion:perf:config",
  describe: "hyperion:perf:describe",
  warmUpSample: "hyperion:perf:warm-up-sample",
  beginRepetition: "hyperion:perf:begin-repetition",
  endRepetition: "hyperion:perf:end-repetition",
  finish: "hyperion:perf:finish",
} as const;

/** What the functions need of the preload's world: `ipcRenderer.invoke`. */
export interface PerfApiDeps {
  readonly invoke: (channel: string, ...args: unknown[]) => Promise<unknown>;
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isStrings(value: unknown): value is ReadonlyArray<string> {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

/** An arrival, or `null` when it is not one. */
export function readArrival(value: unknown): PerfArrival | null {
  if (!isRecord(value)) {
    return null;
  }
  const { universe, system, body } = value;
  return typeof universe === "string" &&
    /^[0-9a-f]{16}$/.test(universe) &&
    typeof system === "string" &&
    (body === null || typeof body === "string")
    ? { universe, system, body }
    : null;
}

/** The main process's `config` answer, or `null` when it is not one. */
export function readPerfConfig(value: unknown): PerfConfig | null {
  if (!isRecord(value)) {
    return null;
  }
  const { launch, switches, gpuTiming, launchMode, hidden, serverUrl } = value;
  const read = readPerfLaunch(launch);
  const arrival = readArrival(value["arrival"]);
  if (
    read === null ||
    arrival === null ||
    !isStrings(switches) ||
    typeof gpuTiming !== "boolean" ||
    (launchMode !== "default" &&
      launchMode !== "vulkan" &&
      launchMode !== "safe" &&
      launchMode !== "software") ||
    typeof hidden !== "boolean" ||
    typeof serverUrl !== "string"
  ) {
    return null;
  }
  return { launch: read, switches, gpuTiming, launchMode, hidden, serverUrl, arrival };
}

/** The main process's warm-up verdict, or `null` when it is not one. */
export function readWarmUpVerdict(value: unknown): PerfWarmUpVerdict | null {
  if (!isRecord(value)) {
    return null;
  }
  const { kind, elapsedS } = value;
  return (kind === "warming" || kind === "steady" || kind === "not-steady") &&
    typeof elapsedS === "number" &&
    Number.isFinite(elapsedS)
    ? { kind, elapsedS }
    : null;
}

/** The main process's answer to a repetition's start, or `null` when it is not one. */
export function readRepetitionStart(value: unknown): PerfRepetitionStart | null {
  if (!isRecord(value)) {
    return null;
  }
  const { serverUrl } = value;
  const arrival = readArrival(value["arrival"]);
  return typeof serverUrl === "string" && arrival !== null ? { serverUrl, arrival } : null;
}

/** `answer`, or a throw naming the call whose answer was not of its shape. */
function shaped<T>(answer: T | null, call: string): T {
  if (answer === null) {
    throw new Error(`the main process's answer to ${call} is not of its shape`);
  }
  return answer;
}

/** Perf mode's functions over `deps`. */
export function perfApi(deps: PerfApiDeps): PerfApi {
  const { invoke } = deps;
  return {
    config: async () => shaped(readPerfConfig(await invoke(PERF_CHANNEL_NAMES.config)), "config"),
    describe: async (description) => {
      await invoke(PERF_CHANNEL_NAMES.describe, description);
    },
    warmUpSample: async (sample) =>
      shaped(
        readWarmUpVerdict(await invoke(PERF_CHANNEL_NAMES.warmUpSample, sample)),
        "warmUpSample",
      ),
    beginRepetition: async (n) =>
      shaped(
        readRepetitionStart(await invoke(PERF_CHANNEL_NAMES.beginRepetition, n)),
        "beginRepetition",
      ),
    endRepetition: async (n, result) => {
      await invoke(PERF_CHANNEL_NAMES.endRepetition, n, result);
    },
    finish: async (end) => {
      await invoke(PERF_CHANNEL_NAMES.finish, end);
    },
  };
}

/**
 * `HyperionApi`'s `perf` member for the renderer's `argv`: perf mode's functions on a
 * `--perf-run` launch, nothing on any other.
 *
 * @throws Error if `argv` carries a malformed perf switch (`perfLaunchFromArgv`).
 */
export function perfMember(
  argv: readonly string[],
  deps: PerfApiDeps,
): { readonly perf?: PerfApi } {
  return perfLaunchFromArgv(argv) === null ? {} : { perf: perfApi(deps) };
}
