/**
 * How the main process hands the several-views check's options (plan R07, T20) to the preload.
 *
 * @remarks
 * As `spikeLaunch.ts` does for the descent spike's: the options ride in the renderer's own `argv`
 * as one extra switch, `--hyperion-views-check=<options>`, present only when the client was
 * launched with `--views-check`, and the preload gives `HyperionApi` its `viewsCheck` member only
 * then. Both sides of the hand-off use this module; it imports types only, so it is safe in either
 * bundle.
 */

import type { ViewsCheckLaunch } from "./api";

export type { ViewsCheckLaunch } from "./api";

/** The switch carrying the options. */
const VIEWS_CHECK_SWITCH = "--hyperion-views-check=";

/** The extra renderer argument that carries `launch`, for `webPreferences.additionalArguments`. */
export function viewsCheckSwitch(launch: ViewsCheckLaunch): string {
  return `${VIEWS_CHECK_SWITCH}${encodeURIComponent(JSON.stringify(launch))}`;
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** `value` as the check's options, or `null` if it is not one: every field present and of its type. */
export function readViewsCheckLaunch(value: unknown): ViewsCheckLaunch | null {
  if (!isRecord(value)) {
    return null;
  }
  const { setting, smoke, out } = value;
  if (
    (setting !== "high" && setting !== "low") ||
    typeof smoke !== "boolean" ||
    !(out === null || (typeof out === "string" && out.length > 0))
  ) {
    return null;
  }
  return { setting, smoke, out };
}

/**
 * The check's options carried by `argv`, the renderer's arguments, or `null` on any other launch.
 *
 * @throws Error if the switch is there but does not hold valid options, which means the window
 * was created with something other than {@link viewsCheckSwitch}'s output.
 */
export function viewsCheckLaunchFromArgv(argv: readonly string[]): ViewsCheckLaunch | null {
  const carrier = argv.find((argument) => argument.startsWith(VIEWS_CHECK_SWITCH));
  if (carrier === undefined) {
    return null;
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(decodeURIComponent(carrier.slice(VIEWS_CHECK_SWITCH.length)));
  } catch (error: unknown) {
    throw new Error(`the renderer's ${VIEWS_CHECK_SWITCH} is not JSON`, { cause: error });
  }
  const launch = readViewsCheckLaunch(parsed);
  if (launch === null) {
    throw new Error(`the renderer's ${VIEWS_CHECK_SWITCH} does not hold the check's options`);
  }
  return launch;
}
