/**
 * Each record fixture's optics times in process (plan R08, R08.T10.a's "each fixture's optics time
 * is recorded, on a quiet machine"; for R08.T11's arrival line): a benchmark, run by hand only,
 * never by `vitest run`:
 *
 *     pnpm --filter hyperion exec vitest bench --run --project "logic (bench)" --reporter=verbose view/atmosphere/opticsTimes
 *
 * The verbose reporter prints each phase's table; the default prints none. `-t <name>` takes one
 * fixture. Each fixture's phases are timed as the optics pool runs them, one sun and the UHD 620
 * host's four shares (`defaultOpticsWorkers` of eight threads): the line tables' load, once a
 * worker; the assembly; one share on a worker that forms the body's specs itself, and the same
 * share on the worker that assembled it, its specs held, the two differing by the specs' forming;
 * and the join of the four. On a quiet machine the slower share is the pool's time for the light
 * set, beside the assembly on its own worker.
 */
import { describe, expect, test } from "vitest";

import type { BodyDetail } from "../../lib/system/model";
import {
  detailOf,
  earthClassLinesDetail,
  plutoHazeDetail,
  recordInput,
  venus58Detail,
  venus92Detail,
} from "../../test/atmosphereRecords";
import { inlinePart } from "../../test/kDistributionFixtures";
import { earthDetail, titanDetail } from "../../test/planetaryFixture";
import type { AbsorberCurveSums } from "./absorbers";
import { loadKDistributions } from "./absorbers/kDistribution";
import { lineSpeciesOf } from "./assemble";
import { OpticsJobs, type OpticsJobsDeps } from "./opticsJobs";
import type { OpticsRequest } from "./opticsMessages";

/** The shares a light set: the UHD 620 host's pool. */
const SHARES = 4;

/** Each phase's runs: a few, since one takes up to seconds. */
const RUNS = { iterations: 3, time: 0, warmup: false } as const;

/** Long enough for the slowest fixture's phases on a loaded machine, ms. */
const TIMEOUT_MS = 600_000;

/** The record fixtures, by name. */
const FIXTURES: ReadonlyArray<readonly [string, () => BodyDetail]> = [
  ["the generated Earth", () => detailOf(earthDetail())],
  ["an Earth-class body with O2's lines", earthClassLinesDetail],
  ["a Venus at 92 bar", venus92Detail],
  ["a Venus at 58 bar", venus58Detail],
  ["the generated Titan", () => detailOf(titanDetail())],
  ["a Tenuous Pluto with haze", plutoHazeDetail],
];

describe("each record fixture's optics, one sun, four shares", () => {
  test.for(FIXTURES)("%s", { timeout: TIMEOUT_MS }, async ([, detail], { bench }) => {
    const input = recordInput(detail());
    const species = lineSpeciesOf(input);
    const tables = await loadKDistributions(species, inlinePart);
    const deps: OpticsJobsDeps = { loadKDistributions: async () => Promise.resolve(tables) };
    const assemble: OpticsRequest = { kind: "assemble", id: 1, input };
    const shareOf = (index: number): OpticsRequest => ({
      kind: "curve-sums",
      id: 2 + index,
      input,
      slots: [0],
      share: { index, count: SHARES },
    });
    // The worker that assembled the body, and every share's sums, taken once: a phase that failed
    // would time a failure, so each phase is first answered and its kind held below.
    const assembler = new OpticsJobs(deps);
    const kinds: string[] = [(await assembler.answer(assemble)).reply.kind];
    const sums: Array<ReadonlyArray<AbsorberCurveSums>> = [];
    for (let index = 0; index < SHARES; index += 1) {
      // Each share as one worker answers it, one after another.
      // oxlint-disable-next-line no-await-in-loop
      const { reply } = await new OpticsJobs(deps).answer(shareOf(index));
      kinds.push(reply.kind);
      sums.push(reply.kind === "curve-sums" ? reply.sums : []);
    }
    const join: OpticsRequest = { kind: "curve-join", id: 9, input, slots: [0], sums };
    kinds.push((await assembler.answer(join)).reply.kind);
    expect(kinds).toEqual([
      "assembled",
      ...Array.from({ length: SHARES }, () => "curve-sums"),
      "curves",
    ]);

    await bench("the line tables' load, once a worker", async () => {
      await loadKDistributions(species, inlinePart);
    }).run(RUNS);
    await bench("the assembly", async () => {
      await new OpticsJobs(deps).answer(assemble);
    }).run(RUNS);
    await bench("a share of four, forming the body's specs", async () => {
      await new OpticsJobs(deps).answer(shareOf(1));
    }).run(RUNS);
    await bench("a share of four on the worker that assembled, its specs held", async () => {
      await assembler.answer(shareOf(1));
    }).run(RUNS);
    await bench("the join of four shares", async () => {
      await assembler.answer(join);
    }).run(RUNS);
  });
});
