/**
 * The descent spike's scene (plan R05, T13.b, Design notes 9, 14, 17 and 19): the test planet,
 * turning, the Sun-like star 1 au away, and the scripted craft flying T13.a's descent, as a
 * `ViewScene` built by hand after `view/scenes/frameChange.ts`, with the craft passed to selection
 * as a `GroundContact` once it is descending.
 *
 * @remarks
 * The planet stands still in the system over the script: its orbital motion over the 1,230 s
 * descent is about 0.014° (360° × 1,230 s ÷ 365.256363 d, the sidereal year), which the spike
 * neglects. The star lies in the planet's equatorial plane over the landing site's meridian at
 * touchdown, an equinox at local noon, so that the site is lit at every latitude the seed can
 * draw: T13.a draws the parametric latitude within ±60°, whose geodetic latitude on WGS 84 is at
 * most 60.083°, so the Sun stands 29.9° or more above the horizon. The script's day exposure
 * (Design note 17) holds at the end of the run, where the streaming load is highest.
 *
 * Altitudes are T13.a's: above the site's terrain, measured once before the run through the
 * surface query (`surfaceQuery.ts`), with the low pass above a true bound of the terrain under its
 * track. The craft is a contact while it is descending by Design note 9, judged on its clearance
 * above the site's terrain (the plan's "above the spheroid under it" reads the datum, which on a
 * planet with kilometres of relief says nothing of contact), and from the last time it starts to
 * descend to the end of the script: the hover's last metre is flown at zero vertical speed, which
 * Design note 9 alone would release just before touchdown. The level low pass between is not
 * descending and holds nothing.
 */

import { dot, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { CameraPose, Quaternion } from "../camera/pose";
import { lookAlong, multiply, quaternionFromRows, rotate } from "../camera/quaternion";
import type { ViewPosition } from "../coords/position";
import { type Rotation3, rotateToBody } from "../coords/rotation";
import { TEST_HULL } from "../scene/hull";
import { bodyKindSymbol, staticRetarded, type ViewScene } from "../scene/model";
import {
  AU_M,
  KEPT_BARYCENTRE,
  KEPT_SYSTEM,
  KEPT_TIDAL_RADIUS_M,
  type KeptScene,
  keptBody,
  keptTime,
} from "../scenes/kept";
import type { SunState } from "../atmosphere/hillaire";
import type { Xyz } from "../terrain/cube";
import {
  finestPatchSizeM,
  type GroundContact,
  heldRadiusM,
  isDescending,
} from "../terrain/grounded";
import { planetGeometry, spheroidNormal } from "../terrain/planet";
import { TEST_PLANET_FIGURE } from "./testPlanetFigure";
import type { DescentPose, DescentProfile } from "./descentProfile";
import { testPlanetRotationAt } from "./rotation";

/** The spike scene's name, as the label block's `SCENE` line shows it. */
export const SPIKE_SCENE_NAME = "DESCENT SPIKE";

/** The scene's star, at the kept system's barycentre. */
export const SPIKE_STAR = keptBody(0);

/** The test planet. */
export const SPIKE_PLANET = keptBody(1);

/** The scripted craft, the scene's own ship. */
export const SPIKE_CRAFT = "spike-craft";

/** The test planet's figure, WGS 84's (Design note 12): T13.a's, one definition for both. */
export { TEST_PLANET_FIGURE } from "./testPlanetFigure";

/**
 * The synthetic coarse field each height worker holds, bytes: the brainstorm's 15 MB, the top of
 * its 2–15 MB range ("The terrain's budget"). R09 Design note 17's layout gives about 11.6 MB for
 * an Earth at level 8 (393,216 cells × 21 B plus 98,304 groups of four × 34 B), at most about
 * 12.8 MB if its climate layer grows, so 15 MB stays an upper bound (R05's Risks, "The coarse
 * field's size").
 */
export const SYNTHETIC_FIELD_BYTES = 15_000_000;

/**
 * The synthetic field, every byte written so that its pages are committed and count in the
 * workers' memory as a real field would; the test planet never reads it.
 */
export function syntheticField(): ArrayBuffer {
  const bytes = new Uint8Array(SYNTHETIC_FIELD_BYTES);
  for (let i = 0; i < bytes.length; i += 1) {
    bytes[i] = (i * 151 + 7) & 0xff;
  }
  return bytes.buffer;
}

/**
 * The Sun's angular radius at 1 au, rad: asin(R⊙ ÷ 1 au) = 0.0046505 (959.23″), with the nominal
 * R⊙ = 6.957 × 10⁸ m (IAU 2015 Resolution B3) and 1 au = 149,597,870,700 m (IAU 2012 Resolution
 * B2).
 */
export const SPIKE_SUN_ANGULAR_RADIUS_RAD = 0.004_650_5;

/** The star's nominal radius, m (IAU 2015 Resolution B3). */
const SUN_RADIUS_M = 6.957e8;

/** The craft's bounding radius, m: the test hull's farthest vertex from its reference point. */
export const SPIKE_CRAFT_RADIUS_M = Math.max(
  ...TEST_HULL.vertices.map((v) => Math.hypot(v.x, v.y, v.z)),
);

/**
 * The clearance at or below which the craft is grounded whatever its vertical speed, metres: the
 * held radius r_g, its bounding radius plus one finest patch (Design note 9), within which its held
 * sphere reaches the ground, as T13.a's `craftContacts` has it.
 */
export const GROUNDED_CLEARANCE_M = heldRadiusM(
  { positionM: vec3(0, 0, 0), radiusM: SPIKE_CRAFT_RADIUS_M },
  finestPatchSizeM(planetGeometry(TEST_PLANET_FIGURE, null)),
);

/** How many points the scene's drawn path has, over the whole script. */
const PATH_SAMPLES = 256;

/** The step the contact's onset is searched with, s: the fixed-step mode's 64 Hz (Design note 19). */
const ONSET_STEP_S = 1 / 64;

/**
 * Whether the craft is grounded or descending at `tS` (Design note 9, on its height above the
 * floor under it, `heightAboveFloorM`): descending by `isDescending`, or within {@link GROUNDED_CLEARANCE_M} of the ground, where
 * a hover at zero vertical speed is a grounded body.
 */
function descendingAt(profile: DescentProfile, tS: number): boolean {
  const pose = profile.poseAt(tS);
  // The height above the floor under the craft, as T13.a's demand record judges it: the true
  // height above the terrain, which over the site is the clearance.
  const h = pose.heightAboveFloorM;
  return isDescending(h, pose.verticalSpeedMps) || h <= GROUNDED_CLEARANCE_M;
}

/** When the scripted craft is a contact (Design note 9, held to the end once it last descends). */
export interface ContactRule {
  /** The first time it is descending, s, or `null` if it never is. */
  readonly firstOnsetS: number | null;
  /** The last time it starts to descend, s, from which it stays a contact; `null` if never. */
  readonly holdFromS: number | null;
}

/** The time in (`low`, `high`] at which `descendingAt` turns from false to true, by bisection. */
function risingEdgeS(profile: DescentProfile, low: number, high: number): number {
  let lo = low;
  let hi = high;
  for (let k = 0; k < 64; k += 1) {
    const mid = lo + (hi - lo) / 2;
    if (mid === lo || mid === hi) {
      break;
    }
    if (descendingAt(profile, mid)) {
      hi = mid;
    } else {
      lo = mid;
    }
  }
  return hi;
}

/**
 * The craft's contact rule: the times it starts to descend, found on the 64 Hz fixed step, each
 * then by bisection to the double's resolution.
 */
export function contactRule(profile: DescentProfile): ContactRule {
  let firstOnsetS: number | null = null;
  let holdFromS: number | null = null;
  let before = 0;
  let was = descendingAt(profile, 0);
  if (was) {
    firstOnsetS = 0;
    holdFromS = 0;
  }
  const steps = Math.ceil(profile.durationS / ONSET_STEP_S);
  for (let n = 1; n <= steps; n += 1) {
    const t = Math.min(n * ONSET_STEP_S, profile.durationS);
    const now = descendingAt(profile, t);
    if (now && !was) {
      const edge = risingEdgeS(profile, before, t);
      firstOnsetS ??= edge;
      holdFromS = edge;
    }
    was = now;
    before = t;
  }
  return { firstOnsetS, holdFromS };
}

/**
 * The craft as selection's contact at `tS`, or `null`: while it is descending, and from
 * `rule.holdFromS` to the end, a sphere of its bounding radius at the point of the site's terrain
 * beneath it (T11.c: the hold is three-dimensional).
 *
 * @remarks
 * The point is at the site's height, so a contact far from the site, where the terrain under the
 * craft stands higher or lower, holds no patch: the forced region works where the craft comes
 * down, at the site.
 */
export function spikeContactAt(
  profile: DescentProfile,
  rule: ContactRule,
  tS: number,
): GroundContact | null {
  const held = rule.holdFromS !== null && tS >= rule.holdFromS;
  if (!held && !descendingAt(profile, tS)) {
    return null;
  }
  return { positionM: profile.poseAt(tS).groundPointM, radiusM: SPIKE_CRAFT_RADIUS_M };
}

/**
 * The unit direction to the star in the planet's non-rotating axes: in its equatorial plane, over
 * the landing site's meridian at the end of the script.
 */
export function sunDirectionBody(profile: DescentProfile): Vec3 {
  const end = profile.durationS;
  const site = rotateToBody(testPlanetRotationAt(end), profile.poseAt(end).groundPointM);
  return normalise(vec3(site.x, site.y, 0));
}

/** The star seen from the planet, for the atmosphere (`HillaireAtmosphere.drawFrame`). */
export function spikeSunState(sunBody: Vec3, rotation: Rotation3): SunState {
  const [r0, r1, r2] = rotation.rows;
  // Rᵀ · s: the body's axes into its body-fixed ones.
  return {
    directionBodyFixed: vec3(
      r0.x * sunBody.x + r1.x * sunBody.y + r2.x * sunBody.z,
      r0.y * sunBody.x + r1.y * sunBody.y + r2.y * sunBody.z,
      r0.z * sunBody.x + r1.z * sunBody.y + r2.z * sunBody.z,
    ),
    distanceAu: 1,
    angularRadiusRad: SPIKE_SUN_ANGULAR_RADIUS_RAD,
  };
}

/** The rotation `R` as a quaternion, body-fixed axes into the body's. */
function rotationQuaternion(rotation: Rotation3): Quaternion {
  return quaternionFromRows(rotation.rows);
}

/** The scripted camera at `tS` in the planet's non-rotating frame: the craft's own pose. */
export function spikeCameraAt(profile: DescentProfile, tS: number): CameraPose {
  const rotation = testPlanetRotationAt(tS);
  const pose = profile.poseAt(tS);
  return {
    frame: { kind: "body", body: SPIKE_PLANET },
    positionM: rotateToBody(rotation, pose.positionM),
    orientation: multiply(rotationQuaternion(rotation), pose.orientation),
  };
}

/** The landing site's direction for the surface query: the datum point beneath the script's end. */
export function siteDirection(profile: DescentProfile): Xyz {
  const d = profile.siteDir;
  return [d.x, d.y, d.z];
}

/**
 * The craft's path still to fly, from `tS` to the end, body-fixed, drawn dashed as its predicted
 * path: the flown part is no prediction, and at the end there is none (decision-r05-spike-ux.md).
 */
function remainingPath(profile: DescentProfile, tS: number): ReadonlyArray<ViewPosition> | null {
  if (tS >= profile.durationS) {
    return null;
  }
  const path: ViewPosition[] = [];
  for (let n = 0; n < PATH_SAMPLES; n += 1) {
    const t = tS + (n / (PATH_SAMPLES - 1)) * (profile.durationS - tS);
    path.push({ kind: "body_fixed", body: SPIKE_PLANET, m: profile.positionAt(t) });
  }
  return path;
}

/**
 * The descent spike as a kept scene: the star, the turning test planet and the scripted craft,
 * the own ship, with its path still to fly as its predicted path; the camera is the craft's own
 * pose.
 */
export function spikeScene(profile: DescentProfile): KeptScene {
  const sunBody = sunDirectionBody(profile);
  const planetCentreM = scale(sunBody, -AU_M);
  return {
    name: SPIKE_SCENE_NAME,
    durationS: profile.durationS,
    cameraAt: (tS) => spikeCameraAt(profile, tS),
    sceneAt: (tS): ViewScene => {
      const rotation = testPlanetRotationAt(tS);
      const pose = profile.poseAt(tS);
      const camera = spikeCameraAt(profile, tS);
      const velocity = rotateToBody(rotation, pose.velocityMps);
      return {
        provenance: { kind: "kept", name: SPIKE_SCENE_NAME },
        time: keptTime(tS),
        timeRate: 1,
        system: KEPT_SYSTEM,
        barycentre: KEPT_BARYCENTRE,
        tidalRadiusM: KEPT_TIDAL_RADIUS_M,
        bodies: [
          {
            id: SPIKE_STAR,
            parent: null,
            kind: "star",
            designation: "TEST STAR",
            radiusM: SUN_RADIUS_M,
            hillRadiusM: null,
            centreM: vec3(0, 0, 0),
            retarded: staticRetarded(vec3(0, 0, 0)),
            rotation: null,
            symbol: bodyKindSymbol("star"),
            orbitNormal: null,
          },
          {
            id: SPIKE_PLANET,
            parent: SPIKE_STAR,
            kind: "planet",
            designation: "TEST PLANET",
            radiusM: TEST_PLANET_FIGURE.equatorialRadiusM,
            // Earth's Hill radius, 1.5 × 10⁹ m (R02.T4's figure, as `frameChange.ts` takes it).
            hillRadiusM: 1.5e9,
            centreM: planetCentreM,
            retarded: staticRetarded(planetCentreM),
            rotation,
            symbol: bodyKindSymbol("planet"),
            orbitNormal: null,
          },
        ],
        rings: [],
        orbits: [],
        craft: [
          {
            id: SPIKE_CRAFT,
            designation: "TEST CRAFT",
            hull: TEST_HULL,
            pose: {
              position: { kind: "body_fixed", body: SPIKE_PLANET, m: pose.positionM },
              attitude: camera.orientation,
            },
            predictedPath:
              remainingPath(profile, tS)?.map((position) => ({
                position,
                attitude: camera.orientation,
              })) ?? null,
            velocityMPerS: velocity,
          },
        ],
        stars: [],
        ownShip: SPIKE_CRAFT,
        defaultPose: camera,
      };
    },
  };
}

/**
 * How many planetary radii out the orbit instrument stands: the disc's angular radius, asin(1 ÷ 7)
 * = 8.2°, then fits the vertical field of a 60° view up to an aspect of about 3.5.
 */
export const ORBIT_INSTRUMENT_RADII = 7;

/**
 * The orbit instrument's camera at `tS`: {@link ORBIT_INSTRUMENT_RADII} planetary radii out over the craft, looking at the
 * planet's centre with the pole up (or the star's direction where the craft is over a pole), so
 * that the graticule, the craft and its path show together.
 */
export function orbitInstrumentPose(profile: DescentProfile, tS: number): CameraPose {
  const craft = spikeCameraAt(profile, tS).positionM;
  const out = normalise(craft);
  const pole = vec3(0, 0, 1);
  const up = Math.abs(dot(out, pole)) > 0.99 ? sunDirectionBody(profile) : pole;
  const positionM = scale(out, ORBIT_INSTRUMENT_RADII * TEST_PLANET_FIGURE.equatorialRadiusM);
  const forward = scale(out, -1);
  return {
    frame: { kind: "body", body: SPIKE_PLANET },
    positionM,
    orientation: lookAlong(forward, normalise(sub(up, scale(forward, dot(up, forward))))),
  };
}

/**
 * The scripted camera's elevation of view at `pose`, degrees: the angle of its forward axis above
 * the local horizontal, the spheroid's tangent plane under it, `-90` at the nadir.
 */
export function cameraElevationDeg(pose: DescentPose): number {
  const g = pose.groundDir;
  const [nx, ny, nz] = spheroidNormal(TEST_PLANET_FIGURE, [g.x, g.y, g.z]);
  const forward = rotate(pose.orientation, vec3(0, 0, -1));
  const sine = forward.x * nx + forward.y * ny + forward.z * nz;
  return (Math.asin(Math.min(1, Math.max(-1, sine))) * 180) / Math.PI;
}
