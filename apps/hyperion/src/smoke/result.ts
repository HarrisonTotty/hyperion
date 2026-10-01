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

/** The run's printed lines and its exit code. */
export function judgeSmokeRun(
  result: SmokeResult,
  cancelled: ReadonlyArray<CancelledRequest>,
): { readonly lines: ReadonlyArray<string>; readonly exitCode: number } {
  const lines = [
    `variant ${result.variant}`,
    `adapter ${result.adapter ?? "none"}`,
    `capabilities ${result.capabilities ?? "none"}`,
    ...result.checks.map(
      (check) => `${check.pass ? "PASS" : "FAIL"} ${check.name}: ${check.detail}`,
    ),
    `cancelled requests [${cancelled.map((request) => `${request.resourceType} ${request.url}`).join(", ")}]`,
  ];
  if (result.setupError !== null) {
    return { lines: [...lines, `SETUP ${result.setupError}`], exitCode: SMOKE_EXIT.setup };
  }
  const failed = result.checks.some((check) => !check.pass) || cancelled.length > 0;
  return { lines, exitCode: failed ? SMOKE_EXIT.failed : SMOKE_EXIT.pass };
}
