/**
 * Where the command-line tools find the store and the table by default: the repository's
 * `docs/measurements/rendering/`, from this file's place in it.
 */

import { fileURLToPath } from "node:url";

/** `docs/measurements/rendering/` in this checkout. */
export const RENDERING_DIR = fileURLToPath(
  new URL("../../../../docs/measurements/rendering/", import.meta.url),
);

/** The store, `runs.v1.jsonl`. */
export const STORE_PATH = `${RENDERING_DIR}runs.v1.jsonl`;

/** The generated table, `budget.md`. */
export const BUDGET_PATH = `${RENDERING_DIR}budget.md`;

/**
 * The value after `flag` in `args`, or `fallback`; the flag and its value are taken out of `args`.
 *
 * @throws Error if the flag is the last argument.
 */
export function takeOption(args: string[], flag: string, fallback: string): string {
  const at = args.indexOf(flag);
  if (at < 0) return fallback;
  const value = args[at + 1];
  if (value === undefined) throw new Error(`${flag} needs a value`);
  args.splice(at, 2);
  return value;
}

/** Whether `flag` is in `args`, taking it out. */
export function takeFlag(args: string[], flag: string): boolean {
  const at = args.indexOf(flag);
  if (at < 0) return false;
  args.splice(at, 1);
  return true;
}
