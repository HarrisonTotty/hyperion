/**
 * The smoke page's one call: its report to the harness's main process (R01 Design note 17).
 */

import { contextBridge, ipcRenderer } from "electron";

import { SMOKE_RESULT_CHANNEL } from "./result";

/** The page's view of the bridge, `window.smoke`. */
export interface SmokeApi {
  report(result: unknown): Promise<void>;
}

const api: SmokeApi = {
  async report(result: unknown): Promise<void> {
    // The channel's name is fixed; none crosses the bridge.
    await ipcRenderer.invoke(SMOKE_RESULT_CHANNEL, result);
  },
};

contextBridge.exposeInMainWorld("smoke", api);
