import { describe, expect, it, vi } from "vitest";

import { add, dot, norm, normalise, scale, sub, vec3 } from "../../geometry/vec3";
import { RENDERER_ROOT, runTimeGraph } from "../../test/runTimeGraph";
import type { KeptBodyAtmosphere } from "../atmosphere/AtmosphereCache";
import { tableRadiusM } from "../atmosphere/hillaire";
import * as cases from "../atmosphere/reference/cases";
import { twilightLimitRad } from "../atmosphere/sources";
import { rotate } from "../camera/quaternion";
import { SUN_RADIUS_M } from "../lighting/hostDisc";
import { TEST_HULL } from "../scene/hull";
import type { ViewBody, ViewScene } from "../scene/model";
import {
  ATMOSPHERE_EARTH,
  ATMOSPHERE_EARTH_CENTRE_M,
  ATMOSPHERE_FIXTURES,
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
  phaseAngleDeg,
  prepareAtmosphereScene,
} from "./atmosphereScene";
import { AU_M } from "./kept";

/** A load the test lands when it chooses, with the real cases. */
function heldLoad(): { readonly fixtures: AtmosphereFixtures; readonly land: () => void } {
  let release: ((loaded: AtmosphereFixtureCases) => void) | null = null;
  const fixtures = new AtmosphereFixtures(
    () =>
      new Promise<AtmosphereFixtureCases>((resolve) => {
        release = resolve;
      }),
  );
  const land = (): void => {
    if (release === null) {
      throw new Error("the load was never asked for");
    }
    release(cases);
  };
  return { fixtures, land };
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

/** A loaded scene's fixtures' media. */
async function loadedScene(): Promise<ViewScene> {
  const { fixtures, land } = heldLoad();
  const scene = atmosphereScene(fixtures);
  prepareAtmosphereScene(fixtures);
  land();
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

  it("are the star, the four air fixtures and the moon, by their designations", () => {
    expect(scene.bodies.map((body) => [body.designation, body.kind])).toEqual([
      ["TEST STAR", "star"],
      ["TEST EARTH", "planet"],
      ["TEST EARTH O3 SLAB", "planet"],
      ["TEST MARS", "planet"],
      ["TEST SATURN", "planet"],
      ["TEST MOON", "moon"],
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

  it("sets TEST MOON airless and the giant surfaceless", () => {
    expect([
      bodyNamed(scene, "TEST MOON").atmosphere,
      bodyNamed(scene, "TEST MOON").radiusM,
      bodyNamed(scene, "TEST SATURN").surfaceless,
    ]).toEqual([{ kind: "airless" }, ATMOSPHERE_MOON_RADIUS_M, true]);
  });

  it("lays the fixtures out about 1 au from the star, TEST EARTH a quarter round from the rest", () => {
    const centres = [...ATMOSPHERE_FIXTURES.map((each) => each.centreM), ATMOSPHERE_MOON_CENTRE_M];
    const fromStar = centres.map((centre) => norm(centre) / AU_M);
    const others = ATMOSPHERE_FIXTURES.filter((each) => each.id !== ATMOSPHERE_EARTH);
    const apart = others.flatMap((a, i) =>
      others.slice(i + 1).map((b) => norm(sub(a.centreM, b.centreM))),
    );
    const fromEarth = others.map((each) => norm(sub(each.centreM, ATMOSPHERE_EARTH_CENTRE_M)));
    expect([
      fromStar.every((au) => au > 0.99 && au < 1.03),
      Math.min(...apart) >= 9.9e8 && Math.max(...apart) <= 2e9,
      fromEarth.every((m) => m > 1.4 * AU_M),
    ]).toEqual([true, true, true]);
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
  it("are loading until the cases' import lands, then each case's medium", async () => {
    const { fixtures, land } = heldLoad();
    const scene = atmosphereScene(fixtures);
    const before = settingKinds(scene.sceneAt(0));
    prepareAtmosphereScene(fixtures);
    land();
    await fixtures.ready();
    expect([before, settingKinds(scene.sceneAt(1))]).toEqual([
      ATMOSPHERE_FIXTURES.map(({ designation }) => [designation, "loading"]),
      ATMOSPHERE_FIXTURES.map(({ designation }) => [designation, "medium"]),
    ]);
  });

  it("holds each built medium from frame to frame, so the cache makes its tables once", async () => {
    const { fixtures, land } = heldLoad();
    const scene = atmosphereScene(fixtures);
    prepareAtmosphereScene(fixtures);
    land();
    await fixtures.ready();
    const [first, later] = [scene.sceneAt(0), scene.sceneAt(30)].map((frame) =>
      ATMOSPHERE_FIXTURES.map(({ designation }) =>
        builtOf(bodyNamed(frame, designation).atmosphere, designation),
      ),
    );
    expect(first?.every((built, i) => built === later?.[i])).toBe(true);
  });

  it("are each its case's medium, with its curves where it has absorbers", async () => {
    const scene = await loadedScene();
    const byCase = {
      earth: cases.EARTH_CASE,
      "earth-ozone-slab": cases.EARTH_OZONE_SLAB_CASE,
      mars: cases.MARS_CASE,
      "saturn-oblate": cases.SATURN_OBLATE_CASE,
    } as const;
    expect(
      ATMOSPHERE_FIXTURES.map(({ designation, caseName }) => {
        const built = builtOf(bodyNamed(scene, designation).atmosphere, designation);
        const referenceCase = byCase[caseName];
        return [
          built.medium === referenceCase.medium,
          built.curves.length === (referenceCase.curves.length === 0 ? 0 : 1),
          built.curves[0] ?? null,
        ];
      }),
    ).toEqual(
      ATMOSPHERE_FIXTURES.map(({ caseName }) => {
        const { curves } = byCase[caseName];
        return [true, true, curves.length === 0 ? null : curves];
      }),
    );
  });

  it("draws each air body on its medium's own figure, the one its tables take", async () => {
    const scene = await loadedScene();
    const drawn = ATMOSPHERE_FIXTURES.map(({ designation }) => {
      const body = bodyNamed(scene, designation);
      const figure = body.appearance?.figure;
      return [figure?.equatorialRadiusM, figure?.polarRadiusM, body.radiusM];
    });
    const tables = ATMOSPHERE_FIXTURES.map(({ designation }) => {
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
    expect(ATMOSPHERE_FIXTURES.map(({ figure }) => figure)).toEqual([
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
    ]);
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

  it("states each air body failed, never unset, and logs once, when the import fails", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const load = vi.fn<() => Promise<AtmosphereFixtureCases>>(() =>
      Promise.reject(new Error("no optics")),
    );
    const fixtures = new AtmosphereFixtures(load);
    const scene = atmosphereScene(fixtures);
    prepareAtmosphereScene(fixtures);
    await fixtures.ready();
    // The page's load is once: VIEW shown again does not retry it (decision-r08-t10b-ux §4).
    prepareAtmosphereScene(fixtures);
    await fixtures.ready();
    expect([
      settingKinds(scene.sceneAt(0)),
      error.mock.calls.length,
      load.mock.calls.length,
    ]).toEqual([ATMOSPHERE_FIXTURES.map(({ designation }) => [designation, "failed"]), 1, 1]);
  });

  it("starts nothing as the scene is made, its load from prepare alone", async () => {
    const { fixtures, land } = heldLoad();
    const scene = atmosphereScene(fixtures);
    const before = fixtures.media.kind;
    expect(() => {
      land();
    }).toThrow("the load was never asked for");
    prepareAtmosphereScene(fixtures);
    land();
    await fixtures.ready();
    expect([before, settingKinds(scene.sceneAt(0))[0]]).toEqual([
      "loading",
      ["TEST EARTH", "medium"],
    ]);
  });

  it("shades each air body as its medium's ground, provisional, BODY PHOTOMETRY stated", async () => {
    const scene = await loadedScene();
    // A Lambert sphere's Bond albedo is p q, q = 3 ÷ 2: the ground the march draws at the limb.
    const shaded = ATMOSPHERE_FIXTURES.map(({ designation }) => {
      const body = bodyNamed(scene, designation);
      const ground = builtOf(body.atmosphere, designation).medium.groundAlbedo;
      const p = body.appearance?.photometry.geometricAlbedo ?? [0, 0, 0];
      return [
        p.every((each, i) => Math.abs(each * 1.5 - (ground[i] ?? Number.NaN)) < 1e-12),
        body.appearance?.labels,
      ];
    });
    expect(shaded).toEqual(
      ATMOSPHERE_FIXTURES.map(() => [true, ["BODY PHOTOMETRY: NOT YET MODELLED"]]),
    );
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

  it("leave the reference cases, their optics and hillaire.ts to the lazy import", () => {
    expect(
      graph.filter(
        (path) =>
          path.endsWith("/view/atmosphere/reference/cases.ts") ||
          path.endsWith("/view/atmosphere/hillaire.ts") ||
          path.endsWith("/view/atmosphere/absorbers.ts"),
      ),
    ).toEqual([]);
  });
});
