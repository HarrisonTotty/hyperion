import { afterEach, describe, expect, it, vi } from "vitest";

import propertiesSource from "../../../../../../../crates/hyperion-surface/src/material/properties.rs?raw";
import weatheredGoldenText from "../../../../../../../crates/hyperion-surface/tests/golden/worker/terrain_test_weathered.golden?raw";
import type { ViewsCheckTerrainFixture } from "../../../../preload/api";
import { SCENE_OPTIONS } from "../../displays/view/viewRun";
import { dot, norm, normalise, sub, type Vec3 } from "../../geometry/vec3";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { sceneFrameFor } from "../camera/state";
import { rotateToBody } from "../coords/rotation";
import { cameraSceneOf, type ViewBody, type ViewScene } from "../scene/model";
import { fieldCellIndex } from "../terrain/coverage";
import { uvToSt, xyzToFaceUv } from "../terrain/cube";
import { datumGroundUnder, finestPatchSizeM } from "../terrain/grounded";
import { handoverRangeM, handoverTauPx, pixelAngleRad } from "../terrain/handover";
import { planetGeometry } from "../terrain/planet";
import { sceneContacts } from "../terrain/session";
import { classRowLaw, PHASE_LAW } from "../terrain/terrainMaterial";
import { keptTime } from "./kept";
import { blockBytes, parseTerrainGolden } from "./terrainGolden";
import {
  LIMB_ONSET_NORMAL_ALBEDO,
  TERRAIN_LOWEST_ALTITUDE_M,
  TERRAIN_MOON_CENTRE_M,
  TERRAIN_SCENE_NAME,
  TERRAIN_TEST_SCRIPT,
  TERRAIN_TEST_UNIVERSE,
  TERRAIN_TIME_RATE,
  terrainMoonPhotometry,
  terrainScene,
  terrainSceneAt,
  terrainSceneFixture,
  terrainShipAltitudeM,
  terrainShipBodyFixedM,
  terrainTestGolden,
} from "./terrainScene";

const scene = terrainScene();

/** `TEST MOON` as the scene holds it at script time `tS`. */
function moonAt(tS: number): { readonly scene: ViewScene; readonly moon: ViewBody } {
  const at = terrainSceneAt(tS);
  const moon = at.bodies.find((body) => body.designation === "TEST MOON");
  if (moon === undefined) {
    throw new Error("the scene holds TEST MOON");
  }
  return { scene: at, moon };
}

/** The script's times from 0 to its end, every `stepS` seconds. */
function times(stepS: number): number[] {
  return Array.from(
    { length: Math.floor(TERRAIN_TEST_SCRIPT.durationS / stepS) + 1 },
    (_, k) => k * stepS,
  );
}

/** The sun's elevation at the ship's ground point at script time `tS`, rad, over the sphere. */
function sunElevationRad(tS: number): number {
  const { moon } = moonAt(tS);
  if (moon.rotation === null) {
    throw new Error("TEST MOON turns");
  }
  const up = normalise(rotateToBody(moon.rotation, terrainShipBodyFixedM(tS)));
  const sun = normalise(sub({ x: 0, y: 0, z: 0 }, TERRAIN_MOON_CENTRE_M));
  return Math.asin(dot(up, sun));
}

/** Whether the cell of the field's level under body-fixed `p` is surveyed at revision 2. */
function surveyedUnder(p: Vec3): boolean {
  const { header, surveys } = terrainTestGolden();
  const { face, u, v } = xyzToFaceUv([p.x, p.y, p.z]);
  const side = 2 ** header.level;
  const cell = (s: number): number => Math.min(side - 1, Math.floor(uvToSt(s) * side));
  const index = fieldCellIndex({ face, level: header.level, i: cell(u), j: cell(v) });
  return (surveys[1]?.ranges ?? []).some(
    ({ start, end, resolution }) => start <= index && index < end && resolution > 0,
  );
}

describe("TERRAIN TEST (R10.T16.d)", () => {
  it("is a kept scene of SCENE_OPTIONS, with the one terrain fixture", () => {
    expect(
      SCENE_OPTIONS.filter((option) => option.terrain !== undefined).map(({ name }) => name),
    ).toEqual([TERRAIN_SCENE_NAME]);
    expect(scene.sceneAt(0).provenance).toEqual({ kind: "kept", name: TERRAIN_SCENE_NAME });
  });

  it("takes TEST MOON's ID, detail seed and figure from the terrain golden", () => {
    const golden = terrainTestGolden();
    const { moon } = moonAt(0);
    expect([moon.id, moon.detailSeed]).toEqual([golden.body, golden.seed]);
    const figure = moon.appearance?.figure;
    expect([
      Object.is(moon.radiusM, golden.header.equatorial_radius_m),
      Object.is(figure?.equatorialRadiusM, golden.header.equatorial_radius_m),
      Object.is(figure?.polarRadiusM, golden.header.polar_radius_m),
    ]).toEqual([true, true, true]);
  });

  it("sits in the synthetic worlds' system, every other body at .0000 or an index of 7 and up", () => {
    const at = scene.sceneAt(0);
    expect(at.system).toBe("5e5e000000000000");
    const indices = at.bodies.map((body) => {
      const [system, index] = body.id.split(".");
      return [system, Number.parseInt(index ?? "", 16), body.designation] as const;
    });
    expect(indices).toEqual([
      ["5e5e000000000000", 0, "TEST STAR"],
      ["5e5e000000000000", 3, "TEST MOON"],
    ]);
  });

  it("runs its clock at a whole 60 s/s, its moon turning", () => {
    const at = scene.sceneAt(10);
    expect([at.timeRate, at.time]).toEqual([TERRAIN_TIME_RATE, keptTime(10 * TERRAIN_TIME_RATE)]);
    expect(moonAt(0).moon.rotation).not.toEqual(moonAt(10).moon.rotation);
  });

  it("starts beyond the hand-over range on either setting and descends to its lowest point", () => {
    const { header } = terrainTestGolden();
    const radPerPx = pixelAngleRad(
      { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 },
      { widthPx: 1920, heightPx: 1080 },
    );
    for (const setting of ["high", "low"] as const) {
      const rangeM = handoverRangeM(
        header.realised_relief_m,
        handoverTauPx("photorealistic", setting),
        radPerPx,
      );
      expect(terrainShipAltitudeM(0)).toBeGreaterThan(1.5 * rangeM);
    }
    expect(terrainShipAltitudeM(TERRAIN_TEST_SCRIPT.descentEndS)).toBeCloseTo(
      TERRAIN_LOWEST_ALTITUDE_M,
      6,
    );
    expect(surveyedUnder(terrainShipBodyFixedM(TERRAIN_TEST_SCRIPT.descentEndS))).toBe(true);
  });

  it("keeps its seat camera in TEST MOON's frame throughout", () => {
    const frames = times(5).map((tS) => {
      const at = scene.sceneAt(tS);
      return sceneFrameFor(scene.cameraAt(tS), cameraSceneOf(at));
    });
    const moon = terrainTestGolden().body;
    expect(frames.every((frame) => frame.kind === "body" && frame.body === moon)).toBe(true);
  });

  it("is never a contact: its lowest point is 4 km over the datum, never closing fast", () => {
    const { header } = terrainTestGolden();
    const figure = {
      equatorialRadiusM: header.equatorial_radius_m,
      polarRadiusM: header.polar_radius_m,
      pole: null,
    };
    const patchM = finestPatchSizeM(planetGeometry(figure, null));
    const contacts = times(1).flatMap((tS) => {
      const { scene: at, moon } = moonAt(tS);
      return sceneContacts(at, moon, datumGroundUnder(figure), patchM);
    });
    expect(contacts).toEqual([]);
  });

  it("sets the sun over the surveyed ground, then crosses the survey's edge", () => {
    const { descentEndS, durationS, sunsetS } = TERRAIN_TEST_SCRIPT;
    expect(sunElevationRad(descentEndS)).toBeGreaterThan((5 * Math.PI) / 180);
    expect(sunElevationRad(sunsetS - 2)).toBeGreaterThan(0);
    expect(sunElevationRad(sunsetS + 2)).toBeLessThan(0);
    expect(surveyedUnder(terrainShipBodyFixedM(sunsetS))).toBe(true);
    const edgeS = times(1).find(
      (tS) => tS > descentEndS && !surveyedUnder(terrainShipBodyFixedM(tS)),
    );
    expect(edgeS).toBeGreaterThan(sunsetS);
    expect(edgeS).toBeLessThan(durationS);
  });

  it("moves its ship at its path's velocity against TEST MOON", () => {
    // Over the traverse the height holds, so the velocity is along the ground: no closing speed.
    const tS = 260;
    const { scene: at, moon } = moonAt(tS);
    const v = at.craft[0]?.velocityMPerS;
    if (v === undefined || v === null || moon.rotation === null) {
      throw new Error("the ship moves and the moon turns");
    }
    const up = normalise(rotateToBody(moon.rotation, terrainShipBodyFixedM(tS)));
    expect(Math.abs(dot(v, up)) / norm(v)).toBeLessThan(1e-3);
  });

  it("takes TEST MOON's photometry from its primary crust under the Regolith law", () => {
    const { header } = terrainTestGolden();
    const crust = header.palette.find((entry) => entry.role === "primary_crust");
    const photometry = terrainMoonPhotometry(header);
    const [b, v, r] = crust?.normal_albedo_bvr ?? [Number.NaN, Number.NaN, Number.NaN];
    expect(photometry.geometricAlbedo).toEqual([r, v, b]);
    expect(photometry.law).toEqual({ ...classRowLaw(PHASE_LAW.regolith, 1), a: [r, v, b] });
    expect(moonAt(0).moon.appearance?.photometry).toEqual(photometry);
    expect(() => terrainMoonPhotometry({ ...header, albedo_scale: 0.8 })).toThrow("albedo scale");
  });

  it("takes the particulate rule's onset from the surface crate's source (a tripwire)", () => {
    const stated = /pub const LUNAR_LIMB_ONSET_NORMAL_ALBEDO: f64 = ([0-9._]+);/u.exec(
      propertiesSource,
    )?.[1];
    if (stated === undefined) {
      throw new Error(
        "crates/hyperion-surface/src/material/properties.rs no longer states LUNAR_LIMB_ONSET_NORMAL_ALBEDO",
      );
    }
    expect(LIMB_ONSET_NORMAL_ALBEDO).toBe(Number(stated.replaceAll("_", "")));
  });
});

/** The first payload's blocks of `fixture`'s golden, as byte arrays. */
function firstBlocksOf(fixture: ViewsCheckTerrainFixture): number[][] {
  return (terrainTestGolden(fixture).payloads[0]?.blocksHex ?? []).map((hex) =>
    Array.from(blockBytes(hex)),
  );
}

describe("TERRAIN TEST on TEST MOON weathered's field (R10.T1.c)", () => {
  const WEATHERED = "terrain-test-weathered";

  it("reads the weathered golden once: maturity 1, rate 1 and its haloed albedo scale", () => {
    const golden = terrainTestGolden(WEATHERED);
    expect([
      golden.header.maturity,
      golden.header.weathering_rate,
      golden.header.albedo_scale,
      golden.header.surface_age_gyr,
      terrainTestGolden(WEATHERED) === golden,
    ]).toEqual([1, 1, 1.2299665675542657, 1, true]);
    expect(golden).toEqual(
      parseTerrainGolden(weatheredGoldenText, "terrain_test_weathered.golden"),
    );
  });

  it("is TEST MOON again: its ID, seed and figure TERRAIN TEST's, in the same system", () => {
    const plain = terrainTestGolden();
    const weathered = terrainTestGolden(WEATHERED);
    expect([
      weathered.body,
      weathered.seed,
      weathered.header.equatorial_radius_m,
      weathered.header.polar_radius_m,
    ]).toEqual([
      plain.body,
      plain.seed,
      plain.header.equatorial_radius_m,
      plain.header.polar_radius_m,
    ]);
  });

  it("takes TEST MOON's photometry from the golden's crust_albedo under the Regolith law", () => {
    const { header, maturity } = terrainTestGolden(WEATHERED);
    const [b, v, r] = maturity?.crustAlbedoBvr ?? [Number.NaN, Number.NaN, Number.NaN];
    const photometry = terrainMoonPhotometry(header, maturity);
    expect([photometry.law, photometry.geometricAlbedo]).toEqual([
      { ...classRowLaw(PHASE_LAW.regolith, 1), a: [r, v, b] },
      [r, v, b],
    ]);
    // The palette's own albedo is not what the weathered field draws: its header scales it.
    expect(() => terrainMoonPhotometry(header)).toThrow("albedo scale");
  });

  it("makes the scene on its field, under TERRAIN TEST's name, its moon drawn with that photometry", () => {
    const { header, maturity } = terrainTestGolden(WEATHERED);
    const kept = terrainScene(WEATHERED);
    const at = kept.sceneAt(0);
    const moon = at.bodies.find((body) => body.designation === "TEST MOON");
    expect([kept.name, at.provenance, moon?.id, moon?.appearance?.photometry]).toEqual([
      TERRAIN_SCENE_NAME,
      { kind: "kept", name: TERRAIN_SCENE_NAME },
      terrainTestGolden().body,
      terrainMoonPhotometry(header, maturity),
    ]);
  });

  describe("its fixture", () => {
    afterEach(() => {
      vi.useRealTimers();
    });

    it("serves the weathered golden's header and first payload's blocks, not TERRAIN TEST's", async () => {
      vi.useFakeTimers();
      const golden = terrainTestGolden(WEATHERED);
      const link = terrainSceneFixture(0, WEATHERED).open();
      const chunks: number[][] = [];
      const pending = link.requests.requestBulk(
        {
          kind: "surface_field",
          universe: TERRAIN_TEST_UNIVERSE,
          body: golden.body,
          have_revision: null,
        },
        (response) => (response.state === "ready" ? response.bulk : null),
        {
          onChunk: (chunk) => {
            chunks.push(Array.from(chunk.payload));
          },
        },
      );
      await vi.runAllTimersAsync();
      const outcome = await pending.outcome;
      link.close();
      expect([
        outcome.ok && outcome.response.state === "ready" ? outcome.response.header : null,
        chunks,
      ]).toEqual([golden.header, firstBlocksOf(WEATHERED)]);
      expect(chunks).not.toEqual(firstBlocksOf("terrain-test"));
    });
  });
});
