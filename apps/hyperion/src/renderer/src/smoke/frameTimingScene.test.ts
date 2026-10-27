import { describe, expect, it } from "vitest";

import { normalise, sub } from "../geometry/vec3";
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
import { SUN_RADIUS_M } from "../view/lighting/hostDisc";
import { type QualitySetting, SETTINGS } from "../view/quality/qualitySetting";
import { AU_M } from "../view/scenes/kept";
import { sphereOutsideView } from "../view/wireframe/submit";
import { FAR_LIMB_TIMING_PLACES, settingOutput, SLICES_EARTH } from "./atmosphere";
import {
  angleBetweenRad,
  earthInput,
  expectedMarch,
  expectedSources,
  farLimbFrameScene,
  type FrameScene,
  GIANT_DISTANCE_M,
  GIANT_EQUATORIAL_RADIUS_M,
  listSources,
  type ListedSources,
  planetshineShare,
  type SourceSet,
  smallEarthScene,
  sourceSetsOf,
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
  it("names the setting's whole count by its sources: x4 on high, x3 on low", () => {
    expect(SKY_SUN_CAP.high).toBe(2);
    expect(SKY_SUN_CAP.low).toBe(2);
    expect(sourceSetsOf("high").map((set) => set.name)).toEqual(["x1", "x2", "x2p", "x4"]);
    expect(sourceSetsOf("low").map((set) => set.name)).toEqual(["x1", "x2", "x2p", "x3"]);
  });

  it("draws every setting's sets at every camera on every medium, giants in half of them", () => {
    expect(CASES).toHaveLength(2 * 3 * 5 * 4);
    expect(CASES.filter(({ scene }) => scene.giants.length > 0)).toHaveLength(2 * 3 * 5 * 2);
  });

  it("keeps exactly each set's sources, kind and light slot, at every camera on every medium", () => {
    const wrong = CASES.filter(({ set, listed }) => {
      const kept = listed.kept.map(({ kind, curveSlot }) => ({ kind, curveSlot }));
      return (
        JSON.stringify(kept) !== JSON.stringify(expectedSources(set)) || listed.left.length > 0
      );
    }).map(({ label }) => label);
    expect(wrong).toEqual([]);
  });

  it("marches the set's two: x2 the two stars, x2p the Sun and the giant, the whole count the stars", () => {
    const wrong = CASES.filter(({ setting, set, listed }) => {
      const march = listed.march.map(({ kind, curveSlot }) => ({ kind, curveSlot }));
      const meant = expectedMarch(set, TABLE_SIZES[setting].marchSourceCap);
      return JSON.stringify(march) !== JSON.stringify(meant);
    }).map(({ label }) => label);
    expect(wrong).toEqual([]);
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
  it("draws the Earth whole at its diameter in pixels, centred, the Sun 40° off the sight line", () => {
    const viewport = settingOutput("high");
    for (const diameterPx of [4, 10, 32]) {
      const scene = smallEarthScene(diameterPx, viewport, SLICES_EARTH.figure);
      const pxPerRad = viewport.widthPx / (2 * Math.tan(scene.camera.fovXRad / 2));
      const distanceM = -scene.earthCentreM.z;
      const drawnPx = 2 * Math.asin(SLICES_EARTH.figure.equatorialRadiusM / distanceM) * pxPerRad;
      expect(drawnPx).toBeCloseTo(diameterPx, 6);
      expect(scene.air.positionM).toEqual({ x: 0, y: 0, z: distanceM });
      const sun = scene.lights[0];
      expect(sun).toBeDefined();
      const offRad = angleBetweenRad(
        sub(sun?.centreM ?? scene.earthCentreM, scene.earthCentreM),
        scene.air.positionM,
      );
      expect((offRad * 180) / Math.PI).toBeCloseTo(40, 6);
    }
  });
});
