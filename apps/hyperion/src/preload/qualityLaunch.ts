/**
 * How the main process hands the launch's quality setting (`--setting`, plan R07, T17) to the
 * preload.
 *
 * @remarks
 * As `serverUrl.ts` does for the server's URL: the setting rides in the renderer's own `argv` as an
 * extra switch, `--hyperion-quality=<high|low>`, on every launch, and the preload gives it to
 * `HyperionApi.setting`. Both sides of the hand-off use this module; it imports types only, so it
 * is safe in either bundle.
 */

import type { QualitySettingName } from "./api";

/** The switch carrying the setting. */
const QUALITY_SWITCH = "--hyperion-quality=";

/** The extra renderer argument that carries `setting`, for `webPreferences.additionalArguments`. */
export function qualitySwitch(setting: QualitySettingName): string {
  return `${QUALITY_SWITCH}${setting}`;
}

/**
 * The quality setting carried by `argv`, the renderer's arguments.
 *
 * @throws Error if no argument carries a setting, which means the window was created without
 *   {@link qualitySwitch}.
 */
export function qualityFromArgv(argv: readonly string[]): QualitySettingName {
  const carrier = argv.find((argument) => argument.startsWith(QUALITY_SWITCH));
  const value = carrier?.slice(QUALITY_SWITCH.length);
  if (value !== "high" && value !== "low") {
    throw new Error(`the renderer was given no ${QUALITY_SWITCH}<high|low>`);
  }
  return value;
}
