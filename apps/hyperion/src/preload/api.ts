/**
 * How this launch runs the GPU: Chromium's own path off Linux (no switches, never relaunched), the
 * forced Vulkan path on Linux, or the declared safe mode, which has no WebGPU.
 */
export type GraphicsLaunchMode = "default" | "vulkan" | "safe";

/** The API the preload script exposes to the renderer as `window.hyperion`. */
export interface HyperionApi {
  readonly platform: string;
  /**
   * WebSocket URL of the hyperion-server this client was launched to link to, from its
   * `--address` and `--port` options.
   */
  readonly serverUrl: string;
  readonly versions: {
    readonly electron: string;
    readonly chrome: string;
    readonly node: string;
  };
}

declare global {
  interface Window {
    readonly hyperion: HyperionApi;
  }
}
