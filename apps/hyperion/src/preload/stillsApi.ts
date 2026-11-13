/**
 * The preload's still functions (plan R11, R11.T10.c): one narrow function an operation, each a call
 * on one fixed channel, which the main process checks (`main/stills.ts`).
 *
 * @remarks
 * The channel names are repeated from `main/stills.ts`'s `STILLS_CHANNELS`, since the preload is
 * bundled apart from the main process; `stillsApi.test.ts` holds them equal. The main process's
 * answers are read back through the label's schema (`view/stills/stillAnswers.ts`) before the
 * renderer sees them. The smoke harness's preload imports nothing of this file's: a module both
 * preloads imported would be bundled as a chunk of its own, which a sandboxed preload cannot
 * `require`.
 */

import type { StillsApi } from "./api";
import { stillsOver } from "../renderer/src/view/stills/stillAnswers";

/** The stills' channels (`main/stills.ts`'s `STILLS_CHANNELS`). */
export const STILLS_CHANNEL_NAMES = {
  save: "hyperion:stills:save",
  list: "hyperion:stills:list",
  read: "hyperion:stills:read",
} as const;

/** What the functions need of the preload's world: `ipcRenderer.invoke`. */
export interface StillsApiDeps {
  readonly invoke: (channel: string, ...args: unknown[]) => Promise<unknown>;
}

/** The still functions over `deps`. */
export function stillsApi(deps: StillsApiDeps): StillsApi {
  const { invoke } = deps;
  return stillsOver({
    save: (png, label) => invoke(STILLS_CHANNEL_NAMES.save, png, label),
    list: () => invoke(STILLS_CHANNEL_NAMES.list),
    read: (id) => invoke(STILLS_CHANNEL_NAMES.read, id),
  });
}
