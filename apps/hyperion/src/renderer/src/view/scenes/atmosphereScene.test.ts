import { describe, expect, it, vi } from "vitest";

import { add, dot, norm, normalise, scale, sub, vec3 } from "../../geometry/vec3";
import type { BodyDetail } from "../../lib/system/model";
import { detailOf, recordInput } from "../../test/atmosphereRecords";
import { handJupiterDetail, handJupiterRecord } from "../../test/giantFixtures";
import { RENDERER_ROOT, runTimeGraph } from "../../test/runTimeGraph";
import { standInThickCases } from "../../test/thickFixtureStandIns";
import type { KeptBodyAtmosphere } from "../atmosphere/AtmosphereCache";
import { assembleMedium } from "../atmosphere/assemble";
import { deckDatumOf } from "../atmosphere/thick/deckRead";
import { tableRadiusM } from "../atmosphere/hillaire";
import * as cases from "../atmosphere/reference/cases";
import giantDeckCase from "../atmosphere/reference/giant-deck.case.json" with { type: "json" };
import * as thick from "../atmosphere/reference/thickCases";
import type {
  FixtureCase,
  ThickCasesReply,
  ThickFixtureCases,
} from "../atmosphere/reference/thickCasesMessages";
import titanCase from "../atmosphere/reference/titan-class.case.json" with { type: "json" };
import venus58Case from "../atmosphere/reference/venus-58bar.case.json" with { type: "json" };
import venus92Case from "../atmosphere/reference/venus-92bar.case.json" with { type: "json" };
import { twilightLimitRad } from "../atmosphere/sources";
import { rotate } from "../camera/quaternion";
import { SUN_RADIUS_M } from "../lighting/hostDisc";
import { TEST_HULL } from "../scene/hull";
import type { ViewBody, ViewScene } from "../scene/model";
import {
  ATMOSPHERE_EARTH,
  ATMOSPHERE_EARTH_CENTRE_M,
  ATMOSPHERE_FIXTURES,
  ATMOSPHERE_JUPITER,
  ATMOSPHERE_MOON_CENTRE_M,
  ATMOSPHERE_MOON_RADIUS_M,
  ATMOSPHERE_SCENE_NAME,
  ATMOSPHERE_SHIP,
  ATMOSPHERE_SHIP_UP,
  ATMOSPHERE_STARS,
  ATMOSPHERE_STAR_COUNT,
  type AtmosphereFixtureCases,
  AtmosphereFixtures,
  atmosphereScene,
  atmosphereSeatM,
  ATMOSPHERE_SHIP_ATTITUDE,
  ATMOSPHERE_SHIP_BODY_FIXED_M,
  builtFigureOf,
  fixtureLoadOf,
  loadThickCases,
  phaseAngleDeg,
  prepareAtmosphereScene,
  type ThickCasesWorkerLike,
} from "./atmosphereScene";
import { AU_M } from "./kept";

/** The thick cases' stand-ins every held load lands, by case. */
const THICK = standInThickCases();

/** The hand Jupiter's record every held load of the giant lands. */
const JUPITER = handJupiterRecord();

/** Loads `TEST JUPITER`'s record at once: {@link JUPITER}. */
const loadJupiter = (): Promise<BodyDetail> => Promise.resolve(JUPITER);

/** The air fixtures whose loads build a medium: every one but the record's. */
const MEDIUM_FIXTURES = ATMOSPHERE_FIXTURES.filter(
  ({ caseName }) => fixtureLoadOf(caseName) !== "giant",
);

/**
 * Loads the test lands when it chooses: the cloud-free cases' with the real cases, the thick
 * cases' with their stand-ins ({@link THICK}), and the giant's with the hand Jupiter's record.
 */
function heldLoad(): {
  readonly fixtures: AtmosphereFixtures;
  readonly land: () => void;
  readonly landThick: () => void;
  readonly landGiant: () => void;
} {
  let release: ((loaded: AtmosphereFixtureCases) => void) | null = null;
  let releaseThick: ((loaded: ThickFixtureCases) => void) | null = null;
  let releaseGiant: ((loaded: BodyDetail) => void) | null = null;
  const fixtures = new AtmosphereFixtures(
    () =>
      new Promise<AtmosphereFixtureCases>((resolve) => {
        release = resolve;
      }),
    () =>
      new Promise<ThickFixtureCases>((resolve) => {
        releaseThick = resolve;
      }),
    () =>
      new Promise<BodyDetail>((resolve) => {
        releaseGiant = resolve;
      }),
  );
  const land = (): void => {
    if (release === null) {
      throw new Error("the load was never asked for");
    }
    release(cases);
  };
  const landThick = (): void => {
    if (releaseThick === null) {
      throw new Error("the thick load was never asked for");
    }
    releaseThick(THICK);
  };
  const landGiant = (): void => {
    if (releaseGiant === null) {
      throw new Error("the giant's load was never asked for");
    }
    releaseGiant(JUPITER);
  };
  return { fixtures, land, landThick, landGiant };
}

/** The air fixtures of `load`. */
function fixturesOf(load: "cases" | "thick" | "giant"): typeof ATMOSPHERE_FIXTURES {
  return ATMOSPHERE_FIXTURES.filter(({ caseName }) => fixtureLoadOf(caseName) === load);
}

/** A case of either load by its name: the real cloud-free case, or the thick one's stand-in. */
function caseNamed(name: (typeof ATMOSPHERE_FIXTURES)[number]["caseName"]): FixtureCase {
  let found: FixtureCase;
  switch (name) {
    case "earth":
      found = cases.EARTH_CASE;
      break;
    case "earth-ozone-slab":
      found = cases.EARTH_OZONE_SLAB_CASE;
      break;
    case "mars":
      found = cases.MARS_CASE;
      break;
    case "saturn-oblate":
      found = cases.SATURN_OBLATE_CASE;
      break;
    case "venus-92bar":
    case "venus-58bar":
    case "titan-class":
    case "giant-deck":
      found = THICK[name];
      break;
    case "hand-jupiter":
      throw new Error("TEST JUPITER carries a record, not a case");
  }
  return found;
}

/** A worker the test answers by hand, recording whether it was ended. */
class FakeThickWorker implements ThickCasesWorkerLike {
  readonly #events = new EventTarget();
  terminated = false;

  addEventListener(type: "message", listener: (event: MessageEvent<ThickCasesReply>) => void): void;
  addEventListener(type: "error" | "messageerror", listener: (event: Event) => void): void;
  addEventListener(
    type: "message" | "error" | "messageerror",
    listener: ((event: MessageEvent<ThickCasesReply>) => void) | ((event: Event) => void),
  ): void {
    this.#events.addEventListener(type, { handleEvent: listener });
  }

  terminate(): void {
    this.terminated = true;
  }

  /** Dispatches `event` to the listeners of its type, as the worker's own would. */
  dispatch(event: Event): void {
    this.#events.dispatchEvent(event);
  }
}

/** A body of `scene` by its designation. */
function bodyNamed(scene: ViewScene, designation: string): ViewBody {
  const body = scene.bodies.find((each) => each.designation === designation);
  if (body === undefined) {
    throw new Error(`no body ${designation}`);
  }
  return body;
}

/** Each air fixture's setting's kind, by designation. */
function settingKinds(scene: ViewScene): ReadonlyArray<readonly [string, string | undefined]> {
  return ATMOSPHERE_FIXTURES.map(
    ({ designation }) => [designation, bodyNamed(scene, designation).atmosphere?.kind] as const,
  );
}

/** The setting's kind an air fixture of `load` takes once its load lands. */
function landedKind(load: string): string {
  return load === "giant" ? "record" : "medium";
}

/** A loaded scene's fixtures' media. */
async function loadedScene(): Promise<ViewScene> {
  const { fixtures, land, landThick, landGiant } = heldLoad();
  const scene = atmosphereScene(fixtures);
  prepareAtmosphereScene(fixtures);
  land();
  landThick();
  landGiant();
  await fixtures.ready();
  return scene.sceneAt(0);
}

/**
 * A Lambert sphere's illuminance at TEST EARTH, relative: p (R ÷ Δ)² Φ(α) ÷ r², its sunlight
 * falling as 1 ÷ r², Φ(α) = (sin α + (π − α) cos α) ÷ π.
 */
function lightAtEarth(body: ViewBody): number {
  const toEarthM = sub(ATMOSPHERE_EARTH_CENTRE_M, body.centreM);
  const alphaRad = (phaseAngleDeg(body.centreM, ATMOSPHERE_EARTH_CENTRE_M) * Math.PI) / 180;
  const phase = (Math.sin(alphaRad) + (Math.PI - alphaRad) * Math.cos(alphaRad)) / Math.PI;
  const p = body.appearance?.photometry.geometricAlbedo[1] ?? 0;
  return (p * (body.radiusM / norm(toEarthM)) ** 2 * phase) / norm(body.centreM) ** 2;
}

/** A built medium of a setting, or a failure naming the body. */
function builtOf(setting: KeptBodyAtmosphere | undefined, designation: string) {
  if (setting?.kind !== "medium") {
    throw new Error(`${designation} holds no medium`);
  }
  return setting.built;
}

describe("ATMOSPHERE TEST's bodies", () => {
  const scene = atmosphereScene(heldLoad().fixtures).sceneAt(0);

  it("are the star, the nine air fixtures and the moon, by their designations", () => {
    expect(scene.bodies.map((body) => [body.designation, body.kind])).toEqual([
      ["TEST STAR", "star"],
      ["TEST EARTH", "planet"],
      ["TEST EARTH O3 SLAB", "planet"],
      ["TEST MARS", "planet"],
      ["TEST SATURN", "planet"],
      ["TEST VENUS", "planet"],
      ["TEST VENUS 5.80 MPa", "planet"],
      ["TEST TITAN", "planet"],
      ["TEST SATURN NH3 ICE DECK", "planet"],
      ["TEST JUPITER", "planet"],
      ["TEST MOON", "moon"],
    ]);
  });

  it("carries the four cloud-free cases from the import, the four thick from the worker and the hand Jupiter's record", () => {
    expect(ATMOSPHERE_FIXTURES.map(({ caseName }) => [caseName, fixtureLoadOf(caseName)])).toEqual([
      ["earth", "cases"],
      ["earth-ozone-slab", "cases"],
      ["mars", "cases"],
      ["saturn-oblate", "cases"],
      ["venus-92bar", "thick"],
      ["venus-58bar", "thick"],
      ["titan-class", "thick"],
      ["giant-deck", "thick"],
      ["hand-jupiter", "giant"],
    ]);
  });

  it("is a kept scene under its name, with the seat on the own ship standing on TEST EARTH", () => {
    const ship = scene.craft.find((craft) => craft.id === ATMOSPHERE_SHIP);
    expect([
      scene.provenance,
      scene.ownShip,
      ship?.pose.position.kind === "body_fixed" ? ship.pose.position.body : null,
      scene.defaultPose.frame,
    ]).toEqual([
      { kind: "kept", name: ATMOSPHERE_SCENE_NAME },
      ATMOSPHERE_SHIP,
      ATMOSPHERE_EARTH,
      { kind: "craft", craft: ATMOSPHERE_SHIP },
    ]);
  });

  it("sets TEST MOON airless and the giants surfaceless", () => {
    expect([
      bodyNamed(scene, "TEST MOON").atmosphere,
      bodyNamed(scene, "TEST MOON").radiusM,
      bodyNamed(scene, "TEST SATURN").surfaceless,
      bodyNamed(scene, "TEST SATURN NH3 ICE DECK").surfaceless,
      bodyNamed(scene, "TEST JUPITER").surfaceless,
    ]).toEqual([{ kind: "airless" }, ATMOSPHERE_MOON_RADIUS_M, true, true, true]);
  });

  it("lays the fixtures out about 1 au from the star, TEST EARTH a quarter round from the rest", () => {
    const centres = [...ATMOSPHERE_FIXTURES.map((each) => each.centreM), ATMOSPHERE_MOON_CENTRE_M];
    const fromStar = centres.map((centre) => norm(centre) / AU_M);
    const others = ATMOSPHERE_FIXTURES.filter((each) => each.id !== ATMOSPHERE_EARTH);
    const apart = others.flatMap((a, i) =>
      others.slice(i + 1).map((b) => norm(sub(a.centreM, b.centreM))),
    );
    const legs = others
      .slice(1)
      .map((b, i) => norm(sub(b.centreM, others[i]?.centreM ?? b.centreM)));
    const fromEarth = others.map((each) => norm(sub(each.centreM, ATMOSPHERE_EARTH_CENTRE_M)));
    expect([
      fromStar.every((au) => au > 0.99 && au < 1.03),
      legs.every((m) => Math.abs(m - 1e9) < 1),
      Math.min(...apart) >= 9.9e8,
      fromEarth.every((m) => m > 1.4 * AU_M),
    ]).toEqual([true, true, true, true]);
  });

  it("sees each air fixture after TEST EARTH O3 SLAB at about 60° of phase from the one before", () => {
    const others = ATMOSPHERE_FIXTURES.filter((each) => each.id !== ATMOSPHERE_EARTH);
    const phases = others
      .slice(1)
      .map((b, i) => phaseAngleDeg(b.centreM, others[i]?.centreM ?? b.centreM));
    // 60° from the legs alone, less the star's direction's turn across them, 59.0–59.7°.
    expect(phases.every((deg) => Math.abs(deg - 60) < 1.5)).toBe(true);
  });

  it("stands no air fixture in another's shadow, its penumbra included", () => {
    const shadowed = ATMOSPHERE_FIXTURES.flatMap((a) =>
      ATMOSPHERE_FIXTURES.filter((b) => {
        const axis = normalise(a.centreM);
        const behind = sub(b.centreM, a.centreM);
        const alongM = dot(behind, axis);
        const offAxisM = norm(sub(behind, scale(axis, alongM)));
        const radiusM = a.figure.equatorialRadiusM;
        // The penumbra's radius there: R + d (R☉ + R) ÷ r, the star the Sun's size.
        const penumbraM = radiusM + (alongM * (SUN_RADIUS_M + radiusM)) / norm(a.centreM);
        return b !== a && alongM > 0 && offAxisM - b.figure.equatorialRadiusM < penumbraM;
      }).map((b) => `${b.designation} behind ${a.designation}`),
    );
    expect(shadowed).toEqual([]);
  });
});

describe("ATMOSPHERE TEST's media", () => {
  it("are loading until their load lands, then each case's medium or the record, each load on its own", async () => {
    const { fixtures, land, landThick, landGiant } = heldLoad();
    const scene = atmosphereScene(fixtures);
    const before = settingKinds(scene.sceneAt(0));
    prepareAtmosphereScene(fixtures);
    land();
    await vi.waitFor(() => {
      expect(fixtures.loads.cases.kind).toBe("ready");
    });
    const cloudFree = [settingKinds(scene.sceneAt(1)), fixtures.media.kind];
    landThick();
    await vi.waitFor(() => {
      expect(fixtures.loads.thick.kind).toBe("ready");
    });
    const thickToo = [settingKinds(scene.sceneAt(2)), fixtures.media.kind];
    landGiant();
    await fixtures.ready();
    expect([
      before,
      cloudFree,
      thickToo,
      settingKinds(scene.sceneAt(3)),
      fixtures.media.kind,
    ]).toEqual([
      ATMOSPHERE_FIXTURES.map(({ designation }) => [designation, "loading"]),
      [
        ATMOSPHERE_FIXTURES.map(({ designation, caseName }) => [
          designation,
          fixtureLoadOf(caseName) === "cases" ? "medium" : "loading",
        ]),
        "loading",
      ],
      [
        ATMOSPHERE_FIXTURES.map(({ designation, caseName }) => [
          designation,
          fixtureLoadOf(caseName) === "giant" ? "loading" : "medium",
        ]),
        "loading",
      ],
      ATMOSPHERE_FIXTURES.map(({ designation, caseName }) => [
        designation,
        landedKind(fixtureLoadOf(caseName)),
      ]),
      "ready",
    ]);
  });

  it("holds each built medium and the record from frame to frame, so the cache makes their tables once", async () => {
    const { fixtures, land, landThick, landGiant } = heldLoad();
    const scene = atmosphereScene(fixtures);
    prepareAtmosphereScene(fixtures);
    land();
    landThick();
    landGiant();
    await fixtures.ready();
    const [first, later] = [scene.sceneAt(0), scene.sceneAt(30)].map((frame) =>
      ATMOSPHERE_FIXTURES.map(({ designation }) => bodyNamed(frame, designation).atmosphere),
    );
    expect(first?.every((setting, i) => setting === later?.[i])).toBe(true);
  });

  it("gives TEST JUPITER the hand Jupiter's record, built by the cache as a server body's reply", async () => {
    const scene = await loadedScene();
    const jupiter = bodyNamed(scene, "TEST JUPITER");
    expect([jupiter.id, jupiter.atmosphere]).toEqual([
      ATMOSPHERE_JUPITER,
      { kind: "record", detail: JUPITER },
    ]);
  });

  it("restates TEST JUPITER's floor and its black ground as the hand Jupiter's tables take them", () => {
    const assembled = assembleMedium(recordInput(detailOf(handJupiterDetail())), {
      envelopes: true,
    });
    if (assembled.medium === undefined || assembled.regime.kind !== "cloudDeck") {
      throw new Error("the hand Jupiter draws a medium split at its floor");
    }
    const { body } = assembled.drawn;
    const [fixture] = fixturesOf("giant");
    expect([fixture?.figure, fixture?.groundAlbedo]).toEqual([
      deckDatumOf(body.figure, body.gravity, assembled.regime.deckAltitudeM),
      assembled.medium.groundAlbedo[1],
    ]);
  }, 120_000);

  it("are each its case's medium, with its curves where it has absorbers", async () => {
    const scene = await loadedScene();
    expect(
      MEDIUM_FIXTURES.map(({ designation, caseName }) => {
        const built = builtOf(bodyNamed(scene, designation).atmosphere, designation);
        const referenceCase = caseNamed(caseName);
        return [
          built.medium === referenceCase.medium,
          built.curves.length === (referenceCase.curves.length === 0 ? 0 : 1),
          built.curves[0] ?? null,
        ];
      }),
    ).toEqual(
      MEDIUM_FIXTURES.map(({ caseName }) => {
        const { curves } = caseNamed(caseName);
        return [true, true, curves.length === 0 ? null : curves];
      }),
    );
  });

  it("draws each air body on its medium's own figure, the one its tables take", async () => {
    const scene = await loadedScene();
    const drawn = MEDIUM_FIXTURES.map(({ designation }) => {
      const body = bodyNamed(scene, designation);
      const figure = body.appearance?.figure;
      return [figure?.equatorialRadiusM, figure?.polarRadiusM, body.radiusM];
    });
    const tables = MEDIUM_FIXTURES.map(({ designation }) => {
      // `tablesFigureOf`'s rule, from what the tables are built of: their body's spheroid, or a
      // sphere of their ground's radius.
      const built = builtOf(bodyNamed(scene, designation).atmosphere, designation);
      const a = built.body?.figure.equatorialRadiusM ?? built.bottomRadiusM;
      const c = built.body?.figure.polarRadiusM ?? built.bottomRadiusM;
      return [a, c, a];
    });
    expect(drawn).toEqual(tables);
  });

  it("restates each case's figure, so that the body is drawn on it before the cases load", () => {
    const saturn = cases.SATURN_OBLATE_SPHEROID;
    expect(MEDIUM_FIXTURES.map(({ figure }) => figure)).toEqual([
      {
        equatorialRadiusM: cases.EARTH_REFERENCE_RADIUS_M,
        polarRadiusM: cases.EARTH_REFERENCE_RADIUS_M,
      },
      {
        equatorialRadiusM: cases.EARTH_REFERENCE_RADIUS_M,
        polarRadiusM: cases.EARTH_REFERENCE_RADIUS_M,
      },
      {
        equatorialRadiusM: cases.MARS_REFERENCE_RADIUS_M,
        polarRadiusM: cases.MARS_REFERENCE_RADIUS_M,
      },
      { equatorialRadiusM: saturn.equatorialRadiusM, polarRadiusM: saturn.polarRadiusM },
      { equatorialRadiusM: thick.VENUS_RADIUS_M, polarRadiusM: thick.VENUS_RADIUS_M },
      { equatorialRadiusM: thick.VENUS_RADIUS_M, polarRadiusM: thick.VENUS_RADIUS_M },
      { equatorialRadiusM: thick.TITAN_RADIUS_M, polarRadiusM: thick.TITAN_RADIUS_M },
      { equatorialRadiusM: thick.GIANT_RADIUS_M, polarRadiusM: thick.GIANT_RADIUS_M },
    ]);
  });

  it("restates each thick case's ground, as its committed case file writes it", () => {
    const files = [venus92Case, venus58Case, titanCase, giantDeckCase];
    expect(
      fixturesOf("thick").map(({ caseName, groundAlbedo }) => [caseName, groundAlbedo]),
    ).toEqual(files.map((file) => [file.name, file.groundAlbedo[1]]));
  });

  it("builds Saturn's tables on its spheroid and gravity, their ground R05's table radius", async () => {
    const scene = await loadedScene();
    const built = builtOf(bodyNamed(scene, "TEST SATURN").atmosphere, "TEST SATURN");
    const saturn = cases.SATURN_OBLATE_SPHEROID;
    expect([
      built.bottomRadiusM,
      built.body?.gravity,
      builtFigureOf(built),
      bodyNamed(scene, "TEST SATURN").appearance?.figure?.pole,
    ]).toEqual([
      tableRadiusM(saturn),
      cases.SATURN_OBLATE_GRAVITY,
      { equatorialRadiusM: saturn.equatorialRadiusM, polarRadiusM: saturn.polarRadiusM },
      vec3(0, 0, 1),
    ]);
  });

  it("states each cloud-free body failed, never unset, and logs once, when the import fails", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const load = vi.fn<() => Promise<AtmosphereFixtureCases>>(() =>
      Promise.reject(new Error("no optics")),
    );
    const fixtures = new AtmosphereFixtures(load, () => Promise.resolve(THICK), loadJupiter);
    const scene = atmosphereScene(fixtures);
    prepareAtmosphereScene(fixtures);
    await fixtures.ready();
    // The page's load is once: VIEW shown again does not retry it (decision-r08-t10b-ux §4).
    prepareAtmosphereScene(fixtures);
    await fixtures.ready();
    expect([
      settingKinds(scene.sceneAt(0)),
      fixtures.media.kind,
      error.mock.calls.length,
      load.mock.calls.length,
    ]).toEqual([
      ATMOSPHERE_FIXTURES.map(({ designation, caseName }) => [
        designation,
        { cases: "failed", thick: "medium", giant: "record" }[fixtureLoadOf(caseName)],
      ]),
      "failed",
      1,
      1,
    ]);
  });

  it("states each thick body failed, the others drawn, and logs once, when the worker fails", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const loadThick = vi.fn<() => Promise<ThickFixtureCases>>(() =>
      Promise.reject(new Error("the thick cases' worker failed (error)")),
    );
    const fixtures = new AtmosphereFixtures(() => Promise.resolve(cases), loadThick, loadJupiter);
    const scene = atmosphereScene(fixtures);
    prepareAtmosphereScene(fixtures);
    await fixtures.ready();
    prepareAtmosphereScene(fixtures);
    await fixtures.ready();
    expect([
      settingKinds(scene.sceneAt(0)),
      fixtures.media.kind,
      error.mock.calls.length,
      loadThick.mock.calls.length,
    ]).toEqual([
      ATMOSPHERE_FIXTURES.map(({ designation, caseName }) => [
        designation,
        { cases: "medium", thick: "failed", giant: "record" }[fixtureLoadOf(caseName)],
      ]),
      "failed",
      1,
      1,
    ]);
  });

  it("states TEST JUPITER failed, the others drawn, and logs once, when its record's import fails", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const loadGiant = vi.fn<() => Promise<BodyDetail>>(() =>
      Promise.reject(new Error("no hand Jupiter")),
    );
    const fixtures = new AtmosphereFixtures(
      () => Promise.resolve(cases),
      () => Promise.resolve(THICK),
      loadGiant,
    );
    const scene = atmosphereScene(fixtures);
    prepareAtmosphereScene(fixtures);
    await fixtures.ready();
    prepareAtmosphereScene(fixtures);
    await fixtures.ready();
    expect([
      settingKinds(scene.sceneAt(0)),
      fixtures.media.kind,
      error.mock.calls.length,
      loadGiant.mock.calls.length,
    ]).toEqual([
      ATMOSPHERE_FIXTURES.map(({ designation, caseName }) => [
        designation,
        fixtureLoadOf(caseName) === "giant" ? "failed" : "medium",
      ]),
      "failed",
      1,
      1,
    ]);
  });

  it("starts nothing as the scene is made, its loads from prepare alone", async () => {
    const { fixtures, land, landThick, landGiant } = heldLoad();
    const scene = atmosphereScene(fixtures);
    const before = fixtures.media.kind;
    expect(() => {
      land();
    }).toThrow("the load was never asked for");
    expect(() => {
      landThick();
    }).toThrow("the thick load was never asked for");
    expect(() => {
      landGiant();
    }).toThrow("the giant's load was never asked for");
    prepareAtmosphereScene(fixtures);
    land();
    landThick();
    landGiant();
    await fixtures.ready();
    expect([before, settingKinds(scene.sceneAt(0))[0]]).toEqual([
      "loading",
      ["TEST EARTH", "medium"],
    ]);
  });

  it("shades each cloud-free body as its medium's ground, provisional, BODY PHOTOMETRY stated", async () => {
    const scene = await loadedScene();
    // A Lambert sphere's Bond albedo is p q, q = 3 ÷ 2: the ground the march draws at the limb.
    const shaded = fixturesOf("cases").map(({ designation }) => {
      const body = bodyNamed(scene, designation);
      const ground = builtOf(body.atmosphere, designation).medium.groundAlbedo;
      const p = body.appearance?.photometry.geometricAlbedo ?? [0, 0, 0];
      return [
        p.every((each, i) => Math.abs(each * 1.5 - (ground[i] ?? Number.NaN)) < 1e-12),
        body.appearance?.labels,
      ];
    });
    expect(shaded).toEqual(
      fixturesOf("cases").map(() => [true, ["BODY PHOTOMETRY: NOT YET MODELLED"]]),
    );
  });

  it("shades each thick body as its case's ground, provisional, BODY PHOTOMETRY stated", () => {
    const scene = atmosphereScene(heldLoad().fixtures).sceneAt(0);
    const shaded = fixturesOf("thick").map(({ designation, groundAlbedo }) => {
      const body = bodyNamed(scene, designation);
      const p = body.appearance?.photometry.geometricAlbedo ?? [Number.NaN, 0, 0];
      return [
        p.every((each) => Math.abs(each * 1.5 - groundAlbedo) < 1e-12),
        body.appearance?.labels,
      ];
    });
    expect(shaded).toEqual(
      fixturesOf("thick").map(() => [true, ["BODY PHOTOMETRY: NOT YET MODELLED"]]),
    );
  });
});

describe("ATMOSPHERE TEST's thick cases' worker", () => {
  /** A load from a fake worker, and the worker. */
  function fromFake(): {
    readonly loaded: Promise<ThickFixtureCases>;
    readonly worker: FakeThickWorker;
  } {
    const worker = new FakeThickWorker();
    return { loaded: loadThickCases(() => worker), worker };
  }

  it("lands the cases the worker posts, and ends it", async () => {
    const { loaded, worker } = fromFake();
    const reply: ThickCasesReply = { kind: "built", cases: THICK };
    worker.dispatch(new MessageEvent("message", { data: reply }));
    expect([await loaded, worker.terminated]).toEqual([THICK, true]);
  });

  it("fails with the build's own message where the worker could not build them, and ends it", async () => {
    const { loaded, worker } = fromFake();
    const reply: ThickCasesReply = { kind: "failed", message: "no NH3 index at 645 nm" };
    worker.dispatch(new MessageEvent("message", { data: reply }));
    await expect(loaded).rejects.toThrow(
      "the thick cases' worker could not build them: no NH3 index at 645 nm",
    );
    expect(worker.terminated).toBe(true);
  });

  it("fails with what the worker's script threw, and ends it", async () => {
    const { loaded, worker } = fromFake();
    // An `ErrorEvent`'s message, as the worker raises one for a script that threw.
    worker.dispatch(
      Object.assign(new Event("error"), { message: "Uncaught RangeError: no index" }),
    );
    await expect(loaded).rejects.toThrow(
      "the thick cases' worker failed (error): Uncaught RangeError: no index",
    );
    expect(worker.terminated).toBe(true);
  });

  it("fails where the worker's reply could not be read, and ends it", async () => {
    const { loaded, worker } = fromFake();
    worker.dispatch(new Event("messageerror"));
    await expect(loaded).rejects.toThrow("the thick cases' worker failed (messageerror)");
    expect(worker.terminated).toBe(true);
  });

  it("fails where the worker's script did not load, and ends it", async () => {
    const { loaded, worker } = fromFake();
    worker.dispatch(new Event("error"));
    await expect(loaded).rejects.toThrow("the thick cases' worker failed (error)");
    expect(worker.terminated).toBe(true);
  });
});

describe("ATMOSPHERE TEST's moonlit night", () => {
  const seatM = atmosphereSeatM();
  /** The seat's height over TEST EARTH's ground, m. */
  const heightM = norm(sub(seatM, ATMOSPHERE_EARTH_CENTRE_M)) - cases.EARTH_REFERENCE_RADIUS_M;
  /** The elevation of `towardM`'s direction from the seat over its local horizontal, deg. */
  const elevationDeg = (towardM: typeof seatM): number =>
    (Math.asin(dot(normalise(sub(towardM, seatM)), ATMOSPHERE_SHIP_UP)) * 180) / Math.PI;

  it("stands the hull, pitched up at the moon, clear of the ground", () => {
    const lowestM = Math.min(
      ...TEST_HULL.vertices.map(
        (vertex) =>
          norm(add(ATMOSPHERE_SHIP_BODY_FIXED_M, rotate(ATMOSPHERE_SHIP_ATTITUDE, vertex))) -
          cases.EARTH_REFERENCE_RADIUS_M,
      ),
    );
    expect([lowestM > 0.5, lowestM < 2]).toEqual([true, true]);
  });

  it("is lit by TEST MOON alone: each other body under 1% of its light, below the horizon", () => {
    const scene = atmosphereScene(heldLoad().fixtures).sceneAt(0);
    const moon = lightAtEarth(bodyNamed(scene, "TEST MOON"));
    const others = ATMOSPHERE_FIXTURES.filter((each) => each.id !== ATMOSPHERE_EARTH).map(
      ({ designation }) => {
        const body = bodyNamed(scene, designation);
        return [designation, lightAtEarth(body) / moon < 0.01, elevationDeg(body.centreM) < 0];
      },
    );
    expect(others).toEqual(
      ATMOSPHERE_FIXTURES.filter((each) => each.id !== ATMOSPHERE_EARTH).map(({ designation }) => [
        designation,
        true,
        true,
      ]),
    );
  });

  it("shows TEST MOON full from the seat: under 5° of phase", () => {
    expect(phaseAngleDeg(ATMOSPHERE_MOON_CENTRE_M, seatM)).toBeLessThan(5);
  });

  it("sets TEST MOON high in the seat's sky, its view along the seat's line of sight", () => {
    const scene = atmosphereScene(heldLoad().fixtures).sceneAt(0);
    const toMoon = normalise(sub(ATMOSPHERE_MOON_CENTRE_M, seatM));
    // The camera looks down its −z.
    const forward = rotate(scene.defaultPose.orientation, vec3(0, 0, -1));
    expect([elevationDeg(ATMOSPHERE_MOON_CENTRE_M) > 40, dot(forward, toMoon) > 0.9999]).toEqual([
      true,
      true,
    ]);
  });

  it("keeps TEST MOON out of TEST EARTH's shadow, umbra and penumbra", () => {
    const axis = normalise(ATMOSPHERE_EARTH_CENTRE_M);
    const behind = sub(ATMOSPHERE_MOON_CENTRE_M, ATMOSPHERE_EARTH_CENTRE_M);
    const alongM = dot(behind, axis);
    const offAxisM = norm(sub(behind, scale(axis, alongM)));
    // The penumbra's radius there: R + d (R☉ + R) ÷ 1 au.
    const penumbraM =
      cases.EARTH_REFERENCE_RADIUS_M +
      (alongM * (SUN_RADIUS_M + cases.EARTH_REFERENCE_RADIUS_M)) / norm(ATMOSPHERE_EARTH_CENTRE_M);
    expect(offAxisM - ATMOSPHERE_MOON_RADIUS_M).toBeGreaterThan(penumbraM);
  });

  it("stands the star beyond the measured Earth's twilight limit, and at least 25° down", () => {
    const limitDeg =
      (twilightLimitRad(
        {
          equatorialRadiusM: cases.EARTH_REFERENCE_RADIUS_M,
          polarRadiusM: cases.EARTH_REFERENCE_RADIUS_M,
        },
        heightM,
        cases.EARTH_CASE.medium.topHeightM,
      ) *
        180) /
      Math.PI;
    const depressionDeg = -elevationDeg(vec3(0, 0, 0));
    expect([heightM > 10 && heightM < 20, depressionDeg > limitDeg, depressionDeg >= 25]).toEqual([
      true,
      true,
      true,
    ]);
  });

  it("starts the stage at the seat looking forward", () => {
    const scene = atmosphereScene(heldLoad().fixtures);
    expect(scene.sceneAt(0).defaultPose).toEqual(scene.cameraAt(0));
  });
});

describe("ATMOSPHERE TEST's stars", () => {
  it("are a fixed field of a few hundred unit directions over the sphere, V 0 to 6", () => {
    const magnitudes = ATMOSPHERE_STARS.map((star) => star.absoluteV);
    const north = ATMOSPHERE_STARS.filter((star) => star.direction.z > 0).length;
    expect([
      ATMOSPHERE_STARS.length,
      ATMOSPHERE_STARS.every((star) => Math.abs(norm(star.direction) - 1) < 1e-12),
      Math.min(...magnitudes) >= 0 && Math.max(...magnitudes) <= 6,
      magnitudes.filter((v) => v < 2).length > 50,
      north > ATMOSPHERE_STAR_COUNT * 0.4 && north < ATMOSPHERE_STAR_COUNT * 0.6,
      new Set(ATMOSPHERE_STARS.map((star) => star.id)).size,
    ]).toEqual([ATMOSPHERE_STAR_COUNT, true, true, true, true, ATMOSPHERE_STAR_COUNT]);
  });

  it("are the scene's own, every frame the same", () => {
    const scene = atmosphereScene(heldLoad().fixtures);
    expect([scene.sceneAt(0).stars, scene.sceneAt(5).stars]).toEqual([
      ATMOSPHERE_STARS,
      ATMOSPHERE_STARS,
    ]);
  });
});

describe("ATMOSPHERE TEST's static imports", () => {
  const graph = [...runTimeGraph(`${RENDERER_ROOT}view/scenes/atmosphereScene.ts`)];

  it("are read from the sources they walk", () => {
    expect(graph).toContain(`${RENDERER_ROOT}view/scenes/kept.ts`);
  });

  it("leave the reference cases, their optics and hillaire.ts to the lazy import and the worker", () => {
    expect(
      graph.filter(
        (path) =>
          path.endsWith("/view/atmosphere/reference/cases.ts") ||
          path.endsWith("/view/atmosphere/reference/thickCases.ts") ||
          path.endsWith("/view/atmosphere/hillaire.ts") ||
          path.endsWith("/view/atmosphere/absorbers.ts"),
      ),
    ).toEqual([]);
  });
});
