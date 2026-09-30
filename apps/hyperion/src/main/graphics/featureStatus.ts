/**
 * The two entries of Chromium's GPU feature status that the crash-loop policy watches.
 *
 * @remarks
 * Chromium 152 reports `vulkan` and `webgpu` in `app.getGPUFeatureStatus()`, but Electron 44.4.3's
 * typed `GPUFeatureStatus` declares neither (`electron.d.ts`, `GPUFeatureStatus`), so they are
 * narrowed out of an `unknown` rather than cast (R01 Design note 6). The status read at `ready` is
 * stale; it is read after each `gpu-info-update`.
 */

/** The feature status's `vulkan` and `webgpu` values, each `undefined` when absent or not text. */
export interface FeatureStatusReading {
  /** `enabled_on` while Chromium runs on Vulkan (the probe's value). */
  readonly vulkan: string | undefined;
  /** `enabled` while WebGPU is available (the probe's value). */
  readonly webgpu: string | undefined;
}

function stringEntry(record: object, key: string): string | undefined {
  if (!(key in record)) {
    return undefined;
  }
  const value: unknown = Reflect.get(record, key);
  return typeof value === "string" ? value : undefined;
}

/**
 * Reads the `vulkan` and `webgpu` entries of a feature status.
 *
 * @param status - `app.getGPUFeatureStatus()`, taken as `unknown` because its type lacks both keys.
 */
export function readFeatureStatus(status: unknown): FeatureStatusReading {
  if (typeof status !== "object" || status === null) {
    return { vulkan: undefined, webgpu: undefined };
  }
  return { vulkan: stringEntry(status, "vulkan"), webgpu: stringEntry(status, "webgpu") };
}
