/**
 * How this launch runs the GPU: Chromium's own path off Linux (no switches, never relaunched), the
 * forced Vulkan path on Linux, or the declared safe mode, which has no WebGPU.
 */
export type GraphicsLaunchMode = "default" | "vulkan" | "safe";

/** A GPU-process crash, as the main process reports it to the renderer. */
export interface GpuProcessGoneReport {
  /** Electron's reason for the exit, such as `crashed` or `killed`. */
  readonly reason: string;
  /** GPU-process crashes in this launch, this one included. */
  readonly count: number;
}

/** What the renderer learns of the launch's GPU set-up from the main process. */
export interface GraphicsApi {
  readonly launchMode: GraphicsLaunchMode;
  /** True when `--hyperion-gpu-timing` lifted timestamp quantization for this launch. */
  readonly gpuTiming: boolean;
  /**
   * Registers a listener for GPU-process crashes.
   *
   * @returns The listener's removal.
   */
  onGpuProcessGone(listener: (event: GpuProcessGoneReport) => void): () => void;
}

/** The API the preload script exposes to the renderer as `window.hyperion`. */
export interface HyperionApi {
  readonly platform: string;
  /**
   * WebSocket URL of the hyperion-server this client was launched to link to, from its
   * `--address` and `--port` options.
   */
  readonly serverUrl: string;
  /** The launch's graphics mode and the GPU process's crashes. */
  readonly graphics: GraphicsApi;
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
