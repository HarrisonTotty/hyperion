/**
 * Hands Babylon the adapter the client has already vetted.
 *
 * @remarks
 * `WebGPUEngine.initAsync` always calls `navigator.gpu.requestAdapter` itself and takes no adapter
 * (`webgpuEngine.pure.js:413-417` in 9.28.0), so for the one `initAsync` call of each creation the
 * entry point's `requestAdapter` answers with the vetted adapter, and is restored once the call
 * settles (R01 Design note 7). The wrapper touches a browser API, not an engine internal.
 */

/**
 * Runs `run` while `gpu.requestAdapter` answers with `adapter`, and restores it however `run`
 * settles.
 *
 * @param gpu - The entry point Babylon reads, `navigator.gpu`.
 * @param adapter - Vetted by `requestAdapterOutcome` for this creation alone: an adapter is consumed
 * by its first device.
 */
export async function withHandedAdapter<T>(
  gpu: GPU,
  adapter: GPUAdapter,
  run: () => Promise<T>,
): Promise<T> {
  const own = Object.getOwnPropertyDescriptor(gpu, "requestAdapter");
  const handed: GPU["requestAdapter"] = () => Promise.resolve(adapter);
  Object.defineProperty(gpu, "requestAdapter", {
    configurable: true,
    writable: true,
    value: handed,
  });
  try {
    return await run();
  } finally {
    if (own === undefined) {
      Reflect.deleteProperty(gpu, "requestAdapter");
    } else {
      Object.defineProperty(gpu, "requestAdapter", own);
    }
  }
}
