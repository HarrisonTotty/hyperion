/**
 * The preload's descent-spike functions (plan R05, T13.c): one narrow function an operation, each
 * a call on one fixed channel, which the main process checks (`main/spike.ts`).
 *
 * @remarks
 * The channel names are repeated from `main/spike.ts`'s `SPIKE_CHANNELS`, since the preload is
 * bundled apart from the main process; `spikeApi.test.ts` holds them equal.
 */

import type {
  DescentSpikeReport,
  SpikeApi,
  SpikeEnd,
  SpikeLaunch,
  SpikeResultsAnswer,
  SpikeResultsPaths,
} from "./api";
import { spikeLaunchFromArgv } from "./spikeLaunch";

/** The spike's channels (`main/spike.ts`'s `SPIKE_CHANNELS`). */
export const SPIKE_CHANNEL_NAMES = {
  startTrace: "hyperion:spike:start-trace",
  cycleTrace: "hyperion:spike:cycle-trace",
  stopTrace: "hyperion:spike:stop-trace",
  memory: "hyperion:spike:memory",
  writeResults: "hyperion:spike:write-results",
  writeCapture: "hyperion:spike:write-capture",
  end: "hyperion:spike:end",
} as const;

/** What the functions need of the preload's world: `ipcRenderer.invoke` and the memory reading. */
export interface SpikeApiDeps {
  readonly invoke: (channel: string, ...args: unknown[]) => Promise<unknown>;
  /** The renderer's private memory, KiB (`process.getProcessMemoryInfo().private`). */
  readonly privateKib: () => Promise<number>;
}

function isPaths(value: unknown): value is SpikeResultsPaths {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof Reflect.get(value, "json") === "string" &&
    typeof Reflect.get(value, "markdown") === "string"
  );
}

/**
 * The main process's answer to a results call, or `null` when it is not the answer `launch`
 * expects: a smoke's check, or a full run's file.
 */
function readResultsAnswer(value: unknown, launch: SpikeLaunch): SpikeResultsAnswer | null {
  if (typeof value !== "object" || value === null) {
    return null;
  }
  const kind: unknown = Reflect.get(value, "kind");
  if (launch.smoke) {
    const failure: unknown = Reflect.get(value, "failure");
    return kind === "smoke checked" && (failure === null || typeof failure === "string")
      ? { kind, failure }
      : null;
  }
  const paths: unknown = Reflect.get(value, "paths");
  return kind === "written" && isPaths(paths) ? { kind, paths } : null;
}

/** The spike's functions for `launch`. */
export function spikeApi(launch: SpikeLaunch, deps: SpikeApiDeps): SpikeApi {
  const { invoke } = deps;
  return {
    launch,
    startTrace: async () => {
      await invoke(SPIKE_CHANNEL_NAMES.startTrace);
    },
    cycleTrace: async () => {
      await invoke(SPIKE_CHANNEL_NAMES.cycleTrace);
    },
    stopTrace: async () => {
      await invoke(SPIKE_CHANNEL_NAMES.stopTrace);
    },
    sampleMemory: async () => {
      await invoke(SPIKE_CHANNEL_NAMES.memory, Math.round((await deps.privateKib()) * 1024));
    },
    writeResults: async (report: DescentSpikeReport) => {
      const answer = readResultsAnswer(
        await invoke(SPIKE_CHANNEL_NAMES.writeResults, report),
        launch,
      );
      if (answer === null) {
        throw new Error(
          launch.smoke
            ? "the main process did not check the smoke's trace"
            : "the main process wrote no results file",
        );
      }
      return answer;
    },
    writeCapture: async (capture) => {
      const dir = await invoke(SPIKE_CHANNEL_NAMES.writeCapture, capture);
      if (typeof dir !== "string") {
        throw new Error("the main process wrote no capture");
      }
      return dir;
    },
    end: async (outcome: SpikeEnd) => {
      await invoke(SPIKE_CHANNEL_NAMES.end, outcome);
    },
  };
}

/**
 * `HyperionApi`'s `spike` member for the renderer's `argv`: the spike's functions on a
 * `--descent-spike` launch, nothing on an ordinary one.
 *
 * @throws Error if `argv` carries a malformed spike switch (`spikeLaunchFromArgv`).
 */
export function spikeMember(
  argv: readonly string[],
  deps: SpikeApiDeps,
): { readonly spike?: SpikeApi } {
  const launch = spikeLaunchFromArgv(argv);
  return launch === null ? {} : { spike: spikeApi(launch, deps) };
}
