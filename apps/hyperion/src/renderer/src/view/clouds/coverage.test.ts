import { describe, expect, it } from "vitest";

import {
  areaMeanCoverage,
  calibrateOffset,
  cellLatitudeRad,
  type CoverageInput,
  coverageField,
  FLOW_MAP_PERIOD_S,
  flowMapPhases,
  marineStratus,
  PROVISIONAL_ZONAL_CLIMATE,
  provisionalZonalWindMps,
  pseudoHumidity,
  rowSolidAngleShare,
  sundqvistFraction,
  type SurveyedCell,
  timeRateFade,
  zonalAdvection,
} from "./coverage";

const EQUINOX = { subsolarLatitudeRad: 0 };
const DEG = Math.PI / 180;

/** A universe time of whole `seconds` and `nanos`. */
function at(seconds: number, nanos = 0): { seconds: number; nanos: number } {
  return { seconds, nanos };
}

const zonalInput = (season = EQUINOX): CoverageInput => ({
  zonal: PROVISIONAL_ZONAL_CLIMATE,
  surveyed: null,
  season,
  widthCells: 32,
  heightCells: 90,
});

/** A whole-sphere survey of its own climate: wetter to the east, with a cold sea in one quadrant. */
function wholeSurvey(latitudeRad: number, longitudeRad: number): SurveyedCell {
  return {
    weight: 1,
    precipitationZ: Math.sin(longitudeRad) + Math.cos(3 * latitudeRad),
    seaColdAnomalyK: longitudeRad > 0 && latitudeRad < 0 ? 4 : null,
  };
}

describe("the closure", () => {
  it("forms no cloud at or below the critical humidity, and covers all at saturation", () => {
    expect(sundqvistFraction(0.8)).toBe(0);
    expect(sundqvistFraction(0.5)).toBe(0);
    expect(sundqvistFraction(1)).toBe(1);
    // 1 − √(0.05 ÷ 0.2) = 0.5.
    expect(sundqvistFraction(0.95)).toBeCloseTo(0.5, 12);
  });

  it("raises the pseudo-humidity with the precipitation and lowers it with the offset", () => {
    expect(pseudoHumidity(1, 0)).toBeGreaterThan(pseudoHumidity(0, 0));
    expect(pseudoHumidity(0, 1)).toBeLessThan(pseudoHumidity(0, 0));
    expect(pseudoHumidity(2, 2)).toBeCloseTo(0.5, 15);
  });

  it("adds marine cloud only over a sea colder than its band, saturating at full cover", () => {
    expect(marineStratus(-3)).toBe(0);
    expect(marineStratus(5)).toBeCloseTo(0.285, 12);
    expect(marineStratus(100)).toBe(1);
  });
});

describe("the coverage field", () => {
  it("covers the sphere's area at plan 14's fraction for a fully surveyed field", () => {
    const input: CoverageInput = { ...zonalInput(), surveyed: wholeSurvey };
    for (const fraction of [0.1, 0.4, 0.67, 0.95]) {
      const field = coverageField(input, calibrateOffset(input, fraction));
      expect(Math.abs(areaMeanCoverage(field) - fraction)).toBeLessThan(1e-4);
    }
  });

  it("stops at the marine cloud's own cover where the fraction lies below it", () => {
    // The cold quadrant's 0.228 over a quarter of the sphere is about 0.057 by itself.
    const input: CoverageInput = { ...zonalInput(), surveyed: wholeSurvey };
    const field = coverageField(input, calibrateOffset(input, 0.02));
    expect(areaMeanCoverage(field)).toBeGreaterThan(0.05);
    expect(areaMeanCoverage(field)).toBeLessThan(0.06);
  });

  it("weights the cells by their true solid angle", () => {
    let total = 0;
    for (let j = 0; j < 90; j += 1) {
      total += rowSolidAngleShare(j, 90);
    }
    expect(total).toBeCloseTo(1, 14);
    // A polar row holds far less of the sphere than an equatorial one.
    expect(rowSolidAngleShare(0, 90)).toBeLessThan(0.02 * rowSolidAngleShare(45, 90));
  });

  it("changes no coverage outside the blend band when further cells are surveyed", () => {
    const north = (latitudeRad: number, longitudeRad: number): SurveyedCell | null =>
      latitudeRad > 30 * DEG ? wholeSurvey(latitudeRad, longitudeRad) : null;
    const wider = (latitudeRad: number, longitudeRad: number): SurveyedCell | null =>
      latitudeRad > -10 * DEG ? wholeSurvey(latitudeRad, longitudeRad) : null;
    // The offset is solved once, on the zonal field held before any survey (Design note 8).
    const offset = calibrateOffset(zonalInput(), 0.67);
    const before = coverageField({ ...zonalInput(), surveyed: north }, offset);
    const after = coverageField({ ...zonalInput(), surveyed: wider }, offset);
    const moved: number[] = [];
    for (let j = 0; j < 90; j += 1) {
      const latitude = cellLatitudeRad(j, 90);
      const outside = latitude > 30 * DEG || latitude <= -10 * DEG;
      for (let i = 0; i < 32; i += 1) {
        if (outside && after.fractions[j * 32 + i] !== before.fractions[j * 32 + i]) {
          moved.push(j * 32 + i);
        }
      }
    }
    expect(moved).toEqual([]);
  });

  it("refuses a fraction whose clouds are not this plan's", () => {
    expect(() => calibrateOffset(zonalInput(), 0)).toThrow(RangeError);
    expect(() => calibrateOffset(zonalInput(), 1)).toThrow(RangeError);
  });

  it("gives an Earth-like world ISCCP's zonal shape: the rain band, the dry belts, the storm tracks", () => {
    const input = zonalInput();
    const field = coverageField(input, calibrateOffset(input, 0.67));
    const atLatitude = (deg: number): number => {
      const row = Math.round((90 - deg) / 2 - 0.5);
      return field.fractions[row * 32] ?? Number.NaN;
    };
    expect(atLatitude(1)).toBeGreaterThan(atLatitude(25));
    expect(atLatitude(51)).toBeGreaterThan(atLatitude(25));
    expect(atLatitude(-51)).toBeGreaterThan(atLatitude(-25));
    // The minima lie in the dry belts, between 20° and 30°.
    let least = Infinity;
    let leastAt = 0;
    for (let deg = 1; deg < 45; deg += 2) {
      if (atLatitude(deg) < least) {
        least = atLatitude(deg);
        leastAt = deg;
      }
    }
    expect(leastAt).toBeGreaterThanOrEqual(19);
    expect(leastAt).toBeLessThanOrEqual(31);
  });

  it("moves the rain band towards the summer hemisphere", () => {
    const solstice = { subsolarLatitudeRad: 23.4 * DEG };
    const climate = PROVISIONAL_ZONAL_CLIMATE(solstice);
    expect(climate(9 * DEG)).toBeGreaterThan(climate(-9 * DEG));
  });

  it("is the same bit for bit whatever was computed before it", () => {
    const input = zonalInput({ subsolarLatitudeRad: 10 * DEG });
    const first = coverageField(input, calibrateOffset(input, 0.5));
    coverageField(zonalInput({ subsolarLatitudeRad: -20 * DEG }), 1.5);
    const again = coverageField(input, calibrateOffset(input, 0.5));
    expect(again.offset).toBe(first.offset);
    expect(again.fractions).toEqual(first.fractions);
  });
});

describe("the flow map", () => {
  it("is a rotation of the sphere within a phase under a wind of uniform angular rate", () => {
    const omega = 2e-6;
    const t = 5_000;
    for (const point of [
      { x: 6.4e6, y: 0, z: 0 },
      { x: 3e6, y: -2e6, z: 5e6 },
      { x: -1e6, y: 4e6, z: -5e6 },
    ]) {
      const wind = omega * Math.hypot(point.x, point.y);
      const moved = zonalAdvection(point, wind, t);
      const angle = -omega * t;
      expect(moved.x).toBeCloseTo(Math.cos(angle) * point.x - Math.sin(angle) * point.y, 6);
      expect(moved.y).toBeCloseTo(Math.sin(angle) * point.x + Math.cos(angle) * point.y, 6);
      expect(moved.z).toBe(point.z);
    }
  });

  it("keeps a point on the axis where it is", () => {
    expect(zonalAdvection({ x: 0, y: 0, z: 6e6 }, 10, 1_000)).toEqual({ x: 0, y: 0, z: 6e6 });
  });

  it("weighs its phases to 1, each weightless where it restarts", () => {
    for (let t = 0; t < 2 * FLOW_MAP_PERIOD_S; t += 997) {
      const [a, b] = flowMapPhases(at(t), FLOW_MAP_PERIOD_S);
      expect(a.weight + b.weight).toBeCloseTo(1, 14);
    }
    const before = flowMapPhases(at(FLOW_MAP_PERIOD_S - 1, 999_999_999), FLOW_MAP_PERIOD_S);
    const after = flowMapPhases(at(FLOW_MAP_PERIOD_S), FLOW_MAP_PERIOD_S);
    // Phase 0 restarts at the period: its weight is 0 on both sides, phase 1's 1 and continuous.
    expect(before[0].weight).toBeLessThan(1e-9);
    expect(after[0].weight).toBe(0);
    expect(after[0].cycle).toBe(before[0].cycle + 1);
    expect(Math.abs(after[1].weight - before[1].weight)).toBeLessThan(1e-9);
    expect(Math.abs(after[1].advectionS - before[1].advectionS)).toBeLessThan(1e-5);
  });

  it("keeps a millisecond's step at the far end of the clock's window", () => {
    const far = 8e12;
    const [a] = flowMapPhases(at(far, 0), FLOW_MAP_PERIOD_S);
    const [b] = flowMapPhases(at(far, 1_000_000), FLOW_MAP_PERIOD_S);
    expect(b.advectionS - a.advectionS).toBeCloseTo(1e-3, 9);
  });

  it("is a function of the scene's time alone", () => {
    const t = at(123_456, 750_000_000);
    expect(flowMapPhases(t, FLOW_MAP_PERIOD_S)).toEqual(flowMapPhases(t, FLOW_MAP_PERIOD_S));
    const [a] = flowMapPhases(t, FLOW_MAP_PERIOD_S);
    expect(a.advectionS).toBeGreaterThanOrEqual(-FLOW_MAP_PERIOD_S / 2);
    expect(a.advectionS).toBeLessThan(FLOW_MAP_PERIOD_S / 2);
  });

  it("counts its cycles before the epoch as after it", () => {
    const [a] = flowMapPhases(at(-1, 500_000_000), FLOW_MAP_PERIOD_S);
    expect(a.cycle).toBe(-1);
    expect(a.advectionS).toBeCloseTo(0.5 * FLOW_MAP_PERIOD_S - 0.5, 9);
  });

  it("calms the stand-in wind at the poles and blows the trades from the east", () => {
    expect(Math.abs(provisionalZonalWindMps(Math.PI / 2, EQUINOX))).toBeLessThan(1e-12);
    expect(provisionalZonalWindMps(0, EQUINOX)).toBeLessThan(0);
    expect(provisionalZonalWindMps(45 * DEG, EQUINOX)).toBeGreaterThan(5);
  });
});

describe("the time-rate fade", () => {
  const cellM = 150_000;
  const windMps = 15;
  const named = cellM / (windMps / 3);

  it("draws the whole structure at slow rates and none above the named rate", () => {
    expect(timeRateFade(1, cellM, windMps)).toBe(1);
    expect(timeRateFade(named / 2, cellM, windMps)).toBe(1);
    expect(timeRateFade(named, cellM, windMps)).toBe(0);
    expect(timeRateFade(10 * named, cellM, windMps)).toBe(0);
    expect(timeRateFade(-10 * named, cellM, windMps)).toBe(0);
  });

  it("falls monotonically between them, and holds the structure with no wind", () => {
    let last = 1;
    for (let rate = named / 2; rate <= named; rate += named / 64) {
      const share = timeRateFade(rate, cellM, windMps);
      expect(share).toBeLessThanOrEqual(last);
      last = share;
    }
    expect(timeRateFade(1e9, cellM, 0)).toBe(1);
  });
});
