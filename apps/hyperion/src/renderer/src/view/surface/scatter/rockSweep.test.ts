/**
 * The swept region, into R05's selection (plan R11, T4.d): `couldTouchContacts`' chain covers what
 * a grounded or descending body could touch over the horizon, an unpowered arc stays inside its
 * straight line's widened sweep, a planned path is swept unwidened, and a scripted fast descent in
 * R05's harness never reaches a rock before its patch is drawn.
 */

import { describe, expect, it } from "vitest";

import { add, cross, norm, normalise, scale, sub, type Vec3, vec3 } from "../../../geometry/vec3";
import { goldenLevelTable } from "../../../test/terrainFixtures";
import { xyzToFaceUv, uvToSt, vertexDir } from "../../terrain/cube";
import {
  datumGroundUnder,
  FORCED_REGION_RESIDENCY_S,
  finestPatchSizeM,
  forcedRadiusM,
  type GroundContact,
  isDescending,
} from "../../terrain/grounded";
import { type PatchKey, patchKeyString } from "../../terrain/patchKey";
import { type BodyFigure, planetGeometry, surfacePoint } from "../../terrain/planet";
import { RECORD_SEED, SETTING_VIEWS } from "../../spike/demandRecord";
import {
  datumDirection,
  type DescentPose,
  DescentProfile,
  type DescentSegment,
  landingSiteOf,
} from "../../spike/descentProfile";
import { craftContacts, runFixedStep } from "../../spike/fixedStep";
import { SPIKE_CRAFT_RADIUS_M } from "../../spike/spikeScene";
import { TEST_PLANET_FIGURE } from "../../spike/testPlanetFigure";
import {
  couldTouch,
  couldTouchContacts,
  extrapolationMargin,
  LARGEST_ROCK_REACH_M,
  type SweptBody,
  type SweptSurface,
  sweptWideningM,
} from "./rockSelection";

const PLANET = planetGeometry(TEST_PLANET_FIGURE, goldenLevelTable("off"));
const PATCH_M = finestPatchSizeM(PLANET);
const EARTH_SURFACE: SweptSurface = {
  figure: TEST_PLANET_FIGURE,
  ground: datumGroundUnder(TEST_PLANET_FIGURE),
  patchSizeM: PATCH_M,
};

/** WGS 84's GM, m³/s² (NIMA TR8350.2, 3rd ed., 2000, Table 3.1). */
const EARTH_GM_M3_S2 = 3.986_004_418e14;

/** The Moon's GM, m³/s² (Park et al. 2021, _AJ_ 161, 105, DE440: 4,902.800118 km³/s²). */
const MOON_GM_M3_S2 = 4.902_800_118e12;

/** The Moon as a sphere of its mean radius, 1,737.4 km (Archinal et al. 2018, _CMDA_ 130, 22). */
const MOON: BodyFigure = { equatorialRadiusM: 1_737_400, polarRadiusM: 1_737_400, pole: null };

/** WGS 84's rotation rate, rad/s (NIMA TR8350.2, 3rd ed., 2000, Table 3.1). */
const EARTH_SPIN_RAD_S = 7.292_115e-5;

/** A finest patch of the test planet, inside face 2, which the sweeps start over. */
const SITE: PatchKey = { face: 2, level: PLANET.finestLevel, i: 200_011, j: 300_007 };

/** The datum point of `key`'s vertex (`x`, `y`) at `heightM`, on `figure`. */
function pointOver(figure: BodyFigure, key: PatchKey, heightM: number): Vec3 {
  const [x, y, z] = surfacePoint(figure, vertexDir(key, 32, 32), heightM);
  return vec3(x, y, z);
}

/** The east, north and up unit vectors at the body-fixed point `p`. */
function localFrame(p: Vec3): { readonly east: Vec3; readonly north: Vec3; readonly up: Vec3 } {
  const up = normalise(p);
  const east = normalise(vec3(-up.y, up.x, 0));
  return { east, north: cross(up, east), up };
}

/** Whether `point` lies within some contact's forced radius. */
function forced(point: Vec3, contacts: ReadonlyArray<GroundContact>): boolean {
  return contacts.some((c) => norm(sub(point, c.positionM)) <= forcedRadiusM(c, PATCH_M));
}

/**
 * The points of the could-touch disc of a body of `radiusM` widened by `wideningM` about the ground
 * point `groundM`: its centre and 24 points of its rim, in the ground's tangent plane.
 */
function discPoints(groundM: Vec3, radiusM: number, wideningM: number): Vec3[] {
  const { east, north } = localFrame(groundM);
  const reach = couldTouch({ groundPointM: groundM, radiusM: radiusM + wideningM }).radiusM;
  return [
    groundM,
    ...Array.from({ length: 24 }, (_, k) => {
      const angle = (2 * Math.PI * k) / 24;
      return add(
        groundM,
        add(scale(east, reach * Math.cos(angle)), scale(north, reach * Math.sin(angle))),
      );
    }),
  ];
}

/**
 * A descent at `speedMps`, 15° below the horizontal, heading east, from half the height it would
 * lose over the horizon, at most 500 m, above the site: its line meets the ground within the sweep.
 */
function descent(speedMps: number, gravityMps2: number | null): SweptBody {
  const glide = (15 * Math.PI) / 180;
  const heightM = Math.min(500, 0.5 * speedMps * Math.sin(glide) * FORCED_REGION_RESIDENCY_S);
  const positionM = pointOver(TEST_PLANET_FIGURE, SITE, heightM);
  const { east, up } = localFrame(positionM);
  const velocityMps = scale(
    add(scale(east, Math.cos(glide)), scale(up, -Math.sin(glide))),
    speedMps,
  );
  return { radiusM: 20, path: { kind: "extrapolated", positionM, velocityMps, gravityMps2 } };
}

/**
 * The could-touch points the chain of `body` must force, sampled every 0.05 s of the horizon
 * where the body could be within its footprint and the reach of the ground: its line no higher
 * than those and the fall its gravity allows, and not through the ground.
 */
function sweptPoints(body: SweptBody, surface: SweptSurface): Vec3[] {
  const { path } = body;
  if (path.kind !== "extrapolated") {
    throw new Error("an extrapolated path");
  }
  const points: Vec3[] = [];
  const reachM = body.radiusM + LARGEST_ROCK_REACH_M;
  for (let k = 0; k <= 600; k += 1) {
    const tS = (FORCED_REGION_RESIDENCY_S * k) / 600;
    const move = scale(path.velocityMps, tS);
    const g = path.gravityMps2;
    const under = surface.ground(add(path.positionM, move));
    const fallM = g === null ? 0 : extrapolationMargin(g, tS);
    if (under.heightM - fallM > reachM || under.heightM < -reachM) {
      continue;
    }
    const widening = g === null ? 0 : sweptWideningM(surface.figure, g, tS, norm(move));
    points.push(...discPoints(under.groundPointM, body.radiusM, widening));
  }
  return points;
}

/** The test planet's surface gravity at its datum's least radius, m/s². */
const EARTH_G_MPS2 = EARTH_GM_M3_S2 / TEST_PLANET_FIGURE.polarRadiusM ** 2;

describe("the swept region (R11.T4.d)", () => {
  it("widens an extrapolation by ½ g t², of which the ground beneath takes its turn's share", () => {
    expect(extrapolationMargin(9.80665, 30)).toBeCloseTo(4_412.99, 2);
    expect(sweptWideningM(MOON, 1.62, 30, 0)).toBeCloseTo((729 * 729) / 1_737_400, 6);
    // Never more than the whole margin.
    expect(sweptWideningM(MOON, 1.62, 3_000, 1e9)).toBe(extrapolationMargin(1.62, 3_000));
    // On a spheroid, the normal's turn from the radial.
    expect(sweptWideningM(TEST_PLANET_FIGURE, 9.8, 1, 0)).toBeGreaterThan(4.9 * 2 * (1 / 298.26));
    // In a turning frame, its Coriolis drift ω t² (v₀ + g t ÷ 3) and centrifugal ½ ω² r t².
    const turning = sweptWideningM(TEST_PLANET_FIGURE, 9.8, 30, 0, EARTH_SPIN_RAD_S, 300);
    const still = sweptWideningM(TEST_PLANET_FIGURE, 9.8, 30, 0);
    expect(turning - still).toBeCloseTo(
      EARTH_SPIN_RAD_S * 900 * (300 + 98) + 0.5 * EARTH_SPIN_RAD_S ** 2 * 6_378_137 * 900,
      6,
    );
  });

  it("forces every could-touch region of a grounded body by its own contact", () => {
    const groundM = pointOver(TEST_PLANET_FIGURE, SITE, 0);
    const own: GroundContact = { positionM: groundM, radiusM: 20 };
    expect(discPoints(groundM, 20, 0).every((p) => forced(p, [own]))).toBe(true);
  });

  it.each([2, 20, 300])(
    "forces every could-touch region over the horizon of a descent at %d m/s",
    (speedMps) => {
      for (const gravity of [null, EARTH_G_MPS2]) {
        const body = descent(speedMps, gravity);
        const contacts = couldTouchContacts([body], FORCED_REGION_RESIDENCY_S, EARTH_SURFACE);
        const points = sweptPoints(body, EARTH_SURFACE);
        expect(points.length).toBeGreaterThan(100);
        expect(points.filter((p) => !forced(p, contacts))).toEqual([]);
        // Each contact stands on the ground beneath the path.
        for (const contact of contacts) {
          expect(Math.abs(EARTH_SURFACE.ground(contact.positionM).heightM)).toBeLessThan(1e-6);
        }
        // Few enough for each selection to loop over.
        expect(contacts.length).toBeLessThan(40);
      }
    },
  );

  it.each([
    ["a Moon", MOON, MOON_GM_M3_S2, 0],
    ["an Earth", TEST_PLANET_FIGURE, EARTH_GM_M3_S2, 0],
    ["an Earth in its turning frame", TEST_PLANET_FIGURE, EARTH_GM_M3_S2, EARTH_SPIN_RAD_S],
  ])(
    "keeps the ground beneath an unpowered arc over %s within its straight line's widened sweep",
    (_, figure, gm, spin) => {
      const ground = datumGroundUnder(figure);
      const gravity = gm / Math.min(figure.equatorialRadiusM, figure.polarRadiusM) ** 2;
      const site: PatchKey = { face: 2, level: 16, i: 20_011, j: 30_007 };
      const omega = vec3(0, 0, spin);
      // The body's central pull and, in its turning frame, the Coriolis and centrifugal ones.
      const pull = (p: Vec3, v: Vec3): Vec3 =>
        sub(
          sub(scale(p, -gm / norm(p) ** 3), scale(cross(omega, v), 2)),
          cross(omega, cross(omega, p)),
        );
      for (const [heightM, alongMps, downMps] of [
        [300, 300, 10],
        [1_000, 20, 2],
        [50, 2, 0],
      ] as const) {
        const p0 = pointOver(figure, site, heightM);
        const { east, up } = localFrame(p0);
        const v0 = add(scale(east, alongMps), scale(up, -downMps));
        let p = p0;
        let v = v0;
        const dtS = 0.01;
        let steps = 0;
        let departedM = 0;
        // The fourth-order Runge–Kutta step of the arc.
        for (let n = 1; n <= FORCED_REGION_RESIDENCY_S / dtS; n += 1) {
          const k1v = pull(p, v);
          const k1p = v;
          const k2p = add(v, scale(k1v, dtS / 2));
          const k2v = pull(add(p, scale(k1p, dtS / 2)), k2p);
          const k3p = add(v, scale(k2v, dtS / 2));
          const k3v = pull(add(p, scale(k2p, dtS / 2)), k3p);
          const k4p = add(v, scale(k3v, dtS));
          const k4v = pull(add(p, scale(k3p, dtS)), k4p);
          p = add(p, scale(add(add(k1p, scale(k2p, 2)), add(scale(k3p, 2), k4p)), dtS / 6));
          v = add(v, scale(add(add(k1v, scale(k2v, 2)), add(scale(k3v, 2), k4v)), dtS / 6));
          const tS = n * dtS;
          const under = ground(p);
          if (under.heightM < 0) {
            break;
          }
          const line = add(p0, scale(v0, tS));
          const apartM = norm(sub(under.groundPointM, ground(line).groundPointM));
          const widening = sweptWideningM(figure, gravity, tS, norm(sub(line, p0)), spin, norm(v0));
          // Within a nanometre's rounding of the two feet.
          expect(apartM).toBeLessThanOrEqual(widening + 1e-6);
          departedM = Math.max(departedM, norm(sub(p, line)));
          steps += 1;
        }
        expect(steps).toBeGreaterThan(100);
        // The arc does fall away from its line, and the sweep's widening is a sliver of that.
        expect(departedM).toBeGreaterThan(5);
      }
    },
  );

  it("leaves out the stretches of a path too high above the ground to touch it", () => {
    const high = descent(300, null);
    const { path } = high;
    if (path.kind !== "extrapolated") {
      throw new Error("an extrapolated path");
    }
    const level = { ...path, velocityMps: scale(localFrame(path.positionM).east, 300) };
    expect(couldTouchContacts([{ ...high, path: level }], 30, EARTH_SURFACE)).toEqual([]);
    // The line's own stretch near the ground, and with the fall its gravity allows, an earlier one.
    const unwidened = couldTouchContacts([high], 30, EARTH_SURFACE);
    const widened = couldTouchContacts([descent(300, EARTH_G_MPS2)], 30, EARTH_SURFACE);
    expect(unwidened.length).toBeGreaterThan(0);
    expect(widened.length).toBeGreaterThan(unwidened.length);
  });

  it("stops the chain where its forced patches would pass the cap", () => {
    const body = descent(300, EARTH_G_MPS2);
    const whole = couldTouchContacts([body], 30, EARTH_SURFACE);
    const capped = couldTouchContacts([body], 30, { ...EARTH_SURFACE, maxForcedPatches: 60 });
    expect(capped.length).toBeLessThan(whole.length);
    expect(capped).toEqual(whole.slice(0, capped.length));
  });

  it("sweeps a planned path whole, through its poses, without widening", () => {
    const start = pointOver(TEST_PLANET_FIGURE, SITE, 20);
    const { east, north, up } = localFrame(start);
    const positionsM = [
      start,
      add(start, add(scale(east, 400), scale(up, -5))),
      add(start, add(scale(east, 600), add(scale(north, 300), scale(up, -15)))),
      add(start, add(scale(east, 650), add(scale(north, 320), scale(up, -18)))),
    ];
    const contacts = couldTouchContacts(
      [{ radiusM: 20, path: { kind: "planned", positionsM } }],
      FORCED_REGION_RESIDENCY_S,
      EARTH_SURFACE,
    );
    expect(contacts.map(({ radiusM }) => radiusM)).toEqual(contacts.map(() => 20));
    for (let n = 1; n < positionsM.length; n += 1) {
      const from = positionsM[n - 1] ?? start;
      const to = positionsM[n] ?? start;
      for (let k = 0; k <= 50; k += 1) {
        const groundM = EARTH_SURFACE.ground(add(from, scale(sub(to, from), k / 50))).groundPointM;
        expect(discPoints(groundM, 20, 0).every((q) => forced(q, contacts))).toBe(true);
      }
    }
  });
});

/** The finest patch of the test planet holding the unit direction `dir`. */
function finestHolding(dir: Vec3): PatchKey {
  const { face, u, v } = xyzToFaceUv([dir.x, dir.y, dir.z]);
  const side = 2 ** PLANET.finestLevel;
  const index = (w: number): number =>
    Math.min(side - 1, Math.max(0, Math.floor(uvToSt(w) * side)));
  return { face, level: PLANET.finestLevel, i: index(u), j: index(v) };
}

/** The finest patches under the could-touch disc of the craft at `pose`, sampled a quarter patch apart. */
function patchesUnder(pose: DescentPose): Set<string> {
  const reach = couldTouch({
    groundPointM: pose.groundPointM,
    radiusM: SPIKE_CRAFT_RADIUS_M,
  }).radiusM;
  const { east, north } = localFrame(pose.groundPointM);
  const keys = new Set<string>();
  const step = PATCH_M / 4;
  for (let x = -reach; x <= reach; x += step) {
    for (let y = -reach; y <= reach; y += step) {
      if (Math.hypot(x, y) <= reach) {
        const p = add(pose.groundPointM, add(scale(east, x), scale(north, y)));
        keys.add(patchKeyString(finestHolding(datumDirection(TEST_PLANET_FIGURE, p))));
      }
    }
  }
  return keys;
}

/**
 * A fast descent: 15 s from 600 m to 20 m at 60 m/s over the ground, falling at about 39 m/s, a
 * 10 s flare to 3 m, slowing to 20 m/s, inside the rocks' reach, and 10 s to a stop.
 */
const FAST_DESCENT: ReadonlyArray<DescentSegment> = [
  {
    name: "fast descent",
    durationS: 15,
    startAltitudeM: 600,
    endAltitudeM: 20,
    startSpeedMps: 60,
    endSpeedMps: 60,
    verticalShape: "constant",
    clearsTrack: false,
    stretches: null,
  },
  {
    name: "flare",
    durationS: 10,
    startAltitudeM: 20,
    endAltitudeM: 3,
    startSpeedMps: 60,
    endSpeedMps: 20,
    verticalShape: "constant",
    clearsTrack: false,
    stretches: null,
  },
  {
    name: "stop",
    durationS: 10,
    startAltitudeM: 3,
    endAltitudeM: 3,
    startSpeedMps: 20,
    endSpeedMps: 0,
    verticalShape: "level",
    clearsTrack: false,
    stretches: null,
  },
];

/**
 * The frames of the fast descent where the craft could touch a rock and some finest patch under
 * it is not drawn, with the sweep's chain or the craft's own contact alone.
 *
 * @remarks
 * Each forced request becomes resident {@link FORCED_REGION_RESIDENCY_S} ÷ (finest level + 1)
 * after its first request, the views' own at once: a forced region from the roots down is resident
 * within the residency time, its levels requested in turn, as the harness's demand has it.
 */
function undrawnFrames(sweep: boolean): number {
  const low = SETTING_VIEWS[1];
  if (low === undefined) {
    throw new Error("the low setting's view");
  }
  const profile = new DescentProfile(
    TEST_PLANET_FIGURE,
    landingSiteOf(RECORD_SEED),
    {},
    FAST_DESCENT,
  );
  let undrawn = 0;
  let near = 0;
  runFixedStep({
    profile,
    planet: PLANET,
    setting: low.setting,
    view: low.view,
    rateHz: 8,
    fromS: 0,
    toS: profile.durationS,
    measureFromS: 0,
    maxPatches: low.maxPatches,
    layout: low.layout,
    rangeOf: () => [-1, 1],
    nowMs: () => 0,
    forcedLatencyS: FORCED_REGION_RESIDENCY_S / (PLANET.finestLevel + 1),
    contacts: (pose, patchSizeM) => {
      const own = craftContacts(pose, patchSizeM);
      if (!sweep || !isDescending(pose.heightAboveFloorM, pose.verticalSpeedMps)) {
        return own;
      }
      const path = {
        kind: "extrapolated",
        positionM: pose.positionM,
        velocityMps: pose.velocityMps,
        gravityMps2: null,
      } as const;
      return [
        ...own,
        ...couldTouchContacts(
          [{ radiusM: SPIKE_CRAFT_RADIUS_M, path }],
          FORCED_REGION_RESIDENCY_S,
          {
            ...EARTH_SURFACE,
            patchSizeM,
            maxForcedPatches: Math.floor(low.maxPatches / 2),
          },
        ),
      ];
    },
    onFrame: (pose, _, resident) => {
      if (pose.heightAboveFloorM > SPIKE_CRAFT_RADIUS_M + LARGEST_ROCK_REACH_M) {
        return;
      }
      near += 1;
      if ([...patchesUnder(pose)].some((key) => !resident(key))) {
        undrawn += 1;
      }
    },
  });
  expect(near).toBeGreaterThan(40);
  return undrawn;
}

describe("a scripted fast descent (R11.T4.d)", () => {
  it("never reaches a rock before its patch is drawn, with the sweep", { timeout: 300_000 }, () => {
    expect(undrawnFrames(true)).toBe(0);
  });

  it("would, with the craft's own contact alone: the control", { timeout: 300_000 }, () => {
    expect(undrawnFrames(false)).toBeGreaterThan(0);
  });
});
