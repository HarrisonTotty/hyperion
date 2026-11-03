import { contextBridge, ipcRenderer } from "electron";

import type { GpuProcessGoneReport, HyperionApi } from "./api";
import { compileBudgetFromArgv } from "./compileBudgetLaunch";
import { geometryFromArgv } from "./geometryLaunch";
import { graphicsLaunchFromArgv, subscribeGpuProcessGone } from "./graphicsLaunch";
import { performanceThreadsFromArgv } from "./machineLaunch";
import { qualityFromArgv } from "./qualityLaunch";
import { serverUrlFromArgv } from "./serverUrl";
import { spikeMember } from "./spikeApi";
import { viewsCheckMember } from "./viewsCheckApi";

const graphicsLaunch = graphicsLaunchFromArgv(process.argv, process.platform);
const geometry = geometryFromArgv(process.argv);
const compileBudget = compileBudgetFromArgv(process.argv);

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
  setting: qualityFromArgv(process.argv),
  performanceThreads: performanceThreadsFromArgv(process.argv),
  versions: {
    electron: process.versions.electron,
    chrome: process.versions.chrome,
    node: process.versions.node,
  },
  // Only a spike's or a check's launch that gave `--geometry` has an override (plan R14).
  ...(geometry === null ? {} : { geometry }),
  // Only a spike's or a check's launch that gave `--compile-budget` has one (plan R14).
  ...(compileBudget === null ? {} : { compileBudget }),
  // Only a `--descent-spike` launch has the spike's functions (R05.T13.c).
  ...spikeMember(process.argv, {
    invoke: (channel, ...args) => ipcRenderer.invoke(channel, ...args),
    privateKib: async () => (await process.getProcessMemoryInfo()).private,
  }),
  // Only a `--views-check` launch has the several-views check's functions (R07.T20).
  ...viewsCheckMember(process.argv, {
    invoke: (channel, ...args) => ipcRenderer.invoke(channel, ...args),
  }),
};

contextBridge.exposeInMainWorld("hyperion", api);
