import { describe, expect, it } from "vitest";

import {
  limbShareAt,
  limbShareFromTable,
  mcewenLunarProfile,
  PHASE_F_CLAMP,
  PHASE_TABLE_SAMPLES,
  PHASE_TABLE_STEP_RAD,
  phaseFactor,
  phaseFactorFromTable,
  phaseFactorTable,
  phaseFactorTableOf,
  type PhaseTemplateId,
  type PhotometricLaw,
} from "./law";
import { phaseIntegral, shapePhase } from "./shapes";
import { PHASE_TEMPLATES } from "./templates";

const RAD_PER_DEG = Math.PI / 180;

function lawOf(template: PhaseTemplateId, lommelSeeligerShare: number): PhotometricLaw {
  return { a: [1, 1, 1], lommelSeeligerShare, template, phaseExponent: [1, 1, 1] };
}

const IDS = Object.keys(PHASE_TEMPLATES).filter(
  (id): id is PhaseTemplateId => id in PHASE_TEMPLATES,
);

describe("phaseFactor", () => {
  it.each(IDS)("is one at opposition for %s", (id) => {
    for (const share of [0, 0.5, 1]) {
      for (const f of phaseFactor(lawOf(id, share), 0)) {
        expect(f).toBeCloseTo(1, 12);
      }
    }
  });

  it.each(IDS)("never exceeds the clamp for %s", (id) => {
    for (let deg = 0; deg <= 180; deg += 0.25) {
      for (const f of phaseFactor(lawOf(id, 0), deg * RAD_PER_DEG)) {
        expect(f).toBeLessThanOrEqual(PHASE_F_CLAMP);
        expect(f).toBeGreaterThan(0);
      }
    }
  });

  it("clamps Venus's crescent at 170°, where the unclamped factor would be some 61", () => {
    const venus = lawOf("venus", 0);
    const alpha = 170 * RAD_PER_DEG;
    const unclamped = PHASE_TEMPLATES.venus.phaseV(alpha) / shapePhase(0, alpha);
    expect(unclamped).toBeGreaterThan(50);
    expect(phaseFactor(venus, alpha)[1]).toBe(PHASE_F_CLAMP);
  });

  it("reproduces the template's curve wherever it is not clamped", () => {
    const mars = lawOf("mars", 0.5);
    for (let deg = 0; deg <= 50; deg += 5) {
      const alpha = deg * RAD_PER_DEG;
      const discIntegrated = phaseFactor(mars, alpha)[1] * shapePhase(0.5, alpha);
      expect(discIntegrated).toBeCloseTo(PHASE_TEMPLATES.mars.phaseV(alpha), 12);
    }
  });

  it.each(IDS)("holds %s's factor past its range and is continuous at its end", (id) => {
    const law = lawOf(id, 0.5);
    const end = PHASE_TEMPLATES[id].validToRad;
    const atEnd = phaseFactor(law, end)[1];
    expect(phaseFactor(law, end - 1e-9)[1]).toBeCloseTo(atEnd, 6);
    for (const beyond of [end + 1e-9, (end + Math.PI) / 2, Math.PI]) {
      expect(phaseFactor(law, beyond)[1]).toBe(atEnd);
    }
  });

  it("raises the template to each channel's own exponent", () => {
    const law: PhotometricLaw = {
      a: [1, 1, 1],
      lommelSeeligerShare: 1,
      template: "mercury",
      phaseExponent: [0.9, 1, 1.1],
    };
    const alpha = 60 * RAD_PER_DEG;
    const [r, g, b] = phaseFactor(law, alpha);
    const phaseV = PHASE_TEMPLATES.mercury.phaseV(alpha);
    const shape = shapePhase(1, alpha);
    expect(r).toBeCloseTo(phaseV ** 0.9 / shape, 12);
    expect(g).toBeCloseTo(phaseV / shape, 12);
    expect(b).toBeCloseTo(phaseV ** 1.1 / shape, 12);
    // A steeper blue curve is a redder crescent.
    expect(b).toBeLessThan(r);
  });
});

describe("phaseFactorTableOf", () => {
  it("tabulates a law once and hands back the same table", () => {
    const law = lawOf("venus", 0);
    expect(phaseFactorTableOf(law)).toBe(phaseFactorTableOf(law));
  });

  it("holds the law's own table", () => {
    const law = lawOf("saturn", 0);
    expect(phaseFactorTableOf(law).rgb).toEqual(phaseFactorTable(law).rgb);
  });

  it("keeps a table per law", () => {
    const steep: PhotometricLaw = { ...lawOf("saturn", 0), phaseExponent: [2, 2, 2] };
    const steepTable = phaseFactorTableOf(steep);
    expect(steepTable).not.toBe(phaseFactorTableOf(lawOf("saturn", 0)));
    expect(steepTable.rgb).toEqual(phaseFactorTable(steep).rgb);
  });
});

describe("phaseFactorTable", () => {
  const law = lawOf("jupiter", 0);
  const table = phaseFactorTable(law);

  it("samples every 0.5° from 0° to 180°", () => {
    expect(PHASE_TABLE_SAMPLES).toBe(361);
    expect((PHASE_TABLE_SAMPLES - 1) * PHASE_TABLE_STEP_RAD).toBeCloseTo(Math.PI, 12);
    expect(table.rgb).toHaveLength(3 * PHASE_TABLE_SAMPLES);
  });

  it("holds the exact factor at each sample to f32 precision", () => {
    for (const i of [0, 1, 24, 25, 200, 260, 360]) {
      const exact = phaseFactor(law, i * PHASE_TABLE_STEP_RAD);
      const read = phaseFactorFromTable(table, i * PHASE_TABLE_STEP_RAD);
      for (let c = 0; c < 3; c += 1) {
        expect(Math.abs((read[c] ?? 0) - (exact[c] ?? 0))).toBeLessThan(1e-6 * (exact[c] ?? 0));
      }
    }
  });

  it("interpolates linearly between samples", () => {
    const below = phaseFactorFromTable(table, 100 * PHASE_TABLE_STEP_RAD)[1];
    const above = phaseFactorFromTable(table, 101 * PHASE_TABLE_STEP_RAD)[1];
    expect(phaseFactorFromTable(table, 100.25 * PHASE_TABLE_STEP_RAD)[1]).toBeCloseTo(
      below + 0.25 * (above - below),
      12,
    );
  });

  function worstInterpolationError(of: PhotometricLaw, toDeg: number): number {
    const ofTable = phaseFactorTable(of);
    let worst = 0;
    for (let deg = 0.1; deg < toDeg; deg += 0.35) {
      const exact = phaseFactor(of, deg * RAD_PER_DEG)[1];
      const read = phaseFactorFromTable(ofTable, deg * RAD_PER_DEG)[1];
      worst = Math.max(worst, Math.abs(read - exact) / exact);
    }
    return worst;
  }

  // Linear interpolation at 0.5° errs by h² f″ ÷ 8: some 4 × 10⁻⁴ of a steep crescent's f and at
  // Jupiter's join, 2 × 10⁻³ at the corner where Venus's f meets the clamp. The shader and the
  // reference read the same table, so they share the error; the phase integral moves by 10⁻⁴.
  it.each([
    ["mercury", 1],
    ["jupiter", 0],
    ["venus", 0],
    ["mars", 0.5],
  ] as const)("follows %s's exact factor between samples to 3 × 10⁻³", (id, share) => {
    expect(worstInterpolationError(lawOf(id, share), 180)).toBeLessThan(3e-3);
  });

  it.each([
    ["mercury", 1],
    ["venus", 0],
    ["jupiter", 0],
  ] as const)(
    "keeps %s's phase integral from the table within 10⁻⁴ of the exact one",
    (id, share) => {
      const of = lawOf(id, share);
      const ofTable = phaseFactorTable(of);
      const exact = phaseIntegral((alpha) => phaseFactor(of, alpha)[1] * shapePhase(share, alpha));
      const read = phaseIntegral(
        (alpha) => phaseFactorFromTable(ofTable, alpha)[1] * shapePhase(share, alpha),
      );
      expect(Math.abs(read / exact - 1)).toBeLessThan(1e-4);
    },
  );
});

describe("McEwen's lunar L(α) (plan R10, R10.T10.b; decision-r07-t8b)", () => {
  it("is 1 at 0°, 0.6084 at 30° and 0.1859 at 90°", () => {
    expect(mcewenLunarProfile(0)).toBe(1);
    expect(mcewenLunarProfile(30 * RAD_PER_DEG)).toBeCloseTo(0.6084, 4);
    expect(mcewenLunarProfile(90 * RAD_PER_DEG)).toBeCloseTo(0.1859, 4);
  });

  it("falls to 0 at its root, 103.948°, which decision-r07-t8b's 103.9° rounds, and is held at 0 beyond", () => {
    // The cubic's one real root: it is still 7.7 × 10⁻⁴ at 103.9°.
    expect(mcewenLunarProfile(103.9 * RAD_PER_DEG)).toBeCloseTo(7.7e-4, 5);
    expect(mcewenLunarProfile(103.94 * RAD_PER_DEG)).toBeGreaterThan(0);
    for (const deg of [103.95, 104, 120, 150, 180]) {
      expect(mcewenLunarProfile(deg * RAD_PER_DEG)).toBe(0);
    }
  });

  it("falls monotonically from opposition and is NaN for a NaN phase", () => {
    let last = mcewenLunarProfile(0);
    for (let deg = 0.25; deg <= 180; deg += 0.25) {
      const l = mcewenLunarProfile(deg * RAD_PER_DEG);
      expect(l).toBeLessThanOrEqual(last);
      last = l;
    }
    expect(mcewenLunarProfile(Number.NaN)).toBeNaN();
  });

  it("scales a law's L(0): L(α) = L(0) × the profile, and L(0) at every phase without a curve", () => {
    const curved: PhotometricLaw = {
      ...lawOf("airless-ice", 0.38),
      lommelSeeligerCurve: "mcewen-1996",
    };
    const constant = lawOf("airless-ice", 0.38);
    for (const deg of [0, 30, 90, 120]) {
      const alpha = deg * RAD_PER_DEG;
      expect(limbShareAt(curved, alpha)).toBeCloseTo(0.38 * mcewenLunarProfile(alpha), 15);
      expect(limbShareAt(constant, alpha)).toBe(0.38);
    }
  });
});

describe("the table's L(α) channel (decision-r07-t8b)", () => {
  const constant = lawOf("mercury", 0.7);
  const curved: PhotometricLaw = { ...lawOf("moon", 1), lommelSeeligerCurve: "mcewen-1996" };

  it("holds a law without a curve's L at every sample, its f unchanged", () => {
    const table = phaseFactorTable(constant);
    expect(table.alpha).toHaveLength(PHASE_TABLE_SAMPLES);
    expect([...table.alpha].every((l) => l === Math.fround(0.7))).toBe(true);
    for (const i of [0, 10, 100, 250, 360]) {
      const alpha = i * PHASE_TABLE_STEP_RAD;
      // f against the constant share, as R07 built it.
      const end = Math.min(alpha, PHASE_TEMPLATES.mercury.validToRad);
      const f = Math.min(PHASE_F_CLAMP, PHASE_TEMPLATES.mercury.phaseV(end) / shapePhase(0.7, end));
      expect(table.rgb[3 * i + 1]).toBe(Math.fround(f));
    }
  });

  it("holds a curved law's L(α) at each sample's own phase, read linearly as the shader reads it", () => {
    const table = phaseFactorTable(curved);
    for (const i of [0, 60, 180, 207, 208, 300]) {
      expect(table.alpha[i]).toBe(Math.fround(mcewenLunarProfile(i * PHASE_TABLE_STEP_RAD)));
    }
    const below = table.alpha[60] ?? Number.NaN;
    const above = table.alpha[61] ?? Number.NaN;
    expect(limbShareFromTable(table, 60.25 * PHASE_TABLE_STEP_RAD)).toBeCloseTo(
      below + 0.25 * (above - below),
      12,
    );
  });

  it("tabulates a curved law's f against its L(α), so f Φ_shape(α; L(α)) stays Φ_t where unclamped", () => {
    for (let deg = 0; deg <= 150; deg += 5) {
      const alpha = deg * RAD_PER_DEG;
      const f = phaseFactor(curved, alpha)[1];
      if (f >= PHASE_F_CLAMP) {
        continue;
      }
      const shape = shapePhase(1, alpha, limbShareAt(curved, alpha));
      expect(f * shape).toBeCloseTo(PHASE_TEMPLATES.moon.phaseV(alpha), 12);
    }
  });
});
