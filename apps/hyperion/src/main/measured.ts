/**
 * The form every figure of the descent spike's results takes (plan R05, T14.c): a value, or `null`
 * with the reason it is missing.
 *
 * @remarks
 * Its own module so that `results.ts` and `traceWindows.ts`, which both build figures, do not
 * import each other.
 */

/** A figure, or `null` with the reason it is missing. */
export type Measured<T> =
  { readonly value: T; readonly reason: null } | { readonly value: null; readonly reason: string };

/** A present figure. */
export function measured<T>(value: T): Measured<T> {
  return { value, reason: null };
}

/** A missing figure and why. */
export function missing(reason: string): Measured<never> {
  return { value: null, reason };
}
