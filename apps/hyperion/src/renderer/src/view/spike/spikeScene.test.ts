import { describe, expect, it } from "vitest";

import { cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { CameraPose } from "../camera/pose";
import { lookAlong } from "../camera/quaternion";
import { rotateToBody } from "../coords/rotation";
import { terrainStar, terrainSystem, terrainTestGolden } from "../scenes/terrainScene";
import { MAX_BLOCK_BYTES } from "../terrain/coarseFieldStore";
import { vertexSpacing, type Xyz } from "../terrain/cube";
import { isDescending } from "../terrain/grounded";
import { spheroidNormal, spheroidPoint } from "../terrain/planet";
import { datumDirection, DescentProfile, landingSiteOf } from "./descentProfile";
import { testPlanetRotationAt } from "./rotation";
import {
  cameraElevationDeg,
  contactRule,
  GROUNDED_CLEARANCE_M,
  groundedClearanceM,
  ORBIT_INSTRUMENT_RADII,
  orbitInstrumentPose,
  postSyntheticField,
  siteDirection,
  SPIKE_CRAFT,
  SPIKE_CRAFT_RADIUS_M,
  SPIKE_PLANET,
  spikeCameraAt,
  spikeContactAt,
  spikeScene,
  sunDirectionBody,
  SYNTHETIC_FIELD_BYTES,
  syntheticFieldBlocks,
  TEST_PLANET_FIGURE,
} from "./spikeScene";
import { terrainTestWorld } from "./spikeWorld";

/** A triple as a vector. */
function vecOf([x, y, z]: Xyz) {
  return vec3(x, y, z);
}

const PROFILE = new DescentProfile(TEST_PLANET_FIGURE, landingSiteOf(5n), {
  siteHeightM: 812.5,
  trackMaxHeightM: 1_400,
});

/**
 * T13.b's sun over the test planet, as `sunDirectionBody` computed it before worlds: the site at the
 * script's end in the planet's axes, its x and y with z = 0, normalised.
 */
function t13bSunDirection(profile: DescentProfile): Vec3 {
  const end = profile.durationS;
  const s = rotateToBody(testPlanetRotationAt(end), profile.poseAt(end).groundPointM);
  return normalise(vec3(s.x, s.y, 0));
}

/** T13.b's orbit instrument over the test planet, as `orbitInstrumentPose` built it with pole +z. */
function t13bOrbitPose(profile: DescentProfile, tS: number): CameraPose {
  const out = normalise(spikeCameraAt(profile, tS).positionM);
  const pole = vec3(0, 0, 1);
  const up = Math.abs(dot(out, pole)) > 0.99 ? t13bSunDirection(profile) : pole;
  const forward = scale(out, -1);
  return {
    frame: { kind: "body", body: SPIKE_PLANET },
    positionM: scale(out, ORBIT_INSTRUMENT_RADII * TEST_PLANET_FIGURE.equatorialRadiusM),
    orientation: lookAlong(forward, normalise(sub(up, scale(forward, dot(up, forward))))),
  };
}

describe("the craft as a contact", () => {
  const rule = contactRule(PROFILE);

  it("becomes one exactly when it is first descending on the scripted path", () => {
    let descended = false;
    // The script at the fixed step of Design note 19 (64 Hz), up to the first onset.
    for (let n = 0; n / 64 <= (rule.firstOnsetS ?? Number.NaN); n += 1) {
      const t = n / 64;
      const pose = PROFILE.poseAt(t);
      descended ||= isDescending(pose.heightAboveFloorM, pose.verticalSpeedMps);
      expect(spikeContactAt(PROFILE, rule, t) !== null).toBe(descended);
    }
    const first = rule.firstOnsetS ?? Number.NaN;
    const before = PROFILE.poseAt(first - 1e-6);
    const at = PROFILE.poseAt(first);
    expect(isDescending(before.heightAboveFloorM, before.verticalSpeedMps)).toBe(false);
    expect(isDescending(at.heightAboveFloorM, at.verticalSpeedMps)).toBe(true);
    expect(spikeContactAt(PROFILE, rule, first)).not.toBeNull();
  });

  it("is one while descending, and held from its last descent to the end of the script", () => {
    const hold = rule.holdFromS ?? Number.NaN;
    for (let n = 0; n / 4 <= PROFILE.durationS; n += 1) {
      const t = n / 4;
      const pose = PROFILE.poseAt(t);
      const descending = isDescending(pose.heightAboveFloorM, pose.verticalSpeedMps);
      expect(spikeContactAt(PROFILE, rule, t) !== null).toBe(descending || t >= hold);
    }
    expect(spikeContactAt(PROFILE, rule, PROFILE.durationS)).not.toBeNull();
  });

  it("is one while hovering at zero vertical speed near the ground", () => {
    const hover = new DescentProfile(TEST_PLANET_FIGURE, landingSiteOf(5n), {
      siteHeightM: 812.5,
    });
    const t = hover.durationS;
    const pose = hover.poseAt(t);
    expect(pose.verticalSpeedMps).toBe(0);
    expect(isDescending(pose.heightAboveFloorM, pose.verticalSpeedMps)).toBe(false);
    // The rule alone, without the hold from the last descent.
    const unheld = { firstOnsetS: null, holdFromS: null };
    expect(spikeContactAt(hover, unheld, t)).not.toBeNull();
    expect(pose.clearanceM).toBeLessThanOrEqual(GROUNDED_CLEARANCE_M);
  });

  it("is released over the level low pass", () => {
    const span = PROFILE.segmentSpans().find((each) => each.name === "low fast pass");
    if (span === undefined) {
      throw new Error("the script has no low fast pass");
    }
    const mid = (span.startS + span.endS) / 2;
    expect(spikeContactAt(PROFILE, rule, mid)).toBeNull();
    expect(rule.holdFromS ?? 0).toBeGreaterThan(span.endS - 5);
  });

  it("stands at the site's terrain beneath the craft with the hull's bounding radius", () => {
    const t = PROFILE.durationS;
    expect(spikeContactAt(PROFILE, rule, t)).toEqual({
      positionM: PROFILE.poseAt(t).groundPointM,
      radiusM: SPIKE_CRAFT_RADIUS_M,
    });
    // The stern's corners, (±4, ±2.5, 10) m from the reference point.
    expect(SPIKE_CRAFT_RADIUS_M).toBeCloseTo(Math.hypot(4, 2.5, 10), 12);
  });
});

describe("the datum direction", () => {
  it("is the bake's direction d of a datum point M·d, not the point's geocentric direction", () => {
    const d = normalise({ x: 0.3, y: -0.5, z: 0.81 });
    const p = spheroidPoint(TEST_PLANET_FIGURE, [d.x, d.y, d.z]);
    const back = datumDirection(TEST_PLANET_FIGURE, { x: p[0], y: p[1], z: p[2] });
    expect(back.x).toBeCloseTo(d.x, 14);
    expect(back.y).toBeCloseTo(d.y, 14);
    expect(back.z).toBeCloseTo(d.z, 14);
    // The geocentric direction of the same point differs, here by about 0.05° in z alone.
    const geocentric = normalise({ x: p[0], y: p[1], z: p[2] });
    expect(Math.abs(geocentric.z - d.z)).toBeGreaterThan(5e-4);
  });

  it("gives the site's own direction for its height query", () => {
    const site = landingSiteOf(5n);
    const datum = new DescentProfile(TEST_PLANET_FIGURE, site);
    const [x, y, z] = siteDirection(datum);
    const ground = datum.poseAt(datum.durationS).groundPointM;
    const p = spheroidPoint(TEST_PLANET_FIGURE, [x, y, z]);
    expect(Math.hypot(p[0] - ground.x, p[1] - ground.y, p[2] - ground.z)).toBeLessThan(1e-6);
  });
});

describe("the light", () => {
  it("stands 29.9° or more above the landing site's horizon at touchdown", () => {
    for (const seed of [1n, 2n, 3n, 4n, 5n, 6n, 7n, 8n]) {
      const profile = new DescentProfile(TEST_PLANET_FIGURE, landingSiteOf(seed));
      const end = profile.durationS;
      const g = profile.poseAt(end).groundDir;
      const normal = spheroidNormal(TEST_PLANET_FIGURE, [g.x, g.y, g.z]);
      const up = normalise(
        rotateToBody(testPlanetRotationAt(end), { x: normal[0], y: normal[1], z: normal[2] }),
      );
      expect(dot(up, sunDirectionBody(profile))).toBeGreaterThanOrEqual(
        Math.sin((29.9 * Math.PI) / 180),
      );
    }
  });
});

describe("the scene", () => {
  it("turns the planet and flies the craft on the script", () => {
    const t = 1_000;
    const scene = spikeScene(PROFILE).sceneAt(t);
    const planet = scene.bodies.find((body) => body.id === SPIKE_PLANET);
    const craft = scene.craft.find((each) => each.id === SPIKE_CRAFT);
    expect(planet?.rotation).toEqual(testPlanetRotationAt(t));
    expect(craft?.pose.position).toEqual({
      kind: "body_fixed",
      body: SPIKE_PLANET,
      m: PROFILE.poseAt(t).positionM,
    });
    expect(scene.ownShip).toBe(SPIKE_CRAFT);
    expect(scene.provenance).toEqual({ kind: "kept", name: "DESCENT SPIKE" });
  });

  it("puts the planet 1 au from the star, on the side the light comes from", () => {
    const scene = spikeScene(PROFILE).sceneAt(0);
    const centre = scene.bodies.find((body) => body.id === SPIKE_PLANET)?.centreM;
    if (centre === undefined) {
      throw new Error("the scene has no planet");
    }
    expect(Math.hypot(centre.x, centre.y, centre.z)).toBeCloseTo(149_597_870_700, 0);
    expect(dot(normalise(centre), sunDirectionBody(PROFILE))).toBeCloseTo(-1, 12);
  });
});

describe("the predicted path", () => {
  it("runs from the script's time to its end, and is gone at the end", () => {
    const kept = spikeScene(PROFILE);
    const t = 1_100;
    const path = kept.sceneAt(t).craft[0]?.predictedPath ?? null;
    if (path === null) {
      throw new Error("the craft has no path to fly at 1,100 s");
    }
    expect(path[0]?.position).toEqual({
      kind: "body_fixed",
      body: SPIKE_PLANET,
      m: PROFILE.positionAt(t),
    });
    expect(path.at(-1)?.position).toEqual({
      kind: "body_fixed",
      body: SPIKE_PLANET,
      m: PROFILE.positionAt(PROFILE.durationS),
    });
    expect(kept.sceneAt(PROFILE.durationS).craft[0]?.predictedPath).toBeNull();
  });
});

describe("the camera's elevation", () => {
  it("is the nadir in the hover and 30° down in the low pass", () => {
    expect(cameraElevationDeg(PROFILE.poseAt(PROFILE.durationS))).toBeCloseTo(-90, 6);
    const span = PROFILE.segmentSpans().find((each) => each.name === "low fast pass");
    if (span === undefined) {
      throw new Error("the script has no low fast pass");
    }
    expect(cameraElevationDeg(PROFILE.poseAt((span.startS + span.endS) / 2))).toBeCloseTo(-30, 1);
  });
});

describe("the scene over TERRAIN TEST's moon (R10.T14.c)", () => {
  const world = terrainTestWorld();
  const golden = terrainTestGolden();
  // A site in the survey's middle rows, 28° N, 45° E (`terrainScene.ts`' track), approached from
  // the south-west.
  const site = {
    latitudeRad: (28 * Math.PI) / 180,
    longitudeRad: (45 * Math.PI) / 180,
    azimuthRad: (40 * Math.PI) / 180,
  };
  const profile = new DescentProfile(world.figure, site, { siteHeightM: 1_900 });
  const kept = spikeScene(profile, world);

  it("holds the star and the moon, with the golden's IDs, seed and the moon's figure", () => {
    const scene = kept.sceneAt(600);
    expect(scene.bodies.map((body) => body.id)).toEqual([terrainStar(), golden.body]);
    expect(scene.system).toBe(terrainSystem());
    const moon = scene.bodies[1];
    expect(moon?.kind).toBe("moon");
    expect(moon?.parent).toBe(terrainStar());
    expect(moon?.designation).toBe("TEST MOON");
    expect(moon?.detailSeed).toBe(golden.seed);
    expect(moon?.radiusM).toBe(golden.header.equatorial_radius_m);
    expect(moon?.hillRadiusM).toBe(world.hillRadiusM);
    expect(moon?.appearance).toBe(world.appearance);
    expect(moon?.rotation).toEqual(world.rotationAt(600));
    expect(scene.hostDiscs).toBe(world.hostDiscs);
  });

  it("holds the craft in TEST MOON's fixed frame, and its camera in the moon's frame", () => {
    const t = 1_000;
    const scene = kept.sceneAt(t);
    const craft = scene.craft.find((each) => each.id === SPIKE_CRAFT);
    expect(craft?.pose.position).toEqual({
      kind: "body_fixed",
      body: golden.body,
      m: profile.poseAt(t).positionM,
    });
    expect(craft?.predictedPath?.[0]?.position).toEqual({
      kind: "body_fixed",
      body: golden.body,
      m: profile.positionAt(t),
    });
    expect(scene.ownShip).toBe(SPIKE_CRAFT);
    const camera = spikeCameraAt(profile, t, world);
    expect(camera.frame).toEqual({ kind: "body", body: golden.body });
    expect(camera.positionM).toEqual(
      rotateToBody(world.rotationAt(t), profile.poseAt(t).positionM),
    );
    expect(kept.cameraAt(t)).toEqual(camera);
    const orbit = orbitInstrumentPose(profile, t, world);
    expect(orbit.frame).toEqual({ kind: "body", body: golden.body });
    expect(norm(orbit.positionM)).toBeCloseTo(
      ORBIT_INSTRUMENT_RADII * golden.header.equatorial_radius_m,
      3,
    );
  });

  it("stands the sun in the moon's equator over the site's meridian at the end", () => {
    const end = profile.durationS;
    const rotation = world.rotationAt(end);
    const pole = rotateToBody(rotation, { x: 0, y: 0, z: 1 });
    const siteBody = rotateToBody(rotation, profile.poseAt(end).groundPointM);
    const sun = sunDirectionBody(profile, world);
    expect(norm(sun)).toBeCloseTo(1, 14);
    expect(dot(sun, pole)).toBeCloseTo(0, 14);
    // In the plane of the pole and the site, on the site's side.
    expect(dot(sun, normalise(cross(pole, siteBody)))).toBeCloseTo(0, 14);
    expect(dot(sun, siteBody)).toBeGreaterThan(0);
    // So the site sees it 90° less its latitude above the horizon, about 62°.
    const g = profile.poseAt(end).groundDir;
    const up = normalise(
      rotateToBody(rotation, vecOf(spheroidNormal(world.figure, [g.x, g.y, g.z]))),
    );
    expect((Math.asin(dot(up, sun)) * 180) / Math.PI).toBeGreaterThan(61);
  });

  it("stands the test planet's sun at its site's x and y with z 0, bit for bit, as T13.b did", () => {
    const sun = sunDirectionBody(PROFILE);
    expect(sun).toStrictEqual(t13bSunDirection(PROFILE));
    expect(Object.is(sun.z, 0)).toBe(true);
  });

  it("builds the test planet's orbit pose about the pole +z, bit for bit, as T13.b did", () => {
    const times = [0, 600, 1_100, PROFILE.durationS];
    expect(times.map((t) => orbitInstrumentPose(PROFILE, t))).toStrictEqual(
      times.map((t) => t13bOrbitPose(PROFILE, t)),
    );
  });

  it("grounds the test planet's craft at T13.b's clearance, to the last bit", () => {
    // TEST HULL's √(4² + 2.5² + 10²) = 11.057 m plus 64 of WGS 84's largest vertex spacings at its
    // finest level, 19: 20.741 m. Computed once (2026-10-20) and pinned.
    expect(GROUNDED_CLEARANCE_M).toBe(31.797_310_506_748_033);
  });

  it("gives the test planet's scene no host disc and its planet no detail seed", () => {
    const scene = spikeScene(PROFILE).sceneAt(700);
    expect(["hostDiscs" in scene, scene.bodies.some((body) => "detailSeed" in body)]).toEqual([
      false,
      false,
    ]);
  });

  it("reads the elevation of view on the moon's spheroid, the nadir in the hover", () => {
    expect(cameraElevationDeg(profile.poseAt(profile.durationS), world.figure)).toBeCloseTo(-90, 6);
  });

  it("grounds the craft within its bounding radius and one of the moon's finest patches", () => {
    // TEST MOON's finest level is 17 (a = 1,738 km): one patch is 64 vertex spacings at most.
    const patchM = 64 * vertexSpacing(world.figure.equatorialRadiusM, 17).maxM;
    expect(groundedClearanceM(world.figure)).toBeCloseTo(SPIKE_CRAFT_RADIUS_M + patchM, 9);
    expect(groundedClearanceM(TEST_PLANET_FIGURE)).toBe(GROUNDED_CLEARANCE_M);
  });
});

describe("the synthetic field", () => {
  it("is the brainstorm's 15 MB in pieces of at most 1 MiB, every page written", () => {
    const blocks = syntheticFieldBlocks();
    expect(blocks.every((b) => b.byteLength <= MAX_BLOCK_BYTES)).toBe(true);
    const field = new Uint8Array(SYNTHETIC_FIELD_BYTES);
    let at = 0;
    for (const block of blocks) {
      field.set(new Uint8Array(block), at);
      at += block.byteLength;
    }
    expect(at).toBe(SYNTHETIC_FIELD_BYTES);
    expect(SYNTHETIC_FIELD_BYTES).toBe(15_000_000);
    for (let page = 0; page < field.length; page += 4096) {
      expect(field.subarray(page, page + 4096).some((b) => b !== 0)).toBe(true);
    }
  });

  it("reaches the workers as the test planet's stand-in field: one field_init, then its chunks", () => {
    const calls: string[] = [];
    let bytes = 0;
    postSyntheticField({
      fieldInit: (body, block, form) => {
        calls.push(`init ${body} ${String(form)}`);
        bytes += block.byteLength;
      },
      fieldChunk: (body, block) => {
        calls.push(`chunk ${body}`);
        bytes += block.byteLength;
      },
    });
    expect(calls[0]).toBe(`init ${SPIKE_PLANET} stand-in`);
    expect(calls.slice(1).every((c) => c === `chunk ${SPIKE_PLANET}`)).toBe(true);
    expect(calls).toHaveLength(Math.ceil(SYNTHETIC_FIELD_BYTES / MAX_BLOCK_BYTES));
    expect(bytes).toBe(SYNTHETIC_FIELD_BYTES);
  });
});
