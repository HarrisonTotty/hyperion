/**
 * How the main process hands the descent spike's options (plan R05, T13.c) to the preload.
 *
 * @remarks
 * As `serverUrl.ts` does for the server's URL: the options ride in the renderer's own `argv` as one
 * extra switch, `--hyperion-descent-spike=<options>`, present only when the client was launched
 * with `--descent-spike`, and the preload gives `HyperionApi` its `spike` member only then. Both
 * sides of the hand-off use this module; it imports types only, so it is safe in either bundle.
 */

import type { SpikeLaunch } from "./api";

export type { SpikeLaunch } from "./api";

/** The spike's seed when `--seed` is not given (T13.a's record seed). */
export const DEFAULT_SPIKE_SEED = "7";

/** The switch carrying the options. */
const SPIKE_SWITCH = "--hyperion-descent-spike=";

/** The extra renderer argument that carries `launch`, for `webPreferences.additionalArguments`. */
export function spikeSwitch(launch: SpikeLaunch): string {
  return `${SPIKE_SWITCH}${encodeURIComponent(JSON.stringify(launch))}`;
}

/** The largest u64. */
const U64_MAX = (1n << 64n) - 1n;

/** Whether `value` is a u64 written in decimal without a sign or leading zeros. */
export function isU64Decimal(value: string): boolean {
  return /^(0|[1-9]\d{0,19})$/.test(value) && BigInt(value) <= U64_MAX;
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function oneOf<T extends string>(value: unknown, options: ReadonlyArray<T>): value is T {
  return typeof value === "string" && options.some((option) => option === value);
}

function stringOrNull(value: unknown): value is string | null {
  return value === null || (typeof value === "string" && value.length > 0);
}

/**
 * `value` as spike options, or `null` if it is not one: every field present and of its type, the
 * seed a u64 and the worker count a whole number from 1 to 64.
 */
export function readSpikeLaunch(value: unknown): SpikeLaunch | null {
  if (!isRecord(value)) {
    return null;
  }
  const { setting, seed, smoke, out, workers, vertexPath, normals, ridged, dawnSafety, capture } =
    value;
  if (
    !oneOf(setting, ["high", "low"] as const) ||
    typeof seed !== "string" ||
    !isU64Decimal(seed) ||
    typeof smoke !== "boolean" ||
    !stringOrNull(out) ||
    !(
      workers === null ||
      (Number.isInteger(workers) && Number(workers) >= 1 && Number(workers) <= 64)
    ) ||
    !(vertexPath === null || oneOf(vertexPath, ["baked-offsets", "face-differences"] as const)) ||
    !(normals === null || oneOf(normals, ["double", "mesh"] as const)) ||
    !oneOf(ridged, ["on", "off"] as const) ||
    !oneOf(dawnSafety, ["on", "off"] as const) ||
    !stringOrNull(capture)
  ) {
    return null;
  }
  return {
    setting,
    seed,
    smoke,
    out,
    workers: workers === null ? null : Number(workers),
    vertexPath,
    normals,
    ridged,
    dawnSafety,
    capture,
  };
}

/**
 * The spike's options carried by `argv`, the renderer's arguments, or `null` on an ordinary
 * launch.
 *
 * @throws Error if the switch is there but does not hold valid options, which means the window
 * was created with something other than {@link spikeSwitch}'s output.
 */
export function spikeLaunchFromArgv(argv: readonly string[]): SpikeLaunch | null {
  const carrier = argv.find((argument) => argument.startsWith(SPIKE_SWITCH));
  if (carrier === undefined) {
    return null;
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(decodeURIComponent(carrier.slice(SPIKE_SWITCH.length)));
  } catch (error: unknown) {
    throw new Error(`the renderer's ${SPIKE_SWITCH} is not JSON`, { cause: error });
  }
  const launch = readSpikeLaunch(parsed);
  if (launch === null) {
    throw new Error(`the renderer's ${SPIKE_SWITCH} does not hold the spike's options`);
  }
  return launch;
}
