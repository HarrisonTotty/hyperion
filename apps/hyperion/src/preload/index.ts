import { contextBridge, ipcRenderer } from "electron";

import type { GpuProcessGoneReport, HyperionApi } from "./api";
import { graphicsLaunchFromArgv, subscribeGpuProcessGone } from "./graphicsLaunch";
import { serverUrlFromArgv } from "./serverUrl";
import { spikeMember } from "./spikeApi";

const graphicsLaunch = graphicsLaunchFromArgv(process.argv, process.platform);

const api: HyperionApi = {
  platform: process.platform,
  serverUrl: serverUrlFromArgv(process.argv),
  graphics: {
    launchMode: graphicsLaunch.launchMode,
    gpuTiming: graphicsLaunch.gpuTiming,
    // One fixed channel; no channel name crosses the bridge.
    onGpuProcessGone(listener: (event: GpuProcessGoneReport) => void): () => void {
      return subscribeGpuProcessGone(ipcRenderer, listener);
    },
  },
  versions: {
    electron: process.versions.electron,
    chrome: process.versions.chrome,
    node: process.versions.node,
  },
  // Only a `--descent-spike` launch has the spike's functions (R05.T13.c).
  ...spikeMember(process.argv, {
    invoke: (channel, ...args) => ipcRenderer.invoke(channel, ...args),
    privateKib: async () => (await process.getProcessMemoryInfo()).private,
  }),
};

contextBridge.exposeInMainWorld("hyperion", api);
