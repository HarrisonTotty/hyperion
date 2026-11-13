import { beforeAll, describe, expect, it } from "vitest";

import type { Vec3 } from "../../geometry/vec3";
import {
  fiveLineSlotMedium,
  fourAbsorberMedium,
  iceGiantMethaneBody,
  lineAbsorberMedium,
  r05EarthWithOzoneAbsorber,
  SUN_TEFF_K,
  titanClassMethaneMedium,
} from "../../test/atmosphereFixtures";
import type { Rgb } from "../photometry/toneCurve";
import { HALF_MAX } from "../sky/half";
import { EARTH_REFERENCE, EARTH_REFERENCE_CURVES, R05_EARTH_REFERENCE } from "./earth";
import type { AtmosphereMedium } from "./medium";
import { gravityRatio } from "./oblate";
import { transmittanceUvToRMu } from "./opticalDepth";
import { ABSORBER_CURVE_DEPTH_MAX, type AbsorberCurveSlots, TRANSMITTANCE_SIZE } from "./tables";
import {
  oblatePlanetTwin,
  oblateSunTransmittanceTwin,
  slotDepthTablesTwin,
  tableTransmittanceAtUnitTwin,
  type TwinTable,
  type TwinTransmittance,
  transmittanceTablesTwin,
} from "./tablesCpu";

/**
 * R08.T8.e's lever 5 in the twin (`decision-r08-t8-leans.md` §10 and §13): each light slot's
 * optical depth to the top, tabulated at the transmittance table's texels with that slot's curves
 * applied, as `slotDepths.wgsl` builds it from the stored tables. At a texel's centre the layer's
 * e^(−τ) is the curves' read there; the curves and the sum are clamped as the kernels clamp them;
 * and a binary's second star takes its own curves in its own slot's layer, which the gate, whose
 * sources are all on slot 0, never reads (§13, point 9).
 */

/** The tables' ground radius, m: WGS 84's mean radius R₁. */
const R = 6_371_008.771_4;

/** A medium with the curves its tables are built with. */
interface Absorbing {
  readonly name: string;
  readonly medium: AtmosphereMedium;
  readonly curves: AbsorberCurveSlots;
}

/** The media the layers are held on, each with one light slot. */
function absorbingMedia(): Absorbing[] {
  const four = fourAbsorberMedium();
  const line = lineAbsorberMedium();
  const five = fiveLineSlotMedium();
  return [
    { name: "the measured Earth", medium: EARTH_REFERENCE, curves: EARTH_REFERENCE_CURVES },
    { name: "four absorbers, one a pair", medium: four.medium, curves: four.curves },
    { name: "a line absorber", medium: line.medium, curves: line.curves },
    { name: "five line slots", medium: five.medium, curves: five.curves },
  ];
}

/**
 * The worst relative difference, over every texel centre, channel and light slot, between the
 * slot layer's e^(−τ) and the curves' read: |T_slot − T| ÷ T, or |T_slot| where T is 0.
 */
function worstAtTexelCentres(medium: AtmosphereMedium, tables: TwinTransmittance): number {
  const { widthTexels: w, heightTexels: h } = tables.transmittance;
  let worst = 0;
  for (let slot = 0; slot < tables.curves.length; slot += 1) {
    for (let y = 0; y < h; y += 1) {
      for (let x = 0; x < w; x += 1) {
        const u = x / (w - 1);
        const v = y / (h - 1);
        const today = tableTransmittanceAtUnitTwin(medium, tables, u, v, slot, "curves");
        const bySlot = tableTransmittanceAtUnitTwin(medium, tables, u, v, slot, "slotDepths");
        for (const c of [0, 1, 2] as const) {
          const e = today[c] > 0 ? Math.abs(bySlot[c] / today[c] - 1) : Math.abs(bySlot[c]);
          worst = Math.max(worst, Number.isNaN(e) ? Number.POSITIVE_INFINITY : e);
        }
      }
    }
  }
  return worst;
}

describe("the slot layers' texels (R08.T8.e, lever 5)", () => {
  const built = new Map<string, TwinTransmittance>();
  const media = absorbingMedia();

  beforeAll(() => {
    for (const { name, medium, curves } of media) {
      built.set(name, transmittanceTablesTwin(medium, R, curves));
    }
  }, 120_000);

  it.each(media.map((m) => [m.name, m] as const))(
    "give %s's curves' read of each slot's light at every texel centre, to 10⁻¹²",
    (name, { medium }) => {
      const tables = built.get(name);
      if (tables === undefined) {
        throw new Error(`no tables for ${name}`);
      }
      expect(worstAtTexelCentres(medium, tables)).toBeLessThan(1e-12);
    },
    60_000,
  );

  it("hold no layer for a medium with no absorber, whose stored depth is the whole", () => {
    const tables = transmittanceTablesTwin(R05_EARTH_REFERENCE, R);
    expect(slotDepthTablesTwin(R05_EARTH_REFERENCE, tables)).toEqual([]);
  }, 60_000);
});

/** `table` with its texel (0, 0) zeroed. */
function withFirstTexelZero(table: TwinTable): TwinTable {
  const zeroed = Float64Array.from(table.texels);
  zeroed.fill(0, 0, 4);
  return { ...table, texels: zeroed };
}

/**
 * `tables` with texel (0, 0) of the transmittance table set to `stored`, RGBA, and the further slots'
 * and pressure-weighted columns there 0.
 */
function withFirstTexel(
  tables: TwinTransmittance,
  stored: readonly [number, number, number, number],
): TwinTransmittance {
  const texels = Float64Array.from(tables.transmittance.texels);
  texels.set(stored, 0);
  return {
    ...tables,
    transmittance: { ...tables.transmittance, texels },
    absorberColumns: withFirstTexelZero(tables.absorberColumns),
    absorberPressureColumns: withFirstTexelZero(tables.absorberPressureColumns),
  };
}

describe("the slot layers' clamps (R08.T8.e)", () => {
  // R05's Earth's ozone as an absorber in the linear regime: a column of 10⁶ of its vertical one is
  // past the curve's last node, where every channel stands at its floor, 10⁻⁶.
  const ozone = r05EarthWithOzoneAbsorber();
  let tables: TwinTransmittance | null = null;
  const built = (): TwinTransmittance => {
    if (tables === null) {
      throw new Error("the ozone's tables are built before the tests");
    }
    return tables;
  };

  beforeAll(() => {
    tables = transmittanceTablesTwin(ozone.medium, R, ozone.curves);
  }, 120_000);

  it("hold each curve's optical depth at −ln 10⁻⁶, as every read clamps it", () => {
    const deep = withFirstTexel(built(), [1, 2, 3, 1e6]);
    const [layer] = slotDepthTablesTwin(ozone.medium, deep);
    const texel = Array.from(layer?.texels.subarray(0, 3) ?? []);
    expect(texel.map((depth, c) => depth - (c + 1))).toEqual([
      expect.closeTo(ABSORBER_CURVE_DEPTH_MAX, 12),
      expect.closeTo(ABSORBER_CURVE_DEPTH_MAX, 12),
      expect.closeTo(ABSORBER_CURVE_DEPTH_MAX, 12),
    ]);
  });

  it("hold the sum to the half float's largest, as the transmittance kernel holds every stored value", () => {
    const held = withFirstTexel(built(), [HALF_MAX - 1, HALF_MAX, 0, 1e6]);
    const [layer] = slotDepthTablesTwin(ozone.medium, held);
    expect(Array.from(layer?.texels.subarray(0, 3) ?? [])).toEqual([
      HALF_MAX,
      HALF_MAX,
      expect.closeTo(ABSORBER_CURVE_DEPTH_MAX, 12),
    ]);
  });
});

describe("a binary's second star in its own slot's layer (R08.T8.e, §13 point 9)", () => {
  // The Sun on slot 0 and an M3 dwarf of 3,300 K on slot 1 (R08.T7's pair's companion).
  const binary = fourAbsorberMedium([SUN_TEFF_K, 3_300]);
  let tables: TwinTransmittance | null = null;
  const built = (): TwinTransmittance => {
    if (tables === null) {
      throw new Error("the binary's tables are built before the tests");
    }
    return tables;
  };

  beforeAll(() => {
    tables = transmittanceTablesTwin(binary.medium, R, binary.curves);
  }, 120_000);

  it("gives slot 1 the curves' read of the second star's light at every texel centre, to 10⁻¹²", () => {
    expect(worstAtTexelCentres(binary.medium, built())).toBeLessThan(1e-12);
  }, 60_000);

  it("holds the second star's own depths in slot 1's layer, not the first's", () => {
    const [first, second] = slotDepthTablesTwin(binary.medium, built());
    let largest = 0;
    for (const [i, depth] of (second?.texels ?? []).entries()) {
      largest = Math.max(largest, Math.abs(depth - (first?.texels[i] ?? Number.NaN)));
    }
    expect(largest).toBeGreaterThan(1e-2);
  });
});

// --- The methane record (R08.T8.e; decision-r08-t8-leans.md §10 amendment 6, §13 points 1 and 10) --

/** Design note 10's floor F, a share of the image's brightest light: here the top's, 1. */
const METRIC_FLOOR = 3e-4;

/** Design note 10's 5%. */
const METRIC_SHARE = 0.05;

/** The slot read held against the curves' over a set of sun paths, by channel. */
interface MethaneRecord {
  /** The worst |T_slot − T| ÷ max(0.05 T, F): the metric's own form, passing at 1 or under. */
  readonly worst: readonly [number, number, number];
  /** The paths whose |T_slot − T| exceeds 0.05 T where T is above F. */
  readonly overAboveFloor: readonly [number, number, number];
  /** The same where T is above 20F, where the 5% binds. */
  readonly overAboveTwentyFloors: readonly [number, number, number];
  /** The sun paths read. */
  readonly paths: number;
}

/** A record of the slot read, the first of each pair, against the curves' read, the second. */
function methaneRecordOf(pairs: Iterable<readonly [Rgb, Rgb]>): MethaneRecord {
  const worst: [number, number, number] = [0, 0, 0];
  const overAboveFloor: [number, number, number] = [0, 0, 0];
  const overAboveTwentyFloors: [number, number, number] = [0, 0, 0];
  let paths = 0;
  for (const [bySlot, today] of pairs) {
    paths += 1;
    for (const c of [0, 1, 2] as const) {
      const off = Math.abs(bySlot[c] - today[c]);
      const ratio = off / Math.max(METRIC_SHARE * today[c], METRIC_FLOOR);
      worst[c] = Math.max(worst[c], Number.isNaN(ratio) ? Number.POSITIVE_INFINITY : ratio);
      if (off > METRIC_SHARE * today[c]) {
        overAboveFloor[c] += today[c] > METRIC_FLOOR ? 1 : 0;
        overAboveTwentyFloors[c] += today[c] > 20 * METRIC_FLOOR ? 1 : 0;
      }
    }
  }
  return { worst, overAboveFloor, overAboveTwentyFloors, paths };
}

/** A record as pinned: each worst to 10⁻⁵, the counts exactly. */
function asPinned(record: MethaneRecord): unknown {
  return {
    ...record,
    worst: record.worst.map((w) => expect.closeTo(w, 5)),
  };
}

/**
 * The table's sun paths between its texels on a sphere: every cell's centre, (x + ½, y + ½) in
 * texels, and every edge's midpoint, where the two reads differ most, as unit ranges.
 */
function* betweenTexels(width: number, height: number): Generator<readonly [number, number]> {
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      if (x + 1 < width && y + 1 < height) {
        yield [(x + 0.5) / (width - 1), (y + 0.5) / (height - 1)];
      }
      if (x + 1 < width) {
        yield [(x + 0.5) / (width - 1), y / (height - 1)];
      }
      if (y + 1 < height) {
        yield [x / (width - 1), (y + 0.5) / (height - 1)];
      }
    }
  }
}

describe("the methane record (R08.T8.e, §13 points 1 and 10): the slot read against the curves'", () => {
  it("holds the Titan-class methane, which takes the slot read on its one band, to Design note 10's metric", () => {
    const titan = titanClassMethaneMedium();
    const tables = transmittanceTablesTwin(titan.medium, titan.radiusM, titan.curves);
    const { widthTexels: w, heightTexels: h } = tables.transmittance;
    const record = methaneRecordOf(
      (function* pairs(): Generator<readonly [Rgb, Rgb]> {
        for (const [u, v] of betweenTexels(w, h)) {
          yield [
            tableTransmittanceAtUnitTwin(titan.medium, tables, u, v, 0, "slotDepths"),
            tableTransmittanceAtUnitTwin(titan.medium, tables, u, v, 0, "curves"),
          ];
        }
      })(),
    );
    expect(record).toEqual(asPinned(TITAN_RECORD));
  }, 120_000);

  it("records the ice giant's, whose two bands keep it on the curves' read, the slot read taken in the twin alone", () => {
    const giant = iceGiantMethaneBody();
    const planet = oblatePlanetTwin(giant.medium, giant.body, giant.curves);
    const { figure, gravity } = giant.body;
    const a = figure.equatorialRadiusM;
    const e2 = 1 - (figure.polarRadiusM / a) ** 2;
    const shell = {
      bottomRadiusM: planet.referenceRadiusM,
      topRadiusM: planet.referenceRadiusM + giant.medium.topHeightM,
    };
    const { widthTexels: w, heightTexels: h } = TRANSMITTANCE_SIZE;
    const record = methaneRecordOf(
      (function* pairs(): Generator<readonly [Rgb, Rgb]> {
        for (const latitudeDeg of [0, 15, 30, 45, 60, 75, 90]) {
          const phi = (latitudeDeg * Math.PI) / 180;
          const s = gravityRatio(gravity, phi);
          const n: Vec3 = { x: Math.cos(phi), y: 0, z: Math.sin(phi) };
          const north: Vec3 = { x: -Math.sin(phi), y: 0, z: Math.cos(phi) };
          const east: Vec3 = { x: 0, y: 1, z: 0 };
          const prime = a / Math.sqrt(1 - e2 * Math.sin(phi) ** 2);
          for (let y = 0; y + 1 < h; y += 1) {
            for (let x = 0; x + 1 < w; x += 1) {
              const { rM, mu } = transmittanceUvToRMu(
                shell,
                (x + 0.5) / (w - 1),
                (y + 0.5) / (h - 1),
              );
              const heightM = (rM - shell.bottomRadiusM) / s;
              const place: Vec3 = {
                x: (prime + heightM) * Math.cos(phi),
                y: 0,
                z: (prime * (1 - e2) + heightM) * Math.sin(phi),
              };
              const across = Math.sqrt(Math.max(1 - mu * mu, 0));
              for (const t of [north, east]) {
                const d: Vec3 = {
                  x: mu * n.x + across * t.x,
                  y: mu * n.y + across * t.y,
                  z: mu * n.z + across * t.z,
                };
                yield [
                  oblateSunTransmittanceTwin(planet, place, d, 0, "slotDepths"),
                  oblateSunTransmittanceTwin(planet, place, d, 0, "curves"),
                ];
              }
            }
          }
        }
      })(),
    );
    expect(record).toEqual(asPinned(ICE_GIANT_RECORD));
  }, 300_000);
});

/**
 * The Titan-class methane's record at its table's 48,513 paths between texels (measured
 * 2026-10-23): the interpolation of −ln T_c(u) where the curves' read interpolates u, on one band,
 * s ≡ 1. Its worst uses 0.71% of the metric's tolerance in red and 0.39% in green, and no path is
 * off by 5%.
 */
const TITAN_RECORD: MethaneRecord = {
  worst: [0.007112, 0.003851, 0],
  overAboveFloor: [0, 0, 0],
  overAboveTwentyFloors: [0, 0, 0],
  paths: 48_513,
};

/**
 * The ice giant's record at the cell centres of its table's parameterisation, at seven latitudes
 * and two azimuths, 224,910 paths (measured 2026-10-23): the interpolation and the s effect of a
 * depth divided by s after its curves, twin-only, for R08.T11 (§13, point 1); its ray march keeps the
 * curves' read. Its worst uses 17.8% of the metric's tolerance in red and 6.3% in green, and no path
 * is off by 5%.
 */
const ICE_GIANT_RECORD: MethaneRecord = {
  worst: [0.177_51, 0.063_311, 0],
  overAboveFloor: [0, 0, 0],
  overAboveTwentyFloors: [0, 0, 0],
  paths: 224_910,
};
