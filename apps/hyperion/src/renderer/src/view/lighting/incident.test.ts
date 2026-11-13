import { describe, expect, it } from "vitest";

import { add, normalise, scale, sub } from "../../geometry/vec3";
import { ev100FromIlluminance, ev100FromTriple } from "../photometry/exposure";
import { staticRetarded, type ViewScene } from "../scene/model";
import { keptBody } from "../scenes/kept";
import { PHASE_PLANETS, PHASE_STAR, phaseScene } from "../scenes/phaseScene";
import { incidentIlluminanceLx } from "./incident";

const KEPT = phaseScene();
const SCENE = KEPT.sceneAt(0);
const POSE = KEPT.cameraAt(0);
const DISCS = SCENE.hostDiscs ?? [];

/** The phase scene's half planet, 1 au from its solar twin and airless (a kept scene's). */
const HALF = PHASE_PLANETS[1];

/**
 * Earthrise's exposure (Apollo 8, AS08-14-2383): f/11 at 1/250 s on Ektachrome SO-368 at ISO 64
 * (Apollo 8 Flight Journal, about 075:48 GET, Lovell's "250 at f/11"; recalled, not read in the
 * pod), the stop taken as engraved, 11; its true 2^3.5 = 11.31 gives EV100 15.61.
 */
const EARTHRISE_EV100 = ev100FromTriple({ aperture: 11, shutterS: 1 / 250, iso: 64 });

/** The Moon's mean radius, m (NASA Moon Fact Sheet). */
const MOON_RADIUS_M = 1.7374e6;

/**
 * `SCENE` with a body of `HALF`'s size, or of `radiusM`, `distanceM` from it towards the star,
 * between them.
 */
function withOccluder(distanceM: number, radiusM?: number): ViewScene {
  const planet = SCENE.bodies.find((body) => body.id === HALF);
  const star = SCENE.bodies.find((body) => body.id === PHASE_STAR);
  if (planet === undefined || star === undefined) {
    throw new Error("the phase scene lacks its half planet or its star");
  }
  const centreM = add(
    planet.centreM,
    scale(normalise(sub(star.centreM, planet.centreM)), distanceM),
  );
  const occluder = {
    ...planet,
    id: keptBody(9),
    centreM,
    retarded: staticRetarded(centreM),
    radiusM: radiusM ?? planet.radiusM,
  };
  return { ...SCENE, bodies: [...SCENE.bodies, occluder] };
}

describe("the INCIDENT meter's reading (ux-calls-view, item 3b)", () => {
  it("reads an airless planet 1 au from a solar twin at EV100 15.70, 0.18 EV over Earthrise's 15.53", () => {
    // The Sun at 1 au face-on: 133,462 lx (`SUN_LUX_PER_V0`'s remarks), log₂(E × 100 ÷ 250).
    const ev100 = ev100FromIlluminance(incidentIlluminanceLx(SCENE, HALF, POSE, DISCS) ?? 0);
    expect([
      Math.round(ev100 * 100) / 100,
      Math.round((ev100 - EARTHRISE_EV100) * 100) / 100,
    ]).toEqual([15.7, 0.18]);
  });

  it("sums the discs, a receptor facing each in turn", () => {
    const one = incidentIlluminanceLx(SCENE, HALF, POSE, DISCS) ?? Number.NaN;
    const two = incidentIlluminanceLx(SCENE, HALF, POSE, [...DISCS, ...DISCS]) ?? Number.NaN;
    expect(two / one).toBeCloseTo(2, 12);
  });

  it("reads 0 in another body's umbra, and a share of the light in its penumbra", () => {
    const clear = incidentIlluminanceLx(SCENE, HALF, POSE, DISCS) ?? Number.NaN;
    // An Earth 10⁷ m sunward hides the Sun whole; at 1.3 × 10¹¹ m, 0.0056° across against the
    // Sun's 0.53°, it hides some 0.01% of it.
    const umbra = incidentIlluminanceLx(withOccluder(1e7), HALF, POSE, DISCS);
    const transit = incidentIlluminanceLx(withOccluder(1.3e11), HALF, POSE, DISCS) ?? Number.NaN;
    expect([umbra, transit < clear, transit / clear > 0.999]).toEqual([0, true, true]);
  });

  it("takes no shadow narrower than the body: a Moon's central eclipse of an Earth leaves its reading", () => {
    // At 3.84 × 10⁸ m the Moon's penumbra is some 3,500 km in radius, under the Earth's 6,371 km;
    // the Earth's centre would see 6% of the Sun, its lit face nearly all of it.
    const clear = incidentIlluminanceLx(SCENE, HALF, POSE, DISCS);
    expect(incidentIlluminanceLx(withOccluder(3.84e8, MOON_RADIUS_M), HALF, POSE, DISCS)).toBe(
      clear,
    );
  });

  it("reads 0 with no host discs, and nothing for a body that is not lit, the star", () => {
    expect([
      incidentIlluminanceLx(SCENE, HALF, POSE, []),
      incidentIlluminanceLx(SCENE, PHASE_STAR, POSE, DISCS),
    ]).toEqual([0, null]);
  });
});
