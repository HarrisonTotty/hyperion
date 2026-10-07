import { vi } from "vitest";

import type {
  GraphicsApi,
  HyperionApi,
  QualitySettingName,
  SpikeApi,
  SpikeLaunch,
  ViewsCheckApi,
} from "../../../preload/api";

/** The server the stubbed preload reports the client was launched to link to. */
export const TEST_SERVER_URL = "ws://127.0.0.1:7878/ws";

/** A Linux launch on the forced Vulkan path, with quantized timestamps and no crash ever reported. */
export const TEST_GRAPHICS: GraphicsApi = {
  launchMode: "vulkan",
  gpuTiming: false,
  onGpuProcessGone: () => () => undefined,
};

/**
 * A descent-spike launch as the command line gives it with `--setting low` and every other option
 * at its default: a full run, unprofiled, with no capture.
 */
export const TEST_SPIKE_LAUNCH: SpikeLaunch = {
  setting: "low",
  seed: "7",
  smoke: false,
  out: null,
  workers: null,
  vertexPath: null,
  normals: null,
  ridged: "off",
  dawnSafety: "on",
  capture: null,
  traceProfile: "off",
};

/**
 * Stubs `window.hyperion`, which the preload installs and jsdom has no preload to install.
 *
 * @param serverUrl - The server the client was launched to link to.
 * @param graphics - The launch's graphics set-up and its crash reports.
 * @param spike - The descent spike's functions, for a `--descent-spike` launch; none by default,
 *   as on an ordinary launch.
 * @param viewsCheck - The several-views check's functions, for a `--views-check` launch; none by
 *   default.
 * @param setting - The launch's quality setting (`--setting`): the spike's or the check's own,
 *   else `high`, as the main process gives it.
 */
export function stubHyperionApi(
  serverUrl: string = TEST_SERVER_URL,
  graphics: GraphicsApi = TEST_GRAPHICS,
  spike?: SpikeApi,
  viewsCheck?: ViewsCheckApi,
  setting: QualitySettingName = spike?.launch.setting ?? viewsCheck?.launch.setting ?? "high",
): void {
  const api: HyperionApi = {
    platform: "linux",
    serverUrl,
    graphics,
    setting,
    versions: { electron: "44.4.3", chrome: "142.0.0.0", node: "22.21.1" },
    ...(spike === undefined ? {} : { spike }),
    ...(viewsCheck === undefined ? {} : { viewsCheck }),
  };
  vi.stubGlobal("hyperion", api);
}
