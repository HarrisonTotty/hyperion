import { contextBridge } from "electron";

import type { HyperionApi } from "./api";
import { serverUrlFromArgv } from "./serverUrl";

const api: HyperionApi = {
  platform: process.platform,
  serverUrl: serverUrlFromArgv(process.argv),
  versions: {
    electron: process.versions.electron,
    chrome: process.versions.chrome,
    node: process.versions.node,
  },
};

contextBridge.exposeInMainWorld("hyperion", api);
