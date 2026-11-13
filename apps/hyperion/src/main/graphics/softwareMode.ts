/**
 * The check-only software mode's rules in the main process (plan R01, R01.T16): where its switch
 * is refused, and how much longer its views check and its descent spike may take.
 *
 * @remarks
 * The owner's ruling of 2026-10-29 allows the mode as a test-tool exception to R01 Design note 8:
 * chosen only by an explicit switch, which `viewsCheck.sh --software` and `descentSpike.sh
 * --software` alone pass beside the smoke harness's SwiftShader switches; refused in packaged and
 * player launches; and named on the `GRAPHICS` panel (`SOFTWARE`) and in the record (`launchMode`).
 * A software launch therefore never falls back silently to a software adapter, the brainstorm's
 * reason for keeping the unsafe flag out of the client. The descent spike takes it too (main's
 * ruling, 2026-11-01, on R10.T15.b.2's runs), so that the descent runs in a pod, which has no GPU;
 * its figures are SwiftShader's, never a GPU's.
 */

import { isAbsolute, join, relative, resolve } from "node:path";

import { SOFTWARE_MODE_SWITCH } from "./switches";

/**
 * How many times longer a software-mode views check's waits, watchdog and limit are, and a
 * software-mode descent spike's watchdog and limit: fixed, not a variable (main's ruling,
 * 2026-10-29).
 *
 * @remarks
 * Measured in a pod (AMD EPYC 7571, 4 vCPU, the research lane's prototype, 2026-10-29): SwiftShader
 * made the photorealistic style's pipelines in 52 s, and its media were ready at 64.7 s at the high
 * setting, against the checks' 60 s waits; single frames took up to 22.7 s. The renderer's factor
 * (`check/softwareTiming.ts`) and the scripts' (`viewsCheck.sh`, `descentSpike.sh`) are the same
 * five, which their tests pin.
 */
export const SOFTWARE_WAIT_FACTOR = 5;

/** The test runs that may take {@link SOFTWARE_MODE_SWITCH}: the views check and the descent spike. */
export type SoftwareModeRun = "views check" | "descent spike";

/** What decides whether a launch may take {@link SOFTWARE_MODE_SWITCH}. */
export interface SoftwareModeLaunch {
  /** Whether the switch is on the command line. */
  readonly software: boolean;
  /** `app.isPackaged`. */
  readonly packaged: boolean;
  /** The run the launch makes (`--views-check`, `--descent-spike`), or `null` for neither. */
  readonly run: SoftwareModeRun | null;
  /** The results directory the run names (`--out`), or `null` for its default. */
  readonly out: string | null;
  /** The directory a relative `--out` resolves from: the process's working directory. */
  readonly cwd: string;
  /** The repository's root, under whose `target/` a software descent spike's `--out` may go. */
  readonly repoRoot: string;
}

/**
 * Why a launch may not take the software switch, as one usage line, or `null` where it may.
 *
 * @returns The refusal in a packaged build, then without `--views-check` or `--descent-spike`, on
 * every platform, so that no player's launch takes the switch, then with `--out`, so that a
 * software run's records go under `target/` alone (main's ruling, 2026-10-29): a views check's
 * `--out` always, a descent spike's unless it resolves strictly inside the repository's `target/`,
 * as the Mac batch's sweep's does in a pod's dry run; `null` without the switch.
 */
export function softwareModeRefusal(launch: SoftwareModeLaunch): string | null {
  if (!launch.software) {
    return null;
  }
  if (launch.packaged) {
    return `error: --${SOFTWARE_MODE_SWITCH} is a test switch of the views check and the descent spike, refused in a packaged build`;
  }
  if (launch.run === null) {
    return `error: --${SOFTWARE_MODE_SWITCH} is an option of --views-check and --descent-spike, neither of which was given`;
  }
  if (
    launch.out !== null &&
    !(launch.run === "descent spike" && isUnderTarget(launch.out, launch.cwd, launch.repoRoot))
  ) {
    return `error: --out is refused with --${SOFTWARE_MODE_SWITCH}, whose records go under target/ alone`;
  }
  return null;
}

/**
 * Whether `out`, from `cwd`, names a directory strictly inside `repoRoot`'s `target/`, by its path
 * alone (no link followed).
 */
function isUnderTarget(out: string, cwd: string, repoRoot: string): boolean {
  const inside = relative(join(resolve(repoRoot), "target"), resolve(cwd, out));
  return inside !== "" && !inside.startsWith("..") && !isAbsolute(inside);
}
