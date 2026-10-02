import { describe, expect, it } from "vitest";

import type { PhaseTemplateId } from "./law";
import { PHASE_TEMPLATES } from "./templates";

const RAD_PER_DEG = Math.PI / 180;

function dimmingMag(id: PhaseTemplateId, alphaDeg: number): number {
  return -2.5 * Math.log10(PHASE_TEMPLATES[id].phaseV(alphaDeg * RAD_PER_DEG));
}

const IDS = Object.keys(PHASE_TEMPLATES).filter(
  (id): id is PhaseTemplateId => id in PHASE_TEMPLATES,
);

describe("PHASE_TEMPLATES", () => {
  it.each(IDS)("normalises %s to one at opposition", (id) => {
    expect(PHASE_TEMPLATES[id].phaseV(0)).toBeCloseTo(1, 12);
  });

  it.each(IDS)("keeps %s finite and positive over its whole range", (id) => {
    const template = PHASE_TEMPLATES[id];
    expect(template.validToRad).toBeGreaterThan(0);
    expect(template.validToRad).toBeLessThanOrEqual(Math.PI);
    for (let i = 0; i <= 100; i += 1) {
      const phase = template.phaseV((i / 100) * template.validToRad);
      expect(Number.isFinite(phase)).toBe(true);
      expect(phase).toBeGreaterThan(0);
    }
  });

  it.each(IDS)("holds %s at its range's end beyond it", (id) => {
    const template = PHASE_TEMPLATES[id];
    expect(template.phaseV(template.validToRad + 0.1)).toBe(template.phaseV(template.validToRad));
  });

  it.each([
    ["venus", 163.7],
    ["jupiter", 12],
    ["saturn", 6],
  ] as const)("joins %s's two equations at %f° to within 0.01 mag", (id, joinDeg) => {
    expect(Math.abs(dimmingMag(id, joinDeg + 1e-9) - dimmingMag(id, joinDeg))).toBeLessThan(0.01);
  });

  it("reproduces the dimmings the paper states for its equations", () => {
    // Mallama and Hilton 2018: Saturn's globe dims 0.02 mag by 6.5° (§3.6), Uranus 0.021 mag at
    // 3.1° (§3.7), Neptune 0.015 mag at 1.9° (§3.8).
    expect(dimmingMag("saturn", 6.5)).toBeCloseTo(0.02, 2);
    expect(dimmingMag("uranus", 3.1)).toBeCloseTo(0.021, 3);
    expect(dimmingMag("neptune", 1.9)).toBeCloseTo(0.015, 3);
  });

  it("evaluates Mercury's sixth-order polynomial at 90°", () => {
    // Eq. 2's terms at α = 90°, summed by hand.
    const expected =
      6.328e-2 * 90 -
      1.6336e-3 * 90 ** 2 +
      3.3644e-5 * 90 ** 3 -
      3.4265e-7 * 90 ** 4 +
      1.6893e-9 * 90 ** 5 -
      3.0334e-12 * 90 ** 6;
    expect(dimmingMag("mercury", 90)).toBeCloseTo(expected, 9);
  });

  it.each([
    [0, 0],
    [30, 0.7832],
    [90, 2.6024],
    [150, 5.925],
  ] as const)("dims the Moon by Krisciunas and Schaefer's eq. 9 at %i°: %f mag", (deg, mag) => {
    expect(dimmingMag("moon", deg)).toBeCloseTo(mag, 3);
  });

  it.each(["airless-ice", "snowball"] as const)("gives %s the Moon's curve at L = 1", (id) => {
    expect(PHASE_TEMPLATES[id].lommelSeeligerShare).toBe(1);
    expect(PHASE_TEMPLATES[id].validToRad).toBe(PHASE_TEMPLATES.moon.validToRad);
    expect(dimmingMag(id, 90)).toBe(dimmingMag("moon", 90));
  });

  it("flags exactly the three stand-in templates as provisional", () => {
    const provisional = IDS.filter((id) => PHASE_TEMPLATES[id].provisional).toSorted();
    expect(provisional).toEqual(["airless-ice", "magma", "snowball"]);
    for (const id of provisional) {
      expect(PHASE_TEMPLATES[id].source).toMatch(/^PROVISIONAL/u);
    }
  });
});
