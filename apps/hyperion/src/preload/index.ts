import { contextBridge, type IpcRendererEvent, ipcRenderer } from "electron";

import type { GpuProcessGoneReport, HyperionApi } from "./api";
import {
  GPU_PROCESS_GONE_CHANNEL,
  graphicsLaunchFromArgv,
  readGpuProcessGoneReport,
} from "./graphicsLaunch";
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
      const handler = (_event: IpcRendererEvent, message: unknown): void => {
        const report = readGpuProcessGoneReport(message);
        if (report !== undefined) {
          listener(report);
        }
      };
      ipcRenderer.on(GPU_PROCESS_GONE_CHANNEL, handler);
      return () => {
        ipcRenderer.removeListener(GPU_PROCESS_GONE_CHANNEL, handler);
      };
    },
  },
  versions: {
    electron: process.versions.electron,
    chrome: process.versions.chrome,
    node: process.versions.node,
  },
};

contextBridge.exposeInMainWorld("hyperion", api);
