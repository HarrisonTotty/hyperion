/**
 * R08.T13.c2's boundary and router (plan R08, R08.T13; `decision-r08-t13-scope.md`, ruling 9 and
 * "#107's floor misses"): `THICK_MS_BOUNDARY`'s shape and read, the similarity and the mean cosines
 * the read takes, the two lines, the deck rule, the kept bodies' classes as measured, the collapse
 * and the T4.a depth note. The table's equality to the sweep's recompute is
 * `reference/sweepBoundary.test.ts`'s.
 */
import { describe, expect, it } from "vitest";

import {
  henyeyGreensteinEntries,
  SWEEP_ASYMMETRIES,
  SWEEP_SCAN_GROUND_ALBEDOS,
  SWEEP_SINGLE_SCATTERING_ALBEDOS,
  SWEEP_TAU_CANDIDATES,
  sweepMedium,
} from "../../../test/atmosphereSweep";
import { venusClassMedium } from "../../../test/atmosphereFixtures";
import { gaussLegendre } from "../../lighting/quadrature";
import type { Rgb } from "../../photometry/toneCurve";
import { phaseTableAsymmetry } from "../aerosol";
import { EARTH_REFERENCE, EARTH_SLAB_REFERENCE } from "../earth";
import { forwardPeakEntry } from "../forwardPeak";
import { type AtmosphereMedium, type MediumTerm, phaseAt, phaseTable } from "../medium";
import { MARS_REFERENCE, SATURN_OBLATE_REFERENCE } from "../reference/cases";
import { PHASE_TABLE_U } from "../sizeDistribution";
import { withoutForwardDelta } from "../tables";
import {
  BOUNDARY_FLOOR_OPTICAL_DEPTH,
  type BodyCover,
  CLEAR_COVER,
  type CoverDeck,
  CLOUD_DECK_SPLIT_OPTICAL_DEPTH,
  classifyRegime,
  cornetteShanksMeanCosine,
  envelopeFloorM,
  readThickBoundary,
  regimeChannels,
  THICK_MS_BOUNDARY,
  type ThickBoundary,
  verticalOpticalDepth,
} from "./regime";

/** The boundary's cell (ω row, g column). */
function cellOf(row: number, column: number): { tauStar: number; albedoStar: number } {
  const at = row * THICK_MS_BOUNDARY.g.length + column;
  return {
    tauStar: THICK_MS_BOUNDARY.tauStar[at] ?? Number.NaN,
    albedoStar: THICK_MS_BOUNDARY.albedoStar[at] ?? Number.NaN,
  };
}

/** A grey exponential term of scale height `scaleHeightM`, `phase` its phase. */
function greyTerm(
  name: string,
  scaleHeightM: number,
  scatteringPerM: number,
  absorptionPerM: number,
  phase: MediumTerm["phase"],
): MediumTerm {
  return {
    name,
    density: { kind: "exponential", scaleHeightM },
    scattering: [scatteringPerM, scatteringPerM, scatteringPerM],
    absorption: [absorptionPerM, absorptionPerM, absorptionPerM],
    phase,
  };
}

/** A medium of `terms` to 100 km over a grey ground. */
function mediumOf(terms: ReadonlyArray<MediumTerm>, groundAlbedo = 0): AtmosphereMedium {
  return {
    name: "regime test",
    topHeightM: 100_000,
    groundAlbedo: [groundAlbedo, groundAlbedo, groundAlbedo],
    terms,
  };
}

/** A medium with every coefficient times `factor`. */
function scaledMedium(medium: AtmosphereMedium, factor: number): AtmosphereMedium {
  const times = (rgb: Rgb): Rgb => [rgb[0] * factor, rgb[1] * factor, rgb[2] * factor];
  return {
    ...medium,
    terms: medium.terms.map((t) => ({
      ...t,
      scattering: times(t.scattering),
      absorption: times(t.absorption),
    })),
  };
}

/** A Henyey–Greenstein table of `g` with a forward delta of share `share` on its first entries. */
function tableWithDelta(g: number, share: number): MediumTerm["phase"] {
  const hg = henyeyGreensteinEntries(g);
  const values = hg.map((v, k) => v * (1 - share) + forwardPeakEntry(share, k));
  return {
    kind: "tabulated",
    table: phaseTable(PHASE_TABLE_U, [values, values, values], undefined, [share, share, share]),
  };
}

describe("THICK_MS_BOUNDARY", () => {
  it("holds the sweep's 7 × 7 grid, ω descending from 1 and g ascending from 0", () => {
    expect([Array.from(THICK_MS_BOUNDARY.omega), Array.from(THICK_MS_BOUNDARY.g)]).toEqual([
      [...SWEEP_SINGLE_SCATTERING_ALBEDOS],
      [...SWEEP_ASYMMETRIES],
    ]);
  });

  it("holds 49 cells of τ*, A* and kind", () => {
    expect([
      THICK_MS_BOUNDARY.tauStar.length,
      THICK_MS_BOUNDARY.albedoStar.length,
      THICK_MS_BOUNDARY.kind.length,
    ]).toEqual([49, 49, 49]);
  });

  it("holds every τ* finite, positive and a traced τ of the ladder", () => {
    const ladder = new Set(SWEEP_TAU_CANDIDATES.map((c) => c.tau));
    const off = Array.from(THICK_MS_BOUNDARY.tauStar).filter(
      (t) => !(Number.isFinite(t) && t > 0 && ladder.has(t)),
    );
    expect(off).toEqual([]);
  });

  it("holds a measured drift in every cell", () => {
    expect(THICK_MS_BOUNDARY.kind.filter((k) => k !== "drift")).toEqual([]);
  });

  it("holds every A* at one of the floor scan's grounds or 1", () => {
    const grounds = new Set([...SWEEP_SCAN_GROUND_ALBEDOS, 1]);
    expect(Array.from(THICK_MS_BOUNDARY.albedoStar).filter((a) => !grounds.has(a))).toEqual([]);
  });
});

describe("the boundary's read", () => {
  it("returns each grid node's τ* and A*", () => {
    const misses: string[] = [];
    for (const [row, omega] of THICK_MS_BOUNDARY.omega.entries()) {
      for (const [column, g] of THICK_MS_BOUNDARY.g.entries()) {
        const read = readThickBoundary(omega, g);
        const cell = cellOf(row, column);
        if (
          Math.abs(read.tauStar / cell.tauStar - 1) > 1e-14 ||
          Math.abs(read.albedoStar - cell.albedoStar) > 1e-15
        ) {
          misses.push(`(${omega}, ${g})`);
        }
      }
    }
    expect(misses).toEqual([]);
  });

  it("is bilinear in ln τ* and in A* between the nodes", () => {
    // ω 0.85 is halfway between the rows 0.9 and 0.8, and g 0.375 three eighths of the way from
    // the column 0.3 to 0.5.
    const s = 0.5;
    const t = (0.375 - 0.3) / (0.5 - 0.3);
    const corners = [cellOf(3, 1), cellOf(3, 2), cellOf(4, 1), cellOf(4, 2)] as const;
    const weights = [(1 - s) * (1 - t), (1 - s) * t, s * (1 - t), s * t];
    const lnTau = corners.reduce((sum, c, i) => sum + (weights[i] ?? 0) * Math.log(c.tauStar), 0);
    const albedo = corners.reduce((sum, c, i) => sum + (weights[i] ?? 0) * c.albedoStar, 0);
    const read = readThickBoundary(0.85, 0.375);
    expect(
      [read.tauStar / Math.exp(lnTau), read.albedoStar - albedo].map((x) => x.toFixed(12)),
    ).toEqual([(1).toFixed(12), (0).toFixed(12)]);
  });

  it("reads ω̄ below 0.4 at the 0.4 row, and says so", () => {
    const below = readThickBoundary(0.2, 0.6);
    const row = readThickBoundary(0.4, 0.6);
    expect([below.tauStar, below.albedoStar, below.albedoBelowGrid]).toEqual([
      row.tauStar,
      row.albedoStar,
      true,
    ]);
  });

  it("reads ḡ past 0.9 at 0.9, and says so", () => {
    const past = readThickBoundary(0.97, 0.95);
    const edge = readThickBoundary(0.97, 0.9);
    expect([past.tauStar, past.albedoStar, past.asymmetryOffGrid, edge.asymmetryOffGrid]).toEqual([
      edge.tauStar,
      edge.albedoStar,
      true,
      false,
    ]);
  });

  it("reads ḡ below 0 at 0, and says so", () => {
    const below = readThickBoundary(0.97, -0.2);
    const edge = readThickBoundary(0.97, 0);
    expect([below.tauStar, below.albedoStar, below.asymmetryOffGrid]).toEqual([
      edge.tauStar,
      edge.albedoStar,
      true,
    ]);
  });

  it("reads a neighbour of a τ* of 0 without a fault, the zero-weight corner unread", () => {
    const stopped: ThickBoundary = {
      ...THICK_MS_BOUNDARY,
      tauStar: THICK_MS_BOUNDARY.tauStar.map((t, i) => (i === 1 ? 0 : t)),
    };
    expect(readThickBoundary(1, 0, stopped).tauStar).toBe(readThickBoundary(1, 0).tauStar);
  });

  it("reads ω̄ 1 at the ω 1 row, to the exponential's rounding", () => {
    const read = readThickBoundary(1, 0.3);
    expect([(read.tauStar / cellOf(0, 1).tauStar).toFixed(14), read.albedoStar]).toEqual([
      (1).toFixed(14),
      cellOf(0, 1).albedoStar,
    ]);
  });
});

describe("a channel's lines", () => {
  /** A boundary of τ* 100 and A* 1 everywhere: only the depth past 8 can make a channel thick. */
  const DEEP: ThickBoundary = {
    ...THICK_MS_BOUNDARY,
    tauStar: THICK_MS_BOUNDARY.tauStar.map(() => 100),
    albedoStar: THICK_MS_BOUNDARY.albedoStar.map(() => 1),
  };
  const rayleigh = (tau: number): AtmosphereMedium =>
    mediumOf([
      greyTerm("rayleigh", 8_000, tau / (8_000 * -Math.expm1(-100_000 / 8_000)), 0, {
        kind: "rayleigh",
        depolarisation: [0, 0, 0],
      }),
    ]);

  it("takes a channel past τ 8 as thick, whatever its τ*", () => {
    expect([
      regimeChannels(rayleigh(9), DEEP)[1].thickByDepth,
      regimeChannels(rayleigh(7), DEEP)[1].thickByDepth,
    ]).toEqual([true, false]);
  });

  it("takes a channel of τ 0 as thin, without a fault", () => {
    const empty = mediumOf([greyTerm("nothing", 8_000, 0, 0, { kind: "none" })], 1);
    const [red] = regimeChannels(empty);
    expect([
      classifyRegime(empty, CLEAR_COVER).kind,
      red.opticalDepth,
      red.singleScatteringAlbedo,
      Number.isFinite(red.boundary.tauStar),
    ]).toEqual(["thin", 0, 1, true]);
  });

  it("leaves the absorbers out, as the provisional line does", () => {
    const ozone = EARTH_REFERENCE.terms.filter((t) => t.absorber !== undefined);
    const without = {
      ...EARTH_REFERENCE,
      terms: EARTH_REFERENCE.terms.filter((t) => t.absorber === undefined),
    };
    expect([ozone.length, regimeChannels(EARTH_REFERENCE)[2].opticalDepth]).toEqual([
      1,
      regimeChannels(without)[2].opticalDepth,
    ]);
  });
});

/** A medium with every term's forward delta taken out, as the kernels draw it. */
function deltaFree(medium: AtmosphereMedium): AtmosphereMedium {
  return { ...medium, terms: medium.terms.map(withoutForwardDelta) };
}

/** The sweep's g 0.75 column at ω 1, at τ over a grey ground: τ* √2/16 and A* 0.02 there. */
function groundLinePoint(tau: number, groundAlbedo: number): AtmosphereMedium {
  return sweepMedium({ singleScatteringAlbedo: 1, asymmetry: 0.75, tau, groundAlbedo });
}

/** A deck of a cover. */
function coverDeck(
  opticalDepth550: number,
  baseAltitudeM: number,
  topAltitudeM: number,
): CoverDeck {
  return { opticalDepth550, baseAltitudeM, topAltitudeM };
}

/** A cover over a ground, of `decks` under `cloudFraction`. */
function deckCover(cloudFraction: number, decks: ReadonlyArray<CoverDeck>): BodyCover {
  return { cloudFraction, decks, envelope: false };
}

/** Each channel's lines: the depth line's, then the ground line's, `-` for neither. */
function linesOf(medium: AtmosphereMedium): string[] {
  return regimeChannels(medium).map(
    (c) => `${c.thickByDepth ? "depth" : "-"}/${c.thickByGround ? "ground" : "-"}`,
  );
}

describe("the similarity and the mean cosines", () => {
  const share = 0.3;
  const withDelta = (tau: number): AtmosphereMedium => {
    const beta = tau / (1_200 * -Math.expm1(-100_000 / 1_200));
    return mediumOf([
      greyTerm("aerosol", 1_200, 0.95 * beta, 0.05 * beta, tableWithDelta(0.6, share)),
    ]);
  };

  it("reads a forward delta's term as withoutForwardDelta's term, τ, ω̄ and ḡ", () => {
    const medium = withDelta(0.5);
    const mine = regimeChannels(medium)[1];
    const theirs = regimeChannels(deltaFree(medium))[1];
    expect(
      [
        mine.opticalDepth / theirs.opticalDepth - 1,
        mine.singleScatteringAlbedo - theirs.singleScatteringAlbedo,
        mine.asymmetry - theirs.asymmetry,
      ].map((x) => Math.abs(x) < 1e-6),
    ).toEqual([true, true, true]);
  });

  it("takes the delta's share out of the column, f τ_s", () => {
    const medium = withDelta(0.5);
    const whole = verticalOpticalDepth(medium)[1];
    expect((whole - regimeChannels(medium)[1].opticalDepth) / (0.95 * whole)).toBeCloseTo(
      share,
      12,
    );
  });

  it("classifies a forward delta's term as its delta-free twin does over a ladder of depths", () => {
    const depths = [0.02, 0.04, 0.06, 0.08, 0.1, 0.15, 0.2, 0.4, 1];
    const kinds = depths.map((tau) => {
      const medium = withDelta(tau);
      return [
        classifyRegime(medium, CLEAR_COVER).kind,
        classifyRegime(deltaFree(medium), CLEAR_COVER).kind,
      ];
    });
    expect([kinds.every(([a, b]) => a === b), new Set(kinds.map(([a]) => a)).size]).toEqual([
      true,
      2,
    ]);
  });

  it("holds a table's ḡ to phaseTableAsymmetry", () => {
    const entries = henyeyGreensteinEntries(0.7);
    const table = phaseTable(PHASE_TABLE_U, [entries, entries, entries]);
    const medium = mediumOf([greyTerm("aerosol", 1_200, 1e-4, 0, { kind: "tabulated", table })]);
    expect(regimeChannels(medium)[1].asymmetry - phaseTableAsymmetry(table, 1)).toBeCloseTo(0, 14);
  });

  it("takes Cornette and Shanks's own mean cosine, by quadrature of phaseAt, not its parameter", () => {
    const rule = gaussLegendre(96);
    const moment = (g: number, weight: (mu: number) => number): number =>
      Array.from(rule.x).reduce(
        (sum, mu, i) =>
          sum +
          2 *
            Math.PI *
            (rule.w[i] ?? 0) *
            phaseAt({ kind: "cornette-shanks", asymmetry: g }, mu)[0] *
            weight(mu),
        0,
      );
    const misses = [0.2, 0.584, 0.7, 0.85].filter((g) => {
      const normalised = moment(g, (mu) => mu) / moment(g, () => 1);
      return Math.abs(normalised - cornetteShanksMeanCosine(g)) > 1e-9;
    });
    expect([misses, cornetteShanksMeanCosine(0.584).toFixed(3)]).toEqual([[], "0.650"]);
  });
});

describe("the two-layer media", () => {
  /** Rayleigh over 8 km and a g 0.7 aerosol over 1.2 km, its scattering 57% of the column's. */
  const twoLayer = mediumOf([
    greyTerm("rayleigh", 8_000, 2e-5, 0, { kind: "rayleigh", depolarisation: [0, 0, 0] }),
    greyTerm("aerosol", 1_200, 1.75e-4, 0.9e-5, {
      kind: "tabulated",
      table: phaseTable(PHASE_TABLE_U, [
        henyeyGreensteinEntries(0.7),
        henyeyGreensteinEntries(0.7),
        henyeyGreensteinEntries(0.7),
      ]),
    }),
  ]);
  const [, green] = regimeChannels(twoLayer);
  const atLine = green.boundary.tauStar / green.opticalDepth;

  it("reads the pair between the grid's rows and columns, on the black ground", () => {
    expect([
      green.singleScatteringAlbedo > 0.95 && green.singleScatteringAlbedo < 0.99,
      green.asymmetry > 0.3 && green.asymmetry < 0.5,
      green.groundAlbedo,
    ]).toEqual([true, true, 0]);
  });

  it("classifies the pair just under the boundary its ω̄ and ḡ read as thin", () => {
    expect(classifyRegime(scaledMedium(twoLayer, atLine * 0.99), CLEAR_COVER).kind).toBe("thin");
  });

  it("classifies the pair just over it as thickScattering", () => {
    expect(classifyRegime(scaledMedium(twoLayer, atLine * 1.01), CLEAR_COVER).kind).toBe(
      "thickScattering",
    );
  });
});

describe("the ground line", () => {
  // τ 0.06 lies between 1/32 and the g 0.75 column's τ* √2/16.
  const point = groundLinePoint;

  it("keeps a g 0.75 cell's medium between 1/32 and its τ* thin over a ground of 0.01", () => {
    expect(classifyRegime(point(0.06, 0.01), CLEAR_COVER).kind).toBe("thin");
  });

  it("routes the same medium over a ground of 0.3 thick by the ground line", () => {
    const [, green] = regimeChannels(point(0.06, 0.3));
    expect([
      classifyRegime(point(0.06, 0.3), CLEAR_COVER).kind,
      green.thickByDepth,
      green.thickByGround,
    ]).toEqual(["thickScattering", false, true]);
  });

  it("keeps the medium below τ 1/32 thin over a ground of 0.3", () => {
    expect([
      classifyRegime(point(BOUNDARY_FLOOR_OPTICAL_DEPTH * 0.9, 0.3), CLEAR_COVER).kind,
      classifyRegime(point(BOUNDARY_FLOOR_OPTICAL_DEPTH * 1.01, 0.3), CLEAR_COVER).kind,
    ]).toEqual(["thin", "thickScattering"]);
  });
});

describe("the deck rule", () => {
  const venus = venusClassMedium().medium;
  const deck = coverDeck;
  const cover = deckCover;

  it("does not split a body at a τ 30 deck under a cloud fraction of 0.6", () => {
    expect(classifyRegime(venus, cover(0.6, [deck(30, 48_000, 70_000)])).kind).not.toBe(
      "cloudDeck",
    );
  });

  it("splits a body at a τ 30 deck under a cloud fraction of 1, at the deck's top", () => {
    expect(classifyRegime(venus, cover(1, [deck(30, 48_000, 70_000)]))).toEqual({
      kind: "cloudDeck",
      deckAltitudeM: 70_000,
    });
  });

  it("splits at the shallowest deck past τ 10, though the inventory lists the deeper first", () => {
    const decks = [deck(3_000, 10_000, 15_000), deck(30, 48_000, 70_000), deck(5, 80_000, 82_000)];
    expect(classifyRegime(venus, cover(1, decks))).toEqual({
      kind: "cloudDeck",
      deckAltitudeM: 70_000,
    });
  });

  it("does not split at a deck of exactly τ 10", () => {
    expect(
      classifyRegime(venus, cover(1, [deck(CLOUD_DECK_SPLIT_OPTICAL_DEPTH, 48_000, 70_000)])).kind,
    ).toBe("thickScattering");
  });

  describe("a gas envelope", () => {
    // A synthetic envelope: one grey gas, e^(−h ÷ 20 km) to 400 km, τ 40 in all, so that τ from
    // the top is 40 (e^(−h ÷ H) − e^(−20)) ÷ (1 − e^(−20)), which reaches 10 at h = −H ln(¼ + …).
    const scaleHeightM = 20_000;
    const topM = 400_000;
    const columnM = scaleHeightM * -Math.expm1(-topM / scaleHeightM);
    const envelope: AtmosphereMedium = {
      name: "synthetic envelope",
      topHeightM: topM,
      groundAlbedo: [0, 0, 0],
      terms: [
        greyTerm("gas", scaleHeightM, 40 / columnM, 0, {
          kind: "rayleigh",
          depolarisation: [0, 0, 0],
        }),
      ],
    };
    const tail = Math.exp(-topM / scaleHeightM);
    const floorM = -scaleHeightM * Math.log((10 / 40) * (1 - tail) + tail);

    it("finds its floor where τ from the top reaches 10", () => {
      expect(envelopeFloorM(envelope) - floorM).toBeCloseTo(0, 6);
    });

    it("splits it at its floor", () => {
      const regime = classifyRegime(envelope, { cloudFraction: 0, decks: [], envelope: true });
      expect(regime.kind === "cloudDeck" ? regime.deckAltitudeM - floorM : Number.NaN).toBeCloseTo(
        0,
        6,
      );
    });

    it("splits it at its floor, not at a deck past τ 10 under a full cover", () => {
      const regime = classifyRegime(envelope, {
        cloudFraction: 1,
        decks: [coverDeck(30, 300_000, 350_000)],
        envelope: true,
      });
      expect(regime.kind === "cloudDeck" ? regime.deckAltitudeM - floorM : Number.NaN).toBeCloseTo(
        0,
        6,
      );
    });

    it("splits one shallower than 10 in all at its ground", () => {
      expect(envelopeFloorM(scaledMedium(envelope, 0.2))).toBe(0);
    });
  });
});

describe("the kept bodies, as traced (clear, with a ground beneath)", () => {
  const lines = linesOf;

  it("classifies the measured Earth thickScattering by the depth line on all three channels, blue at 3.2 τ* (main rules on it after the report)", () => {
    const [, , blue] = regimeChannels(EARTH_REFERENCE);
    expect([
      classifyRegime(EARTH_REFERENCE, CLEAR_COVER).kind,
      lines(EARTH_REFERENCE),
      (blue.opticalDepth / blue.boundary.tauStar).toFixed(1),
    ]).toEqual(["thickScattering", ["depth/-", "depth/-", "depth/-"], "3.2"]);
  });

  it("classifies Earth with the ozone slab as the measured Earth: its absorber is left out", () => {
    expect([
      classifyRegime(EARTH_SLAB_REFERENCE, CLEAR_COVER).kind,
      lines(EARTH_SLAB_REFERENCE),
    ]).toEqual(["thickScattering", ["depth/-", "depth/-", "depth/-"]]);
  });

  it("classifies Mars thickScattering by both lines on all three channels, red at 5.4 τ* (the ground line accepted; the depth line main rules on after the report)", () => {
    const [red] = regimeChannels(MARS_REFERENCE);
    expect([
      classifyRegime(MARS_REFERENCE, CLEAR_COVER).kind,
      lines(MARS_REFERENCE),
      (red.opticalDepth / red.boundary.tauStar).toFixed(1),
    ]).toEqual(["thickScattering", ["depth/ground", "depth/ground", "depth/ground"], "5.4"]);
  });

  it("classifies saturn-oblate thickScattering by the depth line on all three channels, blue at 5.8 τ* (main rules on it after the report)", () => {
    const [, , blue] = regimeChannels(SATURN_OBLATE_REFERENCE);
    expect([
      classifyRegime(SATURN_OBLATE_REFERENCE, CLEAR_COVER).kind,
      lines(SATURN_OBLATE_REFERENCE),
      (blue.opticalDepth / blue.boundary.tauStar).toFixed(1),
    ]).toEqual(["thickScattering", ["depth/-", "depth/-", "depth/-"], "5.8"]);
  });
});

describe("the collapse onto τ(1 − ωg)", () => {
  const products = Array.from(THICK_MS_BOUNDARY.tauStar).map((tau, i) => {
    const omega = THICK_MS_BOUNDARY.omega[Math.floor(i / THICK_MS_BOUNDARY.g.length)] ?? Number.NaN;
    const g = THICK_MS_BOUNDARY.g[i % THICK_MS_BOUNDARY.g.length] ?? Number.NaN;
    return tau * (1 - omega * g);
  });
  const mean = Math.exp(products.reduce((sum, p) => sum + Math.log(p), 0) / products.length);
  const octaves = Math.max(...products.map((p) => Math.abs(Math.log2(p / mean))));

  it("does not hold: the drift cells' τ*(1 − ωg) spread 3.0 octaves from their geometric mean, past one half-octave", () => {
    expect(octaves.toFixed(1)).toBe("3.0");
  });

  it("runs from (1, 0.9)'s 0.009 to (0.4, 0)'s 0.5", () => {
    expect([Math.min(...products).toFixed(4), Math.max(...products)]).toEqual(["0.0088", 0.5]);
  });
});

describe("R08.T4.a's depth note", () => {
  it("puts (1, 0)'s τ* at under half an Earth column of dry air on its worst channel, inside the two to three columns past which no three wavelengths hold", () => {
    const air = {
      ...EARTH_REFERENCE,
      terms: EARTH_REFERENCE.terms.filter((t) => t.name === "rayleigh"),
    };
    const worst = Math.max(...verticalOpticalDepth(air));
    const columns = (THICK_MS_BOUNDARY.tauStar[0] ?? Number.NaN) / worst;
    expect(columns.toFixed(2)).toBe("0.49");
  });
});
