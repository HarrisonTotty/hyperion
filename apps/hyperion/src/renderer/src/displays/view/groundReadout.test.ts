import type { BodyIdHex, UniverseTime } from "@hyperion/protocol";
import { describe, expect, it, vi } from "vitest";

import { add, type Vec3, vec3 } from "../../geometry/vec3";
import { moonLikeHeader } from "../../test/coarseFieldFixtures";
import {
  aBody,
  aViewScene,
  FIXTURE_PLANET,
  FIXTURE_PLANET_CENTRE_M,
} from "../../test/viewFixtures";
import type { CameraState, ViewId } from "../../view/camera/state";
import { staticRetarded, type ViewBody, type ViewScene } from "../../view/scene/model";
import { ECLIPSE_GIANT, ECLIPSE_PLANET, eclipseScene } from "../../view/scenes/eclipseScene";
import { PHASE_GIANT, phaseScene } from "../../view/scenes/phaseScene";
import { precisionScene } from "../../view/scenes/precision";
import {
  terrainMoon,
  terrainScene,
  terrainStar,
  terrainSystem,
  terrainTestGolden,
} from "../../view/scenes/terrainScene";
import {
  ANNUNCIATION_ONSET_MS,
  TERRAIN_COMPUTATION_FAILED,
  type TerrainFault,
} from "../../view/terrain/annunciation";
import type { FieldStatus } from "../../view/terrain/coarseFieldStore";
import { CoverageView } from "../../view/terrain/coverage";
import { lineOfSightOf } from "../../view/terrain/readouts";
import type {
  GroundReadoutOptions,
  GroundSight,
  TerrainSession,
  TerrainSurvey,
} from "../../view/terrain/session";
import type { SurfaceGroundReadout } from "../../view/terrain/workers/messages";
import {
  GROUND_READOUT_OPTIONS,
  groundBodyOf,
  GroundReadouts,
  litTimeOf,
  readoutBodyOf,
  surfaceUnsetOf,
} from "./groundReadout";
import { instrumentViewId, PRIMARY_VIEW_ID } from "./viewNames";
import { runPose, startRun, startServerRun, type ViewRun } from "./viewRun";

/** What `GroundReadouts` calls of a terrain session, which the fake gives. */
type ReadoutSession = Pick<
  TerrainSession,
  "survey" | "terrainFault" | "groundReadout" | "failComputation"
>;

/** One terrain readout asked of {@link FakeReadoutSession}: its arguments, and its settling. */
interface Ask {
  readonly view: ViewId;
  readonly body: BodyIdHex;
  readonly sight: GroundSight;
  readonly time: UniverseTime;
  readonly options: GroundReadoutOptions;
  /** Answers it, as the session's worker does. */
  readonly answer: (readout: SurfaceGroundReadout | null) => void;
  /** Refuses it, as a worker's failure or a setting out of range does. */
  readonly refuse: (error: Error) => void;
}

/**
 * A terrain session stand-in: the surveys and the fault the test sets, each readout asked held
 * until the test settles it, and the terrain's faults counted.
 */
class FakeReadoutSession implements ReadoutSession {
  /** The surveys it gives, by body; none for a body it does not hold. */
  readonly surveys = new Map<BodyIdHex, TerrainSurvey>();
  terrainFault: TerrainFault | null = null;
  /** Every readout asked of it, in order. */
  readonly asks: Ask[] = [];
  /** How many times the terrain was faulted (`failComputation`). */
  faults = 0;

  survey(body: BodyIdHex): TerrainSurvey | null {
    return this.surveys.get(body) ?? null;
  }

  groundReadout(
    view: ViewId,
    body: BodyIdHex,
    sight: GroundSight,
    time: UniverseTime,
    options: GroundReadoutOptions,
  ): Promise<SurfaceGroundReadout | null> {
    return new Promise((resolve, reject) => {
      this.asks.push({ view, body, sight, time, options, answer: resolve, refuse: reject });
    });
  }

  failComputation(): void {
    this.faults += 1;
  }
}

/**
 * The fake as `GroundReadouts.publish` takes it. A trust-boundary cast: `TerrainSession` holds
 * private fields, which no stand-in can match structurally, and `GroundReadouts` calls only the
 * members {@link ReadoutSession} picks, which the fake implements.
 */
function asSession(fake: FakeReadoutSession): TerrainSession {
  // The trust boundary: the fake has every member `GroundReadouts` reads, but no private field.
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return fake as unknown as TerrainSession;
}

/** The `n`th readout asked of `fake`, which must have been asked. */
function askAt(fake: FakeReadoutSession, n: number): Ask {
  const ask = fake.asks[n];
  if (ask === undefined) {
    throw new Error(`readout ${String(n)} was not asked; ${String(fake.asks.length)} were`);
  }
  return ask;
}

/** Resolves after `hops` turns of the microtask queue. */
function microtasks(hops: number): Promise<void> {
  return hops === 0 ? Promise.resolve() : Promise.resolve().then(() => microtasks(hops - 1));
}

/** Lets a settled readout's outcome reach `GroundReadouts`, held for its next publish. */
async function settle(): Promise<void> {
  await microtasks(10);
}

/** A field's standing: wanted as the local body, answered, nothing failed, by default. */
function standing(overrides: Partial<FieldStatus> = {}): FieldStatus {
  return { reason: "local-body", pending: false, notModelled: null, failure: null, ...overrides };
}

/** `TEST MOON`'s survey: the terrain golden's header held, live, nothing surveyed, by default. */
function aSurvey(overrides: Partial<TerrainSurvey> = {}): TerrainSurvey {
  return {
    status: standing(),
    coverage: CoverageView.empty(5),
    header: terrainTestGolden().header,
    stale: false,
    ...overrides,
  };
}

/** A session holding `TEST MOON`'s survey, as `TERRAIN TEST`'s fixture comes to. */
function holdingMoon(): FakeReadoutSession {
  const fake = new FakeReadoutSession();
  fake.surveys.set(terrainMoon(), aSurvey());
  return fake;
}

/** `TERRAIN TEST` at its start: the seat on the own ship, the ship held in `TEST MOON`'s frame. */
function terrainRun(): ViewRun {
  return startRun(terrainScene());
}

/** `ECLIPSE TEST` at its start: its free camera held in `TEST PLANET`'s body frame. */
function eclipseRun(): ViewRun {
  return startRun(eclipseScene());
}

/** `run` with its camera's pose put in `body`'s frame, `positionM` from its centre. */
function framedOn(run: ViewRun, body: BodyIdHex, positionM: Vec3 = vec3(0, 0, 1e8)): ViewRun {
  const camera: CameraState = {
    ...run.camera,
    pose: { ...run.camera.pose, frame: { kind: "body", body }, positionM },
  };
  return { ...run, camera };
}

/** `run` with `change` made to each of its scene's bodies. */
function withBodies(
  run: ViewRun,
  change: (bodies: ReadonlyArray<ViewBody>) => ViewBody[],
): ViewRun {
  const scene: ViewScene = { ...run.scene, bodies: change(run.scene.bodies) };
  return { ...run, scene };
}

/** `body` as a kept scene gives it with no terrain: its detail seed left out. */
function withoutDetailSeed(body: ViewBody): ViewBody {
  const { detailSeed, ...rest } = body;
  return detailSeed === undefined ? body : rest;
}

/** `TERRAIN TEST` at its start with `TEST MOON`'s detail seed left out, as a kept scene may. */
function seedlessTerrainRun(): ViewRun {
  return withBodies(terrainRun(), (bodies) =>
    bodies.map((body) => (body.id === terrainMoon() ? withoutDetailSeed(body) : body)),
  );
}

/** A second moon of `TERRAIN TEST`, `TEST MOON`'s twin 10⁶ km beyond it, with its own survey. */
const OTHER_MOON: BodyIdHex = `${terrainSystem()}.0004`;

/** `TERRAIN TEST` at its start with {@link OTHER_MOON} beside `TEST MOON`. */
function twoMoonRun(): ViewRun {
  return withBodies(terrainRun(), (bodies) => {
    const moon = bodies.find((body) => body.id === terrainMoon());
    if (moon === undefined) {
      throw new Error("TERRAIN TEST holds no TEST MOON");
    }
    const centreM = add(moon.centreM, vec3(1e9, 0, 0));
    return [
      ...bodies,
      {
        ...moon,
        id: OTHER_MOON,
        designation: "OTHER MOON",
        centreM,
        retarded: staticRetarded(centreM),
      },
    ];
  });
}

/** `TEST MOON`'s readout where the line of sight meets ground the ship has not surveyed. */
const NOT_SURVEYED: SurfaceGroundReadout = { kind: "not-surveyed" };

/** A readout where the line of sight meets no ground. */
const NO_GROUND: SurfaceGroundReadout = { kind: "no-ground" };

/** A readout where the line of sight meets surveyed ground 41.2 km away. */
const ON_GROUND: SurfaceGroundReadout = {
  kind: "ground",
  sight: {
    rangeM: 41_234,
    sigmaM: 290,
    resolved: false,
    hitM: [1_738_000, 0, 0],
    at: { face: 0, u: 0, v: 0 },
  },
  reading: {
    elevationM: 2_143.7,
    elevationSigmaM: 180,
    slopeRmsRad: 0.21,
    slopeSigmaRad: 0.07,
    slopeBaselineM: 2,
    resolutionM: 410,
    resolved: false,
  },
  material: { classCode: 1, entry: 0 },
  properties: {
    bearingCapacityPa: 3_432_000,
    capacitySlopeRad: 0.21,
    padFriction: 0.2,
    frictionOnTemperature: false,
    temperatureK: 250,
  },
};

/** A bug of a worker's, by which a readout is refused. */
const BUG = new Error("a worker's bug");

/**
 * Publishes `run` at each of `timesMs` in turn, refusing the readout each publish asks before the
 * next, so that each publish takes the refusal before it.
 */
async function refuseAt(
  readouts: GroundReadouts,
  fake: FakeReadoutSession,
  run: ViewRun,
  timesMs: ReadonlyArray<number>,
): Promise<void> {
  const [atMs, ...rest] = timesMs;
  if (atMs === undefined) {
    return;
  }
  readouts.publish(asSession(fake), run, atMs);
  fake.asks.at(-1)?.refuse(BUG);
  await settle();
  await refuseAt(readouts, fake, run, rest);
}

/**
 * A primary's readouts on `TERRAIN TEST` with {@link NOT_SURVEYED} taken at its second publish, at
 * 16 ms, its second readout asked and still out.
 */
async function withAnswerHeld(): Promise<{
  readonly fake: FakeReadoutSession;
  readonly readouts: GroundReadouts;
  readonly run: ViewRun;
}> {
  const fake = holdingMoon();
  const readouts = new GroundReadouts(PRIMARY_VIEW_ID);
  const run = terrainRun();
  readouts.publish(asSession(fake), run, 0);
  askAt(fake, 0).answer(NOT_SURVEYED);
  await settle();
  const taken = readouts.publish(asSession(fake), run, 16);
  if (taken?.readout !== NOT_SURVEYED) {
    throw new Error("the first answer was not taken");
  }
  return { fake, readouts, run };
}

/** The bodies of `scene` it marks as having no surface. */
function surfacelessOf(scene: ViewScene): BodyIdHex[] {
  return scene.bodies.filter((body) => body.surfaceless === true).map((body) => body.id);
}

describe("the body whose terrain a view states (R10.T12; decision-r10-t12-commit3 §5)", () => {
  it("is TERRAIN TEST's moon for the seat of its ship, which the moon's frame holds", () => {
    const run = terrainRun();
    expect([run.camera.pose.frame.kind, groundBodyOf(run)?.id]).toEqual(["craft", terrainMoon()]);
  });

  it.each([
    ["PRECISION TEST's system frame", () => startRun(precisionScene())],
    ["TERRAIN TEST's star", () => framedOn(terrainRun(), terrainStar(), vec3(0, 0, 1e10))],
  ])("is none for %s", (_, run) => {
    expect(groundBodyOf(run())).toBeNull();
  });

  it("is none for a FREE camera put in the frame of ECLIPSE TEST's giant, which has no surface", () => {
    const run = framedOn(eclipseRun(), ECLIPSE_GIANT);
    expect({ preset: run.camera.preset, body: groundBodyOf(run) }).toEqual({
      preset: "free",
      body: null,
    });
  });

  it("marks the giants of ECLIPSE TEST and PHASE TEST as having no surface, and no other body of theirs", () => {
    expect([
      surfacelessOf(eclipseScene().sceneAt(0)),
      surfacelessOf(phaseScene().sceneAt(0)),
    ]).toEqual([[ECLIPSE_GIANT], [PHASE_GIANT]]);
  });

  it.each([
    ["ECLIPSE TEST's planet, a kept body with no detail seed", eclipseRun, ECLIPSE_PLANET, true],
    ["TERRAIN TEST's moon, a kept body with one", terrainRun, terrainMoon(), false],
    [
      "a server's planet, whose terrain its survey answers",
      () => startServerRun(aViewScene()),
      FIXTURE_PLANET,
      false,
    ],
  ])("tells whether a kept scene leaves the surface unset for %s", (_, run, body, unset) => {
    const made = run();
    const framed = groundBodyOf(made);
    expect([framed?.id, framed === null ? null : surfaceUnsetOf(made, framed)]).toEqual([
      body,
      unset,
    ]);
  });

  it.each([
    ["TERRAIN TEST's moon", terrainRun, terrainMoon()],
    ["a server's planet", () => startServerRun(aViewScene()), FIXTURE_PLANET],
    ["none for ECLIPSE TEST's planet, whose surface a kept scene leaves unset", eclipseRun, null],
    ["none for PRECISION TEST's system frame", () => startRun(precisionScene()), null],
  ])("states the readout of %s", (_, run, body) => {
    expect(readoutBodyOf(run())).toBe(body);
  });
});

describe("the instant a view lights a body at (decision-r10-t12-leans §6 (c))", () => {
  it("is the scene's time less the body's light time, to the nanosecond", () => {
    const run = startServerRun(aViewScene({ time: { seconds: 1_000_000, nanos: 3_000_000 } }));
    // About an astronomical unit's light time, exact in binary64: 499 s and 4,882,812.5 ns.
    const body = aBody({
      retarded: {
        centreM: FIXTURE_PLANET_CENTRE_M,
        velocityMPerS: vec3(0, 0, 0),
        lightTimeS: 499.004_882_812_5,
      },
    });
    expect(litTimeOf(run, body)).toEqual({ seconds: 999_500, nanos: 998_117_188 });
  });

  it.each([
    ["the light time is 0, as in a kept scene", staticRetarded(FIXTURE_PLANET_CENTRE_M)],
    ["the body's retarded centre is not known", null],
  ])("is the scene's time where %s", (_, retarded) => {
    const run = startServerRun(aViewScene({ time: { seconds: 1_000_000, nanos: 3_000_000 } }));
    expect(litTimeOf(run, aBody({ retarded }))).toEqual({ seconds: 1_000_000, nanos: 3_000_000 });
  });
});

describe("the primary's terrain readout at each publish (R10.T12; decision-r10-t12-leans §3, §5–§7)", () => {
  it.each([
    ["PRECISION TEST's system frame", () => startRun(precisionScene())],
    ["TERRAIN TEST's star", () => framedOn(terrainRun(), terrainStar(), vec3(0, 0, 1e10))],
    ["ECLIPSE TEST's giant, which has no surface", () => framedOn(eclipseRun(), ECLIPSE_GIANT)],
  ])("states nothing and asks nothing for %s", (_, run) => {
    const fake = holdingMoon();
    fake.surveys.set(terrainStar(), aSurvey());
    fake.surveys.set(ECLIPSE_GIANT, aSurvey({ header: moonLikeHeader(ECLIPSE_GIANT) }));
    const snapshot = new GroundReadouts(PRIMARY_VIEW_ID).publish(asSession(fake), run(), 0);
    expect({ snapshot, asks: fake.asks.length }).toEqual({ snapshot: null, asks: 0 });
  });

  it("reads ECLIPSE TEST's planet, framed by its free camera, as a body whose surface is unset", () => {
    const snapshot = new GroundReadouts(PRIMARY_VIEW_ID).publish(null, eclipseRun(), 0);
    expect(snapshot).toEqual({
      body: ECLIPSE_PLANET,
      surfaceUnset: true,
      survey: null,
      readout: null,
      sessionFault: null,
      slantFromCamera: true,
    });
  });

  it.each([
    ["with a session", true],
    ["without one", false],
  ])(
    "reads a kept body with no detail seed as one whose surface is unset %s, and asks nothing",
    (_, withSession) => {
      const fake = holdingMoon();
      const snapshot = new GroundReadouts(PRIMARY_VIEW_ID).publish(
        withSession ? asSession(fake) : null,
        seedlessTerrainRun(),
        0,
      );
      expect({ snapshot, asks: fake.asks.length }).toEqual({
        snapshot: {
          body: terrainMoon(),
          surfaceUnset: true,
          survey: null,
          readout: null,
          sessionFault: null,
          slantFromCamera: false,
        },
        asks: 0,
      });
    },
  );

  it("states nothing for a server's body with no session", () => {
    expect(
      new GroundReadouts(PRIMARY_VIEW_ID).publish(null, startServerRun(aViewScene()), 0),
    ).toBeNull();
  });

  it("asks nothing where the body's rotation is not known, a server's planet's here", () => {
    const fake = new FakeReadoutSession();
    fake.surveys.set(FIXTURE_PLANET, aSurvey({ header: moonLikeHeader(FIXTURE_PLANET) }));
    const snapshot = new GroundReadouts(PRIMARY_VIEW_ID).publish(
      asSession(fake),
      startServerRun(aViewScene()),
      0,
    );
    expect({ body: snapshot?.body, readout: snapshot?.readout, asks: fake.asks.length }).toEqual({
      body: FIXTURE_PLANET,
      readout: null,
      asks: 0,
    });
  });

  it.each([
    ["no survey of the body is wanted", null],
    [
      "its survey is pending",
      aSurvey({ coverage: null, header: null, status: standing({ pending: true }) }),
    ],
  ])("asks nothing before a header is held: %s", (_, survey) => {
    const fake = new FakeReadoutSession();
    if (survey !== null) {
      fake.surveys.set(terrainMoon(), survey);
    }
    const readouts = new GroundReadouts(PRIMARY_VIEW_ID);
    const run = terrainRun();
    const snapshots = [0, 16, 33].map((atMs) => readouts.publish(asSession(fake), run, atMs));
    expect({ readouts: snapshots.map((s) => s?.readout), asks: fake.asks.length }).toEqual({
      readouts: [null, null, null],
      asks: 0,
    });
  });

  it("reads SLOPE over the 2 m baseline and the capacity under the 1.0 m reference pad", () => {
    expect(GROUND_READOUT_OPTIONS).toEqual({ slopeBaselineM: 2, padWidthM: 1 });
  });

  it("asks along the view's line of sight at the instant it lights the body, with the readout's settings and the view's identity", () => {
    const fake = holdingMoon();
    const view = instrumentViewId(1);
    const run = terrainRun();
    new GroundReadouts(view).publish(asSession(fake), run, 0);
    const moon = groundBodyOf(run);
    if (moon === null) {
      throw new Error("TERRAIN TEST frames no body");
    }
    const ask = askAt(fake, 0);
    expect({
      asks: fake.asks.length,
      view: ask.view,
      body: ask.body,
      sight: ask.sight,
      time: ask.time,
      options: ask.options,
    }).toEqual({
      asks: 1,
      view,
      body: terrainMoon(),
      sight: lineOfSightOf(runPose(run), run.scene, moon, terrainTestGolden().header),
      time: litTimeOf(run, moon),
      options: GROUND_READOUT_OPTIONS,
    });
  });

  it("asks once at each publish whose answer has come", async () => {
    const fake = holdingMoon();
    const readouts = new GroundReadouts(PRIMARY_VIEW_ID);
    const run = terrainRun();
    readouts.publish(asSession(fake), run, 0);
    askAt(fake, 0).answer(NOT_SURVEYED);
    await settle();
    readouts.publish(asSession(fake), run, 16);
    askAt(fake, 1).answer(NO_GROUND);
    await settle();
    readouts.publish(asSession(fake), run, 33);
    expect(fake.asks.length).toBe(3);
  });

  it("keeps at most one readout in flight", () => {
    const fake = holdingMoon();
    const readouts = new GroundReadouts(PRIMARY_VIEW_ID);
    const run = terrainRun();
    for (const atMs of [0, 16, 33, 50]) {
      readouts.publish(asSession(fake), run, atMs);
    }
    expect(fake.asks.length).toBe(1);
  });

  it("states an answer from the publish after it came, and none at the publish that asked it", async () => {
    const fake = holdingMoon();
    const readouts = new GroundReadouts(PRIMARY_VIEW_ID);
    const run = terrainRun();
    const asking = readouts.publish(asSession(fake), run, 0);
    askAt(fake, 0).answer(NOT_SURVEYED);
    await settle();
    const next = readouts.publish(asSession(fake), run, 16);
    expect([asking?.readout, next?.readout]).toEqual([null, NOT_SURVEYED]);
  });

  it("keeps the last answer while the next is out", async () => {
    const { fake, readouts, run } = await withAnswerHeld();
    const snapshots = [33, 50].map((atMs) => readouts.publish(asSession(fake), run, atMs));
    expect(snapshots.map((s) => s?.readout)).toEqual([NOT_SURVEYED, NOT_SURVEYED]);
  });

  it("drops an answer for a body no longer framed", async () => {
    const fake = holdingMoon();
    fake.surveys.set(OTHER_MOON, aSurvey({ header: moonLikeHeader(OTHER_MOON) }));
    const readouts = new GroundReadouts(PRIMARY_VIEW_ID);
    const run = twoMoonRun();
    const onOther = framedOn(run, OTHER_MOON, vec3(0, 0, 1e7));
    readouts.publish(asSession(fake), run, 0);
    readouts.publish(asSession(fake), onOther, 16);
    askAt(fake, 0).answer(NOT_SURVEYED);
    await settle();
    const snapshot = readouts.publish(asSession(fake), onOther, 33);
    expect({
      asked: fake.asks.map((ask) => ask.body),
      body: snapshot?.body,
      readout: snapshot?.readout,
    }).toEqual({ asked: [terrainMoon(), OTHER_MOON], body: OTHER_MOON, readout: null });
  });

  it("drops an answer for a session let go", async () => {
    const before = holdingMoon();
    const after = holdingMoon();
    const readouts = new GroundReadouts(PRIMARY_VIEW_ID);
    const run = terrainRun();
    readouts.publish(asSession(before), run, 0);
    readouts.publish(asSession(after), run, 16);
    askAt(before, 0).answer(NOT_SURVEYED);
    await settle();
    expect(readouts.publish(asSession(after), run, 33)?.readout).toBeNull();
  });

  it("keeps the last answer through a refusal", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { fake, readouts, run } = await withAnswerHeld();
    askAt(fake, 1).refuse(BUG);
    await settle();
    expect(readouts.publish(asSession(fake), run, 33)?.readout).toBe(NOT_SURVEYED);
  });

  it("reports a run of refusals once", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const fake = holdingMoon();
    await refuseAt(new GroundReadouts(PRIMARY_VIEW_ID), fake, terrainRun(), [0, 16, 33, 50]);
    expect(error.mock.calls).toEqual([["the terrain readout's query failed:", BUG]]);
  });

  it("faults the terrain once its refusals have lasted ANNUNCIATION_ONSET_MS, and once only", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const fake = holdingMoon();
    const readouts = new GroundReadouts(PRIMARY_VIEW_ID);
    const run = terrainRun();
    // The first refusal is taken at 100 ms.
    await refuseAt(readouts, fake, run, [0, 100, 100 + ANNUNCIATION_ONSET_MS - 1]);
    const before = fake.faults;
    await refuseAt(readouts, fake, run, [
      100 + ANNUNCIATION_ONSET_MS,
      100 + ANNUNCIATION_ONSET_MS + 16,
    ]);
    expect([before, fake.faults]).toEqual([0, 1]);
  });

  it("ends a run of refusals at an answer", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const fake = holdingMoon();
    const readouts = new GroundReadouts(PRIMARY_VIEW_ID);
    const run = terrainRun();
    // Refusals from 100 ms, an answer taken at 200 ms, refusals again from 200 ms on.
    await refuseAt(readouts, fake, run, [0, 100]);
    readouts.publish(asSession(fake), run, 150);
    askAt(fake, 2).answer(NOT_SURVEYED);
    await settle();
    await refuseAt(readouts, fake, run, [
      200,
      100 + ANNUNCIATION_ONSET_MS,
      100 + ANNUNCIATION_ONSET_MS + 50,
    ]);
    expect(fake.faults).toBe(0);
  });

  /** What withholds the readout, how to bring it about on a session, and how to clear it. */
  const WITHHELD: ReadonlyArray<
    readonly [string, (fake: FakeReadoutSession) => void, (fake: FakeReadoutSession) => void]
  > = [
    [
      "the session's terrain is faulted",
      (fake) => {
        fake.terrainFault = TERRAIN_COMPUTATION_FAILED;
      },
      (fake) => {
        fake.terrainFault = null;
      },
    ],
    [
      "the body's terrain is not modelled",
      (fake) => {
        fake.surveys.set(
          terrainMoon(),
          aSurvey({ coverage: null, header: null, status: standing({ notModelled: "surface" }) }),
        );
      },
      (fake) => {
        fake.surveys.set(terrainMoon(), aSurvey());
      },
    ],
    [
      "the survey's header is dropped",
      (fake) => {
        fake.surveys.set(
          terrainMoon(),
          aSurvey({
            coverage: null,
            header: null,
            status: standing({ failure: { kind: "invalid", check: "payload cut short" } }),
          }),
        );
      },
      (fake) => {
        fake.surveys.set(terrainMoon(), aSurvey());
      },
    ],
  ];

  it.each(WITHHELD)("states no readout and asks nothing while %s", async (_, raise) => {
    const { fake, readouts, run } = await withAnswerHeld();
    raise(fake);
    const snapshots = [33, 50].map((atMs) => readouts.publish(asSession(fake), run, atMs));
    expect({ readouts: snapshots.map((s) => s?.readout), asks: fake.asks.length }).toEqual({
      readouts: [null, null],
      asks: 2,
    });
  });

  it.each(WITHHELD)(
    "states the readout again only with a new answer once %s no longer holds",
    async (_, raise, clear) => {
      const { fake, readouts, run } = await withAnswerHeld();
      raise(fake);
      readouts.publish(asSession(fake), run, 33);
      clear(fake);
      const cleared = readouts.publish(asSession(fake), run, 50);
      // The readout asked before, answered late, and then the one asked afresh.
      askAt(fake, 1).answer(NO_GROUND);
      await settle();
      const late = readouts.publish(asSession(fake), run, 66);
      askAt(fake, 2).answer(ON_GROUND);
      await settle();
      const fresh = readouts.publish(asSession(fake), run, 83);
      expect([cleared?.readout, late?.readout, fresh?.readout]).toEqual([null, null, ON_GROUND]);
    },
  );

  it("carries the session's terrain fault, under which no readout stands", () => {
    const fake = holdingMoon();
    fake.terrainFault = TERRAIN_COMPUTATION_FAILED;
    const snapshot = new GroundReadouts(PRIMARY_VIEW_ID).publish(asSession(fake), terrainRun(), 0);
    expect(snapshot?.sessionFault).toBe(TERRAIN_COMPUTATION_FAILED);
  });
});
