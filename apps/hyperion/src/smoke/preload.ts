/**
 * The smoke page's calls: its report to the harness's main process (R01 Design note 17), and raw
 * calls on the client's still channels (plan R11, R11.T10.c), whose handlers part 28 saves and reads
 * back through; the page reads their answers back itself (`view/stills/stillAnswers.ts`).
 *
 * @remarks
 * It imports nothing the client's preload does but `electron`: a module both imported would be
 * bundled as a chunk of its own, which a sandboxed preload cannot `require`.
 */

import { contextBridge, ipcRenderer } from "electron";

import { SMOKE_RESULT_CHANNEL, SMOKE_STILLS_CHANNELS } from "./result";

/** The raw still calls, each answering what the main process gave. */
export interface SmokeStillCalls {
  save(png: Uint8Array, label: unknown): Promise<unknown>;
  list(): Promise<unknown>;
  read(id: string): Promise<unknown>;
}

/** The page's view of the bridge, `window.smoke`. */
export interface SmokeApi {
  report(result: unknown): Promise<void>;
  readonly stills: SmokeStillCalls;
}

const api: SmokeApi = {
  async report(result: unknown): Promise<void> {
    // The channel's name is fixed; none crosses the bridge.
    await ipcRenderer.invoke(SMOKE_RESULT_CHANNEL, result);
  },
  // Each channel's name is fixed; none crosses the bridge.
  stills: {
    save: (png, label) => ipcRenderer.invoke(SMOKE_STILLS_CHANNELS.save, png, label),
    list: () => ipcRenderer.invoke(SMOKE_STILLS_CHANNELS.list),
    read: (id) => ipcRenderer.invoke(SMOKE_STILLS_CHANNELS.read, id),
  },
};

contextBridge.exposeInMainWorld("smoke", api);
