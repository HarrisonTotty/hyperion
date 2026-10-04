import { describe, expect, it } from "vitest";

import { add, norm, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import {
  aHostDisc,
  MOON_GEOMETRIC_ALBEDO,
  photometryFor,
  planetPhotometry,
} from "../../test/litFixtures";
import type { BodyFigure } from "../appearance/fromWire";
import { discIntegratedPhase, geometricAlbedo } from "../appearance/phase";
import { V0_ILLUMINANCE_LX } from "../photometry/magnitude";
import { AU_M } from "../scenes/kept";
import { DISC_ANNULI_HIGH } from "./annuli";
import type { PlacedLight } from "./hostLights";
import { CHANNEL_LUMINANCE, photopicIlluminance, starIlluminance } from "./illuminance";
import { sphereIrradianceBruteForce, UNIFORM_DISC } from "./oracle";
import {
  litNeighbours,
  PLANETSHINE_SOURCES_HIGH,
  PLANETSHINE_SOURCES_LOW,
  phaseMaximum,
  planetshineIrradiance,
  planetshineSources,
  type ReflectingBody,
  type SecondarySource,
} from "./planetshine";

const RAD = Math.PI / 180;

/** A body's planetshine sources among `lit`, its neighbours lit by `hosts`. */
function sourcesOf(
  body: ReflectingBody,
  lit: ReadonlyArray<ReflectingBody>,
  hosts: ReadonlyArray<PlacedLight>,
  max: number,
): SecondarySource[] {
  return planetshineSources(body, litNeighbours(lit, hosts, DISC_ANNULI_HIGH), max);
}

/** The Earth–Moon distance, m: the Moon's semi-major axis (NASA GSFC, Moon Fact Sheet). */
const EARTH_MOON_M = 3.844e8;

/** Jupiter's semi-major axis, m (NASA GSFC, Jupiter Fact Sheet: 778.479 × 10⁶ km). */
const JUPITER_ORBIT_M = 7.784_79e11;

/** Io's semi-major axis about Jupiter, m (NASA GSFC, Jovian Satellite Fact Sheet: 421.8 × 10³ km). */
const IO_ORBIT_M = 4.218e8;

const sphere = (radiusM: number): BodyFigure => ({
  equatorialRadiusM: radiusM,
  polarRadiusM: radiusM,
  pole: null,
});

/** The Sun at the origin. */
const SUN: PlacedLight = { disc: aHostDisc(), centreM: vec3(0, 0, 0) };

const EARTH: ReflectingBody = {
  id: "0200080020000000.0003",
  centreM: vec3(AU_M, 0, 0),
  figure: sphere(6.371e6),
  photometry: planetPhotometry("Earth"),
};

/** A Moon of p_V 0.12 in every channel on the Moon's template (q at s = 1). */
const MOON_PHOTOMETRY = photometryFor(
  [MOON_GEOMETRIC_ALBEDO, MOON_GEOMETRIC_ALBEDO, MOON_GEOMETRIC_ALBEDO],
  [0.626, 0.626, 0.626],
  "moon",
);

/** The Moon at `angleDeg` round Earth from the sunward direction, in the ecliptic. */
function moonAt(angleDeg: number): ReflectingBody {
  const offset = vec3(-Math.cos(angleDeg * RAD), Math.sin(angleDeg * RAD), 0);
  return {
    id: "0200080020000000.0301",
    centreM: add(EARTH.centreM, scale(offset, EARTH_MOON_M)),
    figure: sphere(1.7374e6),
    photometry: MOON_PHOTOMETRY,
  };
}

/** The photopic illuminance of a body's planetshine, lx, summed over its sources. */
function photopic(body: ReflectingBody, lit: ReadonlyArray<ReflectingBody>): number {
  return sourcesOf(body, lit, [SUN], PLANETSHINE_SOURCES_HIGH).reduce(
    (sum, source) => sum + photopicIlluminance(source.illuminance),
    0,
  );
}

/** E★ p (R ÷ Δ)² at zero phase in photopic terms, lx: the closed form the model must reproduce. */
function fullPhaseLx(neighbour: ReflectingBody, radiusM: number, distanceM: number): number {
  const e = starIlluminance(SUN.disc, norm(sub(neighbour.centreM, SUN.centreM)));
  const p = neighbour.photometry.geometricAlbedo;
  const solid = (radiusM / distanceM) ** 2;
  return (
    solid *
    (CHANNEL_LUMINANCE[0] * e[0] * p[0] +
      CHANNEL_LUMINANCE[1] * e[1] * p[1] +
      CHANNEL_LUMINANCE[2] * e[2] * p[2])
  );
}

describe("planetshine's illuminance", () => {
  it("lights the Moon at full Earth with 7.7 lx of earthshine, the closed form's", () => {
    // The Moon between Sun and Earth, so Earth is full from it. Earth's p is Robinson 2026's (PSJ
    // 7, 12, eq. 14: f = 0.23 in his band ratios 0.277 : 0.226 : 0.221), (r, g, b) 0.210, 0.215
    // and 0.263 with q 1.312, on the current `earth` key; at full phase only p counts
    // (decision-r07-earth-albedo; R07.T4.d moves the fixture's Mallama row). The Moon's shadow on
    // Earth, a smaller body's, is left out.
    const earth: ReflectingBody = {
      ...EARTH,
      photometry: photometryFor([0.21, 0.215, 0.263], [1.312, 1.312, 1.312], "earth"),
    };
    const moon = moonAt(0);
    const [source, ...rest] = sourcesOf(moon, [moon, earth], [SUN], 2);
    expect(rest).toHaveLength(0);
    const lux = photopicIlluminance(source?.illuminance ?? [0, 0, 0]);
    expect(Math.abs(lux / fullPhaseLx(earth, 6.371e6, EARTH_MOON_M) - 1)).toBeLessThan(1e-9);
    // The ruling's 7.66 lx, to its rounding: 7.668 lx under `sunLikeHostDisc`'s warm white.
    expect(lux).toBeCloseTo(7.67, 2);
    // The task's band, 7.7 lx ± 15%: p_V 0.23 ± 0.02 and the weather.
    expect(lux).toBeGreaterThan(7.7 * 0.85);
    expect(lux).toBeLessThan(7.7 * 1.15);
  });

  it("places Earth's source on the Moon–Earth line at its angular radius", () => {
    const moon = moonAt(0);
    const [source] = sourcesOf(moon, [moon, EARTH], [SUN], 2);
    expect(source?.body).toBe(EARTH.id);
    expect(source?.direction.x).toBeCloseTo(1, 12);
    expect(source?.distanceM).toBeCloseTo(EARTH_MOON_M, 3);
    expect(source?.radiusM).toBe(6.371e6);
    expect(source?.angularRadiusRad).toBeCloseTo(Math.asin(6.371e6 / EARTH_MOON_M), 15);
  });

  it("lights Earth with 0.32 lx at full Moon, as the full Moon's V = −12.74 does, to 5%", () => {
    // V = −12.74 (NASA's Moon Fact Sheet; −12.73 in Krisciunas and Schaefer 1991, PASP 103, 1033,
    // eq. 9) is the full Moon extrapolated to zero phase without the opposition surge (their
    // p. 1035): there the Moon is in Earth's shadow, so Earth is left out of the neighbours as an
    // occluder here (the eclipse is tested below).
    const moon = moonAt(180);
    const fromMagnitude = V0_ILLUMINANCE_LX * 10 ** (0.4 * 12.74);
    expect(fromMagnitude).toBeCloseTo(0.3168, 4);
    const [source] = planetshineSources(EARTH, litNeighbours([moon], [SUN], DISC_ANNULI_HIGH), 2);
    const lux = photopicIlluminance(source?.illuminance ?? [0, 0, 0]);
    expect(Math.abs(lux / fromMagnitude - 1)).toBeLessThan(0.05);
    expect(Math.abs(lux / fullPhaseLx(moon, 1.7374e6, EARTH_MOON_M) - 1)).toBeLessThan(1e-9);
  });

  it("takes no light from a moon in its planet's umbra, as in a total lunar eclipse", () => {
    expect(sourcesOf(EARTH, [EARTH, moonAt(180)], [SUN], 2)).toEqual([]);
    // Clear of Earth's penumbra, 2° from opposition, the full Moon shines again.
    expect(photopic(EARTH, [EARTH, moonAt(178)])).toBeGreaterThan(0.29);
  });

  it("lights Io at inferior conjunction with about 70 lx of Jupiter-shine, Io's shadow left out", () => {
    // Jupiter 6.487% oblate (NASA's 71,492 and 66,854 km at 1 bar), its pole across the line; p from
    // Mallama et al. 2017, defined against π a c. Io's own shadow lies on Jupiter's centre.
    const jupiter: ReflectingBody = {
      id: "0200080020000000.0005",
      centreM: vec3(JUPITER_ORBIT_M, 0, 0),
      figure: { equatorialRadiusM: 7.1492e7, polarRadiusM: 6.6854e7, pole: vec3(0, 0, 1) },
      photometry: planetPhotometry("Jupiter"),
    };
    const io: ReflectingBody = {
      id: "0200080020000000.0501",
      centreM: vec3(JUPITER_ORBIT_M - IO_ORBIT_M, 0, 0),
      figure: sphere(1.8215e6),
      photometry: MOON_PHOTOMETRY,
    };
    const lux = photopic(io, [io, jupiter]);
    expect(lux).toBeGreaterThan(70 * 0.9);
    expect(lux).toBeLessThan(70 * 1.1);
    // The spheroid's integral meets the closed form over √(a c) to its quadrature's 10⁻³.
    const closed = fullPhaseLx(jupiter, Math.sqrt(7.1492e7 * 6.6854e7), IO_ORBIT_M);
    expect(Math.abs(lux / closed - 1)).toBeLessThan(1e-3);
    // About 6 stops below the sunlight there (Design note 7).
    const sunlight = photopicIlluminance(starIlluminance(SUN.disc, norm(io.centreM)));
    expect(Math.log2(sunlight / lux)).toBeGreaterThan(5.5);
    expect(Math.log2(sunlight / lux)).toBeLessThan(6.5);
  });

  it("takes about nothing from a neighbour at new phase", () => {
    const full = photopic(moonAt(0), [moonAt(0), EARTH]);
    // The Moon beyond Earth from the Sun: Earth's night side faces it.
    expect(photopic(moonAt(180), [moonAt(180), EARTH])).toBeLessThan(1e-12 * full);
    // Ten degrees from new, Earth's thin crescent still gives under 1% of full.
    expect(photopic(moonAt(170), [moonAt(170), EARTH])).toBeLessThan(1e-2 * full);
  });

  it("takes nothing from a neighbour no star lights, or from the body itself", () => {
    const moon = moonAt(0);
    expect(sourcesOf(moon, [moon, EARTH], [], 2)).toEqual([]);
    expect(sourcesOf(moon, [moon], [SUN], 2)).toEqual([]);
    expect(sourcesOf(moon, [moon, EARTH], [SUN], 0)).toEqual([]);
  });
});

describe("planetshine's sources", () => {
  // A moon with three neighbours at full phase from it, each 4 × 10⁸ m away and brighter in turn.
  const moon = moonAt(0);
  const neighbour = (id: string, radiusM: number, along: Vec3): ReflectingBody => ({
    id: `0200080020000000.${id}`,
    centreM: add(moon.centreM, scale(along, 4e8)),
    figure: sphere(radiusM),
    photometry: planetPhotometry("Earth"),
  });
  const faint = neighbour("0010", 2e6, vec3(1, 0, 0));
  const middle = neighbour("0011", 4e6, vec3(Math.cos(0.3), Math.sin(0.3), 0));
  const bright = neighbour("0012", 8e6, vec3(Math.cos(0.3), -Math.sin(0.3), 0));
  const lit = [faint, moon, middle, bright];

  it("are the two largest on the high setting and the largest on the low, largest first", () => {
    expect(PLANETSHINE_SOURCES_HIGH).toBe(2);
    expect(PLANETSHINE_SOURCES_LOW).toBe(1);
    const high = sourcesOf(moon, lit, [SUN], PLANETSHINE_SOURCES_HIGH);
    expect(high.map((source) => source.body)).toEqual([bright.id, middle.id]);
    const low = sourcesOf(moon, lit, [SUN], PLANETSHINE_SOURCES_LOW);
    expect(low.map((source) => source.body)).toEqual([bright.id]);
    const [first, second] = high;
    expect(photopicIlluminance(first?.illuminance ?? [0, 0, 0])).toBeGreaterThan(
      photopicIlluminance(second?.illuminance ?? [0, 0, 0]),
    );
  });

  it("are the largest of all, the bound skipping none that could be", () => {
    // 40 neighbours of random size, distance and phase about the moon (a fixed linear congruence).
    let seed = 12_345;
    const next = (): number => {
      seed = (seed * 1_103_515_245 + 12_345) % 2_147_483_648;
      return seed / 2_147_483_648;
    };
    const crowd = Array.from({ length: 40 }, (_, i) => {
      const theta = 2 * Math.PI * next();
      const along = vec3(Math.cos(theta), Math.sin(theta), 0.3 * (next() - 0.5));
      return neighbour(
        (0x100 + i).toString(16).padStart(4, "0"),
        1e6 + 7e6 * next(),
        scale(along, 0.5 + 3 * next()),
      );
    });
    const each = crowd.map((other) => ({
      id: other.id,
      lux: photopicIlluminance(
        sourcesOf(moon, [moon, other], [SUN], 1)[0]?.illuminance ?? [0, 0, 0],
      ),
    }));
    const truth = each
      .toSorted((a, b) => b.lux - a.lux)
      .slice(0, 2)
      .map((one) => one.id);
    const chosen = sourcesOf(moon, [moon, ...crowd], [SUN], 2).map((source) => source.body);
    expect(chosen).toEqual(truth);
  });

  it("do not depend on the order of the lit bodies", () => {
    const forward = sourcesOf(moon, lit, [SUN], 2);
    const backward = sourcesOf(moon, lit.toReversed(), [SUN], 2);
    expect(backward).toEqual(forward);
  });

  it("rank a spheroid by its equivalent sphere and light by its own figure", () => {
    const oblate: ReflectingBody = {
      ...bright,
      figure: { equatorialRadiusM: 8e6, polarRadiusM: 7.2e6, pole: vec3(0, 0, 1) },
    };
    const [source] = sourcesOf(moon, [moon, oblate], [SUN], 1);
    const round = sourcesOf(
      moon,
      [moon, { ...bright, figure: sphere(Math.sqrt(8e6 * 7.2e6)) }],
      [SUN],
      1,
    )[0];
    // Equator-on at full phase, p against π a c: the spheroid gives its equivalent sphere's light.
    const ratio =
      photopicIlluminance(source?.illuminance ?? [0, 0, 0]) /
      photopicIlluminance(round?.illuminance ?? [1, 1, 1]);
    expect(Math.abs(ratio - 1)).toBeLessThan(2e-3);
    expect(source?.angularRadiusRad).toBeCloseTo(
      Math.asin(8e6 / norm(sub(oblate.centreM, moon.centreM))),
      15,
    );
  });
});

describe("the ranking's bound", () => {
  it("bounds every channel's p Φ(α) at every phase, for each planet's law and the Moon's", () => {
    const laws = [
      ...["Mercury", "Venus", "Earth", "Mars", "Jupiter", "Saturn", "Uranus", "Neptune"].map(
        (name) => planetPhotometry(name).law,
      ),
      MOON_PHOTOMETRY.law,
    ];
    for (const law of laws) {
      const p = geometricAlbedo(law);
      let worst = 0;
      for (let k = 0; k <= 18_000; k += 1) {
        const phase = discIntegratedPhase(law, (k * Math.PI) / 18_000);
        worst = Math.max(worst, p[0] * phase[0], p[1] * phase[1], p[2] * phase[2]);
      }
      expect(worst).toBeLessThanOrEqual(phaseMaximum(law));
      expect(worst).toBeGreaterThan(phaseMaximum(law) / 1.02);
    }
  });
});

describe("planetshine at a lit point", () => {
  it("is the uniform sphere's irradiance carried from the centre by the inverse square", () => {
    // A neighbour 30 radii away (about Earth from the Moon at 1.9° across is 60), seen from a point
    // 0.5 radii off the body's centre, its normal at 80° and at 91° to the neighbour.
    const centreDistance = 30;
    const sourceRadius = 1;
    for (const phiDeg of [10, 80, 91]) {
      const normal = vec3(Math.cos(phiDeg * RAD), Math.sin(phiDeg * RAD), 0);
      const toSource = sub(vec3(centreDistance, 0, 0), vec3(0, -0.5, 0));
      const d = norm(toSource);
      // The normal's angle to the source's centre as seen from the point.
      const cosPhi = (normal.x * toSource.x + normal.y * toSource.y) / d;
      const truth =
        sphereIrradianceBruteForce(d / sourceRadius, Math.acos(cosPhi), 0, UNIFORM_DISC) *
        (centreDistance / d) ** 2;
      const factor = planetshineIrradiance(toSource, sourceRadius, centreDistance, normal);
      expect(Math.abs(factor - truth)).toBeLessThan(1e-5);
    }
  });

  it("is nothing where the neighbour is wholly below the horizon", () => {
    const normal = vec3(-1, 0, 0);
    expect(planetshineIrradiance(vec3(30, 0, 0), 1, 30, normal)).toBe(0);
  });
});
