/**
 * How the main process tells the preload whether the client offers the `PLANET VIEWER` (plan R15),
 * a development tool's display.
 *
 * @remarks
 * As `machineLaunch.ts` does for the performance threads: the answer rides in the renderer's own
 * `argv` as an extra switch, `--hyperion-planet-viewer=<on|off>`, on every launch, and the preload
 * gives it to `HyperionApi.planetViewer`. The main process sets it on for a client that is not
 * packaged, a development build, and for one launched with `--planet-viewer`. It imports nothing,
 * so it is safe in either bundle.
 */

/** The switch carrying the answer. */
const PLANET_VIEWER_SWITCH = "--hyperion-planet-viewer=";

/** The extra renderer argument that carries whether the viewer is offered. */
export function planetViewerSwitch(offered: boolean): string {
  return `${PLANET_VIEWER_SWITCH}${offered ? "on" : "off"}`;
}

/** Whether `argv`, the renderer's arguments, offers the viewer: only where its switch says `on`. */
export function planetViewerFromArgv(argv: readonly string[]): boolean {
  return argv.includes(`${PLANET_VIEWER_SWITCH}on`);
}
