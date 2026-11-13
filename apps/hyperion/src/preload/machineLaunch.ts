/**
 * How the main process hands the machine's performance threads, with a third of its efficiency
 * cores' (`src/main/performanceThreads.ts`), to the preload.
 *
 * @remarks
 * As `qualityLaunch.ts` does for the quality setting: the count rides in the renderer's own `argv`
 * as an extra switch, `--hyperion-performance-threads=<n|none>`, on every launch, and the preload
 * gives it to `HyperionApi.performanceThreads`. It imports nothing, so it is safe in either bundle.
 */

/** The switch carrying the count. */
const PERFORMANCE_THREADS_SWITCH = "--hyperion-performance-threads=";

/** The extra renderer argument that carries `threads`, for `webPreferences.additionalArguments`. */
export function performanceThreadsSwitch(threads: number | null): string {
  return `${PERFORMANCE_THREADS_SWITCH}${threads === null ? "none" : String(threads)}`;
}

/**
 * The performance threads carried by `argv`, the renderer's arguments: a whole number from 1, or
 * `null` where the machine's OS tells none apart, or no argument carries any.
 */
export function performanceThreadsFromArgv(argv: readonly string[]): number | null {
  const carrier = argv.find((argument) => argument.startsWith(PERFORMANCE_THREADS_SWITCH));
  const threads = Number(carrier?.slice(PERFORMANCE_THREADS_SWITCH.length));
  return Number.isInteger(threads) && threads >= 1 ? threads : null;
}
