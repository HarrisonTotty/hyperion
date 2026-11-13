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
  // Indexed, not `Uint8Array.from` over the string with a callback a byte: that took 30 times as
  // long, 520 ms against 18 ms for H₂O's 13 parts (2026-10-15).
  const text = atob(url.slice(url.indexOf(",") + 1));
  const bytes = new Uint8Array(text.length);
  for (let i = 0; i < text.length; i += 1) {
    bytes[i] = text.charCodeAt(i);
  }
  return bytes.buffer;
};
