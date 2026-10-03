/**
 * What the smoke page reports to the harness's main process, and how the main process turns it
 * into lines and an exit code (R01 Design note 17).
 *
 * @remarks
 * Shared by the page (as types) and the main process, so it imports nothing.
 */

/** One asserted property. */
export interface SmokeCheck {
  readonly name: string;
  readonly pass: boolean;
  /** What was seen, for the log. */
  readonly detail: string;
}

/** A run of the page. */
export interface SmokeResult {
  readonly variant: string;
  /** `vendor/architecture, fallback` and the capability line, or `null` with no adapter. */
  readonly adapter: string | null;
  readonly capabilities: string | null;
  readonly checks: ReadonlyArray<SmokeCheck>;
  /** A setup failure, such as no adapter, which is exit 2 rather than a failed property. */
  readonly setupError: string | null;
}

/** A request the main process cancelled: anything but `file:` and `data:`. */
export interface CancelledRequest {
  readonly url: string;
  readonly resourceType: string;
}

/** The one IPC channel the page reports on, fixed on both sides of the bridge. */
export const SMOKE_RESULT_CHANNEL = "smoke:result";

/** The exit codes of Design note 17. */
export const SMOKE_EXIT = { pass: 0, failed: 1, setup: 2, watchdog: 3 } as const;

/** Whether a URL may load in the harness: the built page and inline data only. */
export function isOfflineUrl(url: string): boolean {
  return url.startsWith("file:") || url.startsWith("data:");
}

/** Whether `value` is a check, by its fields. */
function isCheck(value: unknown): value is SmokeCheck {
  return (
    typeof value === "object" &&
    value !== null &&
    "name" in value &&
    typeof value.name === "string" &&
    "pass" in value &&
    typeof value.pass === "boolean" &&
    "detail" in value &&
    typeof value.detail === "string"
  );
}

/** A string or `null` field of `value`, or `undefined` when it is neither. */
function nullableString(value: object, key: string): string | null | undefined {
  const field: unknown = Reflect.get(value, key);
  return typeof field === "string" || field === null ? field : undefined;
}

/** The page's report, validated, or `null` when it is not one. */
export function readSmokeResult(value: unknown): SmokeResult | null {
  if (typeof value !== "object" || value === null) {
    return null;
  }
  const variant: unknown = Reflect.get(value, "variant");
  const checks: unknown = Reflect.get(value, "checks");
  const adapter = nullableString(value, "adapter");
  const capabilities = nullableString(value, "capabilities");
  const setupError = nullableString(value, "setupError");
  if (
    typeof variant !== "string" ||
    !Array.isArray(checks) ||
    !checks.every(isCheck) ||
    adapter === undefined ||
    capabilities === undefined ||
    setupError === undefined
  ) {
    return null;
  }
  return { variant, adapter, capabilities, checks, setupError };
}

/**
 * An image the page hands the main process to save, for a look by a person (R05.T12.c's
 * atmosphere comparison): RGBA, 8 bits a channel, row by row from the top, base64.
 */
export interface SmokeImage {
  readonly name: string;
  readonly width: number;
  readonly height: number;
  readonly rgba: string;
}

/** The largest side of a saved image, px. */
export const SMOKE_IMAGE_MAX_SIDE = 4096;

/** Whether `side` is a whole number of pixels within (0, {@link SMOKE_IMAGE_MAX_SIDE}]. */
function isSide(side: unknown): side is number {
  return Number.isInteger(side) && Number(side) > 0 && Number(side) <= SMOKE_IMAGE_MAX_SIDE;
}

/**
 * Whether `value` is an image: a name of lower-case letters, digits and hyphens (so it cannot leave
 * the captures' directory), sides within bounds, and base64 that decodes to width × height × 4
 * bytes.
 */
function isImage(value: unknown): value is SmokeImage {
  if (
    typeof value !== "object" ||
    value === null ||
    !("name" in value) ||
    typeof value.name !== "string" ||
    !/^[a-z0-9-]+$/u.test(value.name) ||
    !("width" in value) ||
    !isSide(value.width) ||
    !("height" in value) ||
    !isSide(value.height) ||
    !("rgba" in value) ||
    typeof value.rgba !== "string" ||
    !/^[A-Za-z0-9+/]*={0,2}$/u.test(value.rgba)
  ) {
    return false;
  }
  const padding = value.rgba.endsWith("==") ? 2 : value.rgba.endsWith("=") ? 1 : 0;
  const bytes = (value.rgba.length / 4) * 3 - padding;
  return value.rgba.length % 4 === 0 && bytes === value.width * value.height * 4;
}

/** The report's images, those that are well formed, and how many were not. */
export function readSmokeImages(value: unknown): {
  readonly images: ReadonlyArray<SmokeImage>;
  readonly rejected: number;
} {
  if (typeof value !== "object" || value === null) {
    return { images: [], rejected: 0 };
  }
  const images: unknown = Reflect.get(value, "images");
  if (!Array.isArray(images)) {
    return { images: [], rejected: 0 };
  }
  const valid = images.filter(isImage);
  return { images: valid, rejected: images.length - valid.length };
}

/**
 * What the engine logs, on the page, for a GPU error nothing captured (`webgpu/deviceLoss.ts`'s
 * `logUncapturedErrors`), by which the main process counts them.
 */
export const UNCAPTURED_GPU_ERROR = "the GPU device raised an uncaptured error";

/** Whether a page's console message is the engine's log of an uncaptured GPU error. */
export function isUncapturedGpuError(message: string): boolean {
  return message.includes(UNCAPTURED_GPU_ERROR);
}

/**
 * The run's printed lines and its exit code.
 *
 * @param gpuErrors - The page's uncaptured GPU errors that count against the run: a validation
 * error in a pass no check reads back (a timestamp resolve, a mip pass, an unsampled
 * post-process) would otherwise pass unseen. The broken-WGSL fixture's own are left out by the
 * caller.
 */
export function judgeSmokeRun(
  result: SmokeResult,
  cancelled: ReadonlyArray<CancelledRequest>,
  gpuErrors: ReadonlyArray<string>,
): { readonly lines: ReadonlyArray<string>; readonly exitCode: number } {
  const lines = [
    `variant ${result.variant}`,
    `adapter ${result.adapter ?? "none"}`,
    `capabilities ${result.capabilities ?? "none"}`,
    ...result.checks.map(
      (check) => `${check.pass ? "PASS" : "FAIL"} ${check.name}: ${check.detail}`,
    ),
    `cancelled requests [${cancelled.map((request) => `${request.resourceType} ${request.url}`).join(", ")}]`,
    `uncaptured GPU errors ${gpuErrors.length}`,
    ...gpuErrors.map((message) => `GPU ERROR ${message}`),
  ];
  if (result.setupError !== null) {
    return { lines: [...lines, `SETUP ${result.setupError}`], exitCode: SMOKE_EXIT.setup };
  }
  if (result.checks.length === 0) {
    return { lines: [...lines, "FAIL the page ran no checks"], exitCode: SMOKE_EXIT.failed };
  }
  const failed =
    result.checks.some((check) => !check.pass) || cancelled.length > 0 || gpuErrors.length > 0;
  return { lines, exitCode: failed ? SMOKE_EXIT.failed : SMOKE_EXIT.pass };
}
