import { describe, expect, it } from "vitest";

import { add, dot, norm, normalise, scale, sub, vec3 } from "../../../geometry/vec3";
import { rotate } from "../../../view/camera/quaternion";
import {
  ATMOSPHERE_FIXTURES,
  ATMOSPHERE_SCENE_NAME,
  atmosphereSeatM,
} from "../../../view/scenes/atmosphereScene";
import { cameraPlace } from "../cameraPlace";
import { frameName, SCENE_OPTIONS, startRun } from "../viewRun";
import {
  ATMOSPHERE_GOAL_RADII,
  ATMOSPHERE_SMOKE_TOUR,
  ATMOSPHERE_TOUR,
  atmosphereLegs,
  PLACE_RANGE_TOLERANCE,
  positionRangeM,
} from "./atmosphereCheckRun";
import {
  ATMOSPHERE_LOOKS,
  type AtmospherePlace,
  atmospherePlaces,
  type GroundLook,
  JUPITER_OUTSIDE_RANGE_M,
  placedAtmosphere,
  placedAtmosphereScene,
  placeSceneName,
} from "./atmospherePlaces";

/** The camera's line of sight at a place, along the galactic axes. */
function forwardOf(place: AtmospherePlace) {
  return rotate(place.orientation, vec3(0, 0, -1));
}

/** A fixture's centre by its designation. */
function centreOf(designation: string) {
  const fixture = ATMOSPHERE_FIXTURES.find((each) => each.designation === designation);
  if (fixture === undefined) {
    throw new Error(`no fixture ${designation}`);
  }
  return fixture;
}

/** An angle's degrees. */
function deg(rad: number): number {
  return (rad * 180) / Math.PI;
}

const FULL = atmospherePlaces(ATMOSPHERE_TOUR, ATMOSPHERE_LOOKS);

describe("the software mode's places (R08.T17)", () => {
  it("stand each of the tour's captures at its leg's planned end, looking at its fixture's centre", () => {
    const captures = atmosphereLegs(atmosphereSeatM(), ATMOSPHERE_TOUR).flatMap((leg) =>
      leg.capture === null ? [] : [{ ...leg.capture, toM: leg.toM, centreM: leg.centreM }],
    );
    const tour = atmospherePlaces(ATMOSPHERE_TOUR, []);
    expect([
      tour.map((place) => place.label),
      tour.every((place, i) => {
        const capture = captures[i];
        return (
          capture !== undefined &&
          norm(sub(add(capture.centreM, place.fromCentreM), capture.toM)) < 1e-3 &&
          Math.abs(place.rangeM / capture.rangeM - 1) < 1e-12 &&
          Math.abs(place.phaseDeg - capture.phaseDeg) < 1e-9 &&
          dot(forwardOf(place), normalise(scale(place.fromCentreM, -1))) > 1 - 1e-12 &&
          place.meter === "lit"
        );
      }),
    ]).toEqual([captures.map((capture) => capture.label), true]);
  });

  it("add the by-eye looks after their fixture's last tour capture", () => {
    expect(FULL.map((place) => place.label)).toEqual([
      "fixture-test-earth-o3-slab",
      "fixture-test-mars",
      "ground-test-mars-day",
      "ground-test-mars-sunset",
      "fixture-test-saturn",
      "fixture-test-saturn-pole",
      "fixture-test-venus",
      "ground-test-venus-day",
      "fixture-test-venus-5-80-mpa",
      "fixture-test-titan",
      "fixture-test-titan-9000-km",
      "fixture-test-saturn-nh3-ice-deck",
      "fixture-test-jupiter-950000-km",
      "fixture-test-jupiter-500000-km",
      "fixture-test-jupiter-220000-km",
      "fixture-test-jupiter-150000-km",
      "fixture-test-jupiter-1050000-km",
      "fixture-test-earth",
      "ground-test-earth-noon",
      "ground-test-earth-sunset",
      "ground-test-earth-400-km",
      "ground-test-earth-night-horizon",
    ]);
  });

  it("are the smoke's one fixture alone in a smoke", () => {
    expect(placedAtmosphere(true).places.map((place) => place.label)).toEqual([
      "fixture-test-earth-o3-slab",
    ]);
  });

  it("stand TEST JUPITER outside 10⁹ m on the tour's line, and TEST SATURN at the goal over its pole", () => {
    const at = (label: string): AtmospherePlace => {
      const place = FULL.find((each) => each.label === label);
      if (place === undefined) {
        throw new Error(`no place ${label}`);
      }
      return place;
    };
    const outside = at("fixture-test-jupiter-1050000-km");
    const inside = at("fixture-test-jupiter-950000-km");
    const pole = at("fixture-test-saturn-pole");
    expect([
      Math.abs(outside.rangeM / JUPITER_OUTSIDE_RANGE_M - 1) < 1e-12,
      dot(normalise(outside.fromCentreM), normalise(inside.fromCentreM)) > 1 - 1e-12,
      pole.rangeM / centreOf("TEST SATURN").figure.equatorialRadiusM,
      normalise(pole.fromCentreM).z,
    ]).toEqual([true, true, ATMOSPHERE_GOAL_RADII, 1]);
  });

  it("stand each view from the ground at its height, the star at its elevation over the horizon", () => {
    const ground = ATMOSPHERE_LOOKS.filter((look): look is GroundLook => look.kind === "ground");
    const off = ground.flatMap((look) => {
      const place = FULL.find((each) => each.label === look.label);
      const fixture = centreOf(look.designation);
      if (place === undefined) {
        return [`${look.label}: not placed`];
      }
      const vertical = normalise(place.fromCentreM);
      const towardStar = normalise(scale(add(fixture.centreM, place.fromCentreM), -1));
      const heightM = norm(place.fromCentreM) - fixture.figure.equatorialRadiusM;
      // The star's parallax across the body's radius, under 0.003° at 1 au.
      const elevationDeg = deg(Math.asin(dot(towardStar, vertical)));
      return Math.abs(heightM - look.heightM) < 1e-6 * look.heightM + 1e-6 &&
        Math.abs(elevationDeg - look.sunElevationDeg) < 0.01 &&
        place.meter === "average"
        ? []
        : [`${look.label}: ${String(heightM)} m, the star ${String(elevationDeg)}°`];
    });
    expect(off).toEqual([]);
  });

  it("look round from the star's azimuth by each view's azimuth, at its pitch over the apparent horizon", () => {
    const ground = ATMOSPHERE_LOOKS.filter((look): look is GroundLook => look.kind === "ground");
    const off = ground.flatMap((look) => {
      const place = FULL.find((each) => each.label === look.label);
      const fixture = centreOf(look.designation);
      if (place === undefined) {
        return [`${look.label}: not placed`];
      }
      const radiusM = fixture.figure.equatorialRadiusM;
      const vertical = normalise(place.fromCentreM);
      const forward = forwardOf(place);
      const flat = (v: ReturnType<typeof vec3>) =>
        normalise(sub(v, scale(vertical, dot(v, vertical))));
      const towardStar = normalise(scale(fixture.centreM, -1));
      const dipDeg = deg(Math.acos(radiusM / (radiusM + look.heightM)));
      const pitchDeg = deg(Math.asin(dot(forward, vertical))) + dipDeg;
      const azimuthDeg = deg(Math.acos(Math.min(1, dot(flat(forward), flat(towardStar)))));
      const up = rotate(place.orientation, vec3(0, 1, 0));
      return Math.abs(pitchDeg - look.pitchDeg) < 1e-6 &&
        Math.abs(azimuthDeg - look.azimuthFromStarDeg) < 1e-4 &&
        dot(up, vertical) > 0
        ? []
        : [`${look.label}: pitch ${String(pitchDeg)}°, azimuth ${String(azimuthDeg)}°`];
    });
    expect(off).toEqual([]);
  });

  it("refuse a view from the ground of a body that is no sphere", () => {
    expect(() =>
      atmospherePlaces(
        [],
        [
          {
            kind: "ground",
            label: "ground-test-saturn",
            designation: "TEST SATURN",
            sunElevationDeg: 45,
            heightM: 10,
            azimuthFromStarDeg: 90,
            pitchDeg: 8,
          },
        ],
      ),
    ).toThrow("TEST SATURN is no sphere to stand on");
  });

  it("are each named in the SCENE selector for its capture's label", () => {
    expect([placeSceneName("fixture-test-mars"), FULL[0]?.scene]).toEqual([
      "PLACED FIXTURE-TEST-MARS",
      "PLACED FIXTURE-TEST-EARTH-O3-SLAB",
    ]);
  });

  it("are offered as two scenes after VIEW's own, ATMOSPHERE TEST's among them", () => {
    const { sceneOptions } = placedAtmosphere(false);
    expect(sceneOptions.map((option) => option.name)).toEqual([
      ...SCENE_OPTIONS.map((option) => option.name),
      "PLACE A",
      "PLACE B",
    ]);
    expect(SCENE_OPTIONS.map((option) => option.name)).toContain(ATMOSPHERE_SCENE_NAME);
  });

  it("are each made by the scene the run is told to choose, the two in turn", () => {
    const placed = placedAtmosphere(false);
    const made = placed.places.slice(0, 3).map((place) => {
      const name = placed.offer(place);
      const option = placed.sceneOptions.find((each) => each.name === name);
      return [name, option?.make().sceneAt(0).provenance];
    });
    expect(made).toEqual(
      placed.places
        .slice(0, 3)
        .map((place, i) => [
          i % 2 === 0 ? "PLACE A" : "PLACE B",
          { kind: "kept", name: place.scene },
        ]),
    );
  });
});

describe("a place's kept scene (R08.T17)", () => {
  it("has no own ship, so its camera starts free at the place, photorealistic, on the place's meter", () => {
    const place = FULL.find((each) => each.label === "ground-test-earth-noon");
    if (place === undefined) {
      throw new Error("no place ground-test-earth-noon");
    }
    const kept = placedAtmosphereScene(place);
    const run = startRun(kept);
    const scene = kept.sceneAt(0);
    expect([
      scene.ownShip,
      scene.craft,
      scene.provenance,
      run.camera.preset,
      run.camera.pose,
      run.camera.style,
      kept.startMeter,
    ]).toEqual([
      null,
      [],
      { kind: "kept", name: "PLACED GROUND-TEST-EARTH-NOON" },
      "free",
      kept.cameraAt(0),
      "photorealistic",
      "average",
    ]);
  });

  it("reads its fixture's frame on the FRAME line and its range on the POSITION line, within the run's tolerance", () => {
    const off = FULL.flatMap((place) => {
      const run = startRun(placedAtmosphereScene(place));
      const frame = frameName(run.camera.pose.frame, run.scene);
      const rangeM = positionRangeM(cameraPlace(run, null).position);
      return frame === `BODY ${place.designation}` &&
        rangeM !== null &&
        Math.abs(rangeM / place.rangeM - 1) <= PLACE_RANGE_TOLERANCE
        ? []
        : [`${place.label}: ${frame}, ${String(rangeM)} m`];
    });
    expect(off).toEqual([]);
  });

  it("is the smoke tour's capture place in a smoke, as the flown run's", () => {
    const leg = atmosphereLegs(atmosphereSeatM(), ATMOSPHERE_SMOKE_TOUR).find(
      (each) => each.capture !== null,
    );
    const [place] = placedAtmosphere(true).places;
    expect(place?.rangeM).toBeCloseTo(leg?.capture?.rangeM ?? Number.NaN, 3);
  });
});
