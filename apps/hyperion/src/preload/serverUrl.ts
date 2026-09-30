/**
 * How the main process hands the server URL it resolved from the command line to the preload.
 *
 * @remarks
 * A sandboxed preload cannot read the process command line or the main process's state, so the URL
 * rides in the renderer's own `argv` as an extra switch, which is what `additionalArguments` is
 * for. Both sides of the hand-off use this module; it imports nothing, so it is safe in either
 * bundle.
 */

/** The switch carrying the URL. */
const SERVER_URL_SWITCH = "--hyperion-server-url=";

/** The extra renderer argument that carries `url`, for `webPreferences.additionalArguments`. */
export function serverUrlSwitch(url: string): string {
  return `${SERVER_URL_SWITCH}${url}`;
}

/**
 * The server URL carried by `argv`, the renderer's arguments.
 *
 * @throws Error if no argument carries it, which means the window was created without
 * {@link serverUrlSwitch}.
 */
export function serverUrlFromArgv(argv: readonly string[]): string {
  const carrier = argv.find((argument) => argument.startsWith(SERVER_URL_SWITCH));
  if (carrier === undefined) {
    throw new Error(`the renderer was given no ${SERVER_URL_SWITCH}<url>`);
  }
  return carrier.slice(SERVER_URL_SWITCH.length);
}
