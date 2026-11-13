/**
 * How the main process hands a launch's compile budget (`--compile-budget`, plan R14's Mac
 * measurements) to the preload, for the renderer's engine (`webgpu/compileBudget.ts`).
 *
 * @remarks
 * As `geometryLaunch.ts` does for `--geometry`: the budget rides in the renderer's own `argv` as an
 * extra switch, `--hyperion-compile-budget=<n|unlimited>`, on a spike or a check launch that gives
 * it, and the preload gives it to `HyperionApi.compileBudget`. Without it the engine takes its
 * default. It imports nothing, so it is safe in either bundle.
 */

/** The switch carrying the budget. */
const COMPILE_BUDGET_SWITCH = "--hyperion-compile-budget=";

/** A launch's compile budget: a whole number from 1, or `unlimited`, every compile at once. */
export type CompileBudgetLaunch = number | "unlimited";

/**
 * Reads a budget's text: `unlimited`, or a whole number from 1 to 64 in decimal.
 *
 * @throws Error naming the text for anything else.
 */
export function parseCompileBudget(text: string): CompileBudgetLaunch {
  if (text === "unlimited") {
    return text;
  }
  const size = /^\d{1,2}$/u.test(text) ? Number(text) : Number.NaN;
  if (!Number.isInteger(size) || size < 1 || size > 64) {
    throw new Error(`a compile budget is unlimited or 1 to 64 compiles, not ${text}`);
  }
  return size;
}

/** The extra renderer argument that carries `budget`, for `webPreferences.additionalArguments`. */
export function compileBudgetSwitch(budget: CompileBudgetLaunch): string {
  return `${COMPILE_BUDGET_SWITCH}${String(budget)}`;
}

/**
 * The budget carried by `argv`, the renderer's arguments, or `null` where none is.
 *
 * @throws Error where the switch holds no budget, which only a `argv` the main process did not make
 *   with {@link compileBudgetSwitch} can.
 */
export function compileBudgetFromArgv(argv: readonly string[]): CompileBudgetLaunch | null {
  const carrier = argv.find((argument) => argument.startsWith(COMPILE_BUDGET_SWITCH));
  return carrier === undefined
    ? null
    : parseCompileBudget(carrier.slice(COMPILE_BUDGET_SWITCH.length));
}
