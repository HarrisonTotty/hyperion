/**
 * The atmosphere cache (plan R08, R08.T10.a; `decision-r08-t10a-leans.md` items 3, 4, 6, 8–10 and
 * the follow-up to item 6): its requests and their triggers, `ATMOSPHERE: PENDING` and its hold,
 * the answers it cannot draw, failed builds, its size, kept scenes, its views and its disposal.
 * The boundaries are faked: a real `RequestClient` over a recording send, a scripted builder in
 * the optics pool's place, a fake tables factory and fake timers; the last test runs the real pool
 * on in-process workers.
 */
import type { BodyIdHex, ErrorCode } from "@hyperion/protocol";
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  type MockInstance,
  onTestFinished,
  vi,
} from "vitest";

import { toBodyDetail } from "../../lib/system/bodiesWire";
import type { BodyDetail } from "../../lib/system/model";
import { earthBody, saturnBody, venusClassMedium } from "../../test/atmosphereFixtures";
import { type DetailAnswer, SUN_LIGHT, withRecord } from "../../test/atmosphereRecords";
import { handJupiterRecord } from "../../test/giantFixtures";
import { FakeTables } from "../../test/atmosphereCacheFakes";
import { sweepMedium } from "../../test/atmosphereSweep";
import { BodyDetailSeam } from "../../test/bodyDetailSeam";
import { InProcessOpticsWorker } from "../../test/inProcessOpticsWorker";
import { ignore, microtasks, NOT_MODELLED, toyLoader } from "../../test/opticsFakes";
import {
  earthDetail,
  earthDetailOfAnOlderServer,
  FIXTURE_EARTH,
  FIXTURE_SYSTEM,
  FIXTURE_UNIVERSE,
  lavaWorldDetail,
} from "../../test/planetaryFixture";
import { RENDERER_ROOT, runTimeGraph } from "../../test/runTimeGraph";
import { ScriptedOpticsWorker } from "../../test/scriptedOpticsWorker";
import { between, seededRandom } from "../../test/seededRandom";
import {
  patternedCorrection,
  patternedDeckCorrection,
  patternedDeckTables,
  patternedThickBake,
} from "../../test/thickTables";
import { heldAppearanceOf } from "../appearance/fromWire";
import type { LitRegime } from "../bodies/regime";
import { EngineUnavailable } from "../engine/resilientEngine";
import { builtOfCase } from "../scenes/atmosphereScene";
import { ANNUNCIATION_CLEAR_MS, ANNUNCIATION_ONSET_MS } from "../terrain/annunciation";
import { type AssembledMedium, assembleMedium, mediumTableRadiusM } from "./assemble";
import { groundTemperatureFieldOf } from "./thermal";
import {
  ATMOSPHERE_CACHE_BYTES,
  AtmosphereCache,
  type AtmosphereFrameBody,
  type AtmosphereObserver,
  type AtmosphereScene,
  type AtmosphereView,
  type AtmosphereViewReport,
  type BuiltAtmosphere,
  type KeptBodyAtmosphere,
  PendingLineHold,
  POLARISATION_REASON,
} from "./AtmosphereCache";
import {
  atmosphereKey,
  type BodyAtmosphereInput,
  type BodyLight,
  clampedGroundAlbedo,
} from "./atmosphereInput";
import { EARTH_COLUMN, R05_EARTH_REFERENCE } from "./earth";
import type { AtmosphereMedium, MediumTerm } from "./medium";
import { ATMOSPHERE_ANSWER_UNREADABLE, type AtmosphereLabel } from "./labels";
import { keptBakeLightsOf, type OpticsRequest } from "./opticsMessages";
import {
  type AtmosphereBuilder,
  type DeckBake,
  type DeckBakeAsk,
  type DeckBakeOutcome,
  MAX_CONSECUTIVE_OPTICS_FAILURES,
  type OpticsBuild,
  type OpticsBuildOutcome,
  OpticsWorkerPool,
  type PolarisationBake,
  type PolarisationBakeOutcome,
  type ThickBake,
  type ThickBakeAsk,
  type ThickBakeOutcome,
  type DiscBake,
  type DiscBakeAsk,
  type DiscBakeOutcome,
} from "./opticsPool";
import {
  EARTH_CASE,
  EARTH_REFERENCE_RADIUS_M,
  MARS_CASE,
  referenceCaseBakeLights,
} from "./reference/cases";
import { type AtmosphereBody, kernelMedium, perPlanetRowOf, tablesPlanOf } from "./tables";
import { DISC_LAYER_VALUES, type DiscReflectanceTable } from "./discTable";
import { type MsSourceTable, thickBakeBands } from "./thick/bake";
import { spectralColumnOf } from "./thick/columnLayout";
import type { PolarisationTable } from "./thick/polarisation";
import { keptColumnOf, sphereBakeBody } from "./thick/keptBake";
import {
  type AtmosphereRegime,
  type BodyCover,
  envelopeFloorM,
  THICK_REASON,
} from "./thick/regime";
import { correctionRouted, TWILIGHT_REASON } from "./thick/routing";
import { channelSource, type SpectralColumn } from "./thick/spectralColumn";

/** A build the scripted builder holds, landed by the test. */
interface ScriptedBuild {
  readonly input: BodyAtmosphereInput;
  readonly key: string;
  settled: boolean;
  readonly settle: (outcome: OpticsBuildOutcome) => void;
}

/** A thick bake the scripted builder holds, landed by the test (R08.T14.c). */
interface ScriptedBake {
  readonly ask: ThickBakeAsk;
  settled: boolean;
  cancelled: boolean;
  readonly settle: (outcome: ThickBakeOutcome) => void;
}

/** A deck's bake the scripted builder holds, landed by the test (R08.T15.a.2). */
interface ScriptedDeck {
  readonly ask: DeckBakeAsk;
  settled: boolean;
  readonly settle: (outcome: DeckBakeOutcome) => void;
}

/** A polarisation correction the scripted builder holds, landed by the test (R08.T14.f). */
interface ScriptedCorrection {
  readonly ask: ThickBakeAsk;
  settled: boolean;
  cancelled: boolean;
  readonly settle: (outcome: PolarisationBakeOutcome) => void;
}

/** A disc table the scripted builder holds, landed by the test (R08.T16.b). */
interface ScriptedDisc {
  readonly ask: DiscBakeAsk;
  settled: boolean;
  cancelled: boolean;
  readonly settle: (outcome: DiscBakeOutcome) => void;
}

/**
 * A builder in the optics pool's place, whose builds, thick bakes, polarisation corrections and
 * disc tables the test lands, fails or leaves running.
 */
class ScriptedBuilder implements AtmosphereBuilder {
  readonly builds: ScriptedBuild[] = [];
  readonly bakes: ScriptedBake[] = [];
  readonly corrections: ScriptedCorrection[] = [];
  readonly discs: ScriptedDisc[] = [];
  readonly decks: ScriptedDeck[] = [];
  terminated = false;

  bakeDisc(ask: DiscBakeAsk): DiscBake {
    let resolve: (outcome: DiscBakeOutcome) => void = ignore;
    const outcome = new Promise<DiscBakeOutcome>((settleWith) => {
      resolve = settleWith;
    });
    const disc: ScriptedDisc = {
      ask,
      settled: false,
      cancelled: false,
      settle: (result) => {
        disc.settled = true;
        resolve(result);
      },
    };
    this.discs.push(disc);
    return {
      outcome,
      cancel: () => {
        disc.cancelled = true;
        disc.settle({ kind: "cancelled" });
      },
    };
  }

  bakeDeck(ask: DeckBakeAsk): DeckBake {
    let resolve: (outcome: DeckBakeOutcome) => void = ignore;
    const outcome = new Promise<DeckBakeOutcome>((settleWith) => {
      resolve = settleWith;
    });
    const deck: ScriptedDeck = {
      ask,
      settled: false,
      settle: (result) => {
        deck.settled = true;
        resolve(result);
      },
    };
    this.decks.push(deck);
    return {
      outcome,
      cancel: () => {
        deck.settle({ kind: "cancelled" });
      },
    };
  }

  bakePolarisation(ask: ThickBakeAsk): PolarisationBake {
    let resolve: (outcome: PolarisationBakeOutcome) => void = ignore;
    const outcome = new Promise<PolarisationBakeOutcome>((settleWith) => {
      resolve = settleWith;
    });
    const correction: ScriptedCorrection = {
      ask,
      settled: false,
      cancelled: false,
      settle: (result) => {
        correction.settled = true;
        resolve(result);
      },
    };
    this.corrections.push(correction);
    return {
      outcome,
      cancel: () => {
        correction.cancelled = true;
        correction.settle({ kind: "cancelled" });
      },
    };
  }

  bakeThick(ask: ThickBakeAsk): ThickBake {
    let resolve: (outcome: ThickBakeOutcome) => void = ignore;
    const outcome = new Promise<ThickBakeOutcome>((settleWith) => {
      resolve = settleWith;
    });
    const bake: ScriptedBake = {
      ask,
      settled: false,
      cancelled: false,
      settle: (result) => {
        bake.settled = true;
        resolve(result);
      },
    };
    this.bakes.push(bake);
    return {
      outcome,
      cancel: () => {
        bake.cancelled = true;
        bake.settle({ kind: "cancelled" });
      },
    };
  }

  build(input: BodyAtmosphereInput): OpticsBuild {
    let resolve: (outcome: OpticsBuildOutcome) => void = ignore;
    const outcome = new Promise<OpticsBuildOutcome>((settleWith) => {
      resolve = settleWith;
    });
    const build: ScriptedBuild = {
      input,
      key: atmosphereKey(input),
      settled: false,
      settle: (result) => {
        build.settled = true;
        resolve(result);
      },
    };
    this.builds.push(build);
    return {
      outcome,
      cancel: () => {
        build.settle({ kind: "cancelled" });
      },
    };
  }

  terminate(): void {
    this.terminated = true;
    for (const build of this.builds) {
      build.settle({ kind: "cancelled" });
    }
    for (const bake of this.bakes) {
      bake.settle({ kind: "cancelled" });
    }
    for (const correction of this.corrections) {
      correction.settle({ kind: "cancelled" });
    }
    for (const deck of this.decks) {
      deck.settle({ kind: "cancelled" });
    }
  }

  /** The builds asked for `body`, oldest first. */
  buildsOf(body: BodyIdHex): ScriptedBuild[] {
    return this.builds.filter(
      (build) => build.input.kind === "record" && build.input.detail.record.id === body,
    );
  }

  /**
   * The latest build of `body`.
   *
   * @throws Error where none was asked: a test's bug.
   */
  latestOf(body: BodyIdHex): ScriptedBuild {
    const build = this.buildsOf(body).at(-1);
    if (build === undefined) {
      throw new Error(`no build was asked for ${body}`);
    }
    return build;
  }
}

/** What a test drives. */
interface Harness {
  readonly seam: BodyDetailSeam;
  readonly builder: ScriptedBuilder;
  readonly made: FakeTables[];
  /** The light slots of the thick table the factory was told of, one a tables made. */
  readonly thickSlots: number[];
  readonly cache: AtmosphereCache<FakeTables>;
  readonly view: AtmosphereView;
  /** The fake factory's behaviour: what it throws while set. */
  readonly factory: { throws: Error | null };
}

/** No route and no correction: a harness's defaults, so that a test asks only what it tests. */
function routesNone(): boolean {
  return false;
}

/**
 * A cache over the fakes, its tables reporting `rowBytes` each, or their per-planet row's, routing
 * its thick bakes by `routes`, or none if absent, and asking polarisation corrections by
 * `corrects`, or none if absent; `product`, the product's routing and corrections in their place
 * (R08.T14.d.4's route: an in-range thick world to its bake, every world with air its correction
 * but an unrouted thick one).
 */
function harness(
  rowBytes?: number,
  routes: ((regime: AtmosphereRegime) => boolean) | "product" = routesNone,
  corrects: ((column: SpectralColumn) => boolean) | "product" = routesNone,
): Harness {
  const seam = new BodyDetailSeam();
  const builder = new ScriptedBuilder();
  const made: FakeTables[] = [];
  const thickSlots: number[] = [];
  const factory: { throws: Error | null } = { throws: null };
  const cache = new AtmosphereCache<FakeTables>({
    requests: seam.client,
    builder,
    ...(routes === "product" ? {} : { routes }),
    ...(corrects === "product" ? {} : { corrects }),
    tables: (built, slots) => {
      if (factory.throws !== null) {
        throw factory.throws;
      }
      thickSlots.push(slots);
      const row = perPlanetRowOf(
        built.medium,
        built.curves,
        tablesPlanOf(built.body, built.bottomRadiusM),
      );
      const tables = new FakeTables(built, rowBytes ?? row.baseBytes);
      made.push(tables);
      return tables;
    },
  });
  return { seam, builder, made, thickSlots, cache, view: cache.openView(), factory };
}

/** The scene's designation of record. */
const DESIGNATION = "ATMOSPHERE TEST SYSTEM";

/** The scene's arrival, by identity. */
const ARRIVAL = {};

/** The fixture's scene, its link up. */
function sceneOf(change: Partial<AtmosphereScene> = {}): AtmosphereScene {
  return {
    universe: FIXTURE_UNIVERSE,
    system: FIXTURE_SYSTEM,
    designation: DESIGNATION,
    time: earthDetail().time,
    arrival: ARRIVAL,
    grants: new Map(),
    linkUp: true,
    ...change,
  };
}

/** A body of the fixture's system other than its Earth. */
function bodyId(n: number): BodyIdHex {
  return `${FIXTURE_SYSTEM}.${(0x0400 + n).toString(16).padStart(4, "0")}`;
}

/** A frame's body in `regime`, lit by the Sun. */
function lit(
  id: BodyIdHex,
  regime: LitRegime = "disc",
  lights: ReadonlyArray<BodyLight> = [SUN_LIGHT],
): AtmosphereFrameBody {
  return { id, regime, lights };
}

/** A view's frame of `bodies`, at `nowMs`, in `scene`. */
function note(
  h: Harness,
  bodies: ReadonlyArray<AtmosphereFrameBody>,
  nowMs = 0,
  scene: AtmosphereScene | null = sceneOf(),
): AtmosphereViewReport {
  return h.view.noteFrame({ nowMs, scene, bodies });
}

/** The fixture's answer for `body`: an older server's Earth, its surface not modelled. */
function answerFor(
  body: BodyIdHex,
  base: DetailAnswer = earthDetailOfAnOlderServer(),
): DetailAnswer {
  return withRecord(base, (record) => ({ ...record, id: body }));
}

/** Answers `body`'s latest request and lets the answer be taken. */
async function answer(h: Harness, body: BodyIdHex, answered = answerFor(body)): Promise<void> {
  h.seam.answer(h.seam.latestFor(body).id, answered);
  await microtasks();
}

/** Lands `body`'s latest build and lets it be taken. */
async function land(h: Harness, body: BodyIdHex, outcome: OpticsBuildOutcome): Promise<void> {
  h.builder.latestOf(body).settle(outcome);
  await microtasks();
}

/** R05's Earth as an assembly, one light and no absorber, its regime as given. */
function earthAssembly(
  labels: ReadonlyArray<AtmosphereLabel> = [],
  regime: AtmosphereRegime = { kind: "thin" },
): AssembledMedium {
  return {
    medium: R05_EARTH_REFERENCE,
    drawn: {
      body: earthBody(),
      bottomRadiusM: 6_371_008.8,
      absorbers: [],
      spectra: [SUN_LIGHT.spectrum],
      curves: [[]],
      sources: [],
    },
    labels,
    reasons: [],
    negligible: [],
    regime,
  };
}

/** A routing whose `thickScattering` gate passes over every medium (R08.T14.c). */
function routedThick(regime: AtmosphereRegime): boolean {
  return regime.kind === "thickScattering";
}

/** An NH₄SH stand-in's reason, which a thick bake does not clear (R08.T14.c). */
const STAND_IN = "NH4SH: drawn with a stated stand-in";

/**
 * R05's Earth routed `thickScattering`, as a server body's assembly under `atmosphereApproximate`
 * with `reasons`, its terms' sources by their channels, on `body`'s figure and lit by `lights`
 * (R08.T14.c).
 */
function thickAssembly(
  reasons: ReadonlyArray<string> = [THICK_REASON],
  body: AtmosphereBody = earthBody(),
  lights: ReadonlyArray<BodyLight> = [SUN_LIGHT],
): AssembledMedium {
  return {
    medium: R05_EARTH_REFERENCE,
    drawn: {
      body,
      bottomRadiusM: mediumTableRadiusM(body.figure),
      absorbers: [],
      spectra: lights.map((light) => light.spectrum),
      curves: lights.map(() => []),
      sources: R05_EARTH_REFERENCE.terms.map(channelSource),
    },
    labels: ["atmosphereApproximate"],
    reasons,
    negligible: [],
    regime: { kind: "thickScattering" },
  };
}

/** Asks for, answers, builds and draws `bodies` in one view's frames, at `nowMs`. */
async function drawn(
  h: Harness,
  bodies: ReadonlyArray<AtmosphereFrameBody>,
  assembled: AssembledMedium = earthAssembly(),
  nowMs = 0,
): Promise<AtmosphereViewReport> {
  note(h, bodies, nowMs);
  for (const { id } of bodies) {
    h.seam.answer(h.seam.latestFor(id).id, answerFor(id));
  }
  await microtasks();
  for (const { id } of bodies) {
    h.builder.latestOf(id).settle({ kind: "built", assembled });
  }
  await microtasks();
  return note(h, bodies, nowMs);
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

const A = FIXTURE_EARTH;

describe("the atmosphere cache's requests", () => {
  it("sends one body_detail for each body first wanted in the disc, mesh or terrain regime, and none for a point", () => {
    const h = harness();
    const bodies = [lit(bodyId(1)), lit(bodyId(2), "mesh"), lit(bodyId(3), "terrain")];
    const point = lit(bodyId(4), "point");
    note(h, [...bodies, point]);
    note(h, [...bodies, point]);
    expect([1, 2, 3, 4].map((n) => h.seam.askedFor(bodyId(n)))).toEqual([1, 1, 1, 0]);
  });

  it("asks again once when the scene's arrival changes", async () => {
    const h = harness();
    await drawn(h, [lit(A)]);
    const arrival = sceneOf({ arrival: {} });
    note(h, [lit(A)], 0, arrival);
    note(h, [lit(A)], 0, arrival);
    expect(h.seam.askedFor(A)).toBe(2);
  });

  it("asks again once when the body's granted detail changes", async () => {
    const h = harness();
    note(h, [lit(A)], 0, sceneOf({ grants: new Map([[A, "surface"]]) }));
    await answer(h, A);
    const granted = sceneOf({ grants: new Map([[A, "full"]]) });
    note(h, [lit(A)], 0, granted);
    note(h, [lit(A)], 0, granted);
    expect(h.seam.askedFor(A)).toBe(2);
  });

  it("asks again once at the next frame after an event on the body", async () => {
    const h = harness();
    await drawn(h, [lit(A)]);
    h.cache.noteBodyEvent(A);
    note(h, [lit(A)]);
    note(h, [lit(A)]);
    expect(h.seam.askedFor(A)).toBe(2);
  });

  describe("after a lost link", () => {
    /** A cache whose body's request was lost with the link. */
    async function lostLink(): Promise<Harness> {
      const h = harness();
      note(h, [lit(A)]);
      h.seam.loseLink();
      await microtasks();
      return h;
    }

    it("sends nothing while the link is down", async () => {
      const h = await lostLink();
      note(h, [lit(A)], 0, sceneOf({ linkUp: false }));
      expect(h.seam.askedFor(A)).toBe(1);
    });

    it("asks again once when the link is back up", async () => {
      const h = await lostLink();
      note(h, [lit(A)], 0, sceneOf({ linkUp: false }));
      note(h, [lit(A)]);
      note(h, [lit(A)]);
      expect(h.seam.askedFor(A)).toBe(2);
    });

    it("keeps the body pending while it waits", async () => {
      const h = await lostLink();
      note(h, [lit(A)], 0, sceneOf({ linkUp: false }));
      expect(h.cache.labelsOf(A)).toEqual(["atmospherePending"]);
    });
  });

  it("rebuilds from the reply held, with no request, when the body's lights change", async () => {
    const h = harness();
    await drawn(h, [lit(A)]);
    const second: BodyLight = { star: `${FIXTURE_SYSTEM}.0101`, spectrum: SUN_LIGHT.spectrum };
    note(h, [lit(A, "disc", [SUN_LIGHT, second])]);
    const rebuilt = h.builder.latestOf(A).input;
    expect([
      h.seam.askedFor(A),
      h.builder.buildsOf(A).length,
      rebuilt.kind === "record" ? rebuilt.lights : [],
    ]).toEqual([1, 2, [SUN_LIGHT, second]]);
  });

  it("keeps its entry and builds nothing when a re-request's answer has the drawn entry's key", async () => {
    const h = harness();
    await drawn(h, [lit(A)]);
    const entry = h.cache.entryOf(A);
    note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
    await answer(h, A);
    expect([h.builder.buildsOf(A).length, h.made.length, h.cache.entryOf(A)]).toEqual([
      1,
      1,
      entry,
    ]);
  });

  it("rebuilds when a re-request's answer changes the body's inventory, so its key", async () => {
    const h = harness();
    await drawn(h, [lit(A)]);
    note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
    await answer(h, A, answerFor(A, earthDetail()));
    expect(h.builder.buildsOf(A).map((build) => build.key)).toEqual([
      atmosphereKey(inputOf(answerFor(A))),
      atmosphereKey(inputOf(answerFor(A, earthDetail()))),
    ]);
  });

  it("forms a record's input with its ground's albedo p q per channel, clamped to [0, 1]", async () => {
    const h = harness();
    note(h, [lit(A)]);
    await answer(h, A, earthDetail());
    expect(h.builder.latestOf(A).input).toEqual(inputOf(earthDetail()));
  });
});

describe("the ground's temperature field (R08.T18.a)", () => {
  it("is none before a body's record is held", () => {
    const h = harness();
    note(h, [lit(A)]);
    expect(h.cache.groundFieldOf(A)).toBeNull();
  });

  it("is the held record's surface's field once its reply is taken", async () => {
    const h = harness();
    note(h, [lit(A)]);
    await answer(h, A, answerFor(A, lavaWorldDetail()));
    const converted = toBodyDetail(answerFor(A, lavaWorldDetail()), FIXTURE_SYSTEM, DESIGNATION);
    if (converted.kind !== "ok" || converted.detail.record.surface.state !== "ok") {
      throw new Error("the lava world's record converts, its surface ok");
    }
    expect(h.cache.groundFieldOf(A)).toEqual(
      groundTemperatureFieldOf(converted.detail.record.surface.value),
    );
  });

  it("is a body with a medium's ground's too, seen through its air (R08.T18.b)", async () => {
    const h = harness();
    note(h, [lit(A)]);
    await answer(h, A, answerFor(A, earthDetail()));
    const converted = toBodyDetail(answerFor(A, earthDetail()), FIXTURE_SYSTEM, DESIGNATION);
    if (converted.kind !== "ok" || converted.detail.record.surface.state !== "ok") {
      throw new Error("the Earth fixture's record converts, its surface ok");
    }
    expect(h.cache.groundFieldOf(A)).toEqual(
      groundTemperatureFieldOf(converted.detail.record.surface.value),
    );
  });

  it("reads a reply's field once, however many frames ask it", async () => {
    const h = harness();
    note(h, [lit(A)]);
    await answer(h, A, answerFor(A, lavaWorldDetail()));
    expect(h.cache.groundFieldOf(A)).toBe(h.cache.groundFieldOf(A));
  });
});

/** The input the cache forms from an answer, lit by the Sun, as a test states it. */
function inputOf(answered: DetailAnswer): BodyAtmosphereInput {
  const converted = toBodyDetail(answered, FIXTURE_SYSTEM, DESIGNATION);
  if (converted.kind !== "ok") {
    throw new Error(`the answer does not convert: ${converted.fault}`);
  }
  const { detail } = converted;
  const { geometricAlbedo: p, phaseIntegral: q } = heldAppearanceOf(detail.record).photometry;
  return {
    kind: "record",
    detail,
    lights: [SUN_LIGHT],
    groundAlbedo: clampedGroundAlbedo([p[0] * q[0], p[1] * q[1], p[2] * q[2]]),
  };
}

describe("ATMOSPHERE: PENDING", () => {
  it("is a body's while its request is in flight and while it builds, and its labels once drawn", async () => {
    const h = harness();
    note(h, [lit(A)]);
    const asked = h.cache.labelsOf(A);
    await answer(h, A);
    const building = h.cache.labelsOf(A);
    await land(h, A, { kind: "built", assembled: NOT_MODELLED });
    note(h, [lit(A)]);
    expect([asked, building, h.cache.labelsOf(A)]).toEqual([
      ["atmospherePending"],
      ["atmospherePending"],
      ["atmosphereNotYetModelled"],
    ]);
  });

  it("is none of a body's on a re-request while its previous state is drawn", async () => {
    const h = harness();
    await drawn(h, [lit(A)], NOT_MODELLED);
    const report = note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
    expect([h.seam.askedFor(A), h.cache.labelsOf(A), report.pending]).toEqual([
      2,
      ["atmosphereNotYetModelled"],
      false,
    ]);
  });

  it("shows no line for a body drawn within 250 ms", async () => {
    const h = harness();
    const shown: boolean[] = [];
    shown.push(note(h, [lit(A)], 0).labels.includes("atmospherePending"));
    await answer(h, A);
    await land(h, A, { kind: "built", assembled: NOT_MODELLED });
    for (const nowMs of [200, 249, 400, 2_000]) {
      shown.push(note(h, [lit(A)], nowMs).labels.includes("atmospherePending"));
    }
    expect(shown).toEqual([false, false, false, false, false]);
  });

  it("shows the line once a body has been pending 250 ms, and clears it 1 s after it is drawn", async () => {
    const h = harness();
    const at = (nowMs: number): boolean =>
      note(h, [lit(A)], nowMs).labels.includes("atmospherePending");
    const before = [at(0), at(ANNUNCIATION_ONSET_MS - 1), at(ANNUNCIATION_ONSET_MS)];
    await answer(h, A);
    await land(h, A, { kind: "built", assembled: NOT_MODELLED });
    const drawnAt = 400;
    const after = [
      at(drawnAt),
      at(drawnAt + ANNUNCIATION_CLEAR_MS - 1),
      at(drawnAt + ANNUNCIATION_CLEAR_MS),
    ];
    expect([before, after]).toEqual([
      [false, false, true],
      [true, true, false],
    ]);
  });

  it("comes on at most once in any 1.25 s through a flyby, however the bodies and replies are timed", async () => {
    const h = harness();
    const random = seededRandom(0x0a7e_5f1b);
    const count = 48;
    let t = 0;
    // Bodies come into the disc in bursts and lulls, and most replies are quick, so that a body
    // pending comes and goes many times a second, as a line drawn from it unheld would.
    const flyby = Array.from({ length: count }, (_, n) => {
      t += random() < 0.25 ? between(random, 600, 2_500) : between(random, 10, 300);
      return {
        id: bodyId(n + 1),
        entersMs: t,
        leavesMs: t + between(random, 150, 4_000),
        replyAfterMs: random() < 0.8 ? between(random, 0, 300) : between(random, 300, 2_000),
      };
    });
    const askedAt = new Map<number, number>();
    const answered = new Set<number>();
    const onsets: number[] = [];
    let pendingOnsets = 0;
    let wasPending = false;
    let shown = false;
    const frame = async (nowMs: number): Promise<void> => {
      for (const asked of h.seam.asked) {
        if (!askedAt.has(asked.id)) {
          askedAt.set(asked.id, nowMs);
        }
        const body = flyby.find((each) => each.id === asked.body);
        const since = askedAt.get(asked.id) ?? nowMs;
        if (body !== undefined && !answered.has(asked.id) && nowMs - since >= body.replyAfterMs) {
          answered.add(asked.id);
          h.seam.answer(asked.id, answerFor(asked.body));
        }
      }
      await microtasks();
      for (const build of h.builder.builds) {
        if (!build.settled) {
          build.settle({ kind: "built", assembled: NOT_MODELLED });
        }
      }
      await microtasks();
      const inDisc = flyby.filter((body) => nowMs >= body.entersMs && nowMs < body.leavesMs);
      const report = note(
        h,
        inDisc.map((body) => lit(body.id)),
        nowMs,
      );
      const on = report.labels.includes("atmospherePending");
      if (on && !shown) {
        onsets.push(nowMs);
      }
      if (report.pending && !wasPending) {
        pendingOnsets += 1;
      }
      shown = on;
      wasPending = report.pending;
    };
    for (let nowMs = 0; nowMs < t + 6_000; nowMs += 1_000 / 60) {
      // Each frame waits on the last's answers, as a view's loop does.
      // oxlint-disable-next-line no-await-in-loop
      await frame(nowMs);
    }
    const gaps = onsets.slice(1).map((at, k) => at - (onsets[k] ?? Number.NaN));
    expect([
      onsets.length > 1,
      pendingOnsets > 2 * onsets.length,
      gaps.filter((gap) => !(gap >= ANNUNCIATION_ONSET_MS + ANNUNCIATION_CLEAR_MS)),
    ]).toEqual([true, true, []]);
  });

  it("is held per view, each its own hold, over one request and one entry", async () => {
    const h = harness();
    const other = h.cache.openView();
    note(h, [lit(A)], 0);
    other.noteFrame({ nowMs: 200, scene: sceneOf(), bodies: [lit(A)] });
    const first = note(h, [lit(A)], 300).labels.includes("atmospherePending");
    const second = other
      .noteFrame({ nowMs: 300, scene: sceneOf(), bodies: [lit(A)] })
      .labels.includes("atmospherePending");
    expect([h.seam.askedFor(A), first, second]).toEqual([1, true, false]);
  });
});

describe("ATMOSPHERE: APPROXIMATE's reasons (R08.T10.c)", () => {
  const WATER = "H2O: its absorption estimated, not measured";

  it("reports each wanted body drawn with the label by its reasons, and none for the others", async () => {
    const h = harness();
    vi.spyOn(console, "error").mockImplementation(ignore);
    const approximate = bodyId(1);
    const computing = bodyId(2);
    const failed = bodyId(3);
    const pending = bodyId(4);
    const frame = [approximate, computing, failed, pending].map((id) => lit(id));
    note(h, frame);
    for (const id of [approximate, computing, failed]) {
      h.seam.answer(h.seam.latestFor(id).id, answerFor(id));
    }
    await microtasks();
    await land(h, approximate, {
      kind: "built",
      assembled: { ...earthAssembly(["atmosphereApproximate"]), reasons: [WATER] },
    });
    // A bake's note in APPROXIMATE's place: the reasons stand with that label alone.
    await land(h, computing, {
      kind: "built",
      assembled: { ...earthAssembly(["atmosphereComputing"]), reasons: [WATER] },
    });
    await land(h, failed, { kind: "failed", message: "a bug" });
    const report = note(h, frame);
    expect([[...report.reasons], report.problems.has(failed), report.pending]).toEqual([
      [[approximate, [WATER]]],
      true,
      true,
    ]);
  });

  it("reports none for a body drawn with the label as a point, which no view wants", async () => {
    const h = harness();
    await drawn(h, [lit(A)], { ...earthAssembly(["atmosphereApproximate"]), reasons: [WATER] });
    expect([...note(h, [lit(A, "point")]).reasons]).toEqual([]);
  });
});

describe("PendingLineHold", () => {
  it("shows after 250 ms held and goes after 1 s clear, a short clearing changing nothing", () => {
    const hold = new PendingLineHold();
    const shown = [
      hold.update(true, 0),
      hold.update(true, 249),
      hold.update(true, 250),
      hold.update(false, 300),
      hold.update(true, 900),
      hold.update(false, 1_000),
      hold.update(false, 1_999),
      hold.update(false, 2_000),
    ];
    expect(shown).toEqual([false, false, true, true, true, true, true, false]);
  });
});

describe("a body_detail the cache cannot draw", () => {
  it.each([
    ["unknown_body", true],
    ["internal", false],
  ] as const)(
    "refused %s gives no medium, no label and the rejected problem (refusal %s), with no log",
    async (code, refusal) => {
      const h = harness();
      const errors = vi.spyOn(console, "error").mockImplementation(ignore);
      note(h, [lit(A)]);
      h.seam.refuse(h.seam.latestFor(A).id, code, " the server's reason ");
      await microtasks();
      const report = note(h, [lit(A)]);
      expect([
        h.cache.entryOf(A),
        report.labels,
        report.problems.get(A),
        errors.mock.calls.length,
      ]).toEqual([
        null,
        [],
        { kind: "rejected", code, message: " the server's reason ", refusal },
        0,
      ]);
    },
  );

  it("refused with a code this build does not know gives the rejected problem as a fault, with no log", async () => {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    note(h, [lit(A)]);
    // `ErrorCode` grows without a protocol bump (galaxy plan 04's reserved list), and the decoder
    // trusts the generated types, as the test does where it plays a newer server.
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion
    const code = "body_archived" as ErrorCode;
    h.seam.refuse(h.seam.latestFor(A).id, code, "the body is archived");
    await microtasks();
    const report = note(h, [lit(A)]);
    expect([h.cache.entryOf(A), report.problems.get(A), errors.mock.calls.length]).toEqual([
      null,
      { kind: "rejected", code: "unknown", message: "the body is archived", refusal: false },
      0,
    ]);
  });

  it("drops what was drawn when a re-request is refused, disposing its tables", async () => {
    const h = harness();
    await drawn(h, [lit(A)]);
    note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
    h.seam.refuse(h.seam.latestFor(A).id, "unknown_body", "no such body");
    await microtasks();
    expect([h.cache.entryOf(A), h.made[0]?.disposed]).toEqual([null, true]);
  });

  it("holds an answer toBodyDetail cannot take as invalid, logging it once", async () => {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    note(h, [lit(A)]);
    await answer(h, A, answerFor("0300080020000000.0300"));
    note(h, [lit(A)]);
    note(h, [lit(A)]);
    expect([h.cache.problemOf(A), h.cache.labelsOf(A), errors.mock.calls.length]).toEqual([
      { kind: "invalid", check: expect.stringMatching(/another system/u) },
      [],
      1,
    ]);
  });

  it("holds an answer of another kind as invalid, its answer unreadable", async () => {
    const h = harness();
    vi.spyOn(console, "error").mockImplementation(ignore);
    note(h, [lit(A)]);
    h.seam.answerWith(h.seam.latestFor(A).id, { kind: "unsubscribe" });
    await microtasks();
    expect(h.cache.problemOf(A)).toEqual({ kind: "invalid", check: ATMOSPHERE_ANSWER_UNREADABLE });
  });

  it.each(["too_many_requests", "queue_full", "cancelled"] as const)(
    "answered %s, raises nothing, keeps PENDING and asks again after 2 s, doubling to 32 s",
    async (code) => {
      const h = harness();
      const errors = vi.spyOn(console, "error").mockImplementation(ignore);
      note(h, [lit(A)]);
      const waits: number[] = [];
      for (let n = 0; n < 6; n += 1) {
        h.seam.refuse(h.seam.latestFor(A).id, code, "busy");
        // Each wait follows the last's answer.
        // oxlint-disable-next-line no-await-in-loop
        await microtasks();
        const before = h.seam.asked.length;
        let waited = 0;
        while (h.seam.asked.length === before && waited < 40_000) {
          vi.advanceTimersByTime(1_000);
          waited += 1_000;
        }
        waits.push(waited);
      }
      expect([waits, h.cache.labelsOf(A), h.cache.problemOf(A), errors.mock.calls.length]).toEqual([
        [2, 4, 8, 16, 32, 32].map((s) => s * 1_000),
        ["atmospherePending"],
        null,
        0,
      ]);
    },
  );

  it("starts its busy waits afresh after an answer", async () => {
    const h = harness();
    note(h, [lit(A)]);
    h.seam.refuse(h.seam.latestFor(A).id, "too_many_requests", "busy");
    await microtasks();
    vi.advanceTimersByTime(2_000);
    h.seam.refuse(h.seam.latestFor(A).id, "too_many_requests", "busy");
    await microtasks();
    vi.advanceTimersByTime(4_000);
    await answer(h, A);
    note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
    h.seam.refuse(h.seam.latestFor(A).id, "too_many_requests", "busy");
    await microtasks();
    const before = h.seam.askedFor(A);
    vi.advanceTimersByTime(2_000);
    expect(h.seam.askedFor(A) - before).toBe(1);
  });

  it("stops a busy wait while no view wants the body, and asks once it is wanted again", async () => {
    const h = harness();
    note(h, [lit(A)]);
    h.seam.refuse(h.seam.latestFor(A).id, "queue_full", "busy");
    await microtasks();
    note(h, []);
    vi.advanceTimersByTime(60_000);
    const whileUnwanted = h.seam.askedFor(A);
    note(h, [lit(A)]);
    expect([whileUnwanted, h.seam.askedFor(A)]).toEqual([1, 2]);
  });

  it("holds a refusal through a re-request at the next trigger, its line standing with no PENDING, until an answer clears it", async () => {
    const h = harness();
    note(h, [lit(A)]);
    h.seam.refuse(h.seam.latestFor(A).id, "internal", "down");
    await microtasks();
    const report = note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
    const during = [h.seam.askedFor(A), report.problems.has(A), report.pending];
    await answer(h, A);
    expect([during, h.cache.problemOf(A), h.cache.labelsOf(A)]).toEqual([
      [2, true, false],
      null,
      ["atmospherePending"],
    ]);
  });

  it("asks afresh once failures are forgotten", async () => {
    const h = harness();
    note(h, [lit(A)]);
    h.seam.refuse(h.seam.latestFor(A).id, "unknown_body", "gone");
    await microtasks();
    note(h, [lit(A)]);
    h.cache.forgetFailures();
    note(h, [lit(A)]);
    expect([h.seam.askedFor(A), h.cache.problemOf(A)]).toEqual([2, null]);
  });
});

describe("a build that fails", () => {
  it("gives no medium, no label, one console.error and the failed problem, standing frame after frame", async () => {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    note(h, [lit(A)]);
    await answer(h, A);
    await land(h, A, { kind: "failed", message: "a bug" });
    const reports = [note(h, [lit(A)]), note(h, [lit(A)]), note(h, [lit(A)])];
    expect([
      h.cache.entryOf(A),
      reports.map((report) => [report.labels, report.problems.get(A), report.pending]),
      errors.mock.calls.length,
    ]).toEqual([null, Array.from({ length: 3 }, () => [[], { kind: "failed" }, false]), 1]);
  });

  describe("at the body's next trigger", () => {
    /** A cache whose body's build failed, asked again at a new arrival. */
    async function failedThenAskedAgain(): Promise<{
      readonly h: Harness;
      readonly arrived: AtmosphereScene;
    }> {
      const h = harness();
      vi.spyOn(console, "error").mockImplementation(ignore);
      note(h, [lit(A)]);
      await answer(h, A);
      await land(h, A, { kind: "failed", message: "a bug" });
      const arrived = sceneOf({ arrival: {} });
      note(h, [lit(A)], 0, arrived);
      return { h, arrived };
    }

    it("asks again, its fault standing meanwhile", async () => {
      const { h } = await failedThenAskedAgain();
      expect([h.seam.askedFor(A), h.cache.problemOf(A)]).toEqual([2, { kind: "failed" }]);
    });

    it("clears the fault once a retried build lands", async () => {
      const { h, arrived } = await failedThenAskedAgain();
      await answer(h, A);
      await land(h, A, { kind: "built", assembled: NOT_MODELLED });
      note(h, [lit(A)], 0, arrived);
      expect([h.cache.problemOf(A), h.cache.labelsOf(A)]).toEqual([
        null,
        ["atmosphereNotYetModelled"],
      ]);
    });
  });

  it("holds the body failed, not pending, where taking its build throws", async () => {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    note(h, [lit(A)]);
    await answer(h, A);
    // An assembly whose labels cannot be read: a bug in taking the build, not in making it.
    const unreadable: AssembledMedium = {
      ...NOT_MODELLED,
      get labels(): ReadonlyArray<AtmosphereLabel> {
        throw new Error("a bug in taking the build");
      },
    };
    await land(h, A, { kind: "built", assembled: unreadable });
    const report = note(h, [lit(A)]);
    expect([report.problems.get(A), report.pending, errors.mock.calls.length]).toEqual([
      { kind: "failed" },
      false,
      1,
    ]);
  });

  it("is asked afresh once failures are forgotten", async () => {
    const h = harness();
    vi.spyOn(console, "error").mockImplementation(ignore);
    note(h, [lit(A)]);
    await answer(h, A);
    await land(h, A, { kind: "failed", message: "a bug" });
    h.cache.forgetFailures();
    note(h, [lit(A)]);
    expect([h.seam.askedFor(A), h.cache.labelsOf(A)]).toEqual([2, ["atmospherePending"]]);
  });

  it("fails the same way where the tables' factory throws, once", async () => {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    h.factory.throws = new RangeError("more slices than the tables hold");
    await drawn(h, [lit(A)]);
    note(h, [lit(A)]);
    expect([h.cache.problemOf(A), h.made.length, errors.mock.calls.length]).toEqual([
      { kind: "failed" },
      0,
      1,
    ]);
  });

  it("raises nothing where the device is lost, and makes the tables once it is back", async () => {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    h.factory.throws = new EngineUnavailable("createTexture");
    await drawn(h, [lit(A)]);
    note(h, [lit(A)]);
    const during = [h.cache.problemOf(A), h.cache.entryOf(A), h.cache.labelsOf(A)];
    h.factory.throws = null;
    note(h, [lit(A)]);
    expect([during, h.made.length, h.cache.entryOf(A)?.tables, errors.mock.calls.length]).toEqual([
      [null, null, ["atmospherePending"]],
      1,
      h.made[0],
      0,
    ]);
  });
});

describe("a body whose air throws as it is drawn (decision-r08-t10b-leans §2)", () => {
  /** A cache with A drawn, then its draw reported failed by its view and by another. */
  async function drawFailed(): Promise<{
    readonly h: Harness;
    readonly other: AtmosphereView;
    readonly errors: MockInstance<typeof console.error>;
  }> {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    await drawn(h, [lit(A)]);
    const other = h.cache.openView();
    h.view.reportDrawFailure(A, new Error("a bug in the draw"), "VIEW 1");
    other.reportDrawFailure(A, new Error("the same bug"), "VIEW 2");
    return { h, other, errors };
  }

  it("holds it failed for every view, its tables let go, with one console.error naming the view", async () => {
    const { h, other, errors } = await drawFailed();
    const report = note(h, [lit(A)]);
    const seen = other.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(A)] });
    expect([
      h.cache.entryOf(A),
      h.made[0]?.disposed,
      report.problems.get(A),
      seen.problems.get(A),
      report.labels,
      errors.mock.calls.map(([text]) => text),
    ]).toEqual([
      null,
      true,
      { kind: "failed" },
      { kind: "failed" },
      [],
      [`the atmosphere of ${A} could not be drawn in VIEW 1:`],
    ]);
  });

  it("is drawn again once its retry at the next trigger lands", async () => {
    const { h } = await drawFailed();
    const arrived = sceneOf({ arrival: {} });
    note(h, [lit(A)], 0, arrived);
    h.seam.answer(h.seam.latestFor(A).id, answerFor(A));
    await microtasks();
    await land(h, A, { kind: "built", assembled: earthAssembly() });
    note(h, [lit(A)], 0, arrived);
    expect([h.seam.askedFor(A), h.cache.problemOf(A), h.cache.entryOf(A)?.tables]).toEqual([
      2,
      null,
      h.made.at(-1),
    ]);
  });

  it("is drawn by a new cache, VIEW shown again", async () => {
    await drawFailed();
    const again = harness();
    expect((await drawn(again, [lit(A)])).problems.get(A)).toBeUndefined();
  });

  it("takes no report from a closed view, or of a body it does not hold", async () => {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    await drawn(h, [lit(A)]);
    const closed = h.cache.openView();
    closed.close();
    closed.reportDrawFailure(A, new Error("a closed view's"), "VIEW 2");
    h.view.reportDrawFailure(bodyId(9), new Error("no such body"), "VIEW 1");
    expect([h.cache.problemOf(A), h.cache.problemOf(bodyId(9)), errors.mock.calls.length]).toEqual([
      null,
      null,
      0,
    ]);
  });
});

describe("the cache's size", () => {
  const ROW_BYTES = 2_000_000;
  const BODIES = Array.from({ length: 20 }, (_, n) => bodyId(n + 1));

  it("is its tables' rowBytes, evicting the least recently wanted past 32,000,000 B", async () => {
    const h = harness(ROW_BYTES);
    const sizes: number[] = [];
    for (const id of BODIES) {
      // Each body is drawn in its own frames, so the last drawn are the most recently wanted.
      // oxlint-disable-next-line no-await-in-loop
      await drawn(h, [lit(id)]);
      sizes.push(h.cache.bytes);
    }
    const disposed = h.made.flatMap((tables, n) => (tables.disposed ? [BODIES[n]] : []));
    expect([ATMOSPHERE_CACHE_BYTES, Math.max(...sizes), disposed]).toEqual([
      32_000_000,
      32_000_000,
      BODIES.slice(0, 4),
    ]);
  });

  it("never evicts a body wanted this frame, though the wanted pass its size", async () => {
    const h = harness(ROW_BYTES);
    await drawn(
      h,
      BODIES.map((id) => lit(id)),
    );
    expect([h.cache.bytes, h.made.filter((tables) => tables.disposed).length]).toEqual([
      BODIES.length * ROW_BYTES,
      0,
    ]);
  });

  it("makes an evicted body's tables again when it is wanted, sending no request", async () => {
    const h = harness(ROW_BYTES);
    for (const id of BODIES) {
      // One body after another, so that the first are evicted, as the size test draws them.
      // oxlint-disable-next-line no-await-in-loop
      await drawn(h, [lit(id)]);
    }
    const first = BODIES[0] ?? A;
    note(h, [lit(first)]);
    expect([
      h.seam.askedFor(first),
      h.builder.buildsOf(first).length,
      h.made.length,
      h.cache.entryOf(first)?.tables === h.made.at(-1),
    ]).toEqual([1, 1, BODIES.length + 1, true]);
  });

  it("lets go every body no view wants when the arrival changes", async () => {
    const h = harness();
    await drawn(h, [lit(A)]);
    note(h, [lit(bodyId(1))], 0, sceneOf({ arrival: {} }));
    const size = h.cache.size;
    note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
    expect([size, h.made[0]?.disposed, h.seam.askedFor(A)]).toEqual([1, true, 2]);
  });

  it("lets a closed view's bodies be evicted", async () => {
    const h = harness(20_000_000);
    const other = h.cache.openView();
    await drawn(h, [lit(A)]);
    other.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(bodyId(1))] });
    await answer(h, bodyId(1));
    await land(h, bodyId(1), { kind: "built", assembled: earthAssembly() });
    other.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(bodyId(1))] });
    const both = h.cache.bytes;
    other.close();
    note(h, [lit(A)]);
    expect([both, h.cache.bytes]).toEqual([40_000_000, 20_000_000]);
  });
});

describe("a kept scene's bodies", () => {
  /** A kept body's frame entry. */
  function kept(id: BodyIdHex, setting: KeptBodyAtmosphere): AtmosphereFrameBody {
    return { ...lit(id), kept: setting };
  }

  it("carries the assembly's labels for a body left unset or set airless, asking for neither", () => {
    const h = harness();
    note(h, [kept(bodyId(1), { kind: "unset" }), kept(bodyId(2), { kind: "airless" })], 0, null);
    const assembled = (["unset", "airless"] as const).map(
      (kind) => assembleMedium({ kind: "kept", atmosphere: { kind } }).labels,
    );
    expect([
      [h.cache.labelsOf(bodyId(1)), h.cache.labelsOf(bodyId(2))],
      h.seam.asked.length,
    ]).toEqual([assembled, 0]);
  });

  it("makes a scene's medium's tables as given, once, adding atmosphereApproximate to each the router routes thick, R05's Earth among them (R08.T13.d)", () => {
    const h = harness();
    const venus = venusClassMedium();
    const earth: BuiltAtmosphere = {
      medium: R05_EARTH_REFERENCE,
      bottomRadiusM: 6_371_008.8,
      curves: [],
      body: earthBody(),
    };
    const thick: BuiltAtmosphere = {
      medium: venus.medium,
      bottomRadiusM: venus.radiusM,
      curves: [],
      body: null,
    };
    const bodies = [
      kept(bodyId(1), { kind: "medium", built: earth, labels: [] }),
      kept(bodyId(2), { kind: "medium", built: thick, labels: ["aerosolsNotYetModelled"] }),
    ];
    note(h, bodies, 0, null);
    note(h, bodies, 0, null);
    expect([
      h.cache.labelsOf(bodyId(1)),
      h.cache.labelsOf(bodyId(2)),
      h.made.map((tables) => tables.built),
      h.seam.asked.length,
    ]).toEqual([
      ["atmosphereApproximate"],
      ["aerosolsNotYetModelled", "atmosphereApproximate"],
      [earth, thick],
      0,
    ]);
  });

  it("gives a scene's medium the router routes thick its reason and regime, and a thin one neither (R08.T10.c, R08.T13.d)", () => {
    const h = harness();
    const venus = venusClassMedium();
    const tenuous: BuiltAtmosphere = {
      medium: sweepMedium({
        singleScatteringAlbedo: 1,
        asymmetry: 0,
        tau: 1 / 64,
        groundAlbedo: 0.3,
      }),
      bottomRadiusM: 6_371_008.8,
      curves: [],
      body: null,
    };
    const thick: BuiltAtmosphere = {
      medium: venus.medium,
      bottomRadiusM: venus.radiusM,
      curves: [],
      body: null,
    };
    const bodies = [
      kept(bodyId(1), { kind: "medium", built: tenuous, labels: [] }),
      kept(bodyId(2), { kind: "medium", built: thick, labels: [] }),
    ];
    note(h, bodies, 0, null);
    const report = note(h, bodies, 0, null);
    expect([
      [...report.reasons],
      h.cache.entryOf(bodyId(1))?.reasons,
      h.cache.entryOf(bodyId(2))?.reasons,
      h.cache.entryOf(bodyId(1))?.regime,
      h.cache.entryOf(bodyId(2))?.regime,
    ]).toEqual([
      [[bodyId(2), [THICK_REASON]]],
      [],
      [THICK_REASON],
      { kind: "thin" },
      { kind: "thickScattering" },
    ]);
  });

  it("routes a kept body again when its scene gives another cover, the same medium kept (R08.T13.d)", () => {
    const h = harness();
    const venus = venusClassMedium();
    const built: BuiltAtmosphere = {
      medium: venus.medium,
      bottomRadiusM: venus.radiusM,
      curves: [],
      body: null,
    };
    const deck = { opticalDepth550: 30, baseAltitudeM: 2_000, topAltitudeM: 5_000 };
    note(h, [kept(A, { kind: "medium", built, labels: [] })], 0, null);
    const clear = h.cache.entryOf(A)?.regime;
    const cover = { cloudFraction: 1, decks: [deck], envelope: false };
    note(h, [kept(A, { kind: "medium", built, labels: [], cover })], 0, null);
    expect([clear, h.cache.entryOf(A)?.regime]).toEqual([
      { kind: "thickScattering" },
      { kind: "cloudDeck", deckAltitudeM: 5_000 },
    ]);
  });

  it("holds a body whose scene is still building its medium pending, then draws it (R08.T10.b)", () => {
    const h = harness();
    const view = h.cache.openView();
    const loading = view.noteFrame({
      nowMs: 0,
      scene: null,
      bodies: [kept(A, { kind: "loading" })],
    });
    const held = view.noteFrame({
      nowMs: ANNUNCIATION_ONSET_MS,
      scene: null,
      bodies: [kept(A, { kind: "loading" })],
    });
    const built = view.noteFrame({
      nowMs: ANNUNCIATION_ONSET_MS + 1,
      scene: null,
      bodies: [kept(A, { kind: "medium", built: keptEarth(), labels: [] })],
    });
    expect([
      loading.pending,
      held.labels,
      built.pending,
      h.cache.entryOf(A)?.tables !== null,
    ]).toEqual([true, ["atmospherePending"], false, true]);
    expect(h.seam.asked.length).toBe(0);
  });

  it("holds a body whose scene's load failed as loadFailed, not pending and unlabelled, logging nothing (R08.T10.b)", () => {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    note(h, [kept(A, { kind: "medium", built: keptEarth(), labels: [] })], 0, null);
    const report = note(h, [kept(A, { kind: "failed" })], 0, null);
    expect([
      report.problems.get(A),
      report.pending,
      report.labels,
      h.cache.entryOf(A),
      h.made[0]?.disposed,
      errors.mock.calls.length,
      h.seam.asked.length,
    ]).toEqual([{ kind: "loadFailed" }, false, [], null, true, 0, 0]);
  });

  it("makes a kept body's tables again for another medium, disposing the last", () => {
    const h = harness();
    note(h, [kept(A, { kind: "medium", built: keptEarth(), labels: [] })], 0, null);
    note(h, [kept(A, { kind: "medium", built: keptEarth(), labels: [] })], 0, null);
    expect(h.made.map((tables) => tables.disposed)).toEqual([true, false]);
  });
});

describe("the regime a server body's entry carries (R08.T13.d)", () => {
  it("is the assembly's, as the worker computed it", async () => {
    const h = harness();
    const regime = { kind: "cloudDeck", deckAltitudeM: 4_000 } as const;
    await drawn(h, [lit(A)], earthAssembly(["atmosphereApproximate"], regime));
    expect(h.cache.entryOf(A)?.regime).toEqual(regime);
  });

  it("is null for an assembly that draws no medium", async () => {
    const h = harness();
    await drawn(h, [lit(A)], NOT_MODELLED);
    expect(h.cache.entryOf(A)?.regime).toBeNull();
  });
});

/** R05's Earth as a kept scene builds it, a new object each call. */
function keptEarth(): BuiltAtmosphere {
  return { medium: R05_EARTH_REFERENCE, bottomRadiusM: 6_371_008.8, curves: [], body: null };
}

/** A router of `cloudDeck` alone, a test's: the product routes none before R08.T15.b.2. */
function routedDeck(regime: AtmosphereRegime): boolean {
  return regime.kind === "cloudDeck";
}

/** The latest deck's bake the cache asked, or a test failure. */
function latestDeck(h: Harness): ScriptedDeck {
  const deck = h.builder.decks.at(-1);
  if (deck === undefined) {
    throw new Error("no deck's bake was asked");
  }
  return deck;
}

describe("a cloud deck's bake (R08.T15.a.2)", () => {
  /** A kept body's frame entry. */
  function kept(id: BodyIdHex, setting: KeptBodyAtmosphere): AtmosphereFrameBody {
    return { ...lit(id), kept: setting };
  }

  /** A deck of τ 30 under a cloud fraction of 1, its top 5 km up: `cloudDeck` at 5 km. */
  const COVER: BodyCover = {
    cloudFraction: 1,
    decks: [{ opticalDepth550: 30, baseAltitudeM: 2_000, topAltitudeM: 5_000 }],
    envelope: false,
  };
  /** R05's Earth kept with its sources, so that it bakes. */
  const built: BuiltAtmosphere = {
    ...keptEarth(),
    sources: R05_EARTH_REFERENCE.terms.map(channelSource),
  };
  it("asks none by default: a cloudDeck body is drawn as before, on its own medium's tables", () => {
    const h = harness();
    note(h, [kept(A, { kind: "medium", built, labels: [], cover: COVER })], 0, null);
    expect([
      h.cache.entryOf(A)?.regime,
      h.builder.decks.length,
      h.builder.bakes.length,
      madeAt(h, 0).built.medium,
    ]).toEqual([{ kind: "cloudDeck", deckAltitudeM: 5_000 }, 0, 0, R05_EARTH_REFERENCE]);
  });

  it("asks none under the product's route, which routes no deck while no deck gate passes, keeping APPROXIMATE with its reason (R08.T15.b.2)", () => {
    const h = harness(undefined, "product");
    const report = note(
      h,
      [
        kept(A, { kind: "medium", built, labels: [], cover: COVER }),
        kept(bodyId(2), { kind: "medium", built, labels: [], cover: { ...COVER, envelope: true } }),
      ],
      0,
      null,
    );
    expect([
      h.cache.entryOf(A)?.regime?.kind,
      h.cache.entryOf(bodyId(2))?.regime?.kind,
      h.builder.decks.length,
      h.builder.bakes.length,
      madeAt(h, 0).built.medium,
      report.labels,
      report.reasons.get(A),
    ]).toEqual([
      "cloudDeck",
      "cloudDeck",
      0,
      0,
      R05_EARTH_REFERENCE,
      ["atmosphereApproximate"],
      [THICK_REASON],
    ]);
  });

  it("draws a routed one on its upper medium, asks its deck's bake on its own figure, and gives its tables the deck and the correction over it", async () => {
    const h = harness(undefined, routedDeck);
    const view = h.cache.openView();
    const asked = view.noteFrame({
      nowMs: 0,
      scene: null,
      bodies: [kept(A, { kind: "medium", built, labels: [], cover: COVER })],
    });
    const tables = madeAt(h, 0);
    const { ask } = latestDeck(h);
    expect([
      tables.built.medium.topHeightM,
      tables.built.bottomRadiusM,
      ask.split,
      ask.body.bottomRadiusM,
      ask.column,
      h.builder.bakes.length,
      h.builder.corrections.length,
      asked.labels,
    ]).toEqual([
      R05_EARTH_REFERENCE.topHeightM - 5_000,
      6_371_008.8 + 5_000,
      { altitudeM: 5_000, floor: "ground" },
      6_371_008.8,
      spectralColumnOf(R05_EARTH_REFERENCE, R05_EARTH_REFERENCE.terms.map(channelSource)),
      0,
      0,
      ["atmosphereComputing"],
    ]);
    const bands = tables.plan.bands;
    const decks = patternedDeckTables(bands, tables.built.medium, 1, 5_000);
    const correction = patternedDeckCorrection(bands, tables.built.medium, 1, 5_000);
    latestDeck(h).settle({ kind: "baked", tables: decks, correction });
    await microtasks();
    const baked = view.noteFrame({
      nowMs: 1,
      scene: null,
      bodies: [kept(A, { kind: "medium", built, labels: [], cover: COVER })],
    });
    expect([
      tables.decks.at(-1) === decks,
      Array.from(tables.correctionHeld?.[0]?.altitudesM ?? []).slice(0, 2),
      baked.labels,
    ]).toEqual([
      true,
      Array.from(patternedCorrection(bands, tables.built.medium, 1)[0]?.altitudesM ?? []).slice(
        0,
        2,
      ),
      [],
    ]);
  });

  it("draws an envelope whose τ never reaches the split's as before: no deck stands at its ground", () => {
    const h = harness(undefined, routedDeck);
    note(
      h,
      [kept(A, { kind: "medium", built, labels: [], cover: { ...COVER, envelope: true } })],
      0,
      null,
    );
    // R05's Earth never reaches the envelope's τ 10: its floor is the ground, and no deck stands.
    expect([h.cache.entryOf(A)?.regime, h.builder.decks.length]).toEqual([
      { kind: "cloudDeck", deckAltitudeM: 0 },
      0,
    ]);
  });

  it("asks a routed deck's disc table once its bake lands, on its upper medium, reading the deck's J and its table (R08.T16.b.2)", async () => {
    const h = harness(undefined, routedDeck);
    const view = h.cache.openView();
    const bodies = [kept(A, { kind: "medium", built, labels: [], cover: COVER })];
    view.noteFrame({ nowMs: 0, scene: null, bodies });
    const asked = h.builder.discs.length;
    const tables = madeAt(h, 0);
    const decks = patternedDeckTables(tables.plan.bands, tables.built.medium, 1, 5_000);
    latestDeck(h).settle({ kind: "baked", tables: decks, correction: null });
    await microtasks();
    view.noteFrame({ nowMs: 1, scene: null, bodies });
    const disc = h.builder.discs.at(-1);
    expect([
      asked,
      h.builder.discs.length,
      disc?.ask.air.medium.topHeightM,
      disc?.ask.air.bake === undefined ? null : disc.ask.air.bake.length,
      disc?.ask.air.deck?.bands,
      disc?.ask.air.correction,
    ]).toEqual([
      0,
      1,
      R05_EARTH_REFERENCE.topHeightM - 5_000,
      decks.length,
      decks.length,
      undefined,
    ]);
  });

  it("asks no disc table of a routed deck whose bake failed: the full passes draw it", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const h = harness(undefined, routedDeck);
    const view = h.cache.openView();
    const bodies = [kept(A, { kind: "medium", built, labels: [], cover: COVER })];
    view.noteFrame({ nowMs: 0, scene: null, bodies });
    latestDeck(h).settle({ kind: "failed", message: "a test's" });
    await microtasks();
    view.noteFrame({ nowMs: 1, scene: null, bodies });
    expect([h.builder.discs.length, error.mock.calls.length]).toEqual([0, 1]);
  });

  it("splits an envelope at its floor, its deck read to the envelope's own", () => {
    const h = harness(undefined, routedDeck);
    // A grey cloud of τ 20 to the datum over R05's Earth: τ 10 is reached inside the air.
    const cloud: MediumTerm = {
      name: "aerosol:cloud",
      density: { kind: "exponential", scaleHeightM: 2_000 },
      scattering: [1e-2, 1e-2, 1e-2],
      absorption: [0, 0, 0],
      phase: { kind: "cornette-shanks", asymmetry: 0.8 },
    };
    const medium = { ...R05_EARTH_REFERENCE, terms: [...R05_EARTH_REFERENCE.terms, cloud] };
    const clouded: BuiltAtmosphere = {
      ...keptEarth(),
      medium,
      sources: medium.terms.map(channelSource),
    };
    note(
      h,
      [
        kept(A, {
          kind: "medium",
          built: clouded,
          labels: [],
          cover: { ...COVER, envelope: true },
        }),
      ],
      0,
      null,
    );
    const floorM = envelopeFloorM(medium);
    expect(floorM).toBeGreaterThan(0);
    expect(latestDeck(h).ask.split).toEqual({ altitudeM: floorM, floor: "envelope" });
  });

  it("falls back to APPROXIMATE with its reason where the deck's bake fails, giving its tables nothing and raising no fault", async () => {
    const h = harness(undefined, routedDeck);
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    const body = [kept(A, { kind: "medium", built, labels: [], cover: COVER })];
    note(h, body, 0, null);
    latestDeck(h).settle({ kind: "failed", message: "a deck's solve refused a layer" });
    await microtasks();
    const report = note(h, body, 1, null);
    expect([
      report.labels,
      [...report.reasons],
      report.problems.size,
      madeAt(h, 0).decks.length,
      errors.mock.calls.length,
    ]).toEqual([["atmosphereApproximate"], [[A, [THICK_REASON]]], 0, 0, 1]);
  });

  it("cancels the deck's bake when the body's cover moves it off its deck", () => {
    const h = harness(undefined, routedDeck);
    note(h, [kept(A, { kind: "medium", built, labels: [], cover: COVER })], 0, null);
    const deck = latestDeck(h);
    note(h, [kept(A, { kind: "medium", built, labels: [] })], 1, null);
    expect([deck.settled, h.cache.entryOf(A)?.regime, h.builder.decks.length]).toEqual([
      true,
      { kind: "thickScattering" },
      1,
    ]);
  });

  it("gives its tables the deck at a later frame where the device was lost as it landed", async () => {
    const h = harness(undefined, routedDeck);
    const body = [kept(A, { kind: "medium", built, labels: [], cover: COVER })];
    note(h, body, 0, null);
    const tables = madeAt(h, 0);
    tables.deckThrows = new EngineUnavailable("createTexture");
    const decks = patternedDeckTables(tables.plan.bands, tables.built.medium, 1, 5_000);
    latestDeck(h).settle({ kind: "baked", tables: decks, correction: null });
    await microtasks();
    const during = tables.decks.length;
    tables.deckThrows = null;
    note(h, body, 1, null);
    expect([during, tables.decks.at(-1) === decks]).toEqual([0, true]);
  });
});

/**
 * R05's Earth as a gas envelope's assembly (R08.T16.a.2): `thickAssembly`'s, split at a floor 5 km
 * up (`cloudDeck`), its drawn medium carrying the envelope's floor, or none where `envelope` is
 * false (a surface's air split at its deck's top).
 */
function envelopeAssembly(envelope = true): AssembledMedium {
  const thick = thickAssembly();
  if (thick.medium === undefined) {
    throw new Error("the thick assembly draws a medium");
  }
  return {
    ...thick,
    drawn: {
      ...thick.drawn,
      ...(envelope
        ? {
            envelope: {
              floorPa: 5e4,
              floorAltitudeM: 5_000,
              pressuresPa: new Float64Array([1e5, 5e4]),
              dropped: [],
            },
          }
        : {}),
    },
    regime: { kind: "cloudDeck", deckAltitudeM: 5_000 },
  };
}

/** A record's ground as the cache draws it: its photometry's p q, clamped to [0, 1]. */
function photometryGround(detail: BodyDetail): readonly [number, number, number] {
  const { geometricAlbedo: p, phaseIntegral: q } = heldAppearanceOf(detail.record).photometry;
  return clampedGroundAlbedo([p[0] * q[0], p[1] * q[1], p[2] * q[2]]);
}

/** The latest build the builder was asked, or a test failure. */
function latestBuild(h: Harness): ScriptedBuild {
  const build = h.builder.builds.at(-1);
  if (build === undefined) {
    throw new Error("no build was asked");
  }
  return build;
}

describe("a gas envelope's entry (R08.T16.a.2)", () => {
  /** A kept body's frame entry. */
  function kept(id: BodyIdHex, setting: KeptBodyAtmosphere): AtmosphereFrameBody {
    return { ...lit(id), kept: setting };
  }

  /** The hand Jupiter's record, as `ATMOSPHERE TEST`'s `TEST JUPITER` gives it. */
  const JUPITER = handJupiterRecord();

  it("marks a server body whose assembly drew its gas envelope an envelope's, and asks its routed deck's bake over the envelope's floor", async () => {
    const h = harness(undefined, routedDeck);
    await drawn(h, [lit(A)], envelopeAssembly());
    expect([h.cache.entryOf(A)?.envelope, latestDeck(h).ask.split]).toEqual([
      true,
      { altitudeM: 5_000, floor: "envelope" },
    ]);
  });

  it("keeps a server body's surface air split at its deck over the ground", async () => {
    const h = harness(undefined, routedDeck);
    await drawn(h, [lit(A)], envelopeAssembly(false));
    expect([h.cache.entryOf(A)?.envelope, latestDeck(h).ask.split]).toEqual([
      false,
      { altitudeM: 5_000, floor: "ground" },
    ]);
  });

  it("draws an envelope by the analytic term above its floor under the product's closed route, APPROXIMATE with its reason, asking no bake", async () => {
    const h = harness(undefined, "product", "product");
    const report = await drawn(h, [lit(A)], envelopeAssembly());
    expect([
      h.cache.entryOf(A)?.envelope,
      madeAt(h, 0).built.medium.topHeightM,
      h.builder.decks.length,
      h.builder.bakes.length,
      h.builder.corrections.length,
      report.labels,
      report.reasons.get(A),
    ]).toEqual([
      true,
      R05_EARTH_REFERENCE.topHeightM - 5_000,
      0,
      0,
      0,
      ["atmosphereApproximate"],
      [THICK_REASON],
    ]);
  });

  it("draws an unrouted surface's air split at its deck on its whole medium, as before", async () => {
    const h = harness();
    await drawn(h, [lit(A)], envelopeAssembly(false));
    expect(madeAt(h, 0).built.medium).toBe(R05_EARTH_REFERENCE);
  });

  it("gives a closed envelope's self-luminous tables its temperature from its floor up", async () => {
    const h = harness();
    const assembled = envelopeAssembly();
    if (assembled.medium === undefined) {
      throw new Error("the envelope's assembly draws a medium");
    }
    await drawn(h, [lit(A)], {
      ...assembled,
      drawn: {
        ...assembled.drawn,
        temperature: {
          heightsM: Float64Array.of(0, 4_000, 6_000, 10_000),
          temperaturesK: Float64Array.of(300, 280, 260, 240),
        },
      },
      regime: { kind: "cloudDeck", deckAltitudeM: 5_000, selfLuminous: true },
    });
    const temperature = madeAt(h, 0).built.temperature;
    expect([
      Array.from(temperature?.heightsM ?? []),
      Array.from(temperature?.temperaturesK ?? []),
    ]).toEqual([
      [0, 1_000, 5_000],
      [270, 260, 240],
    ]);
  });

  it("asks an envelope no disc table, which R08.T16.b.2 checks at 10⁹ m, and a surface's split air one", async () => {
    const envelope = harness();
    await drawn(envelope, [lit(A)], envelopeAssembly());
    const surface = harness();
    await drawn(surface, [lit(A)], envelopeAssembly(false));
    expect([envelope.builder.discs.length, surface.builder.discs.length]).toEqual([0, 1]);
  });

  it("builds a kept scene's record as a server body's reply, pending until it lands, asking nothing of the server", async () => {
    const h = harness();
    const setting: KeptBodyAtmosphere = { kind: "record", detail: JUPITER };
    const pending = note(h, [kept(A, setting)], 0, null);
    const build = latestBuild(h);
    const asked = [
      h.seam.asked.length,
      h.builder.builds.length,
      build.input,
      pending.pending,
      h.cache.entryOf(A),
    ];
    build.settle({ kind: "built", assembled: envelopeAssembly() });
    await microtasks();
    const report = note(h, [kept(A, setting)], 1, null);
    expect([
      asked,
      h.builder.builds.length,
      h.cache.entryOf(A)?.envelope,
      h.cache.entryOf(A)?.regime,
      report.pending,
      report.labels,
    ]).toEqual([
      [
        0,
        1,
        {
          kind: "record",
          detail: JUPITER,
          lights: [SUN_LIGHT],
          groundAlbedo: photometryGround(JUPITER),
        },
        true,
        null,
      ],
      1,
      true,
      { kind: "cloudDeck", deckAltitudeM: 5_000 },
      false,
      ["atmosphereApproximate"],
    ]);
  });

  it("builds a kept record again for a changed light set, and not for the same", () => {
    const h = harness();
    const setting: KeptBodyAtmosphere = { kind: "record", detail: JUPITER };
    const other: BodyLight = { star: `${FIXTURE_SYSTEM}.0101`, spectrum: SUN_LIGHT.spectrum };
    note(h, [kept(A, setting)], 0, null);
    note(h, [kept(A, setting)], 1, null);
    const same = h.builder.builds.length;
    note(h, [{ ...lit(A, "disc", [SUN_LIGHT, other]), kept: setting }], 2, null);
    expect([same, h.builder.builds.length, latestBuild(h).input]).toEqual([
      1,
      2,
      expect.objectContaining({ lights: [SUN_LIGHT, other] }),
    ]);
  });

  it("cancels a kept record's build when the scene sets another setting", () => {
    const h = harness();
    note(h, [kept(A, { kind: "record", detail: JUPITER })], 0, null);
    const build = latestBuild(h);
    note(h, [kept(A, { kind: "loading" })], 1, null);
    expect([build.settled, h.cache.entryOf(A)]).toEqual([true, null]);
  });
});

describe("the cache's disposal", () => {
  /**
   * A cache disposed with a body drawn, one asked and unanswered, and one waiting after a busy
   * answer.
   */
  async function disposed(): Promise<Harness> {
    const h = harness();
    await drawn(h, [lit(A)]);
    note(h, [lit(A), lit(bodyId(1)), lit(bodyId(2))]);
    h.seam.refuse(h.seam.latestFor(bodyId(2)).id, "too_many_requests", "busy");
    await microtasks();
    h.cache.dispose();
    return h;
  }

  it("cancels the requests in flight", async () => {
    const h = await disposed();
    expect(h.seam.cancelled).toEqual([h.seam.latestFor(bodyId(1)).id]);
  });

  it("stops the busy waits", async () => {
    const h = await disposed();
    vi.advanceTimersByTime(60_000);
    expect(h.seam.asked.length).toBe(3);
  });

  it("terminates its builder", async () => {
    const h = await disposed();
    expect(h.builder.terminated).toBe(true);
  });

  it("disposes every table", async () => {
    const h = await disposed();
    expect([h.made.length > 0, h.made.every((tables) => tables.disposed)]).toEqual([true, true]);
  });

  it("reports nothing of a later frame", async () => {
    const h = await disposed();
    expect(note(h, [lit(A)])).toEqual({
      labels: [],
      problems: new Map(),
      reasons: new Map(),
      pending: false,
    });
  });
});

/** The latest bake the cache asked, or a test failure. */
function latestBake(h: Harness): ScriptedBake {
  const bake = h.builder.bakes.at(-1);
  if (bake === undefined) {
    throw new Error("no thick bake was asked");
  }
  return bake;
}

/** The tables made `n`th, or a test failure. */
function madeAt(h: Harness, n: number): FakeTables {
  const tables = h.made[n];
  if (tables === undefined) {
    throw new Error(`no tables were made ${n}th`);
  }
  return tables;
}

describe("a thick body's bake (R08.T14.c)", () => {
  /** A routed cache with A drawn thick from `assembled`, its bake asked and running. */
  async function baking(assembled = thickAssembly(), rowBytes?: number): Promise<Harness> {
    const h = harness(rowBytes, routedThick);
    await drawn(h, [lit(A)], assembled);
    return h;
  }

  /** Lands A's latest bake with a bake of its first tables' plan, one slot, and returns it. */
  async function landBake(h: Harness): Promise<ReadonlyArray<MsSourceTable>> {
    const tables = patternedThickBake(madeAt(h, 0).plan.bands, R05_EARTH_REFERENCE, 1);
    latestBake(h).settle({ kind: "baked", tables });
    await microtasks();
    return tables;
  }

  it("asks no bake by default, a thick body drawn under APPROXIMATE with its reason as before", async () => {
    const h = harness();
    const report = await drawn(h, [lit(A)], thickAssembly());
    expect([h.builder.bakes.length, report.labels, [...report.reasons]]).toEqual([
      0,
      ["atmosphereApproximate"],
      [[A, [THICK_REASON]]],
    ]);
  });

  it("shows COMPUTING in APPROXIMATE's place while the bake runs, never beside it and with no reason, though a stand-in holds it", async () => {
    const h = harness(undefined, routedThick);
    const first = await drawn(h, [lit(A)], thickAssembly([STAND_IN, THICK_REASON]));
    const later = note(h, [lit(A)]);
    expect([
      [first.labels, [...first.reasons]],
      [later.labels, [...later.reasons], later.problems.size],
      h.cache.entryOf(A)?.reasons,
    ]).toEqual([[["atmosphereComputing"], []], [["atmosphereComputing"], [], 0], []]);
  });

  it("asks its bake once its tables are made, every band of their plan and every light slot in one request", async () => {
    const h = harness(undefined, routedThick);
    const second: BodyLight = { star: `${FIXTURE_SYSTEM}.0101`, spectrum: SUN_LIGHT.spectrum };
    const lights = [SUN_LIGHT, second];
    const saturn = saturnBody();
    await drawn(h, [lit(A, "disc", lights)], thickAssembly([THICK_REASON], saturn, lights));
    note(h, [lit(A, "disc", lights)]);
    const { ask } = latestBake(h);
    expect([
      h.builder.bakes.length,
      ask.bands,
      thickBakeBands(ask.body).map((band) => band.gravityRatio),
      ask.slots,
      ask.input === h.builder.latestOf(A).input,
      ask.body,
      ask.column,
      ask.kept,
    ]).toEqual([
      1,
      [0, 1, 2, 3],
      madeAt(h, 0).plan.bands.map((band) => band.gravityRatio),
      [0, 1],
      true,
      { ...saturn, bottomRadiusM: mediumTableRadiusM(saturn.figure) },
      spectralColumnOf(R05_EARTH_REFERENCE, R05_EARTH_REFERENCE.terms.map(channelSource)),
      undefined,
    ]);
  });

  it("gives its tables the bake once it lands, every band in its order, and drops APPROXIMATE", async () => {
    const h = await baking(thickAssembly([THICK_REASON], saturnBody()));
    const tables = patternedThickBake(madeAt(h, 0).plan.bands, R05_EARTH_REFERENCE, 1);
    latestBake(h).settle({ kind: "baked", tables });
    await microtasks();
    const report = note(h, [lit(A)]);
    expect([
      madeAt(h, 0).thick.length,
      madeAt(h, 0).thickHeld === tables,
      tables.map((table) => table.band.gravityRatio),
      report.labels,
      [...report.reasons],
      h.builder.bakes.length,
    ]).toEqual([1, true, madeAt(h, 0).plan.bands.map((band) => band.gravityRatio), [], [], 1]);
  });

  it("keeps APPROXIMATE after the bake where a stand-in still holds it, with that reason alone", async () => {
    const h = await baking(thickAssembly([STAND_IN, THICK_REASON]));
    await landBake(h);
    const report = note(h, [lit(A)]);
    expect([report.labels, [...report.reasons]]).toEqual([
      ["atmosphereApproximate"],
      [[A, [STAND_IN]]],
    ]);
  });

  it("falls back to APPROXIMATE with every reason where the bake fails, raising no fault and asking no bake again", async () => {
    const h = await baking(thickAssembly([STAND_IN, THICK_REASON]));
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    latestBake(h).settle({ kind: "failed", message: "the solver refused a layer" });
    await microtasks();
    const reports = [note(h, [lit(A)]), note(h, [lit(A)])];
    expect([
      reports.map((report) => [report.labels, [...report.reasons], report.problems.size]),
      h.cache.problemOf(A),
      h.cache.entryOf(A)?.tables === h.made[0],
      h.builder.bakes.length,
      errors.mock.calls.length,
    ]).toEqual([
      Array.from({ length: 2 }, () => [
        ["atmosphereApproximate"],
        [[A, [STAND_IN, THICK_REASON]]],
        0,
      ]),
      null,
      true,
      1,
      1,
    ]);
  });

  it("fails the bake where its tables refuse it, with one console.error and no fault", async () => {
    const h = await baking();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    madeAt(h, 0).thickThrows = new RangeError("a thick bake of 2 bands is not the tables' 1");
    await landBake(h);
    const report = note(h, [lit(A)]);
    expect([
      report.labels,
      [...report.reasons],
      report.problems.size,
      errors.mock.calls.length,
    ]).toEqual([["atmosphereApproximate"], [[A, [THICK_REASON]]], 0, 1]);
  });

  it("gives its tables the bake at a later frame where the device was lost as it landed", async () => {
    const h = await baking();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    const tables = madeAt(h, 0);
    tables.thickThrows = new EngineUnavailable("createTexture");
    const baked = await landBake(h);
    note(h, [lit(A)]);
    const during = tables.thick.length;
    tables.thickThrows = null;
    note(h, [lit(A)]);
    expect([
      during,
      tables.thick.length,
      tables.thickHeld === baked,
      errors.mock.calls.length,
    ]).toEqual([0, 1, true, 0]);
  });

  it("gives tables made again after an eviction the bake, asking no bake and no record again", async () => {
    const h = await baking(thickAssembly(), 20_000_000);
    const baked = await landBake(h);
    await drawn(h, [lit(bodyId(1))]);
    const evicted = madeAt(h, 0).disposed;
    const report = note(h, [lit(A)]);
    expect([
      evicted,
      h.made.length,
      madeAt(h, 2).built === madeAt(h, 0).built,
      madeAt(h, 2).thickHeld === baked,
      h.builder.bakes.length,
      h.seam.askedFor(A),
      report.labels,
    ]).toEqual([true, 3, true, true, 1, 1, []]);
  });

  it("cancels the bake when a new key's build lands, asking the new entry's", async () => {
    const h = await baking();
    const first = latestBake(h);
    const second: BodyLight = { star: `${FIXTURE_SYSTEM}.0101`, spectrum: SUN_LIGHT.spectrum };
    const lights = [SUN_LIGHT, second];
    note(h, [lit(A, "disc", lights)]);
    await land(h, A, {
      kind: "built",
      assembled: thickAssembly([THICK_REASON], earthBody(), lights),
    });
    const report = note(h, [lit(A, "disc", lights)]);
    expect([
      first.cancelled,
      h.builder.bakes.length,
      latestBake(h).ask.slots,
      report.labels,
    ]).toEqual([true, 2, [0, 1], ["atmosphereComputing"]]);
  });

  it.each([
    [
      "its draw is reported failed",
      (h: Harness): void => {
        h.view.reportDrawFailure(A, new Error("a bug in the draw"), "VIEW 1");
      },
    ],
    [
      "its record is refused",
      (h: Harness): void => {
        note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
        h.seam.refuse(h.seam.latestFor(A).id, "unknown_body", "no such body");
      },
    ],
    [
      "the cache is disposed",
      (h: Harness): void => {
        h.cache.dispose();
      },
    ],
  ])("cancels the bake when %s", async (_, drop) => {
    const h = await baking();
    vi.spyOn(console, "error").mockImplementation(ignore);
    drop(h);
    await microtasks();
    expect(latestBake(h).cancelled).toBe(true);
  });

  describe("of a kept scene's medium", () => {
    /** A kept body's frame entry, lit by `lights`. */
    function keptBody(
      setting: KeptBodyAtmosphere,
      lights: ReadonlyArray<BodyLight> = [SUN_LIGHT],
    ): AtmosphereFrameBody {
      return { ...lit(A, "disc", lights), kept: setting };
    }

    it("asks TEST EARTH's bake with its own lights, its scene's sun, on its sphere's one band", () => {
      const h = harness(undefined, routedThick);
      const built = builtOfCase(EARTH_CASE, "earth");
      const report = note(h, [keptBody({ kind: "medium", built, labels: [] })], 0, null);
      const { ask } = latestBake(h);
      expect([
        report.labels,
        h.builder.bakes.length,
        ask.input,
        ask.slots,
        ask.bands,
        thickBakeBands(ask.body),
        ask.column,
        ask.kept,
      ]).toEqual([
        ["atmosphereComputing"],
        1,
        { kind: "kept", atmosphere: { kind: "unset" } },
        [0],
        [0],
        thickBakeBands(sphereBakeBody(EARTH_REFERENCE_RADIUS_M, EARTH_COLUMN.referenceGravityMS2)),
        keptColumnOf(EARTH_CASE),
        keptBakeLightsOf(EARTH_CASE.medium, [SUN_LIGHT.spectrum]),
      ]);
    });

    it("asks a kept body lit by no star of its scene's under the reference Sun, as the kept cases' curves were formed", () => {
      const h = harness(undefined, routedThick);
      const built = builtOfCase(EARTH_CASE, "earth");
      note(h, [keptBody({ kind: "medium", built, labels: [] }, [])], 0, null);
      const { ask } = latestBake(h);
      expect([ask.slots, ask.kept]).toEqual([[0], referenceCaseBakeLights(EARTH_CASE)]);
    });

    it("holds a kept medium with no sources as a failed bake, under APPROXIMATE with its reason, warning once", () => {
      const h = harness(undefined, routedThick);
      const warnings = vi.spyOn(console, "warn").mockImplementation(ignore);
      const setting: KeptBodyAtmosphere = { kind: "medium", built: keptEarth(), labels: [] };
      note(h, [keptBody(setting)], 0, null);
      const report = note(h, [keptBody(setting)], 0, null);
      expect([
        h.builder.bakes.length,
        report.labels,
        [...report.reasons],
        report.problems.size,
        warnings.mock.calls.length,
      ]).toEqual([0, ["atmosphereApproximate"], [[A, [THICK_REASON]]], 0, 1]);
    });
  });
});

/** The latest polarisation correction asked, or a test failure. */
function latestCorrection(h: Harness): ScriptedCorrection {
  const correction = h.builder.corrections.at(-1);
  if (correction === undefined) {
    throw new Error("no polarisation correction was asked");
  }
  return correction;
}

/** R05's Earth as a thin assembly, its terms' sources by their channels: a world with air. */
function airAssembly(labels: ReadonlyArray<AtmosphereLabel> = []): AssembledMedium {
  return {
    medium: R05_EARTH_REFERENCE,
    drawn: {
      body: earthBody(),
      bottomRadiusM: 6_371_008.8,
      absorbers: [],
      spectra: [SUN_LIGHT.spectrum],
      curves: [[]],
      sources: R05_EARTH_REFERENCE.terms.map(channelSource),
    },
    labels,
    reasons: [],
    negligible: [],
    regime: { kind: "thin" },
  };
}

/** The product's choice of the bodies that take the correction once its route is on (R08.T14.f). */
function routedCorrects(column: SpectralColumn): boolean {
  return correctionRouted(column, true);
}

describe("a body's polarisation correction (R08.T14.f)", () => {
  /** A cache with A drawn from `assembled`, its correction asked as the routed product asks it. */
  async function correcting(
    assembled = airAssembly(),
    routes?: (regime: AtmosphereRegime) => boolean,
    rowBytes?: number,
  ): Promise<Harness> {
    const h = harness(rowBytes, routes, routedCorrects);
    await drawn(h, [lit(A)], assembled);
    return h;
  }

  /** Lands A's latest correction with a correction of its first tables' plan, one slot. */
  async function landCorrection(h: Harness): Promise<ReadonlyArray<PolarisationTable>> {
    const tables = patternedCorrection(madeAt(h, 0).plan.bands, R05_EARTH_REFERENCE, 1);
    latestCorrection(h).settle({ kind: "baked", tables });
    await microtasks();
    return tables;
  }

  it("asks every world with air its correction once its tables are made, every band and light slot in one request", async () => {
    const h = await correcting();
    const { ask } = latestCorrection(h);
    expect([
      h.builder.corrections.length,
      h.builder.bakes.length,
      ask.bands,
      ask.slots,
      ask.input === h.builder.latestOf(A).input,
      ask.column,
    ]).toEqual([
      1,
      0,
      [0],
      [0],
      true,
      spectralColumnOf(R05_EARTH_REFERENCE, R05_EARTH_REFERENCE.terms.map(channelSource)),
    ]);
  });

  it("asks a world with air its correction in the product, since the route (R08.T14.d.4)", async () => {
    const h = harness(undefined, "product", "product");
    const report = await drawn(h, [lit(A)], airAssembly());
    expect([h.builder.corrections.length, h.builder.bakes.length, report.labels]).toEqual([
      1,
      0,
      ["atmosphereComputing"],
    ]);
  });

  it("counts a thin world's correction in its tables' row, one slot a light, and none where it takes none (R08.T14.d.4)", async () => {
    const corrected = await correcting();
    const uncorrected = harness();
    await drawn(uncorrected, [lit(A)], airAssembly());
    expect([corrected.thickSlots, uncorrected.thickSlots]).toEqual([[1], [0]]);
  });

  it("shows COMPUTING while it runs, with no reason, and clears it once the correction lands in its tables", async () => {
    const h = await correcting();
    const during = note(h, [lit(A)]);
    const corrected = await landCorrection(h);
    const after = note(h, [lit(A)]);
    expect([
      [during.labels, [...during.reasons]],
      madeAt(h, 0).correctionHeld === corrected,
      [after.labels, [...after.reasons]],
      h.builder.corrections.length,
    ]).toEqual([[["atmosphereComputing"], []], true, [[], []], 1]);
  });

  it("asks an unrouted thick world no correction, its APPROXIMATE and its reason standing (R08.T14.d.4)", async () => {
    const h = await correcting(thickAssembly(), routesNone);
    const report = note(h, [lit(A)]);
    expect([
      h.builder.corrections.length,
      h.builder.bakes.length,
      [report.labels, [...report.reasons]],
    ]).toEqual([0, 0, [["atmosphereApproximate"], [[A, [THICK_REASON]]]]]);
  });

  it("shows no COMPUTING on a routed thick world whose bake landed while its correction runs, where a stand-in still holds APPROXIMATE", async () => {
    const h = await correcting(thickAssembly([STAND_IN, THICK_REASON]), routedThick);
    const tables = patternedThickBake(madeAt(h, 0).plan.bands, R05_EARTH_REFERENCE, 1);
    latestBake(h).settle({ kind: "baked", tables });
    await microtasks();
    const report = note(h, [lit(A)]);
    expect([report.labels, [...report.reasons]]).toEqual([
      ["atmosphereApproximate"],
      [[A, [STAND_IN]]],
    ]);
  });

  it("holds COMPUTING on a routed thick world until both its bake and its correction land", async () => {
    const h = await correcting(thickAssembly(), routedThick);
    const tables = patternedThickBake(madeAt(h, 0).plan.bands, R05_EARTH_REFERENCE, 1);
    latestBake(h).settle({ kind: "baked", tables });
    await microtasks();
    const baked = note(h, [lit(A)]);
    await landCorrection(h);
    const both = note(h, [lit(A)]);
    expect([
      baked.labels,
      both.labels,
      madeAt(h, 0).thickHeld === tables,
      madeAt(h, 0).correctionHeld !== null,
    ]).toEqual([["atmosphereComputing"], [], true, true]);
  });

  it("falls back to APPROXIMATE with its reason where the correction fails, raising no fault and asking none again", async () => {
    const h = await correcting();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    latestCorrection(h).settle({ kind: "failed", message: "the vector solver refused a layer" });
    await microtasks();
    const reports = [note(h, [lit(A)]), note(h, [lit(A)])];
    expect([
      reports.map((report) => [report.labels, [...report.reasons], report.problems.size]),
      h.cache.problemOf(A),
      h.builder.corrections.length,
      errors.mock.calls.length,
    ]).toEqual([
      Array.from({ length: 2 }, () => [["atmosphereApproximate"], [[A, [POLARISATION_REASON]]], 0]),
      null,
      1,
      1,
    ]);
  });

  it("gives its tables the correction at a later frame where the device was lost as it landed", async () => {
    const h = await correcting();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    const tables = madeAt(h, 0);
    tables.correctionThrows = new EngineUnavailable("createTexture");
    const corrected = await landCorrection(h);
    note(h, [lit(A)]);
    const during = tables.corrections.length;
    tables.correctionThrows = null;
    const later = note(h, [lit(A)]);
    expect([
      during,
      tables.correctionHeld === corrected,
      later.labels,
      errors.mock.calls.length,
    ]).toEqual([0, true, [], 0]);
  });

  it("fails the correction where its tables refuse it, with one console.error and no fault", async () => {
    const h = await correcting();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    madeAt(h, 0).correctionThrows = new RangeError("a correction of 2 bands");
    await landCorrection(h);
    const report = note(h, [lit(A)]);
    expect([
      report.labels,
      [...report.reasons],
      report.problems.size,
      errors.mock.calls.length,
    ]).toEqual([["atmosphereApproximate"], [[A, [POLARISATION_REASON]]], 0, 1]);
  });

  it("gives tables made again after an eviction the correction, asking none again", async () => {
    const h = await correcting(airAssembly(), undefined, 20_000_000);
    const corrected = await landCorrection(h);
    await drawn(h, [lit(bodyId(1))], airAssembly());
    const evicted = madeAt(h, 0).disposed;
    const report = note(h, [lit(A)]);
    const remade = h.made.find((tables, n) => n > 0 && tables.built === madeAt(h, 0).built);
    expect([
      evicted,
      remade?.correctionHeld === corrected,
      h.builder.corrections.length,
      report.labels,
    ]).toEqual([true, true, 2, []]);
  });

  it("counts the correction in tables made again after an eviction, as when first made (R08.T14.d.4)", async () => {
    const h = await correcting(airAssembly(), undefined, 20_000_000);
    await drawn(h, [lit(bodyId(1))], airAssembly());
    const evicted = madeAt(h, 0).disposed;
    note(h, [lit(A)]);
    const remade = h.made.findIndex((tables, n) => n > 0 && tables.built === madeAt(h, 0).built);
    expect([evicted, remade > 0, h.thickSlots[remade]]).toEqual([true, true, 1]);
  });

  it("asks none for a medium with no sources to bake from, drawn as before", async () => {
    const h = harness(undefined, undefined, routedCorrects);
    const report = await drawn(h, [lit(A)], earthAssembly());
    expect([h.builder.corrections.length, report.labels]).toEqual([0, []]);
  });

  it("cancels the correction when the body is dropped", async () => {
    const h = await correcting();
    vi.spyOn(console, "error").mockImplementation(ignore);
    h.view.reportDrawFailure(A, new Error("a bug in the draw"), "VIEW 1");
    await microtasks();
    expect(latestCorrection(h).cancelled).toBe(true);
  });

  it("cancels the correction when a new key's build lands, asking the new entry's", async () => {
    const h = await correcting();
    const first = latestCorrection(h);
    const lights = [SUN_LIGHT, { ...SUN_LIGHT, star: `${FIXTURE_SYSTEM}.0101` }];
    note(h, [lit(A, "disc", lights)]);
    await land(h, A, { kind: "built", assembled: airAssembly() });
    note(h, [lit(A, "disc", lights)]);
    expect([first.cancelled, h.builder.corrections.length]).toEqual([true, 2]);
  });
});

/** The latest disc table asked, or a test failure. */
function latestDisc(h: Harness): ScriptedDisc {
  const disc = h.builder.discs.at(-1);
  if (disc === undefined) {
    throw new Error("no disc table was asked");
  }
  return disc;
}

/** A self-luminous {@link airAssembly} or {@link thickAssembly}: its column at 2,600 K. */
function glowing(assembled: AssembledMedium): AssembledMedium {
  if (assembled.medium === undefined) {
    throw new Error("a glowing assembly draws a medium");
  }
  const temperature = {
    heightsM: Float64Array.of(0, 100_000),
    temperaturesK: Float64Array.of(2_600, 2_600),
  };
  return {
    ...assembled,
    drawn: { ...assembled.drawn, temperature },
    regime: { ...assembled.regime, selfLuminous: true },
  };
}

describe("a self-luminous medium's thermal source (R08.T18.b)", () => {
  it("builds its tables with the column's temperature, and none below the cut", async () => {
    const h = harness();
    await drawn(h, [lit(A)], glowing(airAssembly()));
    expect(madeAt(h, 0).built.temperature?.temperaturesK[0]).toBe(2_600);
    const cold = harness();
    await drawn(cold, [lit(A)], airAssembly());
    expect(madeAt(cold, 0).built.temperature).toBeUndefined();
  });

  it("asks its thick bake's column and its disc table with the temperature", async () => {
    const h = harness(undefined, routedThick);
    await drawn(h, [lit(A)], glowing(thickAssembly()));
    expect(latestBake(h).ask.column.temperature?.temperaturesK[0]).toBe(2_600);
    vi.spyOn(console, "error").mockImplementation(ignore);
    latestBake(h).settle({ kind: "failed", message: "refused" });
    await microtasks();
    expect(latestDisc(h).ask.air.temperature?.temperaturesK[0]).toBe(2_600);
  });
});

describe("a body's disc table (R08.T16.b)", () => {
  /** A disc table of the first tables' plan's bands, one slot, its values zero. */
  function aTable(h: Harness): DiscReflectanceTable {
    const bands = madeAt(h, 0).plan.bands.length;
    return {
      bands,
      slots: 1,
      sunFloor: -0.18,
      values: new Float32Array(bands * DISC_LAYER_VALUES),
    };
  }

  it("asks every drawn world with air its table once its tables are made, its thick bake's request with the medium as the tables hold it", async () => {
    const h = harness();
    await drawn(h, [lit(A)], airAssembly());
    const { ask } = latestDisc(h);
    expect([
      h.builder.discs.length,
      ask.bands,
      ask.slots,
      ask.input === h.builder.latestOf(A).input,
      ask.column,
      ask.air.medium,
      ask.air.kernelMedium,
      ask.air.curves,
      ask.air.bake,
      ask.air.correction,
    ]).toEqual([
      1,
      [0],
      [0],
      true,
      spectralColumnOf(R05_EARTH_REFERENCE, R05_EARTH_REFERENCE.terms.map(channelSource)),
      R05_EARTH_REFERENCE,
      kernelMedium(R05_EARTH_REFERENCE),
      madeAt(h, 0).built.curves,
      undefined,
      undefined,
    ]);
  });

  /** A cache with A drawn with air, its table asked, landed `table` (or {@link aTable}'s). */
  async function landed(h: Harness, table?: DiscReflectanceTable): Promise<DiscReflectanceTable> {
    await drawn(h, [lit(A)], airAssembly());
    const given = table ?? aTable(h);
    latestDisc(h).settle({ kind: "baked", table: given });
    await microtasks();
    return given;
  }

  it("gives the landed table to its tables, with no label of its own", async () => {
    const h = harness();
    const table = await landed(h);
    const after = note(h, [lit(A)]);
    expect([after.labels, madeAt(h, 0).discs]).toEqual([[], [table]]);
  });

  it("logs a failed table once, the body drawn by its full passes with no label or fault", async () => {
    const h = harness();
    await drawn(h, [lit(A)], airAssembly());
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    latestDisc(h).settle({ kind: "failed", message: "refused" });
    await microtasks();
    const report = note(h, [lit(A)]);
    expect([
      report.labels,
      report.problems.size,
      madeAt(h, 0).discs,
      errors.mock.calls.map(([text]) => String(text)),
    ]).toEqual([[], 0, [], [`the disc table of ${A} failed: refused`]]);
  });

  it("gives its tables the table at a later frame where the device was lost as it landed", async () => {
    const h = harness();
    await drawn(h, [lit(A)], airAssembly());
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    const tables = madeAt(h, 0);
    tables.discThrows = new EngineUnavailable("createTexture");
    const table = aTable(h);
    latestDisc(h).settle({ kind: "baked", table });
    await microtasks();
    note(h, [lit(A)]);
    const during = tables.discs.length;
    tables.discThrows = null;
    note(h, [lit(A)]);
    expect([during, tables.discs, errors.mock.calls.length]).toEqual([0, [table], 0]);
  });

  it("fails the table where its tables refuse it, with one console.error, giving it no more", async () => {
    const h = harness();
    await drawn(h, [lit(A)], airAssembly());
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    const tables = madeAt(h, 0);
    tables.discThrows = new RangeError("a table of 2 bands");
    latestDisc(h).settle({ kind: "baked", table: aTable(h) });
    await microtasks();
    tables.discThrows = null;
    const report = note(h, [lit(A)]);
    expect([report.labels, report.problems.size, tables.discs, errors.mock.calls.length]).toEqual([
      [],
      0,
      [],
      1,
    ]);
  });

  it("gives tables made again after an eviction the table, asking none again", async () => {
    const h = harness(20_000_000);
    const table = await landed(h);
    await drawn(h, [lit(bodyId(1))], airAssembly());
    const evicted = madeAt(h, 0).disposed;
    note(h, [lit(A)]);
    const remade = h.made.find((tables, n) => n > 0 && tables.built === madeAt(h, 0).built);
    expect([evicted, remade?.discs, h.builder.discs.length]).toEqual([true, [table], 2]);
  });

  it("asks a table that waited on a bake once an eviction's tables are made again", async () => {
    const h = harness(20_000_000, routedThick);
    await drawn(h, [lit(A)], thickAssembly());
    const bakeOfA = h.builder.bakes[0];
    if (bakeOfA === undefined) {
      throw new Error("A's thick bake was asked");
    }
    await drawn(h, [lit(bodyId(1))], thickAssembly());
    const evicted = madeAt(h, 0).disposed;
    const bake = patternedThickBake(madeAt(h, 0).plan.bands, R05_EARTH_REFERENCE, 1);
    bakeOfA.settle({ kind: "baked", tables: bake });
    await microtasks();
    const whileEvicted = h.builder.discs.length;
    note(h, [lit(A)]);
    expect([
      evicted,
      whileEvicted,
      h.builder.discs.length,
      latestDisc(h).ask.air.bake === bake,
    ]).toEqual([true, 0, 1, true]);
  });

  it("waits for a routed body's thick bake and correction, and reads J from both once landed", async () => {
    const h = harness(undefined, routedThick, routedCorrects);
    await drawn(h, [lit(A)], thickAssembly());
    expect(h.builder.discs).toHaveLength(0);
    const bake = patternedThickBake(madeAt(h, 0).plan.bands, R05_EARTH_REFERENCE, 1);
    latestBake(h).settle({ kind: "baked", tables: bake });
    await microtasks();
    expect(h.builder.discs).toHaveLength(0);
    const correction = patternedCorrection(madeAt(h, 0).plan.bands, R05_EARTH_REFERENCE, 1);
    latestCorrection(h).settle({ kind: "baked", tables: correction });
    await microtasks();
    const { ask } = latestDisc(h);
    expect([
      h.builder.discs.length,
      ask.air.bake === bake,
      ask.air.correction === correction,
    ]).toEqual([1, true, true]);
  });

  it("asks once a routed bake has failed, baking its own J", async () => {
    const h = harness(undefined, routedThick);
    await drawn(h, [lit(A)], thickAssembly());
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    latestBake(h).settle({ kind: "failed", message: "refused" });
    await microtasks();
    expect([h.builder.discs.length, latestDisc(h).ask.air.bake]).toEqual([1, undefined]);
  });

  it("asks none for a medium with no sources", async () => {
    const h = harness();
    const air = airAssembly();
    if (air.drawn === undefined) {
      throw new Error("the assembly draws a medium");
    }
    await drawn(h, [lit(A)], { ...air, drawn: { ...air.drawn, sources: [] } });
    expect(h.builder.discs).toHaveLength(0);
  });

  it("asks none for a body lit by no star, drawn by its full passes", async () => {
    const h = harness();
    await drawn(h, [lit(A, "disc", [])], airAssembly());
    expect([h.made.length, h.builder.discs.length]).toEqual([1, 0]);
  });

  it("cancels the table under way when the cache is disposed", async () => {
    const h = harness();
    await drawn(h, [lit(A)], airAssembly());
    const disc = latestDisc(h);
    h.cache.dispose();
    expect(disc.cancelled).toBe(true);
  });

  it("cancels the table under way when the body is dropped", async () => {
    const h = harness();
    await drawn(h, [lit(A)], airAssembly());
    vi.spyOn(console, "error").mockImplementation(ignore);
    h.view.reportDrawFailure(A, new Error("a bug in the draw"), "VIEW 1");
    await microtasks();
    expect(latestDisc(h).cancelled).toBe(true);
  });

  it("cancels the table under way when a new key's build lands, asking the new entry's", async () => {
    const h = harness();
    await drawn(h, [lit(A)], airAssembly());
    const first = latestDisc(h);
    const lights = [SUN_LIGHT, { ...SUN_LIGHT, star: `${FIXTURE_SYSTEM}.0101` }];
    note(h, [lit(A, "disc", lights)]);
    await land(h, A, { kind: "built", assembled: airAssembly() });
    note(h, [lit(A, "disc", lights)]);
    expect([first.cancelled, h.builder.discs.length]).toEqual([true, 2]);
  });
});

describe("the cache and the pool on the render thread", () => {
  const graph = [
    ...runTimeGraph(`${RENDERER_ROOT}view/atmosphere/AtmosphereCache.ts`),
    ...runTimeGraph(`${RENDERER_ROOT}view/atmosphere/opticsPool.ts`),
  ];

  it("load the tables' module, whose factory the app's cache calls", () => {
    expect(graph).toContain(`${RENDERER_ROOT}view/atmosphere/tables.ts`);
  });

  it("load none of the optics' assembly, data or worker, which stay in the optics workers", () => {
    const optics = graph.filter(
      (path) =>
        /\/view\/atmosphere\/(absorbers|assemble|aerosol|mie|negligible|opticsJobs|optics\.worker)\.ts$/u.test(
          path,
        ) || /\/view\/atmosphere\/(absorbers|materials)\//u.test(path),
    );
    expect(optics).toEqual([]);
  });

  it("load none of the thick bake's solver, its column's optics or the reference media, which stay in the optics workers (R08.T14.c)", () => {
    const thick = graph.filter((path) =>
      /\/view\/atmosphere\/(thick\/(bake|binConversion|discreteOrdinates|keptBake|polarisation|spectralColumn)|earth|reference\/[^/]+)\.ts$/u.test(
        path,
      ),
    );
    expect(thick).toEqual([]);
  });
});

describe("the cache over the optics pool", () => {
  it("draws a body assembled and curved in the pool's workers, with its tables", async () => {
    const seam = new BodyDetailSeam();
    const made: FakeTables[] = [];
    const pool = new OpticsWorkerPool({
      workers: 2,
      createWorker: () => new InProcessOpticsWorker(toyLoader()),
    });
    const cache = new AtmosphereCache<FakeTables>({
      requests: seam.client,
      builder: pool,
      tables: (built) => {
        const tables = new FakeTables(built, 1);
        made.push(tables);
        return tables;
      },
    });
    // The cache owns the pool: disposing it terminates the workers, however the test ends.
    onTestFinished(() => {
      cache.dispose();
    });
    const view = cache.openView();
    view.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(A)] });
    seam.answer(seam.latestFor(A).id, earthDetail());
    await vi.waitFor(
      () => {
        view.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(A)] });
        expect(made.length).toBe(1);
      },
      { timeout: 10_000, interval: 5 },
    );
    const built = made[0]?.built;
    const absorbing = built?.medium.terms.filter((term) => term.absorber !== undefined).length;
    expect([
      (absorbing ?? 0) > 0,
      built?.curves.length,
      built?.curves[0]?.length,
      cache.entryOf(A)?.tables === made[0],
    ]).toEqual([true, 1, absorbing, true]);
  });
});

/** The worker at `index`, or a test failure. */
function workerAt(
  workers: ReadonlyArray<ScriptedOpticsWorker>,
  index: number,
): ScriptedOpticsWorker {
  const worker = workers[index];
  if (worker === undefined) {
    throw new Error(`no worker ${index} was started`);
  }
  return worker;
}

/** The request a worker was last posted, or a test failure. */
function lastPosted(worker: ScriptedOpticsWorker): OpticsRequest {
  const request = worker.received.at(-1);
  if (request === undefined) {
    throw new Error("the worker was posted nothing");
  }
  return request;
}

describe("a thick body's bake over the optics pool (R08.T14.c)", () => {
  /** What a test drives: the cache, its view, and the pool's scripted workers. */
  interface OverPool {
    readonly workers: ScriptedOpticsWorker[];
    readonly view: AtmosphereView;
    readonly cache: AtmosphereCache<FakeTables>;
  }

  /**
   * A routed cache over a pool of one scripted worker, with A assembled thick in it and drawn, its
   * bake posted to the worker.
   */
  async function bakingOverPool(
    options: {
      readonly corrects?: (column: SpectralColumn) => boolean;
      readonly ownWavelengths?: (regime: AtmosphereRegime, medium: AtmosphereMedium) => boolean;
    } = {},
  ): Promise<OverPool> {
    const seam = new BodyDetailSeam();
    const workers: ScriptedOpticsWorker[] = [];
    const pool = new OpticsWorkerPool({
      workers: 1,
      createWorker: () => {
        const worker = new ScriptedOpticsWorker();
        workers.push(worker);
        return worker;
      },
    });
    const cache = new AtmosphereCache<FakeTables>({
      requests: seam.client,
      builder: pool,
      routes: routedThick,
      tables: (built) => new FakeTables(built, 1),
      // The bake's request alone: the correction's is posted in its own tests (R08.T14.f), and
      // the disc table's (R08.T16.b).
      corrects: options.corrects ?? (() => false),
      discs: () => false,
      ...(options.ownWavelengths === undefined ? {} : { ownWavelengths: options.ownWavelengths }),
    });
    onTestFinished(() => {
      cache.dispose();
    });
    const view = cache.openView();
    view.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(A)] });
    seam.answer(seam.latestFor(A).id, answerFor(A));
    await microtasks();
    const worker = workerAt(workers, 0);
    worker.reply({ kind: "assembled", id: lastPosted(worker).id, assembled: thickAssembly() });
    await microtasks();
    // The light's curve share, sent once the assembly freed the worker and dropped by the build,
    // which needs no curves: its late answer frees the worker for the bake.
    worker.reply({ kind: "curve-sums", id: lastPosted(worker).id, sums: [] });
    await microtasks();
    view.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(A)] });
    return { workers, view, cache };
  }

  /** The view's frame of A, after the replies given have been taken. */
  async function frame(over: OverPool): Promise<AtmosphereViewReport> {
    await microtasks();
    return over.view.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(A)] });
  }

  it("posts the bake asking its channels at their own wavelengths where the route says so, and in the product not yet (R08.T14.d.4)", async () => {
    const asked = async (
      ownWavelengths?: (regime: AtmosphereRegime, medium: AtmosphereMedium) => boolean,
    ): Promise<unknown> => {
      const over = await bakingOverPool(ownWavelengths === undefined ? {} : { ownWavelengths });
      const bake = lastPosted(workerAt(over.workers, 0));
      return bake.kind === "thick-bake" ? bake.ownWavelengths : bake.kind;
    };
    expect([await asked(() => true), await asked(() => false), await asked()]).toEqual([
      true,
      undefined,
      undefined,
    ]);
  });

  it("posts the polarisation correction after the bake, every band in one polarisation-bake request, and gives APPROXIMATE with its reason alone where it is answered failed (R08.T14.f)", async () => {
    const over = await bakingOverPool({
      corrects: routedCorrects,
    });
    const worker = workerAt(over.workers, 0);
    const bake = lastPosted(worker);
    if (bake.kind !== "thick-bake") {
      throw new Error(`the worker was posted ${bake.kind}, not the bake`);
    }
    worker.reply({
      kind: "thick-baked",
      id: bake.id,
      tables: patternedThickBake(thickBakeBands(bake.body), R05_EARTH_REFERENCE, 1),
    });
    const computing = await frame(over);
    const posted = lastPosted(worker);
    vi.spyOn(console, "error").mockImplementation(ignore);
    worker.reply({
      kind: "polarisation-bake-failed",
      id: posted.id,
      bands: [0],
      message: "the vector solver refused a layer",
    });
    const report = await frame(over);
    expect([
      worker.received.map((request) => request.kind),
      posted.kind === "polarisation-bake" ? posted.bands : [],
      computing.labels,
      report.labels,
      [...report.reasons],
      report.problems.size,
    ]).toEqual([
      ["assemble", "curve-sums", "thick-bake", "polarisation-bake"],
      [0],
      ["atmosphereComputing"],
      ["atmosphereApproximate"],
      [[A, [POLARISATION_REASON]]],
      0,
    ]);
  });

  it("posts the bake, every band in one thick-bake request, once the body is drawn", async () => {
    const { workers } = await bakingOverPool();
    const worker = workerAt(workers, 0);
    const posted = lastPosted(worker);
    expect([
      worker.received.map((request) => request.kind),
      posted.kind === "thick-bake" ? posted.bands : [],
    ]).toEqual([["assemble", "curve-sums", "thick-bake"], [0]]);
  });

  it("gives APPROXIMATE with THICK_REASON and no fault for a thick-bake-failed answer", async () => {
    const over = await bakingOverPool();
    vi.spyOn(console, "error").mockImplementation(ignore);
    const worker = workerAt(over.workers, 0);
    worker.reply({
      kind: "thick-bake-failed",
      id: lastPosted(worker).id,
      bands: [0],
      message: "the solver refused a layer",
    });
    const report = await frame(over);
    expect([
      report.labels,
      [...report.reasons],
      report.problems.size,
      over.cache.problemOf(A),
    ]).toEqual([["atmosphereApproximate"], [[A, [THICK_REASON]]], 0, null]);
  });

  it("keeps COMPUTING where the bake's worker is lost, the bake asked again of its replacement", async () => {
    const over = await bakingOverPool();
    workerAt(over.workers, 0).fail();
    const report = await frame(over);
    expect([lastPosted(workerAt(over.workers, 1)).kind, report.labels]).toEqual([
      "thick-bake",
      ["atmosphereComputing"],
    ]);
  });

  it("gives APPROXIMATE with THICK_REASON and no fault once every worker is given up", async () => {
    const over = await bakingOverPool();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    for (let n = 0; n < MAX_CONSECUTIVE_OPTICS_FAILURES; n += 1) {
      workerAt(over.workers, n).fail();
    }
    const report = await frame(over);
    expect([
      report.labels,
      [...report.reasons],
      report.problems.size,
      errors.mock.calls.map(([text]) => String(text)),
    ]).toEqual([
      ["atmosphereApproximate"],
      [[A, [THICK_REASON]]],
      0,
      [`the thick bake of ${A} failed: every optics worker was given up`],
    ]);
  });
});

/** A kept frame of `bodies`, each with its setting and, where given, where the camera stands. */
function keptFrame(
  h: Harness,
  bodies: ReadonlyArray<readonly [BodyIdHex, KeptBodyAtmosphere, AtmosphereObserver | undefined]>,
): AtmosphereViewReport {
  return h.view.noteFrame({
    nowMs: 0,
    scene: null,
    bodies: bodies.map(([id, kept, observer]) => ({
      id,
      regime: "disc" as const,
      lights: [SUN_LIGHT],
      kept,
      ...(observer === undefined ? {} : { observer }),
    })),
  });
}

describe("the product's route (R08.T14.d.4)", () => {
  const EARTH: KeptBodyAtmosphere = {
    kind: "medium",
    built: builtOfCase(EARTH_CASE, "earth"),
    labels: [],
  };
  const MARS: KeptBodyAtmosphere = {
    kind: "medium",
    built: builtOfCase(MARS_CASE, "mars"),
    labels: [],
  };
  const B = bodyId(2);

  /** A's thick bake landed: the patterned table on its tables' plan. */
  async function landBake(h: Harness): Promise<void> {
    const tables = patternedThickBake(madeAt(h, 0).plan.bands, kernelMedium(EARTH_CASE.medium), 1);
    latestBake(h).settle({ kind: "baked", tables });
    await microtasks();
  }

  /** A's correction landed: the patterned correction on its tables' plan. */
  async function landCorrectionOfA(h: Harness): Promise<void> {
    latestCorrection(h).settle({
      kind: "baked",
      tables: patternedCorrection(madeAt(h, 0).plan.bands, EARTH_CASE.medium, 1),
    });
    await microtasks();
  }

  it("bakes and corrects an Earth-class world, and leaves a Mars-class one APPROXIMATE with neither", () => {
    const h = harness(undefined, "product", "product");
    keptFrame(h, [
      [A, EARTH, undefined],
      [B, MARS, undefined],
    ]);
    expect([
      h.builder.bakes.length,
      h.builder.corrections.length,
      h.cache.entryOf(A)?.labels,
      h.cache.entryOf(B)?.labels,
      h.cache.entryOf(B)?.reasons,
    ]).toEqual([1, 1, ["atmosphereComputing"], ["atmosphereApproximate"], [THICK_REASON]]);
  });

  it("counts an Earth-class world's thick table in its tables' row, one slot a light, and none for a Mars-class one", () => {
    const h = harness(undefined, "product", "product");
    keptFrame(h, [
      [A, EARTH, undefined],
      [B, MARS, undefined],
    ]);
    expect(h.thickSlots).toEqual([1, 0]);
  });

  it("clears an Earth-class world's APPROXIMATE once its bake and correction land", async () => {
    const h = harness(undefined, "product", "product");
    keptFrame(h, [[A, EARTH, undefined]]);
    await landBake(h);
    await landCorrectionOfA(h);
    const report = keptFrame(h, [[A, EARTH, undefined]]);
    expect([report.labels, [...report.reasons]]).toEqual([[], []]);
  });

  it("says a baked world's twilight is past its range where the camera in its air has its star below the local horizontal", async () => {
    const h = harness(undefined, "product", "product");
    keptFrame(h, [[A, EARTH, undefined]]);
    await landBake(h);
    await landCorrectionOfA(h);
    const at = (heightM: number, lightElevationSine: number): AtmosphereViewReport =>
      keptFrame(h, [[A, EARTH, { heightM, lightElevationSine }]]);
    const below = at(2, -0.05);
    expect([
      [below.labels, [...below.reasons]],
      at(2, 0.05).reasons.get(A),
      at(2e6, -0.05).reasons.get(A),
    ]).toEqual([[["atmosphereApproximate"], [[A, [TWILIGHT_REASON]]]], undefined, undefined]);
  });

  it("shows COMPUTING in twilight while the bake runs, and nothing of twilight", () => {
    const h = harness(undefined, "product", "product");
    const baking = keptFrame(h, [[A, EARTH, { heightM: 2, lightElevationSine: -0.5 }]]);
    expect([baking.labels, [...baking.reasons]]).toEqual([["atmosphereComputing"], []]);
  });

  it("shows twilight's reason in place of the correction's COMPUTING, which leaves it", async () => {
    const h = harness(undefined, "product", "product");
    keptFrame(h, [[A, EARTH, undefined]]);
    await landBake(h);
    const correcting = keptFrame(h, [[A, EARTH, { heightM: 2, lightElevationSine: -0.5 }]]);
    expect([correcting.labels, [...correcting.reasons]]).toEqual([
      ["atmosphereApproximate"],
      [[A, [TWILIGHT_REASON]]],
    ]);
  });
});
