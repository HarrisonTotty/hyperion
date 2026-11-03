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

/** How long a run of the page's checks may take before the watchdog ends it, ms: its hang detector. */
export const SMOKE_WATCHDOG_BASE_MS = 180_000;

/**
 * What part 1's watchdog gains when the captures are asked for, ms: twice the capture groups'
 * measured total, rounded up to the minute (the 2026-10-22 infra ruling, item 3). The captures
 * are part 1's, beside its checks, and alone they overran its 180 s.
 *
 * @remarks
 * Measured 2026-10-22 on the development machine (SwiftShader, both variants side by side as
 * `just test-render` runs them, under the heavy-test and GPU locks, in a quiet window at a 1-minute
 * load of 0.3 at the start), default / no-subgroups: the comparison 131.5 / 131.9 s, R08.T8.b's
 * ascent 143.4 / 140.0 s, the terrain 3.0 / 3.5 s, the spike 80.3 / 81.7 s, the occultation
 * 23.5 / 23.2 s and the eclipse 28.8 / 28.7 s: 410.5 / 409.0 s in all, and part 1 489.6 / 489.2 s.
 * Twice 410.5 s is 821 s, 14 minutes rounded up. The ruling named five groups, before R08.T8.b
 * added the ascent (267.1 / 269.0 s without it), which set 840 s. Measured again 2026-10-22 in
 * quiet window 15 (load 0.12 at the start), on R08.T8.c's kernels with its far limb's captures and
 * R10.T14.e's spike settle: the comparison 179.6 / 176.9 s, the ascent 160.5 / 162.2 s, the
 * terrain 2.6 / 4.8 s, the spike 211.9 / 212.6 s, the occultation 21.8 / 21.6 s, the eclipse
 * 28.1 / 28.2 s and the far limb 62.7 / 64.6 s: 667.2 / 670.9 s in all, and part 1 757.9 /
 * 756.1 s. Twice 670.9 s is 1,341.8 s, 23 minutes rounded up. fix-view-light's two groups, measured
 * since on a lane's pod (SwiftShader, the default variant alone, its watchdog scale 2), the view's
 * light with the Sun in frame 53.4 s and the Sun's glare through the air 192.8 s (2026-11-02), add
 * twice their 246.2 s: 1,834.2 s, 31 minutes rounded up, until the development machine measures
 * them.
 *
 * `scripts/testRender.sh` adds the same allowance to its outer limit, as `captures_allowance_s`
 * (each times the same scale, {@link SmokeWatchdogInputs.scale}), so that the outer limit stays
 * 120 s over the watchdog; `result.test.ts` holds the two equal.
 */
export const CAPTURES_WATCHDOG_MS = 1_860_000;

/**
 * What part 1's watchdog gains when the run lifts timestamp quantization (`--smoke-gpu-timing=1`,
 * the hardware runs of R01.T11), ms: the timing groups' allowance, R08.T8.c's far limb's timings
 * and R08.T11's photorealistic frame's timings (`frameTiming.ts`), which run only then and are
 * part 1's, beside its checks.
 *
 * @remarks
 * Three times the two groups' measured time, rounded up to the minute. Measured 2026-10-26 on the
 * RTX 3080 in an untimed, hidden run of part 1 on a shared machine (a 1-minute load of 12.4 at the
 * start): R08.T8.c's timings 2.4 s and R08.T11's group 145.8 s, 148.2 s in all, and part 1's
 * groups 172.5 s. Three times 148.2 s is 444.6 s, 8 minutes rounded up. No `just test-render` run
 * lifts quantization (SwiftShader has no `timestamp-query`), so `scripts/testRender.sh`'s outer
 * limit takes none of it.
 */
export const GPU_TIMING_WATCHDOG_MS = 480_000;

/**
 * What part 1's watchdog gains besides {@link GPU_TIMING_WATCHDOG_MS} when a timing run also forces
 * the thick read (`--smoke-thick-read=1`), ms: the allowance of R08.T11's rows only that flag runs
 * (`frameTiming.ts`), R08.T14.c's (K) and (L), R08.T14.f's (M) and (N), R08.T15.a.2's (Q) and the
 * routes' (R) to (U), which RM4's one QW takes (research-rm4-rm5-close-out §3).
 *
 * @remarks
 * Three times their time, rounded up to the minute, as {@link GPU_TIMING_WATCHDOG_MS} is sized.
 * Measured on the RTX 3080 in QW32 (2026-10-27): (K)'s bake, (K) and (L) 39 s. The rest estimated
 * from QW32's time a combination, 0.73 s for (K)'s and 1.2 s for (L)'s: (M) and (N) as (K) and (L),
 * 37 s; (Q) as (K), 29 s, and its atmosphere alone at (B)'s 0.58 s a combination, 6 s; (R) to (U),
 * 54 combinations at (L)'s pace, 65 s, and their media 45 s (`titan-class`'s haze optics some 20 s
 * and the Venus-class cases' Mie some 60 s on a pod's thread). 221 s in all, 663 s three times, 12
 * minutes rounded up. Without it the QW's run of part 1 would stand at some 550 s of its 660 s (an
 * estimate: part 1's groups 180 s and the plain timing group about 150 s besides), past two thirds
 * of it. No `just test-render` run lifts quantization, so `scripts/testRender.sh`'s outer limit
 * takes none of it.
 */
export const THICK_READ_WATCHDOG_MS = 720_000;

/**
 * The largest whole factor `--smoke-watchdog-scale` may stretch a run's watchdog by
 * (`scripts/testRender.sh`'s `HYPERION_SMOKE_WATCHDOG_SCALE`).
 *
 * @remarks
 * For a machine much slower than the development one. Measured 2026-10-29 on a cloud machine
 * (an AMD EPYC 7571, Zen 1 at 2.5 GHz, 4 cores with SMT, no GPU): with both variants side by
 * side, as `just test-render` runs them, 8 of the 13 parts outlasted 180 s at a 1-minute load of
 * 9 to 17, with no check failing; at a scale of 2 the longest, part 7, took 255 s of its 360 s. A
 * scale is a hang detector's margin, not a budget: no check reads it. Four is a cap, so that a
 * hung part of `just test-render` still ends within 12 minutes (part 1 with the captures within
 * 104).
 */
export const SMOKE_WATCHDOG_SCALE_MAX = 4;

/**
 * What the transfer check's run gains (`--smoke-transfer`; plan R10, R10.T15.b), ms: the check's
 * waits for the server's welcome and the field's answer (`smoke/transfer.ts`' `TRANSFER_TIMING`,
 * 60 s and 540 s, which a level-8 coarse pass of about a minute natively, R09.T16.a, may take on a
 * slower machine), and a minute's margin.
 */
export const TRANSFER_WATCHDOG_MS = 660_000;

/** What sets a run's watchdog: the main process's switches. */
export interface SmokeWatchdogInputs {
  /** `--smoke-part`'s value: `1` to `17`, or `all`. */
  readonly part: string;
  /** Whether `--smoke-captures` asks for the capture frames. */
  readonly captures: boolean;
  /** Whether `--smoke-gpu-timing=1` lifts timestamp quantization, which part 1's timings take. */
  readonly gpuTiming: boolean;
  /** Whether `--smoke-thick-read=1` forces the thick read, whose rows part 1's timings add. */
  readonly thickRead: boolean;
  /** `--smoke-soak`'s seconds, 0 for none. */
  readonly soakSeconds: number;
  /** `--smoke-child`'s seconds, 0 for none. */
  readonly childSeconds: number;
  /**
   * `--smoke-watchdog-scale`'s whole factor, 1 to {@link SMOKE_WATCHDOG_SCALE_MAX} (1 without it),
   * by which the allowances for the page's work are stretched.
   */
  readonly scale: number;
  /** Whether `--smoke-transfer` runs the transfer check in place of the checks. */
  readonly transfer: boolean;
}

/**
 * A run's watchdog, ms: {@link SMOKE_WATCHDOG_BASE_MS}, plus {@link CAPTURES_WATCHDOG_MS} where the
 * captures are asked for and {@link GPU_TIMING_WATCHDOG_MS} where timestamp quantization is
 * lifted, with {@link THICK_READ_WATCHDOG_MS} where the thick read is forced too, each where the
 * run takes part 1, whose groups they are (part 1 or all of them), each of these stretched by the
 * scale; plus the soak's and the child window's seconds, which are the clock's and
 * are not scaled; plus {@link TRANSFER_WATCHDOG_MS} for the transfer check, whose waits are its own
 * and are not scaled either.
 */
export function smokeWatchdogMs(inputs: SmokeWatchdogInputs): number {
  const { part, captures, gpuTiming, thickRead, soakSeconds, childSeconds, scale, transfer } =
    inputs;
  const partOne = part === "1" || part === "all";
  const capturesMs = captures && partOne ? CAPTURES_WATCHDOG_MS : 0;
  const thickMs = thickRead ? THICK_READ_WATCHDOG_MS : 0;
  const timingMs = gpuTiming && partOne ? GPU_TIMING_WATCHDOG_MS + thickMs : 0;
  const transferMs = transfer ? TRANSFER_WATCHDOG_MS : 0;
  return (
    (SMOKE_WATCHDOG_BASE_MS + capturesMs + timingMs) * scale +
    (soakSeconds + childSeconds) * 1000 +
    transferMs
  );
}

/**
 * `--smoke-watchdog-scale`'s value as a factor, or `null` unless it is a whole number from 1 to
 * {@link SMOKE_WATCHDOG_SCALE_MAX} in plain digits (`01`, `+2` and `2.0` are refused).
 */
export function readWatchdogScale(text: string): number | null {
  const scale = Number(text);
  return /^[1-9]\d*$/u.test(text) && scale <= SMOKE_WATCHDOG_SCALE_MAX ? scale : null;
}

/** Whether a URL may load in the harness: the built page and inline data only. */
export function isOfflineUrl(url: string): boolean {
  return url.startsWith("file:") || url.startsWith("data:");
}

/** The loopback hosts the transfer check's server may be on, as a URL's `hostname` reads them. */
const LOOPBACK_HOSTS: ReadonlySet<string> = new Set(["127.0.0.1", "localhost", "[::1]"]);

/**
 * The transfer check's server (`--smoke-transfer`; plan R10, R10.T15.b), the one request beside
 * {@link isOfflineUrl}'s that the harness lets out: a `ws:` URL of a loopback host, with a port,
 * at the server's `/ws`, as the WHATWG parser writes it; `null` for anything else, so that the
 * check reaches this machine's server alone and the offline rule holds for every other request.
 */
export function transferSocketUrl(value: string): string | null {
  const url = URL.canParse(value) ? new URL(value) : null;
  if (
    url === null ||
    url.protocol !== "ws:" ||
    !LOOPBACK_HOSTS.has(url.hostname) ||
    url.port === "" ||
    url.pathname !== "/ws" ||
    url.search !== "" ||
    url.hash !== "" ||
    url.username !== "" ||
    url.password !== ""
  ) {
    return null;
  }
  return url.href;
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
