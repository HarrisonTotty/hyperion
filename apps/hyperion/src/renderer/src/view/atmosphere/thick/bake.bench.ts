/**
 * One whole thick bake of each thick fixture (plan R08, R08.T14.b's timing, against
 * `BAKE_CEILING_S`): a benchmark, run by hand only, never by `vitest run`, on one thread:
 *
 *     systemd-run --user --scope -p MemoryMax=4G -p MemorySwapMax=0 -- timeout 1800 nice -n 10 \
 *       pnpm --filter hyperion exec vitest bench --run --project "logic (bench)" \
 *       --reporter=verbose --maxWorkers=1 view/atmosphere/thick/bake
 *
 * The verbose reporter prints each phase's table. Each fixture's phases are timed as the optics
 * worker runs them for one band and one light: its absorbers' bin curves under the Sun; the
 * column's optics in the 15 bins alone (an aerosol's Mie or MMF, the larger share of the
 * preparation); the whole preparation (the optics, the layers and the exponential sums); and one
 * band's bake, every bin and term, m = 0 and 1. A phase's name carries its solves and terms.
 */
import { describe, expect, test } from "vitest";

import { BAKE_WAVELENGTHS_NM } from "../medium";
import { bakeMultipleScattering, prepareBake } from "./bake";
import { lightOf, type ThickFixture, titanClassFixture, venusClassFixture } from "./bakeFixtures";
import { spectralOptics } from "./spectralColumn";

/** Each phase's runs: a few, since one takes seconds. */
const RUNS = { iterations: 3, time: 0, warmup: false } as const;

/** Long enough for the slowest fixture's phases on a loaded machine, ms. */
const TIMEOUT_MS = 1_200_000;

/** The thick fixtures, by name. */
const FIXTURES: ReadonlyArray<readonly [string, () => Promise<ThickFixture>]> = [
  ["the Venus-class column at 92 bar, with its sulphuric acid cloud", venusClassFixture],
  [
    "the Titan-class column, with its tholin haze",
    async () => Promise.resolve(titanClassFixture()),
  ],
];

describe("one thick bake, one band and one light", () => {
  test.for(FIXTURES)("%s", { timeout: TIMEOUT_MS }, async ([, make], { bench }) => {
    const fixture = await make();
    const light = lightOf(fixture);
    const prepared = prepareBake(fixture.column, [light]);
    const slot = bakeMultipleScattering(prepared, fixture.body, 0).slots[0];
    expect(slot).toBeDefined();
    const terms = slot?.bins.map((bin) => bin.terms).join(" ") ?? "";
    const solves = slot?.solves ?? 0;

    await bench("the absorbers' bin curves under the Sun", () => {
      lightOf(fixture);
    }).run(RUNS);
    await bench("the column's optics in the 15 bins", () => {
      spectralOptics(fixture.column, BAKE_WAVELENGTHS_NM);
    }).run(RUNS);
    await bench("the preparation: the optics, the layers and the exponential sums", () => {
      prepareBake(fixture.column, [light]);
    }).run(RUNS);
    await bench(`one band's bake: ${solves} solves, the bins' terms ${terms}`, () => {
      bakeMultipleScattering(prepared, fixture.body, 0);
    }).run(RUNS);
  });
});
