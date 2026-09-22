import { vi } from "vitest";

import type { HyperionApi } from "../../../preload/api";

/** The server the stubbed preload reports the client was launched to link to. */
export const TEST_SERVER_URL = "ws://127.0.0.1:7878/ws";

/**
 * Stubs `window.hyperion`, which the preload installs and jsdom has no preload to install.
 *
 * @param serverUrl - The server the client was launched to link to.
 */
export function stubHyperionApi(serverUrl: string = TEST_SERVER_URL): void {
  const api: HyperionApi = {
    platform: "linux",
    serverUrl,
    versions: { electron: "44.4.3", chrome: "142.0.0.0", node: "22.21.1" },
  };
  vi.stubGlobal("hyperion", api);
}
