import { describe, expect, it } from "vitest";

import { vec3, type Vec3 } from "../../geometry/vec3";
import { aHostDisc } from "../../test/litFixtures";
import type { BodyFigure } from "../appearance/fromWire";
import type { Rgb } from "../photometry/toneCurve";
import { shapeGeometricAlbedo, shapePhase } from "../appearance/shapes";
import { AU_M } from "../scenes/kept";
import { DISC_ANNULI_HIGH, DISC_ANNULI_LOW, eclipseVisible, type LimbDarkenedDisc } from "./annuli";
import { discEclipseVisible, type EclipsedBody } from "./discEclipse";
import type { PlacedLight } from "./hostLights";
import { discEclipseBruteForce, type FarView } from "./oracle";

const RAD = Math.PI / 180;

/** A sphere's figure. */
const sphere = (radiusM: number): BodyFigure => ({
  equatorialRadiusM: radiusM,
  polarRadiusM: radiusM,
  pole: null,
});

/** The Sun-like disc at the origin. */
const SUN: PlacedLight = { disc: aHostDisc(), centreM: vec3(0, 0, 0) };

/** One channel of a placed light as the eclipse term takes it (V: the disc's middle law). */
function channelOf(light: PlacedLight, channel: 0 | 1 | 2): LimbDarkenedDisc {
  // R06's limb laws are B, V, R; the display's channels r, g, b.
  const [b, v, r] = light.disc.limb;
  const law = [r, v, b][channel] ?? v;
  return {
    centreM: light.centreM,
    radiusM: light.disc.radius_m,
    limbC: law.c,
    limbAlpha: law.alpha,
  };
}

/** The far point at phase α from a body lit along −x, tilted out of the plane by 45°. */
function towardsAt(phaseDeg: number): Vec3 {
  const alpha = phaseDeg * RAD;
  return vec3(-Math.cos(alpha), Math.sin(alpha) * Math.SQRT1_2, Math.sin(alpha) * Math.SQRT1_2);
}

/**
 * A far point's view at phase α of a sphere of radius R lit along −x, with its whole,
 * π R² [L + ⅔ (1 − L)] Φ_shape(α).
 */
function viewOf(radiusM: number, phaseDeg: number, share: number): FarView {
  const alpha = phaseDeg * RAD;
  return {
    towards: towardsAt(phaseDeg),
    share,
    totalM2: Math.PI * radiusM * radiusM * shapeGeometricAlbedo(share) * shapePhase(share, alpha),
  };
}

/**
 * A body of radius R at `distanceM` from the Sun along +x, and an occluder of radius `ratio` × R
 * `behindM` sunward of it, whose shadow axis crosses the body's plane `axisM` off its centre.
 */
function shadowScene(
  distanceM: number,
  radiusM: number,
  ratio: number,
  behindM: number,
  axisM: number,
): { readonly body: EclipsedBody; readonly occluder: { centreM: Vec3; radiusM: number } } {
  const occluderDistanceM = distanceM - behindM;
  return {
    body: { centreM: vec3(distanceM, 0, 0), figure: sphere(radiusM) },
    occluder: {
      centreM: vec3(occluderDistanceM, (axisM * occluderDistanceM) / distanceM, 0),
      radiusM: ratio * radiusM,
    },
  };
}

/** The exact tangent cones' radii at x behind an occluder (R_o + x sin γ) ÷ cos γ, m. */
function coneRadius(occluderRadiusM: number, sinGamma: number, behindM: number): number {
  return (occluderRadiusM + behindM * sinGamma) / Math.sqrt(1 - sinGamma * sinGamma);
}

/** A Sun-like star at the origin with the given B, V and R limb laws. */
function starWith(
  b: { readonly c: number; readonly alpha: number },
  v: { readonly c: number; readonly alpha: number },
  r: { readonly c: number; readonly alpha: number },
): PlacedLight {
  return { disc: aHostDisc({ limb: [b, v, r] }), centreM: vec3(0, 0, 0) };
}

/** The first time in 0 to 2,000 s that a falling `visible` drops below `below`, by bisection. */
function firstWhen(visible: (timeS: number) => number, below: number): number {
  let early = 0;
  let late = 2_000;
  for (let step = 0; step < 60; step += 1) {
    const middle = (early + late) / 2;
    if (visible(middle) < below) {
      late = middle;
    } else {
      early = middle;
    }
  }
  return late;
}

describe("a body's eclipse over its disc against brute force", () => {
  // Two families: a Moon-sized body 4 × 10⁸ m behind its occluder at 1 au, and a Jupiter-sized one
  // 4.2 × 10⁸ m behind at 5.2 au, so that every ratio has a real penumbra on the disc.
  const cases = [
    { ratio: 0.02, distanceM: 5.2 * AU_M, radiusM: 7e7, behindM: 4.2e8 },
    { ratio: 0.1, distanceM: 5.2 * AU_M, radiusM: 7e7, behindM: 4.2e8 },
    { ratio: 0.3, distanceM: AU_M, radiusM: 2e6, behindM: 4e8 },
    { ratio: 1, distanceM: AU_M, radiusM: 2e6, behindM: 4e8 },
    { ratio: 3, distanceM: AU_M, radiusM: 2e6, behindM: 4e8 },
    { ratio: 10, distanceM: AU_M, radiusM: 2e6, behindM: 4e8 },
    { ratio: 30, distanceM: AU_M, radiusM: 2e6, behindM: 4e8 },
  ] as const;
  const viewsOf = (radiusM: number): FarView[] =>
    [0, 60, 120].flatMap((phaseDeg) => [0, 1].map((share) => viewOf(radiusM, phaseDeg, share)));

  it.each(cases)(
    "meets the f64 surface integral of 10⁶ points to 10⁻³ at a radius ratio of $ratio, across the penumbra, at 0°, 60° and 120° of phase and L 0 and 1",
    ({ ratio, distanceM, radiusM, behindM }) => {
      const r = ratio * radiusM;
      const along = distanceM - behindM;
      const penumbra = coneRadius(r, (SUN.disc.radius_m + r) / along, behindM);
      const umbra = coneRadius(r, (r - SUN.disc.radius_m) / along, behindM);
      // The axis from where the disc's edge meets the umbra (or the centre) out to first contact.
      const nearest = Math.max(0, umbra - radiusM);
      const furthest = penumbra + radiusM;
      const views = viewsOf(radiusM);
      let worst = 0;
      let worstShare = 0;
      for (const across of [0.15, 0.5, 0.85]) {
        const { body, occluder } = shadowScene(
          distanceM,
          radiusM,
          ratio,
          behindM,
          nearest + across * (furthest - nearest),
        );
        const truth = discEclipseBruteForce(
          channelOf(SUN, 1),
          { centreM: body.centreM, radiusM },
          occluder,
          views,
          DISC_ANNULI_HIGH,
          1000,
        );
        views.forEach((view, n) => {
          const visible = discEclipseVisible(
            SUN,
            body,
            [occluder],
            view.towards,
            view.share,
            DISC_ANNULI_HIGH,
          )[1];
          const exact = truth[n] ?? Number.NaN;
          worst = Math.max(worst, Math.abs(visible - exact));
          if (1 - exact > 1e-3) {
            worstShare = Math.max(worstShare, Math.abs(visible - exact) / (1 - exact));
          }
        });
      }
      expect(worst).toBeLessThan(1e-3);
      // And, where the eclipse takes over a thousandth, to 1% of what it takes.
      expect(worstShare).toBeLessThan(0.01);
    },
    60_000,
  );
});

describe("a body's eclipse over its disc", () => {
  it("is exactly 0 for a body wholly in an umbra", () => {
    // Earth's umbra is 4,600 km in radius at the Moon's distance; the Moon's radius 1,737 km.
    const deep = shadowScene(AU_M, 1.7374e6, 6.371e6 / 1.7374e6, 3.844e8, 1e5);
    expect(
      discEclipseVisible(SUN, deep.body, [deep.occluder], towardsAt(30), 1, DISC_ANNULI_HIGH),
    ).toEqual([0, 0, 0]);
  });

  it("is exactly 1 for a body clear of every penumbra", () => {
    // Earth's penumbra's radius is 8,175 km there: a Moon 12,000 km off the axis is clear.
    const clear = shadowScene(AU_M, 1.7374e6, 6.371e6 / 1.7374e6, 3.844e8, 1.2e7);
    expect(
      discEclipseVisible(SUN, clear.body, [clear.occluder], towardsAt(30), 1, DISC_ANNULI_HIGH),
    ).toEqual([1, 1, 1]);
  });

  it("is exactly 1 with no occluders", () => {
    const clear = shadowScene(AU_M, 1.7374e6, 6.371e6 / 1.7374e6, 3.844e8, 1.2e7);
    expect(discEclipseVisible(SUN, clear.body, [], towardsAt(30), 1, DISC_ANNULI_HIGH)).toEqual([
      1, 1, 1,
    ]);
  });

  it("ignores an occluder beyond the star or behind the body", () => {
    const body: EclipsedBody = { centreM: vec3(AU_M, 0, 0), figure: sphere(1.7374e6) };
    const beyond = { centreM: vec3(-1e10, 0, 0), radiusM: 7e7 };
    const behind = { centreM: vec3(AU_M + 3.844e8, 0, 0), radiusM: 6.371e6 };
    expect(
      discEclipseVisible(SUN, body, [beyond, behind], towardsAt(0), 0, DISC_ANNULI_HIGH),
    ).toEqual([1, 1, 1]);
  });

  it("leaves 0.893 of a full Earth's light towards the Moon in a central solar eclipse, the ruling's oracle, under a uniform Sun's parallel light", () => {
    // Lambert, full phase, the Moon 384,400 km sunward on the axis (decision-r07-earth-albedo,
    // Q2). The ruling's 0.893 matches the shadow of parallel light, the Sun at infinity at its
    // angular radius (its script is not kept); at 1 au the cone widens over the 384,400 km by
    // (1 + x ÷ d)² = 1.005 in area, and 0.8922 is left, as the surface integral finds.
    const uniform = { c: 0, alpha: 1 };
    const flat = (scaleUp: number): PlacedLight => ({
      disc: aHostDisc({
        radius_m: 6.957e8 * scaleUp,
        limb: [uniform, uniform, uniform],
      }),
      centreM: vec3(0, 0, 0),
    });
    const at = (scaleUp: number): number => {
      const { body, occluder } = shadowScene(
        scaleUp * AU_M,
        6.371e6,
        1.7374e6 / 6.371e6,
        3.844e8,
        0,
      );
      return discEclipseVisible(
        flat(scaleUp),
        body,
        [occluder],
        towardsAt(0),
        0,
        DISC_ANNULI_HIGH,
      )[1];
    };
    expect(Math.abs(at(1e5) - 0.893)).toBeLessThan(5e-4);
    const { body, occluder } = shadowScene(AU_M, 6.371e6, 1.7374e6 / 6.371e6, 3.844e8, 0);
    const [truth] = discEclipseBruteForce(
      channelOf(flat(1), 1),
      { centreM: body.centreM, radiusM: 6.371e6 },
      occluder,
      [viewOf(6.371e6, 0, 0)],
      DISC_ANNULI_HIGH,
      1000,
    );
    expect(Math.abs(at(1) - (truth ?? 0))).toBeLessThan(1e-5);
    expect(at(1)).toBeCloseTo(0.8922, 4);
  });

  it("takes 1.5 (R_Io ÷ √(a c))² of a full Jupiter's light in Io's central shadow transit, not all of it", () => {
    // Jupiter 71,492 by 66,854 km at 778.479 × 10⁶ km, Io of 1,821.5 km 421,800 km sunward on its
    // centre (NASA GSFC's Jupiter and Jovian Satellite Fact Sheets). Its shadow takes the light it stops, π R_Io² (1 + x ÷ d)² of the
    // plane, weighted by Lambert's μ at the centre against ⅔ over the disc.
    const jupiter: EclipsedBody = {
      centreM: vec3(7.78479e11, 0, 0),
      figure: { equatorialRadiusM: 7.1492e7, polarRadiusM: 6.6854e7, pole: vec3(0, 0, 1) },
    };
    const io = { centreM: vec3(7.78479e11 - 4.218e8, 0, 0), radiusM: 1.8215e6 };
    const expected =
      1.5 * (1.8215e6 / Math.sqrt(7.1492e7 * 6.6854e7)) ** 2 * (1 + 4.218e8 / 7.78479e11) ** 2;
    const visible = discEclipseVisible(SUN, jupiter, [io], towardsAt(0), 0, DISC_ANNULI_HIGH);
    for (const c of [0, 1, 2] as const) {
      expect(Math.abs(1 - visible[c] - expected)).toBeLessThan(0.01 * expected);
    }
    // From its centre, as a point took it before R07.T10.b, Jupiter is in Io's umbra.
    expect(eclipseVisible(channelOf(SUN, 1), jupiter.centreM, [io], DISC_ANNULI_HIGH)).toBe(0);
  });

  it("fades a point Io entering Jupiter's shadow from 1 to 0 over about 254 s, where its centre takes 44 s", () => {
    // Io (1,821.5 km, L = 1) 421,800 km behind Jupiter (71,492 km) at 778.479 × 10⁶ km, on a
    // straight path across the axis at 2π a ÷ P = 17.338 km/s (P 1.769138 d; NASA GSFC's fact
    // sheets), seen from the Sun's side. Over the disc the fade runs from first contact with the
    // penumbra to the last limb's entering the umbra, (2 R_Io + w_p) ÷ v; from the centre over the
    // penumbra's 754 km alone (decision-r07-earth-albedo, Q2). Io's real path meets the shadow
    // 9.8° from opposition, more slowly across it, and takes about 257 s.
    const jupiterM = 7.78479e11;
    const behindM = 4.218e8;
    const speedMPerS = (2 * Math.PI * behindM) / (1.769_138 * 86_400);
    const ioRadiusM = 1.8215e6;
    const jupiter = { centreM: vec3(jupiterM, 0, 0), radiusM: 7.1492e7 };
    const ioAt = (timeS: number): EclipsedBody => ({
      centreM: vec3(jupiterM + behindM, 8e7 - speedMPerS * timeS, 0),
      figure: sphere(ioRadiusM),
    });
    const disc = (timeS: number): number =>
      discEclipseVisible(SUN, ioAt(timeS), [jupiter], towardsAt(0), 1, DISC_ANNULI_HIGH)[1];
    const centre = (timeS: number): number =>
      eclipseVisible(channelOf(SUN, 1), ioAt(timeS).centreM, [jupiter], DISC_ANNULI_HIGH);
    const along = jupiterM;
    const r = jupiter.radiusM;
    const width =
      coneRadius(r, (SUN.disc.radius_m + r) / along, behindM) -
      coneRadius(r, (r - SUN.disc.radius_m) / along, behindM);
    expect(width / 1e3).toBeCloseTo(754, 0);
    const discFade = firstWhen(disc, Number.MIN_VALUE) - firstWhen(disc, 1);
    expect(Math.abs(discFade / ((2 * ioRadiusM + width) / speedMPerS) - 1)).toBeLessThan(0.01);
    expect(discFade).toBeGreaterThan(250);
    expect(discFade).toBeLessThan(256);
    const centreFade = firstWhen(centre, Number.MIN_VALUE) - firstWhen(centre, 1);
    expect(Math.abs(centreFade / (width / speedMPerS) - 1)).toBeLessThan(0.01);
    // The disc's fade falls monotonically.
    const start = firstWhen(disc, 1);
    let previous = 1;
    for (let timeS = start; timeS <= start + discFade; timeS += 5) {
      const visible = disc(timeS);
      expect(visible).toBeLessThanOrEqual(previous);
      previous = visible;
    }
  });

  it("counts no shadow the far point cannot see", () => {
    // Io's shadow on Jupiter's sub-solar point, seen from 120° of phase: out of view.
    const { body, occluder } = shadowScene(5.2 * AU_M, 7e7, 0.026, 4.2e8, 0);
    expect(discEclipseVisible(SUN, body, [occluder], towardsAt(120), 0, DISC_ANNULI_HIGH)).toEqual([
      1, 1, 1,
    ]);
    expect(
      discEclipseVisible(SUN, body, [occluder], towardsAt(0), 0, DISC_ANNULI_HIGH)[1],
    ).toBeLessThan(1);
  });

  it("adds the deficits of separate shadows", () => {
    // Two small moons' shadows apart on a giant.
    const one = shadowScene(5.2 * AU_M, 7e7, 0.026, 4.2e8, 2e7);
    const other = shadowScene(5.2 * AU_M, 7e7, 0.026, 4.2e8, -2.5e7);
    const towards = towardsAt(10);
    const each = (occluders: ReadonlyArray<{ centreM: Vec3; radiusM: number }>): number =>
      discEclipseVisible(SUN, one.body, occluders, towards, 0, DISC_ANNULI_HIGH)[1];
    const both = each([one.occluder, other.occluder]);
    expect(both).toBeCloseTo(1 - (1 - each([one.occluder])) - (1 - each([other.occluder])), 14);
  });

  /** Three B, V and R limb laws that differ. */
  const B_LAW = { c: 0.85, alpha: 0.8 };
  const V_LAW = { c: 0.77, alpha: 0.7 };
  const R_LAW = { c: 0.62, alpha: 0.5 };
  // A Moon in the middle of Earth's penumbra, where the profile across it follows the limb law.
  const penumbral = shadowScene(AU_M, 1.7374e6, 6.371e6 / 1.7374e6, 3.844e8, 6.5e6);

  it("takes each display channel's eclipse from the star's own law, r from R, g from V, b from B", () => {
    const visible = (star: PlacedLight): Rgb =>
      discEclipseVisible(
        star,
        penumbral.body,
        [penumbral.occluder],
        towardsAt(20),
        1,
        DISC_ANNULI_HIGH,
      );
    const mixed = visible(starWith(B_LAW, V_LAW, R_LAW));
    const [r] = visible(starWith(R_LAW, R_LAW, R_LAW));
    const [, g] = visible(starWith(V_LAW, V_LAW, V_LAW));
    const [, , b] = visible(starWith(B_LAW, B_LAW, B_LAW));
    // The channels differ by far more than the quadrature's splits move any one of them.
    const gap = Math.min(Math.abs(r - g), Math.abs(g - b), Math.abs(r - b));
    expect(gap).toBeGreaterThan(1e-4);
    expect(Math.abs(mixed[0] - r)).toBeLessThan(1e-6);
    expect(Math.abs(mixed[1] - g)).toBeLessThan(1e-6);
    expect(Math.abs(mixed[2] - b)).toBeLessThan(1e-6);
  });

  it("takes the low setting's three annuli to within the eclipse term's own error", () => {
    const star = starWith(B_LAW, V_LAW, R_LAW);
    const at = (k: number): number =>
      discEclipseVisible(star, penumbral.body, [penumbral.occluder], towardsAt(20), 1, k)[1];
    // T6.b's worst error at K = 3 is 1.02% of the star's light in V, at a single point.
    expect(Math.abs(at(DISC_ANNULI_LOW) - at(DISC_ANNULI_HIGH))).toBeLessThan(0.01);
  });

  it("falls continuously from clear to 0 in the umbra", () => {
    // A Moon from clear of Earth's penumbra into its umbra: the eclipse falls to 0 with no step,
    // at the exact 1 or the exact 0.
    let previous = 1;
    for (let axisKm = 8_500; axisKm >= 0; axisKm -= 50) {
      const { body, occluder } = shadowScene(
        AU_M,
        1.7374e6,
        6.371e6 / 1.7374e6,
        3.844e8,
        axisKm * 1e3 + 1.7374e6,
      );
      const visible = discEclipseVisible(
        SUN,
        body,
        [occluder],
        towardsAt(0),
        1,
        DISC_ANNULI_HIGH,
      )[1];
      expect(visible).toBeLessThanOrEqual(previous);
      expect(previous - visible).toBeLessThan(0.05);
      previous = visible;
    }
    expect(previous).toBe(0);
  });
});
