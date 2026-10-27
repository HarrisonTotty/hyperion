/**
 * One band of the polarisation correction on each test column (plan R08, R08.T14.e's timing, test
 * 7), beside the thick bake's band: a benchmark, run by hand only, never by `vitest run`, on one
 * thread:
 *
 *     systemd-run --user --scope -p MemoryMax=4G -p MemorySwapMax=0 -- timeout 1800 nice -n 10 \
 *       pnpm --filter hyperion exec vitest bench --run --project "logic (bench)" \
 *       --reporter=verbose --maxWorkers=1 view/atmosphere/thick/polarisation
 *
 * The verbose reporter prints each phase's table. Each column's phases are timed as the optics
 * worker runs them for one band and one light: the correction's preparation (the optics at the
 * channels and 570 nm, the layers and the channels' bins' exponential sums); one band's correction,
 * its 8-stream vector and scalar solves, m = 0 … 2, at the channels; the thick bake's band on the
 * same column (16 streams, m = 0 and 1, in the 15 bins), whose preparation `bake.bench.ts` times;
 * and one 16-stream vector solve of the column's green channel, m = 0 … 2, the tests' reference.
 */
import { describe, expect, test } from "vitest";

import { bakeMultipleScattering, prepareBake } from "./bake";
import { lightOf, titanClassFixture, type ThickFixture, venusClassFixture } from "./bakeFixtures";
import {
  bakePolarisationCorrection,
  polarisationSources,
  preparePolarisation,
} from "./polarisation";
import { earthFixture, threeBarFixture } from "./polarisationFixtures";

/** Each phase's runs: a few, since one takes seconds. */
const RUNS = { iterations: 3, time: 0, warmup: false } as const;

/** Long enough for the slowest column's phases on a loaded machine, ms. */
const TIMEOUT_MS = 1_800_000;

/** The test columns, by name. */
const COLUMNS: ReadonlyArray<readonly [string, () => Promise<ThickFixture>]> = [
  ["the generated Earth", earthFixture],
  ["the three-bar N₂ world", async () => Promise.resolve(threeBarFixture())],
  ["the Venus-class column at 92 bar, with its sulphuric acid cloud", venusClassFixture],
  [
    "the Titan-class column, with its tholin haze",
    async () => Promise.resolve(titanClassFixture()),
  ],
];

describe("one band of the polarisation correction, one light", () => {
  test.for(COLUMNS)("%s", { timeout: TIMEOUT_MS }, async ([, make], { bench }) => {
    const fixture = await make();
    const light = lightOf(fixture);
    const prepared = preparePolarisation(fixture.column, [light]);
    const slot = bakePolarisationCorrection(prepared, fixture.body, 0).slots[0];
    expect(slot).toBeDefined();
    const thick = prepareBake(fixture.column, [light]);
    const solves = slot?.solves ?? 0;

    await bench("the correction's preparation", () => {
      preparePolarisation(fixture.column, [light]);
    }).run(RUNS);
    await bench(`one band's correction: ${solves} pairs of 8-stream solves`, () => {
      bakePolarisationCorrection(prepared, fixture.body, 0);
    }).run(RUNS);
    await bench("the thick bake's band on the same column", () => {
      bakeMultipleScattering(thick, fixture.body, 0);
    }).run(RUNS);
    await bench("the green channel's 16-stream vector and scalar solves, m = 0 … 2", () => {
      polarisationSources(prepared, fixture.body, 0, 1, { streams: 16 });
    }).run(RUNS);
  });
});
