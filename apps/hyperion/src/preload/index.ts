import { contextBridge } from "electron";

import type { HyperionApi } from "./api";

const api: HyperionApi = {
  platform: process.platform,
  versions: {
    electron: process.versions.electron,
    chrome: process.versions.chrome,
    node: process.versions.node,
  },
};

contextBridge.exposeInMainWorld("hyperion", api);
