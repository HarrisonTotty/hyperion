/**
 * The relaunch that takes a Wayland session through XWayland.
 *
 * @remarks
 * The brainstorm runs every Linux machine through X11 or XWayland. Researched 2026-09-29 by probe
 * on Electron 44.4.3: the browser process chooses its Ozone platform from `XDG_SESSION_TYPE` before
 * main-process JavaScript runs, so an appended `ozone-platform` switch leaves a Wayland session on
 * Wayland, and it reaches the GPU process's command line, where the mismatch gave no adapter and
 * black output. Only the flag on the real command line works, so a Wayland launch without it is
 * relaunched with it (R01 Design note 3). The packaged launcher and the `client` recipe put the
 * flag on the command line themselves, so the relaunch is a fallback, not the rule.
 */

/** The flag that runs Chromium on X11, through XWayland in a Wayland session. */
export const OZONE_X11_FLAG = "--ozone-platform=x11";

/**
 * `RelaunchOptions.args` for a Wayland session to run through XWayland, or `undefined` when the
 * launch needs no relaunch.
 *
 * @param args - `process.argv.slice(1)`: Electron supplies the executable itself.
 * @param env - The process environment, read for `XDG_SESSION_TYPE`.
 * @returns `args` with {@link OZONE_X11_FLAG} appended on Linux when the session is Wayland and the
 * flag is not already there; `undefined` under X11, with no session type, with the flag present
 * and on every other platform.
 */
export function x11RelaunchArgs(
  args: readonly string[],
  env: NodeJS.ProcessEnv,
  platform: NodeJS.Platform,
): readonly string[] | undefined {
  if (platform !== "linux" || env["XDG_SESSION_TYPE"] !== "wayland") {
    return undefined;
  }
  if (args.includes(OZONE_X11_FLAG)) {
    return undefined;
  }
  return [...args, OZONE_X11_FLAG];
}
