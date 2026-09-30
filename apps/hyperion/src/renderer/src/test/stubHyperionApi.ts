import { vi } from "vitest";

import type { GraphicsApi, HyperionApi } from "../../../preload/api";

/** The server the stubbed preload reports the client was launched to link to. */
export const TEST_SERVER_URL = "ws://127.0.0.1:7878/ws";

/** A Linux launch on the forced Vulkan path, with quantized timestamps and no crash ever reported. */
export const TEST_GRAPHICS: GraphicsApi = {
  launchMode: "vulkan",
  gpuTiming: false,
  onGpuProcessGone: () => () => undefined,
};

/**
 * Stubs `window.hyperion`, which the preload installs and jsdom has no preload to install.
 *
 * @param serverUrl - The server the client was launched to link to.
 * @param graphics - The launch's graphics set-up and its crash reports.
 */
export function stubHyperionApi(
  serverUrl: string = TEST_SERVER_URL,
  graphics: GraphicsApi = TEST_GRAPHICS,
): void {
  const api: HyperionApi = {
    platform: "linux",
    serverUrl,
    graphics,
    versions: { electron: "44.4.3", chrome: "142.0.0.0", node: "22.21.1" },
  };
  vi.stubGlobal("hyperion", api);
}
