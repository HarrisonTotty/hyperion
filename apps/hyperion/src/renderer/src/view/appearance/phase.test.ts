import { describe, expect, it } from "vitest";

import { SOLAR_SYSTEM_PHOTOMETRY } from "../../test/litFixtures";
import type { Rgb } from "../photometry/toneCurve";
import { PHASE_F_CLAMP, phaseFactor, type PhaseTemplateId, type PhotometricLaw } from "./law";
import { discIntegratedPhase, geometricAlbedo, lawFor } from "./phase";
import { phaseIntegral, shapePhase } from "./shapes";
import { PHASE_TEMPLATES } from "./templates";

const RAD_PER_DEG = Math.PI / 180;

const IDS = Object.keys(PHASE_TEMPLATES).filter(
  (id): id is PhaseTemplateId => id in PHASE_TEMPLATES,
);

/** The law's phase integral per channel, from the table as the renderer reads it. */
function lawPhaseIntegral(law: PhotometricLaw): Rgb {
  const channel = (c: 0 | 1 | 2): number =>
    phaseIntegral((alpha) => discIntegratedPhase(law, alpha)[c]);
  return [channel(0), channel(1), channel(2)];
}

/** q of the template's own curve at s = 1, held past its range with f unclamped. */
function unclampedPhaseIntegral(id: PhaseTemplateId): number {
  const { phaseV, validToRad, lommelSeeligerShare: l } = PHASE_TEMPLATES[id];
  const endShape = shapePhase(l, validToRad);
  return phaseIntegral((alpha) =>
    alpha <= validToRad
      ? phaseV(alpha)
      : endShape > 0
        ? (phaseV(validToRad) / endShape) * shapePhase(l, alpha)
        : 0,
  );
}

/** The phase integral of the template's clamped law at s = 1, from the exact f. */
function clampedPhaseIntegral(id: PhaseTemplateId): number {
  const share = PHASE_TEMPLATES[id].lommelSeeligerShare;
  const law: PhotometricLaw = {
    a: [1, 1, 1],
    lommelSeeligerShare: share,
    template: id,
    phaseExponent: [1, 1, 1],
  };
  return phaseIntegral((alpha) => phaseFactor(law, alpha)[1] * shapePhase(share, alpha));
}

/** V dimming, mag, of a law against opposition. */
function dimmingOf(law: PhotometricLaw, alphaDeg: number, channel: 0 | 1 | 2): number {
  return -2.5 * Math.log10(discIntegratedPhase(law, alphaDeg * RAD_PER_DEG)[channel]);
}

describe("discIntegratedPhase", () => {
  it.each(IDS)("is one at opposition for %s", (id) => {
    const law = lawFor([0.3, 0.3, 0.3], [1, 1, 1], id);
    for (const value of discIntegratedPhase(law, 0)) {
      expect(value).toBeCloseTo(1, 6);
    }
  });
});

describe("lawFor", () => {
  it.each(IDS)("gives %s's law the geometric albedo asked for", (id) => {
    const p: Rgb = [0.3, 0.25, 0.2];
    const law = lawFor(p, [1, 1, 1], id);
    const albedo = geometricAlbedo(law);
    for (let c = 0; c < 3; c += 1) {
      expect(albedo[c]).toBeCloseTo(p[c] ?? 0, 12);
    }
  });

  it.each(IDS)("recovers %s's own phase integral and others near it to 0.5%", (id) => {
    const own = clampedPhaseIntegral(id);
    const q: Rgb = [own * 1.05, own, own * 0.95];
    const recovered = lawPhaseIntegral(lawFor([0.3, 0.3, 0.3], q, id));
    for (let c = 0; c < 3; c += 1) {
      expect(Math.abs((recovered[c] ?? 0) / (q[c] ?? 1) - 1)).toBeLessThan(0.005);
    }
  });

  it.each(SOLAR_SYSTEM_PHOTOMETRY)("recovers $name's p and q_V to 0.5%", (planet) => {
    const p: Rgb = [planet.geometricAlbedo.r, planet.geometricAlbedo.v, planet.geometricAlbedo.b];
    const law = lawFor(p, [planet.qV, planet.qV, planet.qV], planet.template);
    expect(geometricAlbedo(law)[1]).toBeCloseTo(planet.geometricAlbedo.v, 12);
    expect(Math.abs(lawPhaseIntegral(law)[1] / planet.qV - 1)).toBeLessThan(0.005);
    expect(law.phaseExponent[1]).toBeCloseTo(1, 2);
  });

  // The clamp at f = 4 lowers q only where a Lambert crescent would need more: Venus by 0.32%,
  // Earth by 0.011% and Uranus by 0.008%; the rest never reach it. Both sides use the exact f.
  it.each([
    ["moon", 0],
    ["mercury", 0],
    ["mars", 0],
    ["venus", -0.00317],
    ["earth", -0.00011],
    ["jupiter", 0],
    ["saturn", 0],
    ["uranus", -0.00008],
    ["neptune", 0],
    ["airless-ice", 0],
    ["snowball", 0],
    ["magma", 0],
  ] as const)("states the clamp's departure in %s's q as %f", (id, departure) => {
    expect(clampedPhaseIntegral(id) / unclampedPhaseIntegral(id) - 1).toBeCloseTo(departure, 5);
  });

  // decision-phase-curves (2026-10-02): the icy analogues' measured q reached through s, and their
  // stated ratio p_V q_V ÷ A_Bond within T2.b's 5%: Ganymede (p_V 0.43, q 0.80, A 0.35; Squyres and
  // Veverka 1981) for airless ice, Europa (p 0.67, q 1.01, A 0.68; Grundy et al. 2007) scaled to
  // galaxy's snowball Bond albedo 0.50 for the snowball.
  it.each([
    ["airless-ice", 0.43, 0.8, 0.35, 0.82],
    ["snowball", 0.49, 1.01, 0.5, 0.67],
  ] as const)("reaches %s's analogue q and ratio", (id, pV, qV, bond, exponent) => {
    const law = lawFor([pV, pV, pV], [qV, qV, qV], id);
    const q = lawPhaseIntegral(law)[1];
    expect(Math.abs(q / qV - 1)).toBeLessThan(0.005);
    expect(law.phaseExponent[1]).toBeCloseTo(exponent, 2);
    expect(Math.abs((geometricAlbedo(law)[1] * q) / bond - 1)).toBeLessThan(0.05);
  });

  // q depends on L only past a template's range, where f is held while the shape still varies
  // with L (the spread over L = 0, 0.5 and 1 at s = 1 reaches 0.10 for Mars); so the law's
  // independence of L is checked inside the range, and each template's q at its own L.
  it.each(IDS)("gives Φ = Φ_t^s for any L inside %s's range, where f is not clamped", (id) => {
    const { phaseV, validToRad } = PHASE_TEMPLATES[id];
    let worst = 0;
    let checked = 0;
    for (const share of [0, 0.5, 1]) {
      const law: PhotometricLaw = {
        a: [1, 1, 1],
        lommelSeeligerShare: share,
        template: id,
        phaseExponent: [0.8, 1, 1.2],
      };
      for (let i = 0; i <= 20; i += 1) {
        const alpha = (i / 20) * validToRad;
        const shape = shapePhase(share, alpha);
        const f = phaseFactor(law, alpha);
        law.phaseExponent.forEach((s, c) => {
          const target = phaseV(alpha) ** s;
          if (shape > 0 && target / shape < PHASE_F_CLAMP) {
            worst = Math.max(worst, Math.abs((f[c] ?? 0) * shape - target));
            checked += 1;
          }
        });
      }
    }
    expect(checked).toBeGreaterThan(100);
    expect(worst).toBeLessThan(1e-12);
  });

  it.each(IDS)("solves %s's q at the template's own L", (id) => {
    const law = lawFor([0.3, 0.3, 0.3], [1, 1, 1], id);
    expect(law.lommelSeeligerShare).toBe(PHASE_TEMPLATES[id].lommelSeeligerShare);
  });

  it("measures those departures against a clamp of 4", () => {
    expect(PHASE_F_CLAMP).toBe(4);
  });

  it("takes the bracket's end for a q no exponent reaches", () => {
    const law = lawFor([0.3, 0.3, 0.3], [5, 1, 1e-6], "mercury");
    expect(law.phaseExponent[0]).toBe(1 / 16);
    expect(law.phaseExponent[2]).toBe(16);
  });

  it("reddens a crescent whose blue phase integral is the smaller", () => {
    const own = clampedPhaseIntegral("mercury");
    const mercury = SOLAR_SYSTEM_PHOTOMETRY.find((planet) => planet.name === "Mercury");
    if (mercury === undefined) {
      throw new Error("no Mercury in the fixture");
    }
    const { r, v, b } = mercury.geometricAlbedo;
    // The fixture gives q in V only; a split of ±5% stands in for Mercury's measured reddening.
    const law = lawFor([r, v, b], [own * 1.05, own, own * 0.95], "mercury");
    const bMinusVGrowth =
      dimmingOf(law, 100, 2) -
      dimmingOf(law, 100, 1) -
      (dimmingOf(law, 0, 2) - dimmingOf(law, 0, 1));
    // Design note 5: B − V grows by some 0.1–0.2 mag from opposition to about 100°.
    expect(bMinusVGrowth).toBeGreaterThan(0.1);
    expect(bMinusVGrowth).toBeLessThan(0.2);
  });
});

/** The paper's V(α) at r = Δ = 1 au, eqs. 2, 3, 5, 6 and 8–9, written out independently. */
const PAPER_V: Readonly<Partial<Record<string, (alphaDeg: number) => number>>> = {
  Mercury: (a) =>
    -0.613 +
    6.328e-2 * a -
    1.6336e-3 * a ** 2 +
    3.3644e-5 * a ** 3 -
    3.4265e-7 * a ** 4 +
    1.6893e-9 * a ** 5 -
    3.0334e-12 * a ** 6,
  Venus: (a) => -4.384 - 1.044e-3 * a + 3.687e-4 * a ** 2 - 2.814e-6 * a ** 3 + 8.938e-9 * a ** 4,
  Earth: (a) => -3.99 - 1.06e-3 * a + 2.054e-4 * a ** 2,
  Mars: (a) => -1.601 + 0.02267 * a - 0.0001302 * a ** 2,
  Saturn: (a) =>
    a <= 6
      ? -8.95 - 3.7e-4 * a + 6.16e-4 * a ** 2
      : -8.94 + 2.446e-4 * a + 2.672e-4 * a ** 2 - 1.505e-6 * a ** 3 + 4.767e-9 * a ** 4,
  Uranus: (a) => -7.11 + 6.587e-3 * a + 1.045e-4 * a ** 2,
  Neptune: (a) => -7.0 + 7.944e-3 * a + 9.617e-5 * a ** 2,
  Jupiter: (a) => {
    if (a <= 12) {
      return -9.395 - 3.7e-4 * a + 6.16e-4 * a ** 2;
    }
    const x = a / 180;
    return (
      -9.428 -
      2.5 *
        Math.log10(
          1 - 1.507 * x - 0.363 * x ** 2 - 0.062 * x ** 3 + 2.809 * x ** 4 - 1.876 * x ** 5,
        )
    );
  },
};

describe("planets' V from their laws", () => {
  it.each([
    ["Mercury", [10, 50, 90, 130, 165], 0.01],
    ["Venus", [10, 50, 90, 130, 150], 0.01],
    ["Jupiter", [5, 12, 40, 80, 120], 0.01],
    ["Mars", [5, 20, 35, 45], 0.01],
    ["Saturn", [3, 6, 40, 100, 140], 0.01],
    ["Uranus", [3, 50, 100, 150], 0.01],
    ["Neptune", [2, 50, 100, 130], 0.01],
  ] as const)(
    "matches %s's equation in V at the stated phases",
    (name, phasesDeg, toleranceMag) => {
      const planet = SOLAR_SYSTEM_PHOTOMETRY.find((candidate) => candidate.name === name);
      const paper = PAPER_V[name];
      if (planet === undefined || paper === undefined) {
        throw new Error(`no fixture or equation for ${name}`);
      }
      const p: Rgb = [planet.geometricAlbedo.r, planet.geometricAlbedo.v, planet.geometricAlbedo.b];
      const law = lawFor(p, [planet.qV, planet.qV, planet.qV], planet.template);
      for (const alphaDeg of phasesDeg) {
        const modelled = planet.templateV10Mag + dimmingOf(law, alphaDeg, 1);
        expect(Math.abs(modelled - paper(alphaDeg))).toBeLessThan(toleranceMag);
      }
    },
  );

  it("matches Earth's equation in V flux to 30%", () => {
    const earth = SOLAR_SYSTEM_PHOTOMETRY.find((candidate) => candidate.name === "Earth");
    const paper = PAPER_V["Earth"];
    if (earth === undefined || paper === undefined) {
      throw new Error("no fixture or equation for Earth");
    }
    const { r, v, b } = earth.geometricAlbedo;
    const law = lawFor([r, v, b], [earth.qV, earth.qV, earth.qV], "earth");
    for (const alphaDeg of [10, 60, 110, 150]) {
      const modelled = earth.templateV10Mag + dimmingOf(law, alphaDeg, 1);
      expect(Math.abs(10 ** (-0.4 * (modelled - paper(alphaDeg))) - 1)).toBeLessThan(0.3);
    }
  });
});
