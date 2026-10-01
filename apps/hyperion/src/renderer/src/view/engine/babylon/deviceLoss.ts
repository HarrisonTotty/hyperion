/**
 * Watching the engine's device for loss and for errors nothing captured.
 *
 * @remarks
 * Babylon's own restore after `device.lost` runs its rebuild unawaited, so it can run against the
 * lost device, and skips wrapped external textures, which every view is; the engine is made with
 * `doNotHandleContextLost: true` and the adapter handles loss itself (R01 Design note 9). A
 * GPU-process crash reaches the page as `device.lost` with reason `unknown` and no
 * `uncapturederror` (probe of 2026-09-29), so the loss is the signal; an uncaptured error is
 * reported for diagnosis only.
 */

import type { GraphicsFault } from "../status";

/**
 * Reports `device`'s loss once, unless the engine disposed it first.
 *
 * @param isDisposed - Whether the engine has disposed itself, which destroys the device and
 * resolves `lost` with reason `destroyed`; that loss is the engine's own and is not reported.
 * @returns The watch's end: a loss after it is not reported.
 */
export function watchDeviceLoss(
  device: Pick<GPUDevice, "lost">,
  isDisposed: () => boolean,
  report: (fault: GraphicsFault & { readonly kind: "device-lost" }) => void,
): () => void {
  let watching = true;
  void device.lost
    .then((info): void => {
      if (watching && !isDisposed()) {
        report({ kind: "device-lost", reason: info.reason, message: info.message });
      }
      return undefined;
    })
    .catch((error: unknown) => {
      console.error("watching the device for loss failed:", error);
    });
  return () => {
    watching = false;
  };
}

/**
 * Logs every validation, out-of-memory or internal error the device raises and nothing captured.
 *
 * @returns The listener's removal.
 */
export function logUncapturedErrors(device: EventTarget): () => void {
  device.addEventListener("uncapturederror", logUncapturedError);
  return () => {
    device.removeEventListener("uncapturederror", logUncapturedError);
  };
}

function logUncapturedError(event: Event): void {
  const error: unknown = "error" in event ? event.error : undefined;
  const message =
    typeof error === "object" && error !== null && "message" in error
      ? String(error.message)
      : "an error with no message";
  console.error(`the GPU device raised an uncaptured error: ${message}`);
}
