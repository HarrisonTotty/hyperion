/**
 * The line absorbers' test fixtures (plan R08, R08.T4.d): the toy tables
 * (`kDistributionToys.ts`), and a reader of the committed parts as `?inline` assets, since
 * renderer code, tests included, reads no files through `node:fs` (R04.T10.c).
 */

import type { PartReader } from "../view/atmosphere/absorbers/kDistribution";

export { type ToyCell, toyPart, toySidecar } from "./kDistributionToys";

/** The committed parts as `?inline` data URLs, by path relative to `absorbers/`. */
const INLINE_PARTS = import.meta.glob<string>(
  [
    "../view/atmosphere/absorbers/kDistributions/*.bin",
    "../view/atmosphere/absorbers/exomol/kDistributions/*.bin",
  ],
  { query: "?inline", import: "default" },
);

/**
 * Reads a committed part in a test, as an inlined asset (`assetsInclude`, `vitest.config.mts`), the
 * way the surface tests read the wasm.
 */
export const inlinePart: PartReader = async (path) => {
  const load = INLINE_PARTS[`../view/atmosphere/absorbers/${path}`];
  if (load === undefined) {
    throw new Error(`no committed k-distribution part ${path}`);
  }
  const url = await load();
  const bytes = Uint8Array.from(atob(url.slice(url.indexOf(",") + 1)), (c) => c.charCodeAt(0));
  return bytes.buffer;
};
