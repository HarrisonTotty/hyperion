/**
 * The optics pool's bin shares (plan R08, R08.T14.d's bake time): a band of a bake whose bands do
 * not fill the workers is baked a share of its bins a job and gathered in one job asked ahead of
 * the queue, on scripted workers; and a band's bins baked in shares gather to the bits of the band
 * baked whole, directly and through in-process workers.
 */
import { describe, expect, it, onTestFinished } from "vitest";

import { saturnBody } from "../../test/atmosphereFixtures";
import { bitsOf } from "../../test/bits";
import { InProcessOpticsWorker } from "../../test/inProcessOpticsWorker";
import { microtasks, toyLoader } from "../../test/opticsFakes";
import { ScriptedOpticsWorker } from "../../test/scriptedOpticsWorker";
import {
  patternedCorrection,
  patternedDeckTables,
  patternedThickBake,
} from "../../test/thickTables";
import type { BodyAtmosphereInput } from "./atmosphereInput";
import { EARTH_COLUMN, R05_EARTH_REFERENCE, R06_SOLAR_BAKE_SPECTRUM } from "./earth";
import { BAKE_BIN_COUNT, CHANNEL_WAVELENGTHS_NM, type RenderChannel } from "./medium";
import { OpticsJobs } from "./opticsJobs";
import { keptBakeLightsOf, type OpticsRequest } from "./opticsMessages";
import {
  BAKE_BINS,
  binShares,
  binSharesFor,
  type DeckBakeAsk,
  OpticsWorkerPool,
  type ThickBakeAsk,
} from "./opticsPool";
import {
  EARTH_CASE,
  EARTH_REFERENCE_RADIUS_M,
  referenceCaseBakeLights,
  SATURN_OBLATE_GRAVITY,
  SATURN_OBLATE_REFERENCE,
  SATURN_OBLATE_REFERENCE_SOURCES,
  SATURN_OBLATE_SPHEROID,
} from "./reference/cases";
import { tableRadiusM } from "./hillaire";
import {
  bakeBins,
  bakeMultipleScattering,
  gatherBins,
  type MsBinShare,
  prepareBake,
  THICK_BAKE_REFERENCE_BIN,
  thickBakeBands,
  thickBakeLights,
} from "./thick/bake";
import {
  bakeDeck,
  bakeDeckBins,
  bakeDeckPolarisation,
  type DeckBinShare,
  deckPolarisationChannel,
  gatherDeckBins,
  prepareDeck,
  prepareDeckPolarisation,
} from "./thick/deck";
import {
  bakePolarisationCorrection,
  gatherChannels,
  type PolarisationChannelShare,
  polarisationChannel,
  preparePolarisation,
} from "./thick/polarisation";
import { spectralColumnOf } from "./thick/columnLayout";
import { keptColumnOf, sphereBakeBody } from "./thick/keptBake";
import { channelSource } from "./thick/spectralColumn";

const KEPT_UNSET: BodyAtmosphereInput = { kind: "kept", atmosphere: { kind: "unset" } };

/** A pool of `count` scripted workers, terminated when the test ends however it ends. */
function scriptedPool(count: number): {
  readonly pool: OpticsWorkerPool;
  readonly workers: ScriptedOpticsWorker[];
} {
  const workers: ScriptedOpticsWorker[] = [];
  const pool = new OpticsWorkerPool({
    workers: count,
    createWorker: () => {
      const worker = new ScriptedOpticsWorker();
      workers.push(worker);
      return worker;
    },
  });
  onTestFinished(() => {
    pool.terminate();
  });
  return { pool, workers };
}

/** Every request the workers hold unanswered, oldest first, worker by worker. */
function pendingOf(workers: ReadonlyArray<ScriptedOpticsWorker>): OpticsRequest[] {
  return workers.flatMap((worker) => worker.pending);
}

/** The scripted worker a request was posted to, or a test failure. */
function workerOf(
  workers: ReadonlyArray<ScriptedOpticsWorker>,
  request: OpticsRequest,
): ScriptedOpticsWorker {
  const worker = workers.find((each) => each.received.includes(request));
  if (worker === undefined) {
    throw new Error(`no worker was posted request ${request.id}`);
  }
  return worker;
}

/** An ask of R05's Earth's column, by its channels, on a sphere's one band or Saturn's four. */
function askOf(saturn: boolean): ThickBakeAsk {
  const body = saturn
    ? { ...saturnBody(), bottomRadiusM: 6_371_008.8 }
    : sphereBakeBody(6_371_008.8, 9.806_65);
  return {
    input: KEPT_UNSET,
    slots: [0],
    bands: thickBakeBands(body).map((_, band) => band),
    column: spectralColumnOf(R05_EARTH_REFERENCE, R05_EARTH_REFERENCE.terms.map(channelSource)),
    body,
  };
}

/** A share of a band's bins as a scripted worker answers it: its bins, with a buffer to transfer. */
function shareOf(bins: ReadonlyArray<number>): MsBinShare {
  const band = thickBakeBands(askOf(false).body)[0];
  if (band === undefined) {
    throw new Error("a sphere has a band");
  }
  return {
    band,
    altitudesM: new Float64Array(33),
    sunCosines: new Float64Array(46),
    travelCosines: new Float64Array(16),
    conversions: [],
    thermal: false,
    bins: bins.map((bin) => ({
      bin,
      verticalOpticalDepth: bin,
      perScattering: new Float64Array(33),
      lights: [],
    })),
  };
}

/** A correction's channel share as a scripted worker answers it. */
function channelShareOf(channel: RenderChannel): PolarisationChannelShare {
  const band = shareOf([]).band;
  return {
    channel,
    band,
    altitudesM: new Float64Array(33),
    sunCosines: new Float64Array(46),
    travelCosines: new Float64Array(16),
    lights: [],
  };
}

/** A deck's share of a band's bins as a scripted worker answers it. */
function deckShareOf(bins: ReadonlyArray<number>): DeckBinShare {
  const above = shareOf(bins);
  return {
    above,
    band: above.band,
    deckAltitudeM: 3_000,
    floorAltitudeM: 0,
    floor: "envelope",
    sunCosines: new Float64Array(46),
    viewCosines: new Float64Array(8),
    modes: 8,
    diffuseModes: 3,
    below: null,
    bins: bins.map((bin) => ({ bin, deckOpticalDepth: bin, lights: [], below: null })),
  };
}

describe("the optics pool's bin shares (R08.T14.d)", () => {
  it("splits a band into two shares of its bins a worker where its bake's bands do not fill them", () => {
    expect([
      binSharesFor(1, 4),
      binSharesFor(2, 4),
      binSharesFor(3, 4),
      binSharesFor(4, 4),
      binSharesFor(1, 1),
      binSharesFor(0, 4),
      binSharesFor(1, 16),
    ]).toEqual([8, 4, 3, 1, 1, 1, 15]);
  });

  it("deals the bins to the shares in turn, every bin once", () => {
    expect([binShares(4), BAKE_BINS]).toEqual([
      [
        [0, 4, 8, 12],
        [1, 5, 9, 13],
        [2, 6, 10, 14],
        [3, 7, 11],
      ],
      BAKE_BIN_COUNT,
    ]);
  });

  it("asks a sphere's band of a share of its bins a job, then gathers them ahead of the jobs waiting", async () => {
    const { pool, workers } = scriptedPool(2);
    const ask = askOf(false);
    const bake = pool.bakeThick(ask);
    const correctionAsk = askOf(true);
    const correction = pool.bakePolarisation(correctionAsk);
    expect([pendingOf(workers).map((request) => request.kind), pool.queuedCount]).toEqual([
      ["thick-bake-bins", "thick-bake-bins"],
      6,
    ]);
    const answered: MsBinShare[] = [];
    for (let k = 0; k < 4; k += 1) {
      const request = pendingOf(workers).find((each) => each.kind === "thick-bake-bins");
      if (request?.kind !== "thick-bake-bins") {
        throw new Error(`share ${k} was not asked`);
      }
      const share = shareOf(request.bins);
      answered.push(share);
      workerOf(workers, request).reply({ kind: "thick-bins-baked", id: request.id, share });
    }
    await microtasks();
    // Both workers hold the correction's first two bands; the gather waits ahead of its other two.
    const first = pendingOf(workers).find((request) => request.kind === "polarisation-bake");
    if (first === undefined) {
      throw new Error("the correction's first band was not asked");
    }
    const corrections = patternedCorrection(
      thickBakeBands(correctionAsk.body),
      R05_EARTH_REFERENCE,
      1,
    );
    expect(pool.queuedCount).toBe(3);
    workerOf(workers, first).reply({
      kind: "polarisation-baked",
      id: first.id,
      tables: corrections.slice(0, 1),
    });
    const gather = pendingOf(workers).find((request) => request.kind === "thick-gather");
    if (gather?.kind !== "thick-gather") {
      throw new Error("the shares were not gathered");
    }
    // The shares in their own order, though answered in another.
    expect([
      gather.shares.map((share) => share.bins.map((part) => part.bin)),
      gather.shares.every((share) => answered.includes(share)),
      pool.queuedCount,
    ]).toEqual([binShares(4), true, 2]);
    const table = patternedThickBake(thickBakeBands(ask.body), R05_EARTH_REFERENCE, 1);
    workerOf(workers, gather).reply({ kind: "thick-baked", id: gather.id, tables: table });
    const outcome = await bake.outcome;
    correction.cancel();
    expect(outcome.kind === "baked" && outcome.tables[0] === table[0]).toBe(true);
  });

  it("asks a sphere's correction of one render channel a job, then gathers them ahead of the queue", async () => {
    const { pool, workers } = scriptedPool(2);
    const ask = askOf(false);
    const correction = pool.bakePolarisation(ask);
    for (let c = 0; c < 3; c += 1) {
      const request = pendingOf(workers).find((each) => each.kind === "polarisation-bake-channel");
      if (request?.kind !== "polarisation-bake-channel") {
        throw new Error(`channel ${c} was not asked`);
      }
      workerOf(workers, request).reply({
        kind: "polarisation-channel-baked",
        id: request.id,
        share: channelShareOf(request.channel),
      });
    }
    await microtasks();
    const gather = pendingOf(workers).find((request) => request.kind === "polarisation-gather");
    if (gather?.kind !== "polarisation-gather") {
      throw new Error("the channels were not gathered");
    }
    const tables = patternedCorrection(thickBakeBands(ask.body), R05_EARTH_REFERENCE, 1);
    workerOf(workers, gather).reply({ kind: "polarisation-baked", id: gather.id, tables });
    const outcome = await correction.outcome;
    expect([
      gather.shares.map((share) => share.channel),
      outcome.kind === "baked" && outcome.tables[0] === tables[0],
    ]).toEqual([[0, 1, 2], true]);
  });

  it.each([
    ["every channel holds a correction, which it gathers", true],
    ["the column needs no correction, and gathers none", false],
  ])(
    "asks a sphere's deck of a share of its bins and a channel of its correction a job, then gathers them: %s",
    async (_, corrected) => {
      const { pool, workers } = scriptedPool(4);
      const ask: DeckBakeAsk = { ...askOf(false), split: { altitudeM: 3_000, floor: "ground" } };
      const deck = pool.bakeDeck(ask);
      const kinds = new Set<string>();
      for (let n = 0; n < 11; n += 1) {
        const [request] = pendingOf(workers);
        if (request?.kind === "deck-bake-bins") {
          workerOf(workers, request).reply({
            kind: "deck-bins-baked",
            id: request.id,
            share: deckShareOf(request.bins),
          });
        } else if (request?.kind === "deck-polarisation-channel") {
          workerOf(workers, request).reply({
            kind: "deck-channel-baked",
            id: request.id,
            share: corrected ? channelShareOf(request.channel) : null,
          });
        } else {
          throw new Error(`job ${n} was asked ${request?.kind ?? "of none"}`);
        }
        kinds.add(request.kind);
      }
      await microtasks();
      const gather = pendingOf(workers).find((request) => request.kind === "deck-gather");
      if (gather?.kind !== "deck-gather") {
        throw new Error("the deck was not gathered");
      }
      const tables = patternedDeckTables(thickBakeBands(ask.body), R05_EARTH_REFERENCE, 1, 3_000);
      const correction = corrected
        ? patternedCorrection(thickBakeBands(ask.body), R05_EARTH_REFERENCE, 1)
        : null;
      workerOf(workers, gather).reply({ kind: "deck-baked", id: gather.id, tables, correction });
      const outcome = await deck.outcome;
      expect([
        [...kinds],
        gather.decks.map((share) => share.bins.map((part) => part.bin)),
        gather.channels?.map((share) => share.channel) ?? null,
        outcome.kind === "baked" && outcome.tables[0] === tables[0],
        outcome.kind === "baked" ? (outcome.correction?.length ?? null) : outcome,
      ]).toEqual([
        ["deck-bake-bins", "deck-polarisation-channel"],
        binShares(8),
        corrected ? [0, 1, 2] : null,
        true,
        corrected ? 1 : null,
      ]);
    },
  );

  it("fails a band whose share fails, its other shares given up", async () => {
    const { pool, workers } = scriptedPool(2);
    const bake = pool.bakeThick(askOf(false));
    const [first] = pendingOf(workers);
    if (first === undefined) {
      throw new Error("no share was asked");
    }
    workerOf(workers, first).reply({
      kind: "thick-bake-failed",
      id: first.id,
      bands: [0],
      message: "the solver refused a layer",
    });
    expect([await bake.outcome, pool.queuedCount]).toEqual([
      { kind: "failed", message: "the solver refused a layer" },
      0,
    ]);
  });
});

/** `saturn-oblate`'s body: its figure, gravity and R_ref. */
function saturnOblateBody(): ThickBakeAsk["body"] {
  return {
    figure: SATURN_OBLATE_SPHEROID,
    gravity: SATURN_OBLATE_GRAVITY,
    bottomRadiusM: tableRadiusM(SATURN_OBLATE_SPHEROID),
  };
}

// A band's bake of `saturn-oblate` takes about 1 s on a pod's thread, whole and in shares; and
// the kept TEST EARTH's about 2 s each way (2026-10-31): ten times that, rounded up.
const BAKE_TIMEOUT_MS = 60_000;

describe("a band's bins baked in shares, to the bit (R08.T14.d)", () => {
  it(
    "gathers saturn-oblate's first band, its bins baked in three shares, to the bits of the band baked whole",
    { timeout: BAKE_TIMEOUT_MS },
    async () => {
      const column = spectralColumnOf(SATURN_OBLATE_REFERENCE, SATURN_OBLATE_REFERENCE_SOURCES);
      const kept = keptBakeLightsOf(SATURN_OBLATE_REFERENCE, [R06_SOLAR_BAKE_SPECTRUM]);
      const lights = thickBakeLights(column, kept.absorbers, kept.spectra);
      const body = saturnOblateBody();
      const whole = bakeMultipleScattering(prepareBake(column, lights), body, 0);
      const parts = binShares(3).map((bins) =>
        bakeBins(prepareBake(column, lights, undefined, undefined, undefined, bins), body, 0),
      );
      expect(await bitsOf(gatherBins(parts.toReversed()))).toBe(await bitsOf(whole));
    },
  );

  it(
    "gathers a sphere's correction, a render channel a share, to the bits of the band's correction baked whole",
    { timeout: BAKE_TIMEOUT_MS },
    async () => {
      const column = spectralColumnOf(
        R05_EARTH_REFERENCE,
        R05_EARTH_REFERENCE.terms.map(channelSource),
      );
      const lights = [{ spectrum: R06_SOLAR_BAKE_SPECTRUM, absorbers: [] }];
      const body = sphereBakeBody(6_371_008.8, 9.806_65);
      const whole = bakePolarisationCorrection(preparePolarisation(column, lights), body, 0);
      const channels = CHANNEL_WAVELENGTHS_NM.map((nm) =>
        polarisationChannel(
          preparePolarisation(column, lights, { wavelengthsNm: [nm] }),
          body,
          0,
          0,
        ),
      );
      expect(await bitsOf(gatherChannels(channels.toReversed()))).toBe(await bitsOf(whole));
    },
  );

  it(
    "gathers a deck over a surface, its bins baked in three shares, to the bits of the band baked whole, and its correction a channel a share",
    { timeout: BAKE_TIMEOUT_MS },
    async () => {
      const column = spectralColumnOf(
        R05_EARTH_REFERENCE,
        R05_EARTH_REFERENCE.terms.map(channelSource),
      );
      const lights = [{ spectrum: R06_SOLAR_BAKE_SPECTRUM, absorbers: [] }];
      const split = { altitudeM: 3_000, floor: "ground" } as const;
      const body = sphereBakeBody(6_371_008.8, 9.806_65);
      const prepared = prepareDeck(column, lights, split);
      const whole = bakeDeck(prepared, body, 0);
      const parts = binShares(3).map((bins) =>
        bakeDeckBins(prepareDeck(column, lights, split, { bins }), body, 0),
      );
      const corrected = bakeDeckPolarisation(
        prepared,
        prepareDeckPolarisation(prepared, lights),
        body,
        0,
      );
      const channels = CHANNEL_WAVELENGTHS_NM.map((nm) => {
        const share = prepareDeck(column, lights, split, { bins: [THICK_BAKE_REFERENCE_BIN] });
        return deckPolarisationChannel(
          share,
          prepareDeckPolarisation(share, lights, { wavelengthsNm: [nm] }),
          body,
          0,
          0,
        );
      });
      expect(whole.below).not.toBeNull();
      expect([
        await bitsOf(gatherDeckBins(parts.toReversed())),
        await bitsOf(gatherChannels(channels.toReversed())),
      ]).toEqual([await bitsOf(whole), await bitsOf(corrected)]);
    },
  );

  it(
    "bakes the kept TEST EARTH's one band split by its bins over four workers to the bytes one worker bakes it in",
    { timeout: BAKE_TIMEOUT_MS },
    async () => {
      const ask: ThickBakeAsk = {
        input: KEPT_UNSET,
        slots: [0],
        bands: [0],
        column: keptColumnOf(EARTH_CASE),
        body: sphereBakeBody(EARTH_REFERENCE_RADIUS_M, EARTH_COLUMN.referenceGravityMS2),
        kept: referenceCaseBakeLights(EARTH_CASE),
      };
      const asked: OpticsRequest = { kind: "thick-bake", id: 1, ...ask };
      const one = (await new OpticsJobs(toyLoader()).answer(asked)).reply;
      if (one.kind !== "thick-baked") {
        throw new Error(`the whole band was answered ${one.kind}`);
      }
      const pool = new OpticsWorkerPool({
        workers: 4,
        createWorker: () => new InProcessOpticsWorker(toyLoader()),
      });
      onTestFinished(() => {
        pool.terminate();
      });
      const spread = await pool.bakeThick(ask).outcome;
      if (spread.kind !== "baked") {
        throw new Error(`the spread band ended ${spread.kind}`);
      }
      expect(await bitsOf(spread.tables)).toBe(await bitsOf(one.tables));
    },
  );
});
