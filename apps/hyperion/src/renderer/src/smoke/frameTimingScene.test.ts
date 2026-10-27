import { describe, expect, it } from "vitest";

import { add, dot, norm, normalise, sub, vec3 } from "../geometry/vec3";
import { NO_NORMAL_ABOVE, type NormalMapTexels } from "../view/bodies/discNormals";
import { type ClassMapTexels, classMapSide } from "../view/bodies/discSurface";
import { SATURN_OBLATE_CASE } from "../view/atmosphere/reference/cases";
import { builtOfCase } from "../view/scenes/atmosphereScene";
import { CLASS_MAP_FACE_TEXELS } from "../view/terrain/classMap";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  EARTH_SLAB_REFERENCE,
  EARTH_SLAB_REFERENCE_CURVES,
  R05_EARTH_REFERENCE,
} from "../view/atmosphere/earth";
import { geodeticOf, TABLE_SIZES } from "../view/atmosphere/hillaire";
import type { AtmosphereMedium } from "../view/atmosphere/medium";
import { SKY_SUN_CAP } from "../view/atmosphere/settings";
import { horizonDipRad } from "../view/atmosphere/sources";
import {
  absorberCount,
  type AbsorberCurveSlots,
  NO_ABSORBER_CURVES,
} from "../view/atmosphere/tables";
import { bodyLightSources } from "../view/bodies/draw";
import { rotate } from "../view/camera/quaternion";
import { SUN_RADIUS_M } from "../view/lighting/hostDisc";
import { type QualitySetting, SETTINGS } from "../view/quality/qualitySetting";
import { AU_M } from "../view/scenes/kept";
import { sphereOutsideView } from "../view/wireframe/submit";
import { FAR_LIMB_TIMING_PLACES, settingOutput, SLICES_EARTH } from "./atmosphere";
import {
  angleBetweenRad,
  COMPANION_DISTANCE_M,
  earthInput,
  farLimbFrameScene,
  type FrameScene,
  GENERATED_CLASSES,
  GENERATED_REGION_TEXELS,
  generatedClassLaws,
  generatedClassMapTexels,
  generatedNormalMapTexels,
  GIANT_DISTANCE_M,
  GIANT_EQUATORIAL_RADIUS_M,
  listSources,
  type ListedSources,
  NORMAL_MAP_LEVEL,
  placesScaledTo,
  planetshineShare,
  SMALL_TRIO,
  type SourceSet,
  smallEarthScene,
  smallEarthsScene,
  smokeClassLaws,
  smokeClassMapTexels,
  smokeNormalMapTexels,
  sourcesAsSet,
  sourceSetsOf,
  STAR_SETS,
  trioSourceSet,
} from "./frameTimingScene";

/** The three media the frames are timed on, each with its tables' light slots' curves. */
const MEDIA: ReadonlyArray<{
  readonly name: string;
  readonly medium: AtmosphereMedium;
  readonly curves: AbsorberCurveSlots;
}> = [
  { name: "R05's Earth", medium: R05_EARTH_REFERENCE, curves: NO_ABSORBER_CURVES },
  {
    name: "the measured Earth",
    medium: EARTH_REFERENCE,
    curves: [...EARTH_REFERENCE_CURVES, ...EARTH_REFERENCE_CURVES],
  },
  {
    name: "the slab Earth",
    medium: EARTH_SLAB_REFERENCE,
    curves: [...EARTH_SLAB_REFERENCE_CURVES, ...EARTH_SLAB_REFERENCE_CURVES],
  },
];

const SETTINGS_TIMED: readonly QualitySetting[] = ["high", "low"];

/** One frame of the timings: its setting, set, camera and medium, the scene and its sky's sources. */
interface Case {
  readonly label: string;
  readonly setting: QualitySetting;
  readonly set: SourceSet;
  readonly scene: FrameScene;
  readonly listed: ListedSources;
}

/** Every frame the timings draw, with its sources as the plan makes them (`bodyLightSources`). */
function cases(): Case[] {
  return SETTINGS_TIMED.flatMap((setting) => {
    const viewport = settingOutput(setting);
    const photoreal = SETTINGS[setting].photoreal;
    return MEDIA.flatMap(({ name, medium, curves }) =>
      FAR_LIMB_TIMING_PLACES.flatMap((place) =>
        sourceSetsOf(setting).map((set): Case => {
          const scene = farLimbFrameScene(place.distanceM, viewport, set);
          const earth = earthInput("0200080020000000.0311", medium, scene.earthCentreM);
          const sources = bodyLightSources(earth, [earth, ...scene.giants], scene.lights, {
            annuli: photoreal.discAnnuli,
            planetshine: photoreal.planetshineSources,
          });
          const lightSlots = absorberCount(medium) > 0 ? curves.length : Infinity;
          const listed = listSources(
            sources,
            scene.air,
            SLICES_EARTH.figure,
            medium,
            lightSlots,
            TABLE_SIZES[setting].marchSourceCap,
          );
          return {
            label: `${setting} ${name} ${place.label} ${set.name}`,
            setting,
            set,
            scene,
            listed,
          };
        }),
      ),
    );
  });
}

const CASES = cases();

describe("the photorealistic frame's timed sets", () => {
  it("takes the sky's two stars at each setting's whole count", () => {
    expect([SKY_SUN_CAP.high, SKY_SUN_CAP.low]).toEqual([2, 2]);
  });

  it("names the setting's whole count by its sources: x4 on high, x3 on low", () => {
    expect(sourceSetsOf("high").map((set) => set.name)).toEqual(["x1", "x2", "x2p", "x4"]);
    expect(sourceSetsOf("low").map((set) => set.name)).toEqual(["x1", "x2", "x2p", "x3"]);
  });

  it("draws every setting's sets at every camera on every medium, giants in half of them", () => {
    expect(CASES).toHaveLength(2 * 3 * 5 * 4);
    expect(CASES.filter(({ scene }) => scene.giants.length > 0)).toHaveLength(2 * 3 * 5 * 2);
  });

  it("keeps exactly each set's sources, kind and slot, and marches the set's two, everywhere", () => {
    const wrong = CASES.filter(
      ({ setting, set, listed }) => !sourcesAsSet(listed, set, TABLE_SIZES[setting].marchSourceCap),
    ).map(({ label }) => label);
    expect(wrong).toEqual([]);
  });

  it("tells a march of one source from the set's two", () => {
    const passed = CASES.filter(
      ({ set, listed }) => set.stars + set.giants > 1 && sourcesAsSet(listed, set, 1),
    ).map(({ label }) => label);
    expect(passed).toEqual([]);
  });

  it("leaves none of the plan's sources out of the sky", () => {
    const left = CASES.filter(({ listed }) => listed.left.length > 0).map(({ label }) => label);
    expect(left).toEqual([]);
  });

  it("keeps every giant out of the view", () => {
    const seen = CASES.filter(({ setting, scene }) =>
      scene.giants.some(
        (giant) =>
          !sphereOutsideView(
            giant.centreM,
            giant.figure.equatorialRadiusM,
            scene.camera,
            settingOutput(setting),
          ),
      ),
    ).map(({ label }) => label);
    expect(seen).toEqual([]);
  });

  it("keeps every giant above the camera's horizon dip, so that the twilight skip keeps it", () => {
    const below = CASES.filter(({ scene }) => {
      const geodetic = geodeticOf(scene.air.positionM, SLICES_EARTH.figure);
      const dipRad = horizonDipRad(SLICES_EARTH.figure, geodetic.heightM);
      return scene.giants.some((giant) => {
        const toGiant = normalise(giant.centreM);
        const sine =
          toGiant.x * geodetic.normal.x +
          toGiant.y * geodetic.normal.y +
          toGiant.z * geodetic.normal.z;
        return !(Math.asin(sine) > -dipRad);
      });
    }).map(({ label }) => label);
    expect(below).toEqual([]);
  });

  it("gives each giant's planetshine over 5e-4 of the Sun at the camera, under the second star", () => {
    // Under the binary's second star's tenth, so that the march's two are the stars.
    const outside = CASES.filter(({ set, listed }) => {
      const share = planetshineShare(listed);
      return set.giants === 0 ? share !== null : share === null || share < 5e-4 || share >= 0.05;
    }).map(({ label }) => label);
    expect(outside).toEqual([]);
  });

  it("lets no giant eclipse the Sun for the Earth", () => {
    const radiiRad =
      Math.asin(GIANT_EQUATORIAL_RADIUS_M / GIANT_DISTANCE_M) + Math.asin(SUN_RADIUS_M / AU_M);
    const eclipsed = CASES.filter(({ scene }) => {
      const sun = scene.lights[0];
      return (
        sun === undefined ||
        scene.giants.some(
          (giant) =>
            !(
              angleBetweenRad(
                sub(sun.centreM, scene.earthCentreM),
                sub(giant.centreM, scene.earthCentreM),
              ) >
              radiiRad + Math.PI / 6
            ),
        )
      );
    }).map(({ label }) => label);
    expect(eclipsed).toEqual([]);
  });
});

describe("the small air bodies' frames", () => {
  const viewport = settingOutput("high");
  const DIAMETERS_PX = [4, 10, 32] as const;
  const scenes = DIAMETERS_PX.map((diameterPx) => ({
    diameterPx,
    scene: smallEarthScene(diameterPx, viewport, SLICES_EARTH.figure),
  }));

  it("draws the Earth whole at its diameter in pixels", () => {
    const drawn = scenes.map(({ scene }) => {
      const pxPerRad = viewport.widthPx / (2 * Math.tan(scene.camera.fovXRad / 2));
      const distanceM = -scene.earthCentreM.z;
      const px = 2 * Math.asin(SLICES_EARTH.figure.equatorialRadiusM / distanceM) * pxPerRad;
      return Number(px.toFixed(6));
    });
    expect(drawn).toEqual([...DIAMETERS_PX]);
  });

  it("centres the Earth on the view's axis, the air's camera on the far side of its centre", () => {
    for (const { scene } of scenes) {
      expect(scene.earthCentreM).toEqual({ x: 0, y: 0, z: scene.earthCentreM.z });
      expect(scene.air.positionM).toEqual({ x: 0, y: 0, z: -scene.earthCentreM.z });
    }
  });

  it("lights the Earth by the Sun 40° off the line of sight", () => {
    const offDeg = scenes.map(({ scene }) => {
      const sun = scene.lights[0];
      const offRad =
        sun === undefined
          ? Number.NaN
          : angleBetweenRad(sub(sun.centreM, scene.earthCentreM), scene.air.positionM);
      return Number(((offRad * 180) / Math.PI).toFixed(6));
    });
    expect(offDeg).toEqual([40, 40, 40]);
  });
});

describe("TEST SATURN's frames", () => {
  const built = builtOfCase(SATURN_OBLATE_CASE, "saturn-oblate");
  const figure = built.body?.figure ?? SLICES_EARTH.figure;
  const places = placesScaledTo(figure);
  const frames = SETTINGS_TIMED.flatMap((setting) =>
    places.flatMap((place) =>
      STAR_SETS.map((set) => {
        const scene = farLimbFrameScene(place.distanceM, settingOutput(setting), set, figure);
        return { label: `${setting} ${place.label} ${set.name}`, setting, set, scene };
      }),
    ),
  );

  it("is drawn on Saturn's spheroid, not the Earth's", () => {
    expect(built.body).not.toBeNull();
    expect(figure.equatorialRadiusM).toBeGreaterThan(9 * SLICES_EARTH.figure.equatorialRadiusM);
    expect(figure.polarRadiusM).toBeLessThan(0.95 * figure.equatorialRadiusM);
  });

  it("sees its disc as the Earth's at each of the Earth's five cameras", () => {
    const earthRad = FAR_LIMB_TIMING_PLACES.map(({ distanceM }) =>
      Math.asin(SLICES_EARTH.figure.equatorialRadiusM / distanceM),
    );
    const saturnRad = places.map(({ distanceM }) =>
      Math.asin(figure.equatorialRadiusM / distanceM),
    );
    expect(saturnRad.map((rad, i) => Math.abs(rad - (earthRad[i] ?? Number.NaN)))).toEqual(
      places.map(() => expect.closeTo(0, 12)),
    );
  });

  it("names each camera by its distance", () => {
    expect(places.map(({ label }) => label)).toEqual([
      "6.40e7 m",
      "9.45e7 m",
      "1.46e8 m",
      "2.41e8 m",
      "9.45e8 m",
    ]);
  });

  it("aims each view's centre ray at Saturn's datum, at a tangent height of 0", () => {
    const off = frames
      .filter(({ scene }) => {
        const p = scene.air.positionM;
        const along = dot(p, rotate(scene.air.orientation, vec3(0, 0, -1)));
        const missM = Math.sqrt(dot(p, p) - along * along);
        return !(Math.abs(missM - figure.equatorialRadiusM) < 1e-9 * figure.equatorialRadiusM);
      })
      .map(({ label }) => label);
    expect(off).toEqual([]);
  });

  it("gives its body Saturn's radii about the frame's +z", () => {
    const body = earthInput("0200080020000000.0315", built.medium, vec3(0, 0, 0), figure);
    expect(body.figure).toEqual({
      equatorialRadiusM: figure.equatorialRadiusM,
      polarRadiusM: figure.polarRadiusM,
      pole: vec3(0, 0, 1),
    });
  });

  it("stands every camera above its air's top", () => {
    const inside = frames
      .filter(
        ({ scene }) => !(geodeticOf(scene.air.positionM, figure).heightM > built.medium.topHeightM),
      )
      .map(({ label }) => label);
    expect(inside).toEqual([]);
  });

  it("keeps exactly the stars' sets' sources, each star on its own slot, and marches them", () => {
    const wrong = frames
      .filter(({ setting, set, scene }) => {
        const photoreal = SETTINGS[setting].photoreal;
        const body = earthInput("0200080020000000.0315", built.medium, scene.earthCentreM, figure);
        const sources = bodyLightSources(body, [body], scene.lights, {
          annuli: photoreal.discAnnuli,
          planetshine: photoreal.planetshineSources,
        });
        const listed = listSources(
          sources,
          scene.air,
          figure,
          built.medium,
          absorberCount(built.medium) > 0 ? built.curves.length : Infinity,
          TABLE_SIZES[setting].marchSourceCap,
        );
        return (
          listed.left.length > 0 || !sourcesAsSet(listed, set, TABLE_SIZES[setting].marchSourceCap)
        );
      })
      .map(({ label }) => label);
    expect(wrong).toEqual([]);
  });
});

describe("three small air bodies in one view", () => {
  const viewport = settingOutput("high");
  const set = sourceSetsOf("high").find((each) => each.name === "x2");
  if (set === undefined) {
    throw new Error("high's sets hold x2");
  }
  const scene = smallEarthsScene(viewport, SLICES_EARTH.figure, set);
  const pxPerRad = viewport.widthPx / (2 * Math.tan(scene.camera.fovXRad / 2));
  const middle = scene.bodies[1];
  if (middle === undefined) {
    throw new Error("the trio has a middle body");
  }

  it("draws each at its diameter, on the on-axis reckoning", () => {
    const drawn = scene.bodies.map(({ centreM }) =>
      Number(
        (2 * Math.asin(SLICES_EARTH.figure.equatorialRadiusM / norm(centreM)) * pxPerRad).toFixed(
          6,
        ),
      ),
    );
    expect(drawn).toEqual(SMALL_TRIO.map(({ diameterPx }) => diameterPx));
  });

  it("stands them 6° apart", () => {
    const apartDeg = scene.bodies
      .slice(1)
      .map((body, i) =>
        Number(
          (
            (angleBetweenRad(scene.bodies[i]?.centreM ?? body.centreM, body.centreM) * 180) /
            Math.PI
          ).toFixed(9),
        ),
      );
    expect(apartDeg).toEqual([6, 6]);
  });

  it("leaves more than 150 px between their discs' edges", () => {
    const gapsPx = scene.bodies.slice(1).map((body, i) => {
      const before = scene.bodies[i];
      return before === undefined
        ? Number.NaN
        : angleBetweenRad(before.centreM, body.centreM) * pxPerRad -
            (before.diameterPx + body.diameterPx) / 2;
    });
    expect(gapsPx.filter((gap) => !(gap > 150))).toEqual([]);
  });

  it("puts each body's air camera on the far side of its centre", () => {
    const off = scene.bodies.filter((body) => !(norm(add(body.air.positionM, body.centreM)) === 0));
    expect(off).toEqual([]);
  });

  it("gives each body's air camera the frame's axes", () => {
    expect(scene.bodies.map((body) => body.air.orientation)).toEqual(
      scene.bodies.map(() => scene.camera.orientation),
    );
  });

  it("lights the middle body by the Sun 40° off its line of sight", () => {
    const sun = scene.lights[0];
    const offDeg =
      sun === undefined
        ? Number.NaN
        : (angleBetweenRad(sub(sun.centreM, middle.centreM), middle.air.positionM) * 180) / Math.PI;
    expect(offDeg).toBeCloseTo(40, 9);
  });

  it("turns the second star 40° from the Sun about the pole, at √10 au", () => {
    const sun = scene.lights[0];
    const second = scene.lights[1];
    if (sun === undefined || second === undefined) {
      throw new Error("x2 places two stars");
    }
    const s = normalise(sub(sun.centreM, middle.centreM));
    const c = sub(second.centreM, middle.centreM);
    const turnDeg = ((Math.atan2(c.y, c.x) - Math.atan2(s.y, s.x)) * 180) / Math.PI;
    expect([norm(c) / COMPANION_DISTANCE_M, c.z / norm(c) - s.z, turnDeg]).toEqual([
      expect.closeTo(1, 12),
      expect.closeTo(0, 12),
      expect.closeTo(40, 9),
    ]);
  });

  it("draws each body's sources as the set's two stars and the other two's planetshine", () => {
    expect(trioSourceSet(set, "high")).toEqual({ name: "x2", stars: 2, giants: 2 });
  });

  it("lights every body by both stars, each on its own slot, and the other two's planetshine", () => {
    const photoreal = SETTINGS.high.photoreal;
    const drawn = trioSourceSet(set, "high");
    const ids = ["0200080020000000.0341", "0200080020000000.0342", "0200080020000000.0343"];
    const inputs = scene.bodies.map((body, i) =>
      earthInput(ids[i] ?? "0200080020000000.0340", EARTH_REFERENCE, body.centreM),
    );
    const wrong = inputs.filter((input, i) => {
      const body = scene.bodies[i];
      if (body === undefined) {
        return true;
      }
      const listed = listSources(
        bodyLightSources(input, inputs, scene.lights, {
          annuli: photoreal.discAnnuli,
          planetshine: photoreal.planetshineSources,
        }),
        body.air,
        SLICES_EARTH.figure,
        EARTH_REFERENCE,
        2 * EARTH_REFERENCE_CURVES.length,
        TABLE_SIZES.high.marchSourceCap,
      );
      return (
        listed.left.length > 0 || !sourcesAsSet(listed, drawn, TABLE_SIZES.high.marchSourceCap)
      );
    });
    expect(wrong).toEqual([]);
  });

  it("takes no giant", () => {
    const x2p: SourceSet = { name: "x2p", stars: 1, giants: 1 };
    expect(() => smallEarthsScene(viewport, SLICES_EARTH.figure, x2p)).toThrow(RangeError);
  });
});

/** Each face texel's byte per class of `map`, face by face, row by row. */
function texelBytes(map: ClassMapTexels): number[][] {
  const side = classMapSide(map.faceTexels);
  const n = map.faceTexels;
  return Array.from({ length: 6 * n * n }, (_, t) => {
    const face = Math.floor(t / (n * n));
    const i = t % n;
    const j = Math.floor(t / n) % n;
    return Array.from({ length: map.classes }, (_unused, k) => {
      const layer = face + 6 * Math.floor(k / 4);
      return map.data[((layer * side + (j + 1)) * side + (i + 1)) * 4 + (k % 4)] ?? Number.NaN;
    });
  });
}

/** Whether each face cell of `normals` holds a normal, face by face, row by row. */
function normalsHeld(normals: NormalMapTexels): boolean[] {
  const n = normals.faceTexels;
  const side = classMapSide(n);
  return Array.from({ length: 6 * n * n }, (_, t) => {
    const face = Math.floor(t / (n * n));
    const i = t % n;
    const j = Math.floor(t / n) % n;
    const x = normals.data[((face * side + (j + 1)) * side + (i + 1)) * 2] ?? Number.NaN;
    return Math.abs(x) <= NO_NORMAL_ABOVE;
  });
}

/** The classes a texel's bytes hold, as `0,3,5`. */
function classesOf(texel: ReadonlyArray<number>): string {
  return texel.flatMap((b, k) => (b > 0 ? [k] : [])).join(",");
}

describe("R10's smoke class map", () => {
  const map = smokeClassMapTexels();
  const bytes = texelBytes(map);
  const n = map.faceTexels;

  it("holds two classes at R10's level with no maturity, as the smoke's R10.T10.d map", () => {
    expect([map.classes, map.faceTexels, map.maturity]).toEqual([
      2,
      CLASS_MAP_FACE_TEXELS,
      undefined,
    ]);
  });

  it("shades its classes by the smoke's lunar and terrestrial laws", () => {
    expect(smokeClassLaws().map((law) => law.template)).toEqual(["moon", "earth"]);
  });

  it("surveys half of every face, the edge column half, and nothing past it", () => {
    const off = bytes.filter((texel, t) => {
      const i = t % n;
      const sum = texel.reduce((a, b) => a + b, 0);
      return i < n / 2 - 1 ? sum !== 255 : i === n / 2 - 1 ? Math.abs(sum - 128) > 1 : sum !== 0;
    });
    expect(off).toEqual([]);
  });

  it("carries its normals at level 7 over the same half of every face", () => {
    const normals = smokeNormalMapTexels(SLICES_EARTH.figure);
    const m = 2 ** NORMAL_MAP_LEVEL;
    const wrong = normalsHeld(normals).filter((held, t) => held !== t % m < m / 2);
    expect([normals.faceTexels, wrong]).toEqual([128, []]);
  });
});

describe("a generated palette's class map", () => {
  const map = generatedClassMapTexels();
  const bytes = texelBytes(map);
  const n = map.faceTexels;
  const r = GENERATED_REGION_TEXELS;

  it("holds a generated patch's eight classes at R10's level", () => {
    expect([map.classes, map.faceTexels]).toEqual([8, CLASS_MAP_FACE_TEXELS]);
  });

  it("draws every class from its maturity nodes", () => {
    expect(map.maturity?.drawn).toEqual(Array.from({ length: GENERATED_CLASSES }, () => true));
  });

  it("gives each class a law", () => {
    expect(generatedClassLaws()).toHaveLength(GENERATED_CLASSES);
  });

  it("surveys every texel whole, three classes a texel", () => {
    const off = bytes.filter(
      (texel) =>
        texel.reduce((sum, b) => sum + b, 0) !== 255 || texel.filter((b) => b > 0).length !== 3,
    );
    expect([bytes.length, off]).toEqual([6 * 64 * 64, []]);
  });

  it("keeps one region's three classes across it", () => {
    const split = bytes.filter((texel, t) => {
      const face = Math.floor(t / (n * n));
      const i = t % n;
      const j = Math.floor(t / n) % n;
      const corner = bytes[face * n * n + (j - (j % r)) * n + (i - (i % r))] ?? [];
      return classesOf(texel) !== classesOf(corner);
    });
    expect(split).toEqual([]);
  });

  it("holds every class of the palette somewhere", () => {
    const held = new Set(bytes.flatMap((texel) => texel.flatMap((b, k) => (b > 0 ? [k] : []))));
    expect([...held].toSorted((a, b) => a - b)).toEqual([0, 1, 2, 3, 4, 5, 6, 7]);
  });

  it("carries the field's normals at level 7 on every cell", () => {
    const normals = generatedNormalMapTexels(SLICES_EARTH.figure);
    expect([normals.faceTexels, normalsHeld(normals).filter((held) => !held)]).toEqual([
      2 ** NORMAL_MAP_LEVEL,
      [],
    ]);
  });
});
