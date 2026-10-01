/**
 * The one file that touches Babylon's internals, each access pinned by the typecheck.
 *
 * @remarks
 * There is no public way to render a camera into an external canvas context, to reach the device
 * that Babylon made, or to reach a texture's `GPUTexture` (R01 Design notes 13 and 18). The members
 * read here are declared `@internal` in `@babylonjs/core` 9.28.0's `.d.ts`, so an upgrade that
 * renames one fails `pnpm typecheck`; one that keeps the name and changes the meaning is caught by
 * the smoke harness's orientation check and cube round trip (R01.T9). `.oxlintrc.json` bans
 * dangling underscores, so each access carries its own one-line exception.
 */

import type { WebGPUEngine } from "@babylonjs/core/Engines/webgpuEngine.pure";
import { WebGPUHardwareTexture } from "@babylonjs/core/Engines/WebGPU/webgpuHardwareTexture";
import type { WebGPURenderTargetWrapper } from "@babylonjs/core/Engines/WebGPU/webgpuRenderTargetWrapper";
import type { InternalTexture } from "@babylonjs/core/Materials/Textures/internalTexture";

/**
 * The device the engine requested in `initAsync`.
 *
 * @remarks
 * `WebGPUEngine._device`, `webgpuEngine.pure.d.ts:216`: each view configures its own canvas
 * context against it, and the adapter's own passes, copies and readbacks encode on it.
 */
export function engineDevice(engine: WebGPUEngine): GPUDevice {
  // The device is internal to Babylon and has no public accessor (R01 Design note 13).
  // oxlint-disable-next-line no-underscore-dangle
  return engine._device;
}

/**
 * Renders into `wrapper` as into the canvas: no Y flip, the main framebuffer's winding.
 *
 * @remarks
 * `WebGPURenderTargetWrapper._disableEngineYFlip`, `webgpuRenderTargetWrapper.d.ts:20`: every
 * view and offscreen target renders in WebGPU's own orientation, so that R02's projection is used
 * as given (R01 Design note 18).
 */
export function disableEngineYFlip(wrapper: WebGPURenderTargetWrapper): void {
  // The flag lives on the wrapper and is internal; Babylon sets it only for XR layers (Design note 13).
  // oxlint-disable-next-line no-underscore-dangle
  wrapper._disableEngineYFlip = true;
}

/**
 * The `GPUTexture` behind a Babylon texture, or `null` before it exists.
 *
 * @remarks
 * `InternalTexture._hardwareTexture`, `internalTexture.d.ts:251`, whose `underlyingResource` is
 * the texture: the packed cube's level writes, the adapter's raw passes and every readback reach
 * the texture through it (R01 Design notes 18 and 19).
 */
export function gpuTextureOf(texture: InternalTexture): GPUTexture | null {
  // The hardware wrapper is internal to Babylon (Design note 18).
  // oxlint-disable-next-line no-underscore-dangle
  const hardware = texture._hardwareTexture;
  if (hardware === null) {
    return null;
  }
  const resource: unknown = hardware.underlyingResource;
  return isGpuTexture(resource) ? resource : null;
}

/** Whether `value` is a `GPUTexture`, by the members a texture has and a buffer lacks. */
function isGpuTexture(value: unknown): value is GPUTexture {
  return (
    typeof value === "object" &&
    value !== null &&
    "createView" in value &&
    "format" in value &&
    "mipLevelCount" in value
  );
}

/**
 * Gives a wrapped canvas texture the format its render attachment views and pipelines use.
 *
 * @remarks
 * `wrapWebGPUTexture` reports the texture's own format on its hardware wrapper
 * (`webgpuEngine.pure.js:1938`), from which Babylon builds the attachment view and the pipeline's
 * colour target. Setting the canvas's `-srgb` view format there makes Babylon render through the
 * sRGB view, so that blending happens in linear light and the store encodes (R01 Design note 18).
 * The canvas is configured with that view format among its `viewFormats`.
 */
export function setAttachmentFormat(texture: InternalTexture, format: GPUTextureFormat): void {
  // The hardware wrapper is internal to Babylon (Design note 18).
  // oxlint-disable-next-line no-underscore-dangle
  const hardware = texture._hardwareTexture;
  if (!(hardware instanceof WebGPUHardwareTexture)) {
    throw new Error("a wrapped canvas texture has no hardware wrapper to give a view format");
  }
  hardware.format = format;
}

/**
 * Gives a wrapped texture the view Babylon samples it through.
 *
 * @remarks
 * `wrapWebGPUTexture` makes no view, and `updateWrappedWebGPUTexture`'s view has either one mip or
 * the whole chain; a texture made by the engine has the mips its specification names, so its view
 * is made here, through the hardware wrapper's `createView`.
 */
export function setSampledView(
  texture: InternalTexture,
  descriptor: GPUTextureViewDescriptor,
): void {
  // The hardware wrapper is internal to Babylon (Design note 18).
  // oxlint-disable-next-line no-underscore-dangle
  const hardware = texture._hardwareTexture;
  if (!(hardware instanceof WebGPUHardwareTexture)) {
    throw new Error("a wrapped texture has no hardware wrapper to give a view");
  }
  hardware.createView(descriptor);
}

/**
 * Submits what Babylon has encoded so far, so that a pass the adapter encodes itself runs after it.
 *
 * @remarks
 * `WebGPUEngine.flushFramebuffer`, declared `@internal` in `webgpuEngine.pure.d.ts:977`: Babylon
 * records a frame into its own encoders and submits them at `endFrame`, so a dispatch, copy or raw
 * pass submitted by the adapter in between would otherwise overtake the passes recorded before it.
 */
export function flushEngine(engine: WebGPUEngine): void {
  engine.flushFramebuffer();
}
