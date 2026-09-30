import { contextBridge, ipcRenderer } from "electron";

import type { GpuProcessGoneReport, HyperionApi } from "./api";
import { graphicsLaunchFromArgv, subscribeGpuProcessGone } from "./graphicsLaunch";
import { serverUrlFromArgv } from "./serverUrl";

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
};

contextBridge.exposeInMainWorld("hyperion", api);
