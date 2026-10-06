/**
 * The preload's several-views check functions (plan R07, T20): one narrow function an operation,
 * each a call on one fixed channel, which the main process checks (`main/viewsCheck.ts`).
 *
 * @remarks
 * The channel names are repeated from `main/viewsCheck.ts`'s `VIEWS_CHECK_CHANNELS`, since the
 * preload is bundled apart from the main process; `viewsCheckApi.test.ts` holds them equal.
 */

import type {
  SpikeEnd,
  SpikeResultsPaths,
  ViewsCheckApi,
  ViewsCheckLaunch,
  ViewsCheckPhaseName,
  ViewsCheckRecord,
  ViewsCheckWindow,
} from "./api";
import { viewsCheckLaunchFromArgv } from "./viewsCheckLaunch";

/** The check's channels (`main/viewsCheck.ts`'s `VIEWS_CHECK_CHANNELS`). */
export const VIEWS_CHECK_CHANNEL_NAMES = {
  startPhase: "hyperion:views-check:start-phase",
  endPhase: "hyperion:views-check:end-phase",
  askRightWayUp: "hyperion:views-check:ask-right-way-up",
  writeResults: "hyperion:views-check:write-results",
  end: "hyperion:views-check:end",
} as const;

/** What the functions need of the preload's world: `ipcRenderer.invoke`. */
export interface ViewsCheckApiDeps {
  readonly invoke: (channel: string, ...args: unknown[]) => Promise<unknown>;
}

function isPaths(value: unknown): value is SpikeResultsPaths {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof Reflect.get(value, "json") === "string" &&
    typeof Reflect.get(value, "markdown") === "string"
  );
}

/** The check's functions for `launch`. */
export function viewsCheckApi(launch: ViewsCheckLaunch, deps: ViewsCheckApiDeps): ViewsCheckApi {
  const { invoke } = deps;
  return {
    launch,
    startPhase: async (name: ViewsCheckPhaseName) => {
      await invoke(VIEWS_CHECK_CHANNEL_NAMES.startPhase, name);
    },
    endPhase: async (name: ViewsCheckPhaseName, window: ViewsCheckWindow) => {
      await invoke(VIEWS_CHECK_CHANNEL_NAMES.endPhase, name, window);
    },
    askRightWayUp: async () => {
      await invoke(VIEWS_CHECK_CHANNEL_NAMES.askRightWayUp);
    },
    writeResults: async (record: ViewsCheckRecord) => {
      const paths = await invoke(VIEWS_CHECK_CHANNEL_NAMES.writeResults, record);
      if (!isPaths(paths)) {
        throw new Error("the main process wrote no results file");
      }
      return paths;
    },
    end: async (outcome: SpikeEnd) => {
      await invoke(VIEWS_CHECK_CHANNEL_NAMES.end, outcome);
    },
  };
}

/**
 * `HyperionApi`'s `viewsCheck` member for the renderer's `argv`: the check's functions on a
 * `--views-check` launch, nothing on any other.
 *
 * @throws Error if `argv` carries a malformed check switch (`viewsCheckLaunchFromArgv`).
 */
export function viewsCheckMember(
  argv: readonly string[],
  deps: ViewsCheckApiDeps,
): { readonly viewsCheck?: ViewsCheckApi } {
  const launch = viewsCheckLaunchFromArgv(argv);
  return launch === null ? {} : { viewsCheck: viewsCheckApi(launch, deps) };
}
