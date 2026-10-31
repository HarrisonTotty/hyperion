/**
 * The optics pool's bands to the bit (plan R08, R08.T14.d's bake time): each reference world's
 * bakes asked whole of one worker, every band in one job as before the pool spread them, and spread
 * over four in-process workers, one job a band, give the same bytes: the thick bake, the
 * polarisation correction and the disc's table read from them.
 *
 * @remarks
 * The Saturn-class `saturn-oblate`, of four bands, runs always, and so does a self-luminous disc's
 * four bands on patterned J, whose thermal layers every band's glow scales. With
 * `HYPERION_SLOW_TESTS=1` every reference world runs too, each of one band and so asked as before:
 * the generated Earth, a Venus at 92 bar and at 58 bar, and the generated Titan (their lines on the
 * toy tables), and the Saturn-class NH₃ deck's four bands with the correction over each.
 */
import { describe, expect, it, onTestFinished } from "vitest";

import { planckBakeSpectrum, saturnBody, SUN_TEFF_K } from "../../test/atmosphereFixtures";
import { detailOf, recordInput, venus58Detail, venus92Detail } from "../../test/atmosphereRecords";
import { nh3DeckFixture, saturnBakeBody } from "../../test/deckFixtures";
import { InProcessOpticsWorker } from "../../test/inProcessOpticsWorker";
import { toyLoader } from "../../test/opticsFakes";
import { patternedThickBake } from "../../test/thickTables";
import { earthDetail, titanDetail } from "../../test/planetaryFixture";
import type { BodyAtmosphereInput } from "./atmosphereInput";
import { R05_EARTH_REFERENCE, R06_SOLAR_BAKE_SPECTRUM } from "./earth";
import { tableRadiusM } from "./hillaire";
import type { AtmosphereMedium } from "./medium";
import { OpticsJobs } from "./opticsJobs";
import { keptBakeLightsOf, type OpticsReply, type OpticsRequest } from "./opticsMessages";
import {
  type DeckBakeAsk,
  type DiscBakeAsk,
  OpticsWorkerPool,
  type ThickBakeAsk,
} from "./opticsPool";
import {
  SATURN_OBLATE_GRAVITY,
  SATURN_OBLATE_REFERENCE,
  SATURN_OBLATE_REFERENCE_SOURCES,
  SATURN_OBLATE_SPHEROID,
} from "./reference/cases";
import { kernelMedium } from "./tables";
import { thickBakeBands } from "./thick/bake";
import { channelSource, spectralColumnOf } from "./thick/spectralColumn";

/** Whether every reference world runs: by hand, `HYPERION_SLOW_TESTS=1`. */
function slowTests(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_SLOW_TESTS"] === "1";
}

const SLOW = slowTests();

/** The workers the bakes are spread over: the UHD 620 host's pool. */
const SPREAD = 4;

// `saturn-oblate`'s bakes, whole and spread, take some 17 s on a pod's thread (2026-10-31): ten
// times that, rounded up.
const WORLD_TIMEOUT_MS = 300_000;

/**
 * A column isothermal at 1,500 K up to R05's Earth's top: a glow whose green and blue are
 * negative, as a cool one's are, so that every band's thermal layer is held over one scale.
 */
const GLOWING = {
  heightsM: Float64Array.of(0, R05_EARTH_REFERENCE.topHeightM),
  temperaturesK: Float64Array.of(1_500, 1_500),
};

/** A world as its bakes are asked: the thick bake's fields, and what its disc marches with. */
interface World {
  readonly ask: ThickBakeAsk;
  readonly medium: AtmosphereMedium;
  readonly curves: DiscBakeAsk["air"]["curves"];
}

const KEPT_UNSET: BodyAtmosphereInput = { kind: "kept", atmosphere: { kind: "unset" } };

/** `saturn-oblate`, a kept body of four bands under R06's Sun. */
function saturnOblate(): World {
  const body = {
    figure: SATURN_OBLATE_SPHEROID,
    gravity: SATURN_OBLATE_GRAVITY,
    bottomRadiusM: tableRadiusM(SATURN_OBLATE_SPHEROID),
  };
  return {
    ask: {
      input: KEPT_UNSET,
      slots: [0],
      bands: thickBakeBands(body).map((_, band) => band),
      column: spectralColumnOf(SATURN_OBLATE_REFERENCE, SATURN_OBLATE_REFERENCE_SOURCES),
      body,
      kept: keptBakeLightsOf(SATURN_OBLATE_REFERENCE, [R06_SOLAR_BAKE_SPECTRUM]),
    },
    medium: SATURN_OBLATE_REFERENCE,
    curves: [],
  };
}

/** A record's world as the atmosphere cache asks its bakes, its medium as `pool` builds it. */
async function recordWorld(pool: OpticsWorkerPool, input: BodyAtmosphereInput): Promise<World> {
  const built = await pool.build(input).outcome;
  if (built.kind !== "built" || built.assembled.medium === undefined) {
    throw new Error(`the record was built ${built.kind} with no medium`);
  }
  const { medium, drawn } = built.assembled;
  const body = { ...drawn.body, bottomRadiusM: drawn.bottomRadiusM };
  return {
    ask: {
      input,
      slots: drawn.spectra.map((_, slot) => slot),
      bands: thickBakeBands(body).map((_, band) => band),
      column: spectralColumnOf(medium, drawn.sources, drawn.temperature),
      body,
    },
    medium,
    curves: drawn.curves,
  };
}

/** A pool of `count` in-process workers on the toy line tables, terminated when the test ends. */
function inProcessPool(count: number): OpticsWorkerPool {
  const pool = new OpticsWorkerPool({
    workers: count,
    createWorker: () => new InProcessOpticsWorker(toyLoader()),
  });
  onTestFinished(() => {
    pool.terminate();
  });
  return pool;
}

/**
 * SHA-256 of every number and typed array's bytes `value` holds, in its order, with its keys and
 * strings: the same bits give the same digest.
 */
async function bitsOf(value: unknown): Promise<string> {
  const chunks: Uint8Array[] = [];
  const text = new TextEncoder();
  const walk = (item: unknown): void => {
    if (ArrayBuffer.isView(item)) {
      chunks.push(text.encode(`${item.constructor.name}:`));
      chunks.push(new Uint8Array(item.buffer, item.byteOffset, item.byteLength));
    } else if (typeof item === "number") {
      chunks.push(new Uint8Array(Float64Array.of(item).buffer));
    } else if (Array.isArray(item)) {
      chunks.push(text.encode(`[${item.length}`));
      item.forEach(walk);
    } else if (typeof item === "object" && item !== null) {
      for (const [key, member] of Object.entries(item)) {
        chunks.push(text.encode(`{${key}`));
        walk(member);
      }
    } else {
      chunks.push(text.encode(String(item)));
    }
  };
  walk(value);
  const bytes = new Uint8Array(chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0));
  let at = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, at);
    at += chunk.byteLength;
  }
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

/** A reply that is not the kind asked, as a test failure. */
function unexpected(request: OpticsRequest, reply: OpticsReply): Error {
  return new Error(`a ${request.kind} was answered ${reply.kind}: ${JSON.stringify(reply)}`);
}

/** A bake that did not bake, as a test failure. */
function unbaked(outcome: { readonly kind: string }): Error {
  return new Error(`a bake ended ${outcome.kind}: ${JSON.stringify(outcome)}`);
}

/** The digests of a world's thick bake, correction and disc's table, asked whole of one worker. */
async function whole(world: World): Promise<ReadonlyArray<string>> {
  const jobs = new OpticsJobs(toyLoader());
  const { ask } = world;
  const thickAsked: OpticsRequest = { kind: "thick-bake", id: 1, ...ask };
  const thick = (await jobs.answer(thickAsked)).reply;
  if (thick.kind !== "thick-baked") {
    throw unexpected(thickAsked, thick);
  }
  const correctionAsked: OpticsRequest = { kind: "polarisation-bake", id: 2, ...ask };
  const correction = (await jobs.answer(correctionAsked)).reply;
  if (correction.kind !== "polarisation-baked") {
    throw unexpected(correctionAsked, correction);
  }
  const air = {
    medium: world.medium,
    kernelMedium: kernelMedium(world.medium),
    curves: world.curves,
    bake: thick.tables,
    correction: correction.tables,
  };
  const discAsked: OpticsRequest = { kind: "disc-bake", id: 3, ...ask, air };
  const disc = (await jobs.answer(discAsked)).reply;
  if (disc.kind !== "disc-baked") {
    throw unexpected(discAsked, disc);
  }
  return Promise.all([bitsOf(thick.tables), bitsOf(correction.tables), bitsOf(disc.table)]);
}

/** The same digests, the bakes spread over {@link SPREAD} workers, one job a band. */
async function spread(world: World, pool: OpticsWorkerPool): Promise<ReadonlyArray<string>> {
  const { ask } = world;
  const [thick, correction] = await Promise.all([
    pool.bakeThick(ask).outcome,
    pool.bakePolarisation(ask).outcome,
  ]);
  if (thick.kind !== "baked") {
    throw unbaked(thick);
  }
  if (correction.kind !== "baked") {
    throw unbaked(correction);
  }
  const air = {
    medium: world.medium,
    kernelMedium: kernelMedium(world.medium),
    curves: world.curves,
    bake: thick.tables,
    correction: correction.tables,
  };
  const disc = await pool.bakeDisc({ ...ask, air }).outcome;
  if (disc.kind !== "baked") {
    throw unbaked(disc);
  }
  return Promise.all([bitsOf(thick.tables), bitsOf(correction.tables), bitsOf(disc.table)]);
}

describe("the optics pool's bands, to the bit (R08.T14.d)", () => {
  it(
    "bakes saturn-oblate's four bands spread over four workers to the bytes one worker bakes them in",
    { timeout: WORLD_TIMEOUT_MS },
    async () => {
      const world = saturnOblate();
      expect(world.ask.bands).toHaveLength(4);
      expect(await spread(world, inProcessPool(SPREAD))).toEqual(await whole(world));
    },
  );

  it(
    "gathers a self-luminous disc's four bands spread over four workers to the bytes one worker bakes it in",
    { timeout: WORLD_TIMEOUT_MS },
    async () => {
      const body = { ...saturnBody(), bottomRadiusM: 6_371_008.8 };
      const bands = thickBakeBands(body);
      const ask: DiscBakeAsk = {
        input: KEPT_UNSET,
        slots: [0],
        bands: bands.map((_, band) => band),
        column: spectralColumnOf(R05_EARTH_REFERENCE, R05_EARTH_REFERENCE.terms.map(channelSource)),
        body,
        air: {
          medium: R05_EARTH_REFERENCE,
          kernelMedium: kernelMedium(R05_EARTH_REFERENCE),
          curves: [],
          bake: patternedThickBake(bands, R05_EARTH_REFERENCE, 1),
          temperature: GLOWING,
        },
      };
      const asked: OpticsRequest = { kind: "disc-bake", id: 1, ...ask };
      const one = (await new OpticsJobs(toyLoader()).answer(asked)).reply;
      if (one.kind !== "disc-baked") {
        throw unexpected(asked, one);
      }
      const many = await inProcessPool(SPREAD).bakeDisc(ask).outcome;
      if (many.kind !== "baked") {
        throw unbaked(many);
      }
      expect(one.table.thermalScale).toBeGreaterThan(0);
      expect(await bitsOf(many.table)).toBe(await bitsOf(one.table));
    },
  );

  it.skipIf(!SLOW).each([
    ["the generated Earth", () => recordInput(detailOf(earthDetail()))],
    ["a Venus at 92 bar", () => recordInput(venus92Detail())],
    ["a Venus at 58 bar", () => recordInput(venus58Detail())],
    ["the generated Titan", () => recordInput(detailOf(titanDetail()))],
  ])(
    "bakes %s spread over four workers to the bytes one worker bakes it in",
    { timeout: WORLD_TIMEOUT_MS },
    async (_, input) => {
      const pool = inProcessPool(SPREAD);
      const world = await recordWorld(pool, input());
      expect(await spread(world, pool)).toEqual(await whole(world));
    },
  );

  it.skipIf(!SLOW)(
    "bakes the Saturn-class NH₃ deck's four bands and their corrections spread over four workers to the bytes one worker bakes them in",
    { timeout: WORLD_TIMEOUT_MS },
    async () => {
      const fixture = nh3DeckFixture();
      const body = saturnBakeBody();
      const ask: DeckBakeAsk = {
        input: KEPT_UNSET,
        slots: [0],
        bands: thickBakeBands(body).map((_, band) => band),
        column: fixture.column,
        body,
        kept: { absorbers: [], spectra: [planckBakeSpectrum(SUN_TEFF_K)] },
        split: { altitudeM: fixture.deckAltitudeM, floor: "envelope" },
      };
      const asked: OpticsRequest = { kind: "deck-bake", id: 1, ...ask };
      const one = (await new OpticsJobs(toyLoader()).answer(asked)).reply;
      if (one.kind !== "deck-baked") {
        throw unexpected(asked, one);
      }
      const many = await inProcessPool(SPREAD).bakeDeck(ask).outcome;
      if (many.kind !== "baked") {
        throw unbaked(many);
      }
      expect(ask.bands).toHaveLength(4);
      expect(await bitsOf([many.tables, many.correction])).toBe(
        await bitsOf([one.tables, one.correction]),
      );
    },
  );
});
