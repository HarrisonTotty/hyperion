import {
  type BodyIdHex,
  type BodySummaryDto,
  formatBodyId,
  type SystemBodiesDto,
  type UniverseTime,
} from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { runPose, startRun } from "../../displays/view/viewRun";
import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import {
  placedTrack,
  type SceneFrame,
  sceneAt,
  shipObserver,
  systemPlacements,
  type SystemTrack,
} from "../../lib/scene/apparent";
import {
  lightTime,
  type Span,
  SPEED_OF_LIGHT_M_PER_S,
  timeBefore,
} from "../../lib/scene/lightTime";
import { aHostDisc } from "../../test/litFixtures";
import {
  FIXTURE_JUPITER,
  FIXTURE_SYSTEM,
  populatedBodies,
  sliceBodies,
} from "../../test/planetaryFixture";
import {
  sceneModelOf,
  shipFrameOf,
  systemSceneState,
  viewBodyOf,
  viewSceneOf,
} from "../../test/sceneFixture";
import type { CameraPose } from "../camera/pose";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { relativeToCamera } from "../coords/relative";
import { type RetardedCentre, sceneOrigins, type ViewBody, type ViewScene } from "../scene/model";
import type { KeptScene } from "../scenes/kept";
import { frameChangeScene } from "../scenes/frameChange";
import { phaseScene } from "../scenes/phaseScene";
import { precisionScene } from "../scenes/precision";
import { DescentProfile, landingSiteOf } from "../spike/descentProfile";
import { spikeScene, TEST_PLANET_FIGURE } from "../spike/spikeScene";
import { hostLights, placeLights } from "./hostLights";
import {
  type LightingFrame,
  lightingFrameOf,
  lightingFramesOf,
  RETARDATION_STEPS,
  retardedFrom,
} from "./retarded";

/** The fixtures' Sun-like star, body index 0. */
const STAR = formatBodyId({ system: FIXTURE_SYSTEM, bodyIndex: 0 });

/** The populated answer's Earth at 1 au and its Moon, 384,400 km out. */
const EARTH = `${FIXTURE_SYSTEM}.0100`;
const MOON = `${FIXTURE_SYSTEM}.0101`;

/** An Io added about the slice's Jupiter. */
const IO = `${FIXTURE_SYSTEM}.0501`;

const DISCS = [aHostDisc({ star: 0 })];

/**
 * The fixture's own μ, m³/s²: the Earth's orbit's, G(M☉ + M⊕), and the Moon's, G(M⊕ + M☾)
 * (`mu_m3_s2`).
 */
const GM_SUN_M3_S2 = 1.327_128_386_004e20;
const GM_EARTH_M3_S2 = 4.035_032e14;

/** Jupiter's GM, m³/s²: IAU 2015 Resolution B3's nominal (GM)ᴺ_J (Prša et al. 2016, AJ 152, 41). */
const GM_JUPITER_M3_S2 = 1.266_865_3e17;

/** Io's orbit (NASA GSFC, Jovian Satellite Fact Sheet): a 421.8 × 10³ km, P 1.769138 d, e 0.004. */
const IO_ORBIT_M = 4.218e8;
const IO_PERIOD_S = 1.769_138 * 86_400;
const IO_ECCENTRICITY = 0.004;

/**
 * A time at which the fixture's Moon is in front of its Sun from the Earth: the shadow's axis
 * passes 7,166 km from the Earth's centre, and the penumbra touches the Earth (a partial eclipse).
 */
const T_ECLIPSE: UniverseTime = { seconds: 4_727_768, nanos: 0 };

/** The Earth–Moon bodies: the populated answer, which places only its Earth, Moon and dwarf. */
function earthMoonBodies(): SystemBodiesDto {
  const { kind: _kind, ...bodies } = populatedBodies();
  return bodies;
}

/**
 * The slice's Sun, Earth and Jupiter with an Io: the populated answer's Moon moved onto Io's
 * orbit about the slice's Jupiter (NASA GSFC, Jovian Satellite Fact Sheet: a 421.8 × 10³ km, P
 * 1.769138 d, e 0.004, radius 1,821.5 km; GM_J from IAU 2015 Resolution B3), in Jupiter's
 * orbital plane.
 */
function jupiterIoBodies(): SystemBodiesDto {
  const { kind: _kind, ...slice } = sliceBodies();
  const moon = earthMoonBodies().bodies.find((body) => body.id === MOON);
  if (moon === undefined || moon.orbit.state !== "ok" || moon.bulk.state !== "ok") {
    throw new Error("the populated answer has a Moon with an orbit and a bulk section");
  }
  const io: BodySummaryDto = {
    ...moon,
    id: IO,
    parent: { type: "body", id: FIXTURE_JUPITER },
    orbit: {
      ...moon.orbit,
      value: {
        ...moon.orbit.value,
        parent: { type: "body", id: FIXTURE_JUPITER },
        orbit: {
          ...moon.orbit.value.orbit,
          period_s: IO_PERIOD_S,
          semi_major_axis_m: IO_ORBIT_M,
          eccentricity: IO_ECCENTRICITY,
          // In Jupiter's orbital plane (the slice's 1.02 and 2.48 rad), within the 3.13° of
          // Jupiter's obliquity (NASA GSFC, Jupiter Fact Sheet) that Io keeps of it, so that its
          // shadow crosses Jupiter.
          inclination_rad: 1.02,
          ascending_node_rad: 2.48,
          mu_m3_s2: GM_JUPITER_M3_S2,
        },
      },
    },
    bulk: { ...moon.bulk, value: { ...moon.bulk.value, radius_m: 1.8215e6 } },
  };
  return { ...slice, bodies: [...slice.bodies, io] };
}

/** A body's track in `bodies`' system. */
function trackOf(bodies: SystemBodiesDto, id: string): SystemTrack {
  const model = sceneModelOf(systemSceneState(bodies, T_ECLIPSE, vec3(0, 0, 0), vec3(0, 0, 0)));
  if (model.system === null) {
    throw new Error("the test's scene has a system");
  }
  return placedTrack(systemPlacements(model.system), id);
}

function positionOf(track: SystemTrack, time: UniverseTime): Vec3 {
  const at = track.positionAt(time);
  if (at === null) {
    throw new Error("a placed body's track is never absent");
  }
  return at;
}

/** What one camera sees and lights: its frame, its scene and its pose. */
interface Seen {
  readonly frame: SceneFrame;
  readonly scene: ViewScene;
  readonly pose: CameraPose;
}

/**
 * The scene seen from a camera at the ship, at `shipM` moving at `velocityMPerS`, at `time`;
 * `previous`, the frame before, for the frame rule's hysteresis.
 */
function seenFrom(
  bodies: SystemBodiesDto,
  time: UniverseTime,
  shipM: Vec3,
  velocityMPerS: Vec3 = vec3(0, 0, 0),
  previous: SceneFrame | null = null,
): Seen {
  const model = sceneModelOf(systemSceneState(bodies, time, shipM, velocityMPerS));
  const observer = shipObserver(model, time);
  const frame =
    previous === null
      ? shipFrameOf(model, time)
      : observer === null
        ? null
        : sceneAt(model, observer, time, previous);
  if (frame === null) {
    throw new Error("the test's scene has a frame");
  }
  const pose: CameraPose = {
    frame: { kind: "system", system: FIXTURE_SYSTEM },
    positionM: shipM,
    orientation: IDENTITY_QUATERNION,
  };
  return { frame, scene: viewSceneOf(model, frame), pose };
}

function retardedOf(seen: Seen, id: string): RetardedCentre {
  const retarded = viewBodyOf(seen.scene, id).retarded;
  if (retarded === null) {
    throw new Error(`${id} is placed, with a retarded centre`);
  }
  return retarded;
}

/** A body's drawn centre from the camera, m. */
function drawnOf(seen: Seen, id: string): Vec3 {
  return relativeToCamera(
    { kind: "system", system: seen.scene.system, m: viewBodyOf(seen.scene, id).centreM },
    seen.pose,
    sceneOrigins(seen.scene),
  );
}

function frameOf(seen: Seen, lit: BodyIdHex): LightingFrame {
  const frame = lightingFrameOf(seen.scene, lit, seen.pose, DISCS);
  if (frame === null) {
    throw new Error(`${lit} is a lit body of the scene`);
  }
  return frame;
}

function occluderOf(frame: LightingFrame, id: string): Vec3 {
  const found = frame.occluders.find((each) => each.id === id);
  if (found === undefined) {
    throw new Error(`the frame holds no occluder ${id}`);
  }
  return found.centreM;
}

function starOf(frame: LightingFrame): Vec3 {
  const [light] = frame.lights;
  if (light === undefined) {
    throw new Error("the frame has a light");
  }
  return light.centreM;
}

/**
 * Where the shadow axis from `star` through `occluder` passes `body`: its offset from the body's
 * centre across the axis, m (the Besselian fundamental plane's x and y, in galactic axes).
 */
function shadowOffset(body: Vec3, star: Vec3, occluder: Vec3): Vec3 {
  const axis = normalise(sub(occluder, star));
  const fromBody = sub(occluder, body);
  return sub(fromBody, scale(axis, dot(fromBody, axis)));
}

/** The lit body's retarded time t_B, when the light the camera sees left it. */
function litTime(frame: SceneFrame, id: string): UniverseTime {
  const entry = frame.bodies.find((each) => each.id === id);
  if (entry === undefined || entry.kind !== "placed") {
    throw new Error(`the frame places ${id}`);
  }
  return entry.emitted;
}

/** `time` plus `span`. */
function after(time: UniverseTime, span: Span): UniverseTime {
  const nanos = time.nanos + span.nanos;
  const carry = Math.floor(nanos / 1e9);
  return { seconds: time.seconds + span.seconds + carry, nanos: nanos - carry * 1e9 };
}

/**
 * Where `track` was when the light reaching `receiverM` at `received` left it: the light time
 * solved to the nanosecond on the exact track.
 */
function exactlyRetarded(track: SystemTrack, receiverM: Vec3, received: UniverseTime): Vec3 {
  let at = positionOf(track, received);
  for (let k = 0; k < 20; k += 1) {
    const next = positionOf(track, timeBefore(received, lightTime(norm(sub(at, receiverM)))));
    if (norm(sub(next, at)) < 1e-6) {
      return next;
    }
    at = next;
  }
  throw new Error("the light time converges");
}

/** `retardedFrom(lit, source)`'s distance from the exact track at the lit body's t_B, m. */
function retardationErrorM(
  seen: Seen,
  bodies: SystemBodiesDto,
  lit: string,
  source: string,
): number {
  const litCentre = retardedOf(seen, lit);
  const exact = exactlyRetarded(
    trackOf(bodies, source),
    litCentre.centreM,
    litTime(seen.frame, lit),
  );
  return norm(sub(retardedFrom(litCentre, retardedOf(seen, source)), exact));
}

/** The extrapolation's span δ = τ_lit + d ÷ c − τ_s, s, and 2 d ÷ c, d between the centres. */
function spanOf(seen: Seen, lit: string, source: string): { deltaS: number; twiceS: number } {
  const litCentre = retardedOf(seen, lit);
  const sourceCentre = retardedOf(seen, source);
  const crossingS = norm(sub(sourceCentre.centreM, litCentre.centreM)) / SPEED_OF_LIGHT_M_PER_S;
  return {
    deltaS: litCentre.lightTimeS + crossingS - sourceCentre.lightTimeS,
    twiceS: 2 * crossingS,
  };
}

/** The Moon's shadow axis about the Earth at t_B from the exact tracks, m. */
function exactShadowOffset(t: UniverseTime): Vec3 {
  const bodies = earthMoonBodies();
  const earth = positionOf(trackOf(bodies, EARTH), t);
  return shadowOffset(
    earth,
    exactlyRetarded(trackOf(bodies, STAR), earth, t),
    exactlyRetarded(trackOf(bodies, MOON), earth, t),
  );
}

/** The Moon's shadow axis about the Earth from the Earth's lighting frame, m. */
function litShadowOffset(seen: Seen): Vec3 {
  const frame = frameOf(seen, EARTH);
  return shadowOffset(drawnOf(seen, EARTH), starOf(frame), occluderOf(frame, MOON));
}

/** The same from the drawn (apparent) centres, as T8.a placed lights and occluders. */
function drawnShadowOffset(seen: Seen): Vec3 {
  return shadowOffset(drawnOf(seen, EARTH), drawnOf(seen, STAR), drawnOf(seen, MOON));
}

const earthAt = (t: UniverseTime): Vec3 => positionOf(trackOf(earthMoonBodies(), EARTH), t);
const moonAt = (t: UniverseTime): Vec3 => positionOf(trackOf(earthMoonBodies(), MOON), t);

/** From the Earth towards the Moon at the eclipse, unit. */
const TOWARDS_MOON = normalise(sub(moonAt(T_ECLIPSE), earthAt(T_ECLIPSE)));

/** Cameras about the Earth at the eclipse, m from the barycentre. */
const NEAR_EARTH = add(earthAt(T_ECLIPSE), vec3(1e7, 0, 0));
/** Twice the Moon's distance out, past it, inside the Earth's Hill sphere (Earth local). */
const PAST_MOON = add(earthAt(T_ECLIPSE), scale(TOWARDS_MOON, 7.7e8));
/** Eight times the Moon's distance out, past it and the Earth's Hill sphere (no local body). */
const FAR_PAST_MOON = add(earthAt(T_ECLIPSE), scale(TOWARDS_MOON, 3.1e9));
/** A tenth of an au off, across the Earth–Moon line. */
const ACROSS = add(
  earthAt(T_ECLIPSE),
  scale(normalise(cross(TOWARDS_MOON, vec3(0, 0, 1))), 1.5e10),
);

/** A 30 km/s change of the camera's velocity. */
const VELOCITY_CHANGE_M_PER_S = scale(normalise(vec3(1, 1, 1)), 3e4);

/**
 * The Moon's largest barycentric acceleration at the eclipse: the Sun's pull at its distance then
 * plus the Earth's at the Moon's perigee (363,300 km), m/s².
 */
const MOON_ACCELERATION_MAX_M_S2 =
  GM_SUN_M3_S2 / norm(moonAt(T_ECLIPSE)) ** 2 + GM_EARTH_M3_S2 / 3.633e8 ** 2;

/**
 * Io's largest barycentric acceleration: its track's own pull at Io's pericentre, μ = 4π² a³ ÷ P²
 * (0.09% above GM_J, which the period's J2 carries), plus the Sun's, m/s².
 */
function ioAccelerationMaxMS2(jupiterM: Vec3): number {
  const trackMuM3S2 = (4 * Math.PI ** 2 * IO_ORBIT_M ** 3) / IO_PERIOD_S ** 2;
  return (
    trackMuM3S2 / (IO_ORBIT_M * (1 - IO_ECCENTRICITY)) ** 2 +
    GM_SUN_M3_S2 / (norm(jupiterM) - IO_ORBIT_M) ** 2
  );
}

/** Io's scene seen from 5 × 10¹⁰ m past it, Jupiter the camera's local body. */
function ioFromJupitersSphere(): Seen {
  const bodies = jupiterIoBodies();
  const jupiterNow = positionOf(trackOf(bodies, FIXTURE_JUPITER), T_ECLIPSE);
  const towardsIo = normalise(sub(positionOf(trackOf(bodies, IO), T_ECLIPSE), jupiterNow));
  // Entered at 2 × 10¹⁰ m, so that the frame rule's hysteresis keeps Jupiter at 5 × 10¹⁰ m.
  const entered = seenFrom(bodies, T_ECLIPSE, add(jupiterNow, scale(towardsIo, 2e10)));
  return seenFrom(
    bodies,
    T_ECLIPSE,
    add(jupiterNow, scale(towardsIo, 5e10)),
    vec3(0, 0, 0),
    entered.frame,
  );
}

/** A scene whose Moon is a contact, with no retarded centre. */
function withMoonAsContact(scene: ViewScene): ViewScene {
  return {
    ...scene,
    bodies: scene.bodies.map((body) => (body.id === MOON ? { ...body, retarded: null } : body)),
  };
}

describe("retardedFrom", () => {
  it("puts the Moon where the exact track was when the light reaching the Earth left it, to 1 m", () => {
    for (const camera of [NEAR_EARTH, PAST_MOON, FAR_PAST_MOON, ACROSS]) {
      const seen = seenFrom(earthMoonBodies(), T_ECLIPSE, camera);
      expect(retardationErrorM(seen, earthMoonBodies(), EARTH, MOON)).toBeLessThan(1);
    }
  });

  it("leaves the Moon within ½ a δ² of its track, under 3 cm, the linear retardation's stated error", () => {
    const seen = seenFrom(earthMoonBodies(), T_ECLIPSE, FAR_PAST_MOON);
    const { deltaS } = spanOf(seen, EARTH, MOON);
    const errorM = retardationErrorM(seen, earthMoonBodies(), EARTH, MOON);
    expect(deltaS).toBeGreaterThan(2.5);
    expect(errorM).toBeLessThan(0.5 * MOON_ACCELERATION_MAX_M_S2 * deltaS ** 2);
    expect(errorM).toBeLessThan(0.03);
  });

  it("leaves Io within ½ a δ² of its track, under 3 m, seen from past it", () => {
    const bodies = jupiterIoBodies();
    const jupiterNow = positionOf(trackOf(bodies, FIXTURE_JUPITER), T_ECLIPSE);
    const towardsIo = normalise(sub(positionOf(trackOf(bodies, IO), T_ECLIPSE), jupiterNow));
    // Past Io and outside Jupiter's Hill sphere (5.05 × 10¹⁰ m at pericentre), so that δ is near
    // 2 d ÷ c.
    const seen = seenFrom(bodies, T_ECLIPSE, add(jupiterNow, scale(towardsIo, 6e10)));
    const { deltaS } = spanOf(seen, FIXTURE_JUPITER, IO);
    const errorM = retardationErrorM(seen, bodies, FIXTURE_JUPITER, IO);
    expect(deltaS).toBeGreaterThan(2.5);
    expect(errorM).toBeLessThan(0.5 * ioAccelerationMaxMS2(jupiterNow) * deltaS ** 2);
    expect(errorM).toBeLessThan(3);
  });

  it("leaves Io under 3 m when Jupiter is the camera's local body, 5 × 10¹⁰ m off", () => {
    const seen = ioFromJupitersSphere();
    expect(seen.frame.localBody).toBe(FIXTURE_JUPITER);
    expect(retardationErrorM(seen, jupiterIoBodies(), FIXTURE_JUPITER, IO)).toBeLessThan(3);
  });

  it("extrapolates over δ in [0, 2 d ÷ c] when the lit body is the camera's local body", () => {
    const seen = ioFromJupitersSphere();
    const { deltaS, twiceS } = spanOf(seen, FIXTURE_JUPITER, IO);
    expect(seen.frame.localBody).toBe(FIXTURE_JUPITER);
    expect(deltaS).toBeGreaterThanOrEqual(0);
    expect(deltaS).toBeLessThanOrEqual(twiceS);
  });

  it("leaves a source at rest exactly where it is, as in a kept scene", () => {
    const centreM = vec3(1.5e11, -2e9, 3);
    const source = { centreM, velocityMPerS: vec3(0, 0, 0), lightTimeS: 0 };
    const lit = { centreM: vec3(0, 0, 0), velocityMPerS: vec3(0, 3e4, 0), lightTimeS: 12 };
    expect(retardedFrom(lit, source)).toBe(centreM);
  });

  it(`takes ${String(RETARDATION_STEPS)} steps on the light time, where one leaves the Moon metres off`, () => {
    // At 3,000 s the Earth–Moon line lies near the Earth's motion, so the light time across it
    // changes at nearly v: the step's (v ÷ c) δ is at its largest.
    const atQuarter: UniverseTime = { seconds: 3_000, nanos: 0 };
    const outwards = normalise(sub(moonAt(atQuarter), earthAt(atQuarter)));
    const seen = seenFrom(
      earthMoonBodies(),
      atQuarter,
      add(earthAt(atQuarter), scale(outwards, 3.1e9)),
    );
    const earth = retardedOf(seen, EARTH);
    const moon = retardedOf(seen, MOON);
    const exact = exactlyRetarded(
      trackOf(earthMoonBodies(), MOON),
      earth.centreM,
      litTime(seen.frame, EARTH),
    );
    const oneStep = sub(moon.centreM, scale(moon.velocityMPerS, spanOf(seen, EARTH, MOON).deltaS));
    expect(norm(sub(oneStep, exact))).toBeGreaterThan(1);
    expect(norm(sub(retardedFrom(earth, moon), exact))).toBeLessThan(0.1);
  });
});

describe("the Moon's shadow on the Earth", () => {
  it("does not move when the camera's velocity changes by 30 km/s", () => {
    for (const camera of [NEAR_EARTH, PAST_MOON, FAR_PAST_MOON, ACROSS]) {
      const still = seenFrom(earthMoonBodies(), T_ECLIPSE, camera);
      const moving = seenFrom(earthMoonBodies(), T_ECLIPSE, camera, VELOCITY_CHANGE_M_PER_S);
      expect(norm(sub(litShadowOffset(moving), litShadowOffset(still)))).toBeLessThan(1e3);
    }
  });

  it("moves by tens of kilometres from apparent centres when the camera's velocity changes", () => {
    // Aberrated by 10⁻⁴ rad, the apparent centres move it the ruling's "about 40 km" seen from far
    // off; from beside the Earth they are right to first order (Besselian elements).
    for (const camera of [FAR_PAST_MOON, ACROSS]) {
      const still = seenFrom(earthMoonBodies(), T_ECLIPSE, camera);
      const moving = seenFrom(earthMoonBodies(), T_ECLIPSE, camera, VELOCITY_CHANGE_M_PER_S);
      expect(norm(sub(drawnShadowOffset(moving), drawnShadowOffset(still)))).toBeGreaterThan(1e4);
    }
  });

  it("lies where the exact tracks put it, to 1 km, from a camera at the Earth", () => {
    const atEarth = seenFrom(earthMoonBodies(), T_ECLIPSE, NEAR_EARTH);
    const exact = exactShadowOffset(litTime(atEarth.frame, EARTH));
    expect(atEarth.frame.localBody).toBe(EARTH);
    expect(norm(sub(litShadowOffset(atEarth), exact))).toBeLessThan(1e3);
  });

  it("lies in the same place from a camera at the Earth and one past the Moon, at one t_B", () => {
    const atEarth = seenFrom(earthMoonBodies(), T_ECLIPSE, NEAR_EARTH);
    const tB = litTime(atEarth.frame, EARTH);
    // The far camera sees the Earth a light time back: start its scene that much after t_B.
    let past = seenFrom(earthMoonBodies(), tB, FAR_PAST_MOON);
    for (let k = 0; k < 4; k += 1) {
      const entry = past.frame.bodies.find((each) => each.id === EARTH);
      if (entry === undefined || entry.kind !== "placed") {
        throw new Error("the frame places the Earth");
      }
      past = seenFrom(earthMoonBodies(), after(tB, entry.lightTime), FAR_PAST_MOON);
    }
    expect(past.frame.localBody).toBeNull();
    expect(litTime(past.frame, EARTH)).toEqual(tB);
    expect(norm(sub(litShadowOffset(past), litShadowOffset(atEarth)))).toBeLessThan(1e3);
  });
});

describe("lightingFrameOf", () => {
  it("places another body from the lit body's drawn centre as its retarded centre lies from the lit body's", () => {
    const seen = seenFrom(earthMoonBodies(), T_ECLIPSE, ACROSS);
    const moon = retardedOf(seen, MOON);
    const offset = sub(occluderOf(frameOf(seen, MOON), EARTH), drawnOf(seen, MOON));
    const expected = sub(retardedFrom(moon, retardedOf(seen, EARTH)), moon.centreM);
    expect(norm(sub(offset, expected))).toBeLessThan(1e-3);
  });

  it("lights the camera's local body from its retarded centre, though it is drawn at the present", () => {
    const seen = seenFrom(earthMoonBodies(), T_ECLIPSE, NEAR_EARTH);
    const earth = retardedOf(seen, EARTH);
    const offset = sub(occluderOf(frameOf(seen, EARTH), MOON), drawnOf(seen, EARTH));
    const expected = sub(retardedFrom(earth, retardedOf(seen, MOON)), earth.centreM);
    expect(seen.frame.localBody).toBe(EARTH);
    expect(earth.lightTimeS).toBeGreaterThan(0);
    expect(norm(sub(offset, expected))).toBeLessThan(1e-3);
  });

  it("makes every other lit body an occluder at its radius", () => {
    const frame = frameOf(seenFrom(earthMoonBodies(), T_ECLIPSE, ACROSS), EARTH);
    expect(frame.occluders.map((each) => [each.id, each.radiusM])).toContainEqual([MOON, 1.7374e6]);
  });

  it("does not make the lit body its own occluder", () => {
    const frame = frameOf(seenFrom(earthMoonBodies(), T_ECLIPSE, ACROSS), EARTH);
    expect(frame.occluders.map((each) => each.id)).not.toContain(EARTH);
  });

  it("gives a star no lighting frame", () => {
    const seen = seenFrom(earthMoonBodies(), T_ECLIPSE, ACROSS);
    expect(lightingFrameOf(seen.scene, STAR, seen.pose, DISCS)).toBeNull();
  });

  it("lights a contact from its stars where they are drawn", () => {
    const seen = seenFrom(earthMoonBodies(), T_ECLIPSE, ACROSS);
    const moon = lightingFrameOf(withMoonAsContact(seen.scene), MOON, seen.pose, DISCS);
    expect(moon?.lights.map((light) => light.centreM)).toEqual([drawnOf(seen, STAR)]);
  });

  it("gives a contact no occluder", () => {
    const seen = seenFrom(earthMoonBodies(), T_ECLIPSE, ACROSS);
    const moon = lightingFrameOf(withMoonAsContact(seen.scene), MOON, seen.pose, DISCS);
    expect(moon?.occluders).toEqual([]);
  });

  it("makes a contact no other body's occluder", () => {
    const seen = seenFrom(earthMoonBodies(), T_ECLIPSE, ACROSS);
    const earth = lightingFrameOf(withMoonAsContact(seen.scene), EARTH, seen.pose, DISCS);
    expect(earth?.occluders.map((each) => each.id)).not.toContain(MOON);
  });
});

describe("the neglected aberration at the lit body", () => {
  it("is at most v ÷ c for the Earth, 1.01 × 10⁻⁴ rad, 0.64 km at its limb", () => {
    // The Earth's largest speed, at perihelion: √(GM (1 + e) ÷ (a (1 − e))), a 1 au, e 0.0167.
    const perihelionMPerS = Math.sqrt((GM_SUN_M3_S2 * 1.0167) / (1.495_978_707e11 * 0.9833));
    const earth = retardedOf(seenFrom(earthMoonBodies(), T_ECLIPSE, ACROSS), EARTH);
    expect(norm(earth.velocityMPerS)).toBeLessThanOrEqual(perihelionMPerS);
    expect(perihelionMPerS / SPEED_OF_LIGHT_M_PER_S).toBeLessThan(1.011e-4);
    expect((perihelionMPerS / SPEED_OF_LIGHT_M_PER_S) * 6.371e6).toBeLessThan(645);
  });
});

describe("lightingFramesOf", () => {
  it("gives each lit body in the scene's order", () => {
    const seen = seenFrom(earthMoonBodies(), T_ECLIPSE, ACROSS);
    expect(lightingFramesOf(seen.scene, seen.pose, DISCS).map((entry) => entry.body.id)).toEqual(
      seen.scene.bodies.filter((body) => body.kind !== "star").map((body) => body.id),
    );
  });

  it("gives each lit body its drawn centre", () => {
    const seen = seenFrom(earthMoonBodies(), T_ECLIPSE, ACROSS);
    for (const entry of lightingFramesOf(seen.scene, seen.pose, DISCS)) {
      expect(entry.centreM).toEqual(drawnOf(seen, entry.body.id));
    }
  });

  it("gives each lit body its lightingFrameOf", () => {
    const seen = seenFrom(earthMoonBodies(), T_ECLIPSE, ACROSS);
    for (const entry of lightingFramesOf(seen.scene, seen.pose, DISCS)) {
      expect(entry.frame).toEqual(frameOf(seen, entry.body.id));
    }
  });
});

/**
 * Every kept scene at rest at a few times, with its camera's pose: all but T10.c's eclipse scene,
 * whose bodies move and are lit where their light finds them (`eclipseScene.test.ts`).
 */
function keptScenes(): ReadonlyArray<{ readonly scene: ViewScene; readonly pose: CameraPose }> {
  const kept: ReadonlyArray<KeptScene> = [
    phaseScene(),
    precisionScene(),
    frameChangeScene(),
    spikeScene(new DescentProfile(TEST_PLANET_FIGURE, landingSiteOf(5n))),
  ];
  return kept.flatMap((each) =>
    [0, each.durationS / 3, (2 * each.durationS) / 3].map((tS) => {
      const run = { ...startRun(each), tS, scene: each.sceneAt(tS) };
      return { scene: run.scene, pose: runPose(run) };
    }),
  );
}

describe("a kept scene at rest's lighting", () => {
  it("is its drawing's, bit for bit: every light and occluder where it is drawn", () => {
    for (const { scene, pose } of keptScenes()) {
      const discs = scene.hostDiscs ?? DISCS;
      const origins = sceneOrigins(scene);
      const drawn = (body: ViewBody): Vec3 =>
        relativeToCamera({ kind: "system", system: scene.system, m: body.centreM }, pose, origins);
      const byId = new Map(scene.bodies.map((body) => [body.id, body]));
      const lights = placeLights(hostLights(scene, discs), (id) => {
        const body = byId.get(id);
        return body === undefined ? null : drawn(body);
      });
      const entries = lightingFramesOf(scene, pose, discs);
      const lit = new Set(entries.map((entry) => entry.body.id));
      expect(entries.length).toBeGreaterThan(0);
      for (const { body, frame } of entries) {
        expect(frame.lights).toEqual(lights);
        expect(frame.occluders).toEqual(
          scene.bodies
            .filter((other) => other.id !== body.id && lit.has(other.id))
            .map((other) => ({ id: other.id, centreM: drawn(other), radiusM: other.radiusM })),
        );
      }
    }
  });
});
