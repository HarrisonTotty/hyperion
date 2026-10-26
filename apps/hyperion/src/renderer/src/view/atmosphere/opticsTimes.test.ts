/**
 * Each record fixture's optics times in process, and the bytes its entry holds (plan R08,
 * R08.T10.a's "each fixture's optics time is recorded, on a quiet machine"; for R08.T11's arrival
 * line). Run by hand only, and asserting nothing:
 *
 *     HYPERION_OPTICS_TIMES=1 pnpm --filter hyperion exec vitest run view/atmosphere/opticsTimes --reporter=verbose
 *
 * Each case annotates its figures, which the verbose reporter prints beside it. The shares a set
 * are the UHD 620 host's pool, four (`defaultOpticsWorkers` of eight threads).
 */
import { describe, it } from "vitest";

import {
  detailOf,
  earthClassLinesDetail,
  plutoHazeDetail,
  recordInput,
  venus58Detail,
  venus92Detail,
} from "../../test/atmosphereRecords";
import { inlinePart } from "../../test/kDistributionFixtures";
import { type OpticsTimes, timeOpticsBuild } from "../../test/opticsTiming";
import { earthDetail, titanDetail } from "../../test/planetaryFixture";
import type { BodyDetail } from "../../lib/system/model";

/** Whether the times are asked for. */
const TIMED = import.meta.env["HYPERION_OPTICS_TIMES"] === "1";

/** The shares a light set: the UHD 620 host's pool. */
const SHARES = 4;

/** Long enough for the slowest fixture on a loaded machine, ms. */
const TIMEOUT_MS = 300_000;

/** The record fixtures, by name. */
const FIXTURES: ReadonlyArray<readonly [string, () => BodyDetail]> = [
  ["the generated Earth", () => detailOf(earthDetail())],
  ["an Earth-class body with O2's lines", earthClassLinesDetail],
  ["a Venus at 92 bar", venus92Detail],
  ["a Venus at 58 bar", venus58Detail],
  ["the generated Titan", () => detailOf(titanDetail())],
  ["a Tenuous Pluto with haze", plutoHazeDetail],
];

/** A time in whole milliseconds. */
function ms(value: number): string {
  return value.toFixed(0);
}

/** The figures in a line. */
function figures(times: OpticsTimes): string {
  const sets = times.shareMs.map(
    (shares, k) =>
      `set ${k}: shares ${shares.map(ms).join("/")} ms (slowest ${ms(Math.max(...shares))}), join ${ms(times.joinMs[k] ?? Number.NaN)} ms`,
  );
  return [
    `load ${ms(times.loadMs)} ms`,
    `assemble ${ms(times.assembleMs)} ms`,
    ...sets,
    `held ${times.mediumBytes.toLocaleString("en-GB")} B of medium and ${times.curvesBytes.toLocaleString("en-GB")} B of curves`,
  ].join("; ");
}

describe.runIf(TIMED)("each record fixture's optics, one sun, four shares", () => {
  it.for(FIXTURES)("%s", { timeout: TIMEOUT_MS }, async ([, detail], { annotate, expect }) => {
    const times = await timeOpticsBuild(recordInput(detail()), inlinePart, SHARES);
    await annotate(figures(times));
    // That every figure was measured, a join for each light set; no time is held to a bound.
    expect([times.shareMs.length, [times.loadMs, times.assembleMs].every(Number.isFinite)]).toEqual(
      [times.joinMs.length, true],
    );
  });
});
