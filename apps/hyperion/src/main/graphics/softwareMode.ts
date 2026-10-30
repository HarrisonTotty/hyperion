/**
 * The check-only software mode's rules in the main process (plan R01, R01.T16): where its switch
 * is refused, and how much longer its views check may take.
 *
 * @remarks
 * The owner's ruling of 2026-10-29 allows the mode as a test-tool exception to R01 Design note 8:
 * chosen only by an explicit switch, which `viewsCheck.sh --software` alone passes beside the smoke
 * harness's SwiftShader switches; refused in packaged and player launches; and named on the
 * `GRAPHICS` panel (`SOFTWARE`) and in the record (`launchMode`). A software launch therefore never
 * falls back silently to a software adapter, the brainstorm's reason for keeping the unsafe flag
 * out of the client.
 */

import { SOFTWARE_MODE_SWITCH } from "./switches";

/**
 * How many times longer a software-mode views check's waits, watchdog and limit are: fixed, not a
 * variable (main's ruling, 2026-10-29).
 *
 * @remarks
 * Measured in a pod (AMD EPYC 7571, 4 vCPU, the research lane's prototype, 2026-10-29): SwiftShader
 * made the photorealistic style's pipelines in 52 s, and its media were ready at 64.7 s at the high
 * setting, against the checks' 60 s waits; single frames took up to 22.7 s. The renderer's factor
 * (`check/softwareTiming.ts`) and the script's (`viewsCheck.sh`) are the same five, which their
 * tests pin.
 */
export const SOFTWARE_WAIT_FACTOR = 5;

/** What decides whether a launch may take {@link SOFTWARE_MODE_SWITCH}. */
export interface SoftwareModeLaunch {
  /** Whether the switch is on the command line. */
  readonly software: boolean;
  /** `app.isPackaged`. */
  readonly packaged: boolean;
  /** Whether the launch is a views check (`--views-check`). */
  readonly viewsCheck: boolean;
  /** Whether the views check names its own results directory (`--out`). */
  readonly out: boolean;
}

/**
 * Why a launch may not take the software switch, as one usage line, or `null` where it may.
 *
 * @returns The refusal in a packaged build, then without `--views-check`, on every platform, so
 * that no player's launch takes the switch, then with `--out`, so that a software run's records go
 * under `target/` alone (main's ruling, 2026-10-29); `null` without the switch.
 */
export function softwareModeRefusal(launch: SoftwareModeLaunch): string | null {
  if (!launch.software) {
    return null;
  }
  if (launch.packaged) {
    return `error: --${SOFTWARE_MODE_SWITCH} is a views check's test switch, refused in a packaged build`;
  }
  if (!launch.viewsCheck) {
    return `error: --${SOFTWARE_MODE_SWITCH} is an option of --views-check, which was not given`;
  }
  if (launch.out) {
    return `error: --out is refused with --${SOFTWARE_MODE_SWITCH}, whose records go under target/ alone`;
  }
  return null;
}
