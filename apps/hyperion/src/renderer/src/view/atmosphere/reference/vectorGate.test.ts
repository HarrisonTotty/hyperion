/**
 * R08.T14.f's vector gates (plan R08, R08.T14.f; Design note 10, _Polarisation_ and _The gates_;
 * `decision-r08-t13c2-classes.md` §6): R08.T13.c1's case gate (`caseGate.ts`) re-run against the
 * four committed references' vector radiance I with the polarisation correction, the record
 * `vectorGate.json`. Every verdict is recorded, never asserted (`decision-r08-t13-scope.md`,
 * ruling 12): a miss is a finding against R08.T14.d's preconditions, recorded with its figures.
 *
 * @remarks
 * - **The route.** Each case is read through the path `thick/regime.ts`'s `classifyRegime` gives it
 *   under a clear sky (`CLEAR_COVER`, how the cases were traced): a `thickScattering` case's twin
 *   reads its thick bake with the correction summed into it (`thick/read.ts`'s `thick` kind), a
 *   `thin` one's the correction alone beside Ψ_ms. All four classify `thickScattering`
 *   (R08.T13.c2). Each is baked from its medium's sources under its one light, the reference Sun,
 *   on the twin's own bands (`thick/keptBake.ts`'s `bakeKeptCase` and `bakeKeptCorrection`,
 *   `twin.ts`'s `twinBakeBody`).
 * - **The diagnostic** beside it: the analytic path with the correction, the twin reading the
 *   correction alone beside Ψ_ms. It cannot pass: c1 measured that path outside the gate.
 * - **Uncorrected**, recorded beside: c1's own record, the uncorrected twin against the scalar I
 *   (`caseGate.json`), and the same twin against the vector I.
 * - **R08.T13's surface irradiance** (check 6) against `earth`'s vector flux aggregate, to 2% by the
 *   tolerance test, uncorrected: the sky's irradiance reads Ψ_ms alone, as the kernels' does on a
 *   body whose tables hold no thick table.
 * - **The ground's irradiance through the route** (R08.T14.d.4, R08.T14.d's precondition 3): each
 *   case's downwelling flux under each of its aggregates' suns, the sky's irradiance built over the
 *   thick table as the kernels' is since R08.T14.d.4 (`tables.ts`'s `IRRADIANCE_THICK_KERNEL`),
 *   against the vector flux aggregate to 2% by the tolerance test, the diagnostic's beside.
 * - **The bake's modes** (R08.T14.d.1, the task's "both m = 0 alone and m = 0..1 are run"): the route
 *   is also gated with the bake's table cut to m = 0 alone and to m ≤ 1, its higher modes set to 0
 *   (a test's option, not a product setting; the solve's modes are independent, so the cut table is
 *   the bake at those modes), the correction kept whole at m ≤ 2; the route itself is m ≤ 2.
 * - **The tables.** Each case's packed table's bytes, and its correction's largest |ΔJ_m| and the
 *   correction alone's half floats under the least normal (R08.T14.e's deferred _Half floats_); the
 *   bake's largest |Jᵐ|, its m = 2's largest share of J⁰, and G₂'s rounding against its `f32`
 *   values, as a share of J⁰, in the bake's own packed table and in the routed one with the
 *   correction summed in (R08.T14.d.1).
 * - **`giant-deck` through its deck** (R08.T15.a.2; the record's `deck`). The case is read through
 *   the `cloudDeck` split `classifyRegime` gives it under its own cover, the NH₃ deck of τ(550) 20
 *   under a cloud fraction of 1 (`thickCases.ts`'s "so that R08.T15 splits the medium at it"; lean,
 *   2026-10-30): split at the deck's top, the deck read to the black ground beneath, its bake and
 *   correction over the deck baked on the twin's body (`bakeKeptDeck`), and its twin drawing the
 *   upper medium over the deck (`twin.ts`'s `deck`), against the vector I over the geometries above
 *   the split. Below it (R08.T15.b.2, `deck.routedBelow`), its ground and aerial views through the
 *   table below the deck, scalar and uncorrected, against the vector I with its tallies against the
 *   scalar I beside, and the ground's downwelling flux under each aggregate against the vector flux
 *   by the tolerance test. Beside the route: the deck's multiple-scattering part cut to m < 3 and
 *   m < 6 (the route's 8, `DECK_MODES`), its sky-lit radiance at the deck's modes
 *   (`diffuseModes`), its floor white, the case split as a gas envelope's at τ 10 (R08.T16.a's
 *   route) over its black floor at τ 1,000 and over a white one, each with its twin ÷ reference by
 *   image, the bake's own plane albedo against each aggregate's upwelling flux, the deck's
 *   co-albedo at its floor, and the deck's share of each orbit image's radiance (its light set to
 *   0). A miss is a finding against R08.T15.b.2's routing.
 * - **The Venus-class cases through their deck** (R08.T15.b.2; the record's `deck.venusClass`; main's
 *   ruling of 2026-11-01). `venus-92bar` and `venus-58bar` are read through the `cloudDeck` split
 *   their cover gives them, the deck of τ(550) 34.155 under a cloud fraction of 1, split at its top
 *   over the ground, its bake on the twin's body, and the uncorrected twin: above the split the
 *   upper medium over the deck, below it the table below the deck. Every geometry is gated against
 *   the scalar I, the ground's downwelling flux under each aggregate against the scalar flux by the
 *   tolerance test, and R08.T12.b's flux sanity takes the twin's luminance fraction at the probe's
 *   sun against `sanityRanges.json`'s 2–4%. Their vector I is never gated: the Stokes mode's I
 *   weight does not converge there, and `thick/polarisationBake.test.ts`'s guard holds what the
 *   scalar reading neglects.
 * - **`titan-class` through its route** (R08.T14.d.4; the record's `titanClass`). The case, re-traced
 *   deep by R08.T12.h, is read as a kept case is: the `thickScattering` route `classifyRegime`
 *   gives it under a clear sky, its whole column baked at m ≤ 2 with its correction on the twin's
 *   body, and the route's twin over every geometry against the vector I, its tallies by image as
 *   the deck's envelope variant's, at the cut modes too; and the ground's downwelling flux under
 *   each aggregate against the vector flux by the tolerance test. Its verdict by image is
 *   `thick/routing.ts`'s `HAZE_GATE_TALLIES`, which opens a Titan-class world's route only on a
 *   pass.
 *
 * The twin's values on the route and the diagnostic, each case setting's radiance and drawn-surface
 * share (the values `thickRuns.test.ts` replaces), are the record's; every other value is c1's.
 * By hand (`HYPERION_VECTOR_GATE=1 pnpm --filter hyperion exec vitest run
 * src/renderer/src/view/atmosphere/reference/vectorGate.test.ts`, with `HYPERION_BLESS=1` to write
 * the record) every case is baked and its twin's values computed on both paths and at the cut
 * modes, whose verdicts alone the record keeps, as it keeps the tables. Otherwise the
 * route's verdicts are formed from the values the record holds against the same references and held
 * to the record's, and `earth`'s route is baked and its twin's values recomputed at
 * {@link SAMPLED} and held equal to them; under `HYPERION_SLOW_TESTS=1` the whole record is formed
 * from its values and held to the committed file byte for byte (some 90 s on a pod's 4 vCPU, the
 * route's alone 30 s; with `giant-deck`'s deck, whose sections are formed from its values and its
 * bakes' figures as the record holds them, some 70 s more for the thick cases' optics).
 */

import { describe, expect, it } from "vitest";

import type { Rgb } from "../../photometry/toneCurve";
import { cos } from "../../../lib/pinnedMath";
import { expectCommittedTable } from "../../../test/bless";
import {
  AGGREGATE_RELATIVE_TOLERANCE,
  AGGREGATE_SIGMA_CAP,
  aggregatePasses,
  aggregateRefusal,
  type FluxReading,
  luminanceOf,
} from "../gate";
import { densityAt } from "../medium";
import { kernelMedium } from "../tables";
import {
  bakeBinSolve,
  type MsSourceTable,
  type ThickBakeBody,
  thickBakeBands,
} from "../thick/bake";
import {
  DECK_MODES,
  deckGround,
  deckLayersOf,
  type DeckSlot,
  type DeckSplit,
  type DeckTables,
} from "../thick/deck";
import {
  belowDeckTableBytes,
  deckAboveTables,
  deckCorrectionAbove,
  deckTableBytes,
  packBelowDeckTable,
  packDeckTable,
  upperMediumOf,
} from "../thick/deckRead";
import { bakeKeptCase, bakeKeptCorrection, bakeKeptDeck, type KeptDeck } from "../thick/keptBake";
import type { PolarisationTable } from "../thick/polarisation";
import {
  type PackedThickTable,
  packThickTable,
  THICK_ROWS,
  thickCurveChannel,
  type ThickTableKind,
  thickTableBytes,
} from "../thick/read";
import {
  type AtmosphereRegime,
  type BodyCover,
  CLEAR_COVER,
  classifyRegime,
} from "../thick/regime";
import committedGate from "./caseGate.json" with { type: "json" };
import {
  blockingValues,
  CASE_GATE_FORMAT,
  caseGateRecord,
  caseGateSection,
  caseTwin,
  type GateCase,
  recorded,
  recordedRgb,
  recordImagesOf,
  recordRatiosOf,
  type TwinValues,
  twinValueKey,
  twinValueOf,
  type TwinValueSpec,
  twinValueSpecs,
  twinValuesOfRecord,
} from "./caseGate";
import { EARTH_CASE, REFERENCE_CASES, type ReferenceCase, referenceCaseBakeLights } from "./cases";
import earthReference from "./earth.reference.json" with { type: "json" };
import earthOzoneSlabReference from "./earth-ozone-slab.reference.json" with { type: "json" };
import giantDeckCase from "./giant-deck.case.json" with { type: "json" };
import giantDeckReference from "./giant-deck.reference.json" with { type: "json" };
import marsReference from "./mars.reference.json" with { type: "json" };
import { fluxReadingsOf, type RadianceKind, type ReferenceFile } from "./references";
import committedSanity from "./sanityRanges.json" with { type: "json" };
import saturnOblateReference from "./saturn-oblate.reference.json" with { type: "json" };
import {
  GIANT_DECK,
  thickReferenceCases,
  titanClassReferenceCase,
  VENUS_DECK,
  VENUS_DECK_MODES,
} from "./thickCases";
import titanClassReference from "./titan-class.reference.json" with { type: "json" };
import { twinBakeBody, type TwinOptions } from "./twin";
import committed from "./vectorGate.json" with { type: "json" };
import venus58Reference from "./venus-58bar.reference.json" with { type: "json" };
import venus92Reference from "./venus-92bar.reference.json" with { type: "json" };

/** The committed references, by case. */
const REFERENCES: Readonly<Record<string, ReferenceFile>> = {
  earth: earthReference,
  "earth-ozone-slab": earthOzoneSlabReference,
  mars: marsReference,
  "saturn-oblate": saturnOblateReference,
};

/** The four cases with their references. */
const CASES: ReadonlyArray<GateCase> = REFERENCE_CASES.map((referenceCase) => {
  const reference = REFERENCES[referenceCase.name];
  if (reference === undefined) {
    throw new Error(`no reference is committed for ${referenceCase.name}`);
  }
  return { referenceCase, reference };
});

/** Whether the bakes are made: by hand, `HYPERION_VECTOR_GATE=1`. */
function bakesRun(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_VECTOR_GATE"] === "1";
}

const BAKES = bakesRun();

/** Whether the whole record is formed from its values: by hand, `HYPERION_SLOW_TESTS=1`. */
function slowTests(): boolean {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return env["HYPERION_SLOW_TESTS"] === "1";
}

const SLOW = slowTests();

/**
 * The limits, ms: by hand, the four cases' bakes and both paths' twins, some 4 minutes on a pod's
 * 4 vCPU (2026-10-29), and the route's twins at the cut modes beside them, each well over ten times
 * its run; and the record formed three times from its values with `earth`'s route baked again, some
 * 20 s there.
 */
const LIMIT_MS = { whole: 3_600_000, record: 600_000 } as const;

/** The record's file, from this test. */
const RECORD_FILE = "./vectorGate.json";

/**
 * The record's format: 2 from R08.T14.d.1, which adds the cut modes and the bake's figures; 3 from
 * R08.T15.a.2, which adds `giant-deck`'s gate through its deck (`deck`); 4 from R08.T14.d.4, which
 * adds the ground's irradiance through the route; 5 from R08.T15.b.2, which adds the Venus-class
 * cases through their deck, scalar (`deck.venusClass`); 6 from R08.T15.b.2's step 3.3, which adds
 * `giant-deck` below its split (`deck.routedBelow`); 7 from R08.T14.d.4's Titan-class gate, which
 * adds `titan-class` through its route (`titanClass`).
 */
const VECTOR_GATE_FORMAT = 7;

/** Who writes the record, as each of its records' heads says. */
const WRITER = "vectorGate.test.ts, through caseGate.ts (plan R08, R08.T14.f)";

/** The twin's values of c1's record, which a path's values replace. */
const C1_VALUES = twinValuesOfRecord(committedGate);

/**
 * The ground's downwelling flux under each aggregate a case's reference holds (R08.T14.d.4,
 * precondition 3): one value a case and aggregate, the reading's name less its `/downwelling`.
 */
const GROUND_SPECS: ReadonlyArray<TwinValueSpec> = CASES.flatMap(({ referenceCase, reference }) =>
  fluxReadingsOf(reference, "vector")
    .filter((reading) => reading.name.endsWith("/downwelling"))
    .map((reading): TwinValueSpec => ({
      quantity: "downwelling",
      setting: "case",
      caseName: referenceCase.name,
      name: reading.name.replace(/\/downwelling$/u, ""),
    })),
);

/**
 * The values a path replaces: each case setting's radiance and drawn-surface share, and the ground's
 * downwelling flux under each of its aggregates (R08.T14.d.4).
 */
const PATH_SPECS: ReadonlyArray<TwinValueSpec> = [
  ...twinValueSpecs(CASES).filter(
    (spec) =>
      spec.setting === "case" && (spec.quantity === "radiance" || spec.quantity === "surfaceShare"),
  ),
  ...GROUND_SPECS,
];

/** The two paths whose values the record holds. */
type PathName = "routed" | "diagnostic";

/**
 * The bake's modes the route is also gated at (the module's remarks), by the record's name: m = 0
 * alone and m ≤ 1, beside the route's m ≤ 2.
 */
const CUT_MODES: ReadonlyArray<readonly [string, number]> = [
  ["m = 0", 1],
  ["m <= 1", 2],
];

/** `earth`'s route's values recomputed in every run, by spec key: its first ground views. */
const SAMPLED: ReadonlyArray<string> = PATH_SPECS.filter(
  (spec) => spec.caseName === EARTH_CASE.name && spec.quantity === "radiance",
)
  .slice(0, 3)
  .map(twinValueKey);

/**
 * A case's regime under a clear sky, the table its route reads (`thick/read.ts`'s kinds), and how
 * the record says it.
 *
 * @throws Error for a case routed `cloudDeck`, whose deck split is R08.T15's: none is.
 */
function routeOf(referenceCase: ReferenceCase): {
  readonly regime: AtmosphereRegime;
  readonly table: ThickTableKind;
  readonly path: string;
} {
  const regime = classifyRegime(referenceCase.medium, CLEAR_COVER);
  let table: ThickTableKind;
  let path: string;
  switch (regime.kind) {
    case "thin":
      table = "correction";
      path = "the correction alone beside Psi_ms";
      break;
    case "thickScattering":
      table = "thick";
      path = "the thick bake with the correction summed into it";
      break;
    case "cloudDeck":
      throw new Error(`${referenceCase.name} routes cloudDeck, whose deck split is R08.T15's`);
  }
  return { regime, table, path };
}

/** A case's bake and correction, one table a band of its twin's. */
interface CaseBakes {
  readonly bands: ReadonlyArray<MsSourceTable>;
  readonly correction: ReadonlyArray<PolarisationTable>;
}

/**
 * A bake's tables with every mode from `modes` up set to 0 (the module's remarks): a test's cut,
 * never the product's.
 */
function cutModes(bands: ReadonlyArray<MsSourceTable>, modes: number): MsSourceTable[] {
  return bands.map((band) => ({
    ...band,
    slots: band.slots.map((slot) => ({
      ...slot,
      values: slot.values.map((value, i) => (Math.floor(i / 3) % band.modes < modes ? value : 0)),
    })),
  }));
}

/** A case baked from its medium's sources under the reference Sun, on its twin's bands. */
function bakesOf(referenceCase: ReferenceCase): CaseBakes {
  const lights = referenceCaseBakeLights(referenceCase);
  const body = twinBakeBody(referenceCase.figure);
  return {
    bands: bakeKeptCase(referenceCase, lights, body),
    correction: bakeKeptCorrection(referenceCase, lights, body),
  };
}

/** A case's twin on `path`, over its bakes. */
function twinOn(
  path: PathName,
  referenceCase: ReferenceCase,
  bakes: CaseBakes,
): ReturnType<typeof caseTwin> {
  const table = path === "routed" ? routeOf(referenceCase).table : "correction";
  let options: Partial<TwinOptions>;
  switch (table) {
    case "thick":
      options = { thick: bakes.bands, polarisation: bakes.correction };
      break;
    case "correction":
      options = { polarisation: bakes.correction };
      break;
  }
  return caseTwin(referenceCase, options);
}

/** A twin's values of `specs`, as recorded. */
function valuesOf(
  twin: ReturnType<typeof caseTwin>,
  referenceCase: ReferenceCase,
  specs: ReadonlyArray<TwinValueSpec>,
): Map<string, Rgb> {
  const values = new Map<string, Rgb>();
  for (const spec of specs) {
    if (spec.caseName === referenceCase.name) {
      values.set(twinValueKey(spec), recordedRgb(twinValueOf(twin, referenceCase, spec)));
    }
  }
  return values;
}

/**
 * The bake's figures (the module's remarks): its largest |Jᵐ| a mode, sr⁻¹; its m = 2's largest
 * |J²| ÷ J⁰; and G₂'s largest |read − f32 value| × sin² θ_v sin² θ₀ ÷ J⁰, the most the word's
 * rounding moves J by (|cos 2Δφ| ≤ 1), in the bake's own packed table and in the routed one with
 * the correction summed in (`thick/read.ts`'s J + ΔJ σ_s ÷ σ_s,k). The shares are over the entries
 * whose J⁰ is at least 10⁻³ of the bake's largest.
 */
function bakeFiguresOf(referenceCase: ReferenceCase, bakes: CaseBakes): Record<string, unknown> {
  const { bands } = bakes;
  const drawn = kernelMedium(referenceCase.medium);
  const largest = [0, 0, 0];
  for (const band of bands) {
    for (const slot of band.slots) {
      for (const [i, value] of slot.values.entries()) {
        const m = Math.floor(i / 3) % band.modes;
        largest[m] = Math.max(largest[m] ?? 0, Math.abs(value));
      }
    }
  }
  const floor = 1e-3 * (largest[0] ?? Number.NaN);
  // The correction's factor at a height, σ_s ÷ σ_s,k, as the packing forms it.
  const wholeShare = (heightM: number, c: number): number => {
    let kernel = 0;
    let whole = 0;
    for (const [t, term] of drawn.terms.entries()) {
      const rho = densityAt(term.density, heightM);
      kernel += (term.scattering[c] ?? Number.NaN) * rho;
      whole += (referenceCase.medium.terms[t]?.scattering[c] ?? Number.NaN) * rho;
    }
    return kernel > 0 ? whole / kernel : 0;
  };
  const roundingOf = (packed: PackedThickTable, summed: boolean): number => {
    let rounding = 0;
    for (const [b, band] of bands.entries()) {
      const views = band.travelCosines.length;
      const suns = band.sunCosines.length;
      for (const [k, slot] of band.slots.entries()) {
        const layer = b * band.slots.length + k;
        const delta = summed ? bakes.correction[b]?.slots[k]?.values : undefined;
        for (const [a, heightM] of band.altitudesM.entries()) {
          for (const [s, mu0] of band.sunCosines.entries()) {
            for (const [v, muV] of band.travelCosines.entries()) {
              const cell = (a * suns + s) * views + v;
              const at = cell * band.modes * 3;
              const sines = (1 - mu0 * mu0) * (1 - muV * muV);
              const word = packed.texels[((layer * THICK_ROWS + a * suns + s) * views + v) * 4 + 3];
              for (let c = 0; c < 3; c += 1) {
                const share = delta === undefined ? 0 : wholeShare(heightM, c);
                const level = (slot.values[at + c] ?? Number.NaN) + share * (delta?.[at + c] ?? 0);
                if (!(level >= floor)) {
                  continue;
                }
                const curve =
                  (slot.values[at + 6 + c] ?? Number.NaN) + share * (delta?.[at + 6 + c] ?? 0);
                const read = thickCurveChannel(word ?? 0, c);
                rounding = Math.max(rounding, (Math.abs(read - curve / sines) * sines) / level);
              }
            }
          }
        }
      }
    }
    return rounding;
  };
  let curveShare = 0;
  for (const band of bands) {
    for (const slot of band.slots) {
      for (let at = 0; at < slot.values.length; at += 3 * band.modes) {
        for (let c = 0; c < 3; c += 1) {
          const level = slot.values[at + c] ?? Number.NaN;
          if (level >= floor) {
            curveShare = Math.max(curveShare, Math.abs(slot.values[at + 6 + c] ?? 0) / level);
          }
        }
      }
    }
  }
  const correction = { bands: bakes.correction, medium: referenceCase.medium };
  return {
    bakeLargestPerSr: largest.map((value) => recorded(value)),
    bakeCurveShareOfJ0: recorded(curveShare),
    bakeCurveRoundingOfJ0: recorded(roundingOf(packThickTable(bands, drawn), false)),
    routedCurveRoundingOfJ0: recorded(roundingOf(packThickTable(bands, drawn, correction), true)),
  };
}

/**
 * A case's tables as packed: their bytes, the correction's largest values and half floats, and the
 * bake's figures.
 */
function tablesOf(referenceCase: ReferenceCase, bakes: CaseBakes): Record<string, unknown> {
  const drawn = kernelMedium(referenceCase.medium);
  const correction = { bands: bakes.correction, medium: referenceCase.medium };
  const alone = packThickTable(null, drawn, correction);
  let routed: PackedThickTable;
  switch (routeOf(referenceCase).table) {
    case "thick":
      routed = packThickTable(bakes.bands, drawn, correction);
      break;
    case "correction":
      routed = alone;
      break;
  }
  const largest = [0, 0, 0];
  for (const band of bakes.correction) {
    for (const slot of band.slots) {
      for (let i = 0; i < slot.values.length; i += 1) {
        const m = Math.floor(i / 3) % 3;
        largest[m] = Math.max(largest[m] ?? 0, Math.abs(slot.values[i] ?? Number.NaN));
      }
    }
  }
  // The correction alone's J⁰ halves under the half's least normal, 2⁻¹⁴, and above 0.
  let subnormal = 0;
  let halves = 0;
  for (let texel = 0; texel < alone.texels.length / 4; texel += 1) {
    const x = alone.texels[texel * 4] ?? 0;
    const y = alone.texels[texel * 4 + 1] ?? 0;
    for (const bits of [x & 0xffff, x >>> 16, y & 0xffff]) {
      halves += 1;
      const exponent = (bits >>> 10) & 0x1f;
      if (exponent === 0 && (bits & 0x3ff) !== 0) {
        subnormal += 1;
      }
    }
  }
  const [first] = bakes.correction;
  return {
    bands: bakes.correction.length,
    slots: first?.slots.length ?? 0,
    kind: routed.kind,
    bytes: thickTableBytes(routed),
    solves: bakes.correction.reduce((sum, band) => sum + (band.slots[0]?.solves ?? 0), 0),
    complexPairs: bakes.correction.reduce(
      (sum, band) => sum + (band.slots[0]?.complexPairs ?? 0),
      0,
    ),
    largestCorrectionPerSr: largest.map((value) => recorded(value)),
    correctionHalvesBelowLeastNormal: subnormal,
    correctionHalves: halves,
    ...bakeFiguresOf(referenceCase, bakes),
  };
}

/** c1's values with `path`'s in their place. */
function withPath(path: TwinValues): TwinValues {
  const values = new Map(C1_VALUES);
  for (const [key, value] of path) {
    values.set(key, value);
  }
  return values;
}

/** Rows of a path's values, `[key, r, g, b]`, in {@link PATH_SPECS}' order. */
function rowsOf(values: TwinValues): ReadonlyArray<readonly [string, number, number, number]> {
  return PATH_SPECS.map((spec) => {
    const key = twinValueKey(spec);
    const value = values.get(key);
    if (value === undefined) {
      throw new Error(`the path holds no value ${key}`);
    }
    return [key, value[0], value[1], value[2]] as const;
  });
}

/**
 * A path's values as the committed record holds them.
 *
 * @throws Error for a record without them.
 */
function committedValues(path: PathName): Map<string, Rgb> {
  const twinValues: unknown = Reflect.get(committed, "twinValues");
  const rows: unknown =
    typeof twinValues === "object" && twinValues !== null ? Reflect.get(twinValues, path) : null;
  if (!Array.isArray(rows)) {
    throw new Error(`vectorGate.json holds no ${path} values`);
  }
  const values = new Map<string, Rgb>();
  for (const row of rows) {
    if (
      Array.isArray(row) &&
      typeof row[0] === "string" &&
      typeof row[1] === "number" &&
      typeof row[2] === "number" &&
      typeof row[3] === "number"
    ) {
      values.set(row[0], [row[1], row[2], row[3]]);
    }
  }
  return values;
}

/** The committed record's section `key` of `caseName`, read back as it stands. */
function committedCaseSection(caseName: string, key: string): unknown {
  const cases: unknown = Reflect.get(committed, "cases");
  const section: unknown =
    typeof cases === "object" && cases !== null ? Reflect.get(cases, caseName) : null;
  return typeof section === "object" && section !== null ? Reflect.get(section, key) : null;
}

/** A record's section `key`, or `null`. */
function sectionOf(record: unknown, key: string): unknown {
  return typeof record === "object" && record !== null ? Reflect.get(record, key) : null;
}

/**
 * c1's case gate on a path's values against the vector I.
 *
 * @throws Error for a value not finite or negative.
 */
function vectorRecordOf(path: TwinValues): unknown {
  const values = withPath(path);
  if (blockingValues(values).length > 0) {
    throw new Error("a path's twin gives values not finite or negative");
  }
  return caseGateRecord(CASES, values, { writer: WRITER, radiance: "vector" });
}

/** The channels as the record names them. */
const CHANNEL_NAMES = ["r", "g", "b"] as const;

/**
 * A flux reading's rows against the twin's value: by channel, the twin's flux, the reference's, its
 * σ, the twin's over it less 1, and the tolerance test's verdict, `refused` where the reading's σ
 * refuses it.
 */
function fluxRowsOf(reading: FluxReading, twin: Rgb): unknown[][] {
  return ([0, 1, 2] as const).map((c) => {
    let verdict = "refused";
    if (aggregateRefusal(reading, c) === undefined) {
      verdict = aggregatePasses(twin[c], reading, c) ? "pass" : "miss";
    }
    return [
      CHANNEL_NAMES[c],
      twin[c],
      recorded(reading.flux[c]),
      recorded(reading.standardError[c]),
      recorded(twin[c] / reading.flux[c] - 1),
      verdict,
    ];
  });
}

/**
 * The ground's irradiance on each path (R08.T14.d.4, precondition 3): for each case and aggregate,
 * each channel's twin flux, the reference's, its σ, the twin's over it less 1, and the tolerance
 * test's verdict, `refused` where the reading's σ refuses it.
 *
 * @throws Error for a path that holds no value of a case and aggregate.
 */
function groundIrradianceOf(path: TwinValues): ReadonlyArray<Record<string, unknown>> {
  return GROUND_SPECS.map((spec) => {
    const gateCase = CASES.find(({ referenceCase }) => referenceCase.name === spec.caseName);
    const reading =
      gateCase === undefined
        ? undefined
        : fluxReadingsOf(gateCase.reference, "vector").find(
            (each) => each.name === `${spec.name}/downwelling`,
          );
    const twin = path.get(twinValueKey(spec));
    if (reading === undefined || twin === undefined) {
      throw new Error(`the path holds no ground irradiance of ${twinValueKey(spec)}`);
    }
    return {
      case: spec.caseName,
      aggregate: `${spec.name}/downwelling`,
      rows: fluxRowsOf(reading, twin),
    };
  });
}

/** The ground's irradiance section of the record: the route's and the diagnostic's. */
function groundIrradianceSection(
  paths: Readonly<Record<PathName, TwinValues>>,
): Record<string, unknown> {
  return {
    tolerance: { relative: AGGREGATE_RELATIVE_TOLERANCE, sigmaCap: AGGREGATE_SIGMA_CAP },
    routed: groundIrradianceOf(paths.routed),
    diagnostic: groundIrradianceOf(paths.diagnostic),
  };
}

/** The routed path's sections of the record, from its case gate's record `routed`. */
function routedSections(routed: unknown): Record<string, unknown> {
  return {
    cases: Object.fromEntries(
      CASES.map(({ referenceCase: { name } }) => [
        name,
        { routed: recordImagesOf(routed, name), routedRatios: recordRatiosOf(routed, name) },
      ]),
    ),
    routedMarsAureole: sectionOf(routed, "marsAureole"),
    routedSaturnOblateness: sectionOf(routed, "saturnOblateness"),
  };
}

/** The committed record's routed sections, as {@link routedSections} forms them. */
function committedRoutedSections(): Record<string, unknown> {
  const cases: unknown = Reflect.get(committed, "cases");
  return {
    cases: Object.fromEntries(
      CASES.map(({ referenceCase: { name } }) => {
        const section = sectionOf(cases, name);
        return [
          name,
          {
            routed: sectionOf(section, "routed"),
            routedRatios: sectionOf(section, "routedRatios"),
          },
        ];
      }),
    ),
    routedMarsAureole: sectionOf(committed, "routedMarsAureole"),
    routedSaturnOblateness: sectionOf(committed, "routedSaturnOblateness"),
  };
}

/**
 * The route's verdicts at each of {@link CUT_MODES}, by case: `values` a cut's values of every case,
 * by its name.
 */
function cutModesRecord(
  values: ReadonlyMap<string, TwinValues>,
): (caseName: string) => Record<string, unknown> {
  const records = CUT_MODES.map(([name]) => {
    const cut = values.get(name);
    if (cut === undefined) {
      throw new Error(`the route has no values at ${name}`);
    }
    return [name, vectorRecordOf(cut)] as const;
  });
  return (caseName) =>
    Object.fromEntries(records.map(([name, record]) => [name, recordImagesOf(record, caseName)]));
}

// --- R08.T15.a.2: `giant-deck` through its deck -----------------------------------------------------

/** The deck's gate's case: R08.T12.b.2's `giant-deck`, a Saturn-class air over its NH₃ ice deck. */
const DECK_CASE = "giant-deck";

/**
 * The deck's gate's variants (the module's remarks), by the record's name: the route, its deck's
 * multiple-scattering part cut to m < 3 and m < 6, its sky-lit radiance at the deck's modes, its
 * floor white (the case's own ground under the deck), the case split at the envelope's floor
 * (R08.T16.a's route) with its black floor at τ 1,000 and white, and no deck at all (the deck's own
 * light 0), whose share of each orbit image's radiance the record gives.
 */
const DECK_VARIANTS = [
  "route",
  "m < 3",
  "m < 6",
  "diffuse at 8",
  "floor at 1",
  "as an envelope",
  "as an envelope, floor at 1",
  "no deck",
] as const;

type DeckVariant = (typeof DECK_VARIANTS)[number];

/** The giant's deck term's top, the highest level of its tabulated density above 0, m. */
function deckTopM(referenceCase: ReferenceCase): number {
  const term = referenceCase.medium.terms.find((t) => t.name === GIANT_DECK.name);
  if (term?.density.kind !== "tabulated") {
    throw new Error(`${referenceCase.name} has its tabulated deck ${GIANT_DECK.name}`);
  }
  const { altitudesM, relative } = term.density;
  const last = relative.findLastIndex((d) => d > 0);
  return altitudesM[Math.min(last + 1, altitudesM.length - 1)] ?? Number.NaN;
}

/**
 * `giant-deck`'s cover (the module's remarks): its NH₃ deck of τ(550) 20 under a cloud fraction of
 * 1, from the ground to its top, so that `classifyRegime` splits it at the deck's top as the case
 * was built for (`thickCases.ts`), the deck read to the black ground beneath; or, `envelope`, a gas
 * envelope's, split at the envelope's floor, τ 10 from the top (R08.T16.a's route).
 */
function giantCover(referenceCase: ReferenceCase, envelope: boolean): BodyCover {
  return {
    cloudFraction: 1,
    decks: [
      {
        opticalDepth550: GIANT_DECK.opticalDepth550,
        baseAltitudeM: 0,
        topAltitudeM: deckTopM(referenceCase),
      },
    ],
    envelope,
  };
}

/** The split `classifyRegime` gives `giant-deck` under {@link giantCover}, its floor the cover's. */
function giantSplit(referenceCase: ReferenceCase, envelope: boolean): DeckSplit {
  const regime = classifyRegime(referenceCase.medium, giantCover(referenceCase, envelope));
  if (regime.kind !== "cloudDeck") {
    throw new Error(`${referenceCase.name} routes ${regime.kind}, not cloudDeck`);
  }
  return { altitudeM: regime.deckAltitudeM, floor: envelope ? "envelope" : "ground" };
}

/**
 * The case's gate over its geometries on one side of the split: above it, which the twin draws from
 * above the deck, or below it, its ground and aerial views, which it draws through the table below
 * the deck (R08.T15.b.2).
 */
function deckGateCase(
  referenceCase: ReferenceCase,
  splitM: number,
  side: "above" | "below" = "above",
): GateCase {
  const kept = new Set(
    referenceCase.geometries
      .filter((g) => g.observer.heightM > splitM === (side === "above"))
      .map((g) => g.name),
  );
  return {
    referenceCase,
    reference: {
      ...giantDeckReference,
      geometries: giantDeckReference.geometries.filter((g) => kept.has(g.name)),
    },
  };
}

/** The deck's gate's values of a twin, each gated geometry's radiance and surface share, as recorded. */
function deckValuesOf(twin: ReturnType<typeof caseTwin>, gateCase: GateCase): Map<string, Rgb> {
  const { referenceCase } = gateCase;
  return valuesOf(
    twin,
    referenceCase,
    gateCase.reference.geometries.flatMap((g) =>
      (["radiance", "surfaceShare"] as const).map((quantity): TwinValueSpec => ({
        quantity,
        setting: "case",
        caseName: referenceCase.name,
        name: g.name,
      })),
    ),
  );
}

/** A case's specs of the ground's downwelling flux under each of its aggregates (R08.T15.b.2). */
function downwellingSpecs(referenceCase: ReferenceCase): TwinValueSpec[] {
  return referenceCase.aggregates.map((a) => ({
    quantity: "downwelling",
    setting: "case",
    caseName: referenceCase.name,
    name: a.name,
  }));
}

/**
 * A case's twin value of the ground's downwelling flux under `aggregate`.
 *
 * @throws Error where `values` hold none.
 */
function downwellingOf(values: TwinValues, caseName: string, aggregate: string): Rgb {
  const value = values.get(
    twinValueKey({ quantity: "downwelling", setting: "case", caseName, name: aggregate }),
  );
  if (value === undefined) {
    throw new Error(`${caseName} holds no downwelling flux under ${aggregate}`);
  }
  return value;
}

/**
 * The ground's downwelling flux under each of `reference`'s aggregates against its `kind` flux,
 * direct and diffuse, by the tolerance test (R08.T15.b.2): by channel, the twin's value, the
 * reference's flux and standard error, the twin ÷ reference − 1 and the verdict.
 */
function deckGroundIrradianceOf(
  reference: ReferenceFile,
  kind: RadianceKind,
  twinOf: (aggregate: string) => Rgb,
): Record<string, unknown> {
  const aggregates = fluxReadingsOf(reference, kind)
    .filter((reading) => reading.name.endsWith("/downwelling"))
    .map((reading) => ({
      aggregate: reading.name,
      rows: fluxRowsOf(reading, twinOf(reading.name.replace(/\/downwelling$/u, ""))),
    }));
  return {
    tolerance: { relative: AGGREGATE_RELATIVE_TOLERANCE, sigmaCap: AGGREGATE_SIGMA_CAP },
    against: `the ${kind} downwelling flux, direct and diffuse (fluxReadingsOf, ${kind})`,
    aggregates,
  };
}

/** A deck's tables with the multiple-scattering part's modes from `modes` up set to 0: a test's cut. */
function cutDeckModes(tables: ReadonlyArray<DeckTables>, modes: number): DeckTables[] {
  const cut = (values: Float32Array): Float32Array =>
    values.map((value, i) => (Math.floor(i / 3) % DECK_MODES < modes ? value : 0));
  return tables.map((band) =>
    withDeckSlots(band, (slot) => ({
      ...slot,
      reflectance: cut(slot.reflectance),
      beamReflection: cut(slot.beamReflection),
    })),
  );
}

/** A deck's tables with its own light 0: every series, G and G_β, its phase kept. */
function darkDeck(tables: ReadonlyArray<DeckTables>): DeckTables[] {
  return tables.map((band) =>
    withDeckSlots(band, (slot) => ({
      ...slot,
      reflectance: new Float32Array(slot.reflectance.length),
      beamReflection: new Float32Array(slot.beamReflection.length),
      diffuse: new Float32Array(slot.diffuse.length),
      singleGeometry: new Float32Array(slot.singleGeometry.length),
      singleBeamGeometry: new Float32Array(slot.singleBeamGeometry.length),
    })),
  );
}

/** A deck band with each slot made by `slotOf`. */
function withDeckSlots(band: DeckTables, slotOf: (slot: DeckSlot) => DeckSlot): DeckTables {
  return { ...band, slots: band.slots.map(slotOf) };
}

/**
 * The deck bake's own plane albedo (the module's remarks): in band 0, each bin's and term's solve of
 * the upper medium over the deck, its upward flux at the top over μ₀ at each of the grid's μ₀ above
 * the horizontal, weighted over the bin's terms and converted to the channels, then linear in μ₀ at
 * each aggregate's sun; as the client's upwelling flux, that times the reference's incident flux.
 */
function deckPlaneAlbedoOf(
  kept: KeptDeck,
  body: ThickBakeBody,
): ReadonlyArray<readonly [string, number, number, number]> {
  const { prepared } = kept;
  const [band] = thickBakeBands(body);
  const light = prepared.upper.lights[0];
  if (band === undefined || light === undefined) {
    throw new Error("the deck has a band and a light");
  }
  const { grid } = prepared.upper;
  const ground = deckGround(prepared, band);
  const suns = grid.sunCosines.length;
  const perBin = light.bins.map((bin, b) => {
    const albedos = new Float64Array(suns);
    for (const term of bin.terms) {
      const { solution } = bakeBinSolve(prepared.upper, band, b, term, ground);
      solution.beams.forEach((beam, s) => {
        const mu0 = grid.sunCosines[s] ?? Number.NaN;
        if (mu0 > 0) {
          albedos[s] =
            (albedos[s] ?? 0) + (term.weight * (beam.fluxes.diffuseUp[0] ?? Number.NaN)) / mu0;
        }
      });
    }
    return albedos;
  });
  const channelAt = (c: number, mu0: number): number => {
    const factors = light.conversion.factors[c] ?? [];
    const at = (s: number): number =>
      perBin.reduce((sum, bin, b) => sum + (factors[b] ?? Number.NaN) * (bin[s] ?? Number.NaN), 0);
    let s = 0;
    while (s + 1 < suns && (grid.sunCosines[s + 1] ?? Number.NaN) < mu0) {
      s += 1;
    }
    const s0 = Math.min(s, suns - 2);
    const m0 = grid.sunCosines[s0] ?? Number.NaN;
    const m1 = grid.sunCosines[s0 + 1] ?? Number.NaN;
    const t = Math.min(Math.max((mu0 - m0) / (m1 - m0), 0), 1);
    return at(s0) * (1 - t) + at(s0 + 1) * t;
  };
  return giantDeckReference.aggregates.map((aggregate) => {
    const asked = giantDeckCase.aggregates.find((a) => a.name === aggregate.name);
    const zenith = asked?.sunDirections[0]?.zenithDeg;
    if (zenith === undefined) {
      throw new Error(`${aggregate.name} has a sun`);
    }
    const mu0 = cos((zenith * Math.PI) / 180);
    return [
      `${aggregate.name}/upwelling`,
      recorded(channelAt(0, mu0) * (aggregate.incidentTop[0] ?? Number.NaN)),
      recorded(channelAt(1, mu0) * (aggregate.incidentTop[1] ?? Number.NaN)),
      recorded(channelAt(2, mu0) * (aggregate.incidentTop[2] ?? Number.NaN)),
    ] as const;
  });
}

/**
 * The deck's single-scattering co-albedo at its floor, 1 − ω of its lowest layer, in band 0, each
 * bin's weighted over its terms and converted to the channels.
 */
function floorCoAlbedoOf(kept: KeptDeck, body: ThickBakeBody): Rgb {
  const { prepared } = kept;
  const [band] = thickBakeBands(body);
  const light = prepared.upper.lights[0];
  if (band === undefined || light === undefined) {
    throw new Error("the deck has a band and a light");
  }
  const perBin = light.bins.map((bin, b) =>
    bin.terms.reduce((sum, term) => {
      const { layers, truncated } = deckLayersOf(prepared, band, b, term);
      const layer = layers.at(-1);
      const f = truncated.at(-1) ?? 0;
      // ω′ = (1 − f)ω ÷ (1 − ωf), undone: ω = ω′ ÷ (1 − f + f ω′).
      const scaled = layer?.singleScatteringAlbedo ?? Number.NaN;
      const omega = scaled / (1 - f + f * scaled);
      return sum + term.weight * (1 - omega);
    }, 0),
  );
  const channel = (c: number): number => {
    const factors = light.conversion.factors[c] ?? [];
    return recorded(perBin.reduce((sum, x, b) => sum + (factors[b] ?? Number.NaN) * x, 0));
  };
  return [channel(0), channel(1), channel(2)];
}

/** What the deck's section takes beside its twins' values: its bakes' own figures. */
interface DeckFigures {
  readonly split: DeckSplit;
  readonly envelopeSplit: DeckSplit;
  readonly floorAltitudeM: number;
  readonly floorCoAlbedo: Rgb;
  /** The bake's upwelling flux at the top by aggregate, `[name, r, g, b]`. */
  readonly planeAlbedo: ReadonlyArray<readonly [string, number, number, number]>;
  readonly tables: Record<string, unknown>;
}

/**
 * `giant-deck`'s gate through its deck (the module's remarks): the route's case section against the
 * vector I above the split and below it, each variant's tallies, the route's ratios, the ground's
 * flux below the deck and the bake's plane albedo against the reference's aggregates, the deck's
 * share of each orbit image's radiance, the bakes' figures and every variant's twin values.
 */
function deckSectionOf(
  values: ReadonlyMap<DeckVariant, TwinValues>,
  figures: DeckFigures,
): Record<string, unknown> {
  const referenceCase = thickReferenceCases()[DECK_CASE];
  const gateCase = deckGateCase(referenceCase, figures.split.altitudeM);
  const variant = (name: DeckVariant): TwinValues => {
    const held = values.get(name);
    if (held === undefined) {
      throw new Error(`the deck's gate has no values ${name}`);
    }
    return held;
  };
  const envelopeCase = deckGateCase(referenceCase, figures.envelopeSplit.altitudeM);
  const gateOf = (name: DeckVariant): unknown =>
    caseGateSection(
      name === "as an envelope" || name === "as an envelope, floor at 1" ? envelopeCase : gateCase,
      variant(name),
      "vector",
    );
  const route = gateOf("route");
  const belowCase = deckGateCase(referenceCase, figures.split.altitudeM, "below");
  const below = caseGateSection(belowCase, variant("route"), "vector");
  const belowScalar = caseGateSection(belowCase, variant("route"), "scalar");
  const asRecord = (section: unknown): unknown => ({ cases: { [DECK_CASE]: section } });
  // A variant's tallies and its twin ÷ reference by image, which a refusal leaves out of the tallies.
  const judged = (name: DeckVariant): Record<string, unknown> => {
    const section = asRecord(gateOf(name));
    return {
      tallies: recordImagesOf(section, DECK_CASE),
      ratios: recordRatiosOf(section, DECK_CASE),
    };
  };
  const gated = gateCase.reference.geometries.map((g) => g.name);
  // Each orbit image's radiance with the deck's light and without it, by channel.
  const images = new Map<string, [number, number, number, number, number, number]>();
  for (const geometry of referenceCase.geometries) {
    if (!gated.includes(geometry.name)) {
      continue;
    }
    const key = (from: DeckVariant): Rgb =>
      variant(from).get(
        twinValueKey({
          quantity: "radiance",
          setting: "case",
          caseName: DECK_CASE,
          name: geometry.name,
        }),
      ) ?? [Number.NaN, Number.NaN, Number.NaN];
    const lit = key("route");
    const dark = key("no deck");
    const held = images.get(geometry.image) ?? [0, 0, 0, 0, 0, 0];
    images.set(geometry.image, [
      held[0] + lit[0],
      held[1] + lit[1],
      held[2] + lit[2],
      held[3] + dark[0],
      held[4] + dark[1],
      held[5] + dark[2],
    ]);
  }
  const fluxes = fluxReadingsOf(giantDeckReference, "vector");
  const channels = [0, 1, 2] as const;
  return {
    case: DECK_CASE,
    route:
      "the deck's cloudDeck split (classifyRegime under its cover), the upper medium the kernels', its J with the correction summed in, the deck its ground at DECK_MODES",
    cover: giantCover(referenceCase, false),
    split: figures.split,
    floorAltitudeM: recorded(figures.floorAltitudeM),
    gated,
    belowTheSplit: referenceCase.geometries
      .filter((g) => !gated.includes(g.name))
      .map((g) => g.name),
    routed: recordImagesOf(asRecord(route), DECK_CASE),
    routedRatios: recordRatiosOf(asRecord(route), DECK_CASE),
    geometries: sectionOf(route, "geometries"),
    routedBelow: {
      radiance: "vector",
      route:
        "beneath the split, the table below the deck (R08.T15.b.2), scalar and uncorrected, its ground black",
      routed: recordImagesOf(asRecord(below), DECK_CASE),
      routedRatios: recordRatiosOf(asRecord(below), DECK_CASE),
      againstScalar: recordImagesOf(asRecord(belowScalar), DECK_CASE),
      geometries: sectionOf(below, "geometries"),
      groundIrradiance: deckGroundIrradianceOf(giantDeckReference, "vector", (aggregate) =>
        downwellingOf(variant("route"), DECK_CASE, aggregate),
      ),
    },
    byDeckModes: Object.fromEntries(
      (["m < 3", "m < 6"] as const).map((name) => [name, judged(name)]),
    ),
    diffuseAtDeckModes: judged("diffuse at 8"),
    floorAtOne: judged("floor at 1"),
    asAnEnvelope: { split: figures.envelopeSplit, ...judged("as an envelope") },
    envelopeFloorAtOne: judged("as an envelope, floor at 1"),
    floorCoAlbedo: figures.floorCoAlbedo,
    planeAlbedo: figures.planeAlbedo.map(([name, r, g, b]) => {
      const reading = fluxes.find((f) => f.name === name);
      if (reading === undefined) {
        throw new Error(`the reference has no aggregate ${name}`);
      }
      const client: Rgb = [r, g, b];
      return [
        name,
        r,
        g,
        b,
        ...recordedRgb(reading.flux),
        ...channels.map((c) => recorded(client[c] / reading.flux[c])),
        ...channels.map((c) =>
          aggregateRefusal(reading, c) !== undefined
            ? "refused"
            : aggregatePasses(client[c], reading, c)
              ? "pass"
              : "miss",
        ),
      ];
    }),
    deckShare: [...images].map(([image, sums]) => {
      const share = (c: 0 | 1 | 2): number => {
        const lit = sums[c];
        return recorded(lit > 0 ? 1 - (sums[c + 3] ?? Number.NaN) / lit : 0);
      };
      return [image, share(0), share(1), share(2)];
    }),
    tables: figures.tables,
    twinValues: Object.fromEntries(
      DECK_VARIANTS.map((name) => [
        name,
        [...variant(name)].map(([key, rgb]) => [key, rgb[0], rgb[1], rgb[2]]),
      ]),
    ),
  };
}

// --- R08.T15.b.2: the Venus-class cases through their deck, scalar ----------------------------

/**
 * The Venus-class cases gated through their deck (R08.T15.b.2; main's ruling of 2026-11-01): by
 * their references' scalar radiance in every view, above the split and below it, through the
 * uncorrected twin over the deck's split, their vector I kept in the files and never gated (the
 * Stokes mode's I weight does not converge over their thousands of Rayleigh scatterings).
 */
const VENUS_CASES = ["venus-92bar", "venus-58bar"] as const;

type VenusCaseName = (typeof VENUS_CASES)[number];

/** `f32`'s least normal, 2⁻¹²⁶ (IEEE 754-2008's binary32), exactly. */
const F32_LEAST_NORMAL = 1.1754943508222875e-38;

/** Their committed references. */
const VENUS_REFERENCES: Readonly<Record<VenusCaseName, ReferenceFile>> = {
  "venus-92bar": venus92Reference,
  "venus-58bar": venus58Reference,
};

/**
 * The Venus-class deck's span over the case's ground, m: its modes' lowest base and highest top,
 * each where its tabulated density first rises from 0 and last falls to it.
 */
function venusDeckSpanM(referenceCase: ReferenceCase): readonly [number, number] {
  let baseM = Number.POSITIVE_INFINITY;
  let topM = 0;
  for (const deckMode of VENUS_DECK_MODES) {
    const term = referenceCase.medium.terms.find((t) => t.name === deckMode.name);
    if (term?.density.kind !== "tabulated") {
      throw new Error(`${referenceCase.name} has its tabulated deck mode ${deckMode.name}`);
    }
    const { altitudesM, relative } = term.density;
    const first = relative.findIndex((d) => d > 0);
    const last = relative.findLastIndex((d) => d > 0);
    baseM = Math.min(baseM, altitudesM[Math.max(first - 1, 0)] ?? Number.NaN);
    topM = Math.max(topM, altitudesM[Math.min(last + 1, altitudesM.length - 1)] ?? Number.NaN);
  }
  return [baseM, topM];
}

/**
 * A Venus-class case's cover: its deck of τ(550) 34.155 (`thickCases.ts`'s `VENUS_DECK` over its
 * modes' shares) under a cloud fraction of 1, over the deck's span, so that `classifyRegime` splits
 * the medium at the deck's top over the ground (lean, 2026-11-01, as `giant-deck`'s).
 */
function venusCover(referenceCase: ReferenceCase): BodyCover {
  const [baseAltitudeM, topAltitudeM] = venusDeckSpanM(referenceCase);
  const opticalDepth550 = VENUS_DECK_MODES.reduce(
    (sum, deckMode) => sum + deckMode.share * VENUS_DECK.opticalDepth550,
    0,
  );
  return {
    cloudFraction: 1,
    decks: [{ opticalDepth550, baseAltitudeM, topAltitudeM }],
    envelope: false,
  };
}

/** The split `classifyRegime` gives a Venus-class case under {@link venusCover}. */
function venusSplit(referenceCase: ReferenceCase): DeckSplit {
  const regime = classifyRegime(referenceCase.medium, venusCover(referenceCase));
  if (regime.kind !== "cloudDeck") {
    throw new Error(`${referenceCase.name} routes ${regime.kind}, not cloudDeck`);
  }
  return { altitudeM: regime.deckAltitudeM, floor: "ground" };
}

/**
 * A thick case's specs (a Venus-class case's, `titan-class`'s): each geometry's radiance and surface
 * share, each aggregate's flux.
 */
function caseSpecs(referenceCase: ReferenceCase): TwinValueSpec[] {
  const caseName = referenceCase.name;
  return [
    ...referenceCase.geometries.flatMap((g) =>
      (["radiance", "surfaceShare"] as const).map((quantity): TwinValueSpec => ({
        quantity,
        setting: "case",
        caseName,
        name: g.name,
      })),
    ),
    ...downwellingSpecs(referenceCase),
  ];
}

/** What a Venus-class case's section takes beside its twin's values: its bake's own figures. */
interface VenusFigures {
  readonly split: DeckSplit;
  readonly floorAltitudeM: number;
  readonly tables: Record<string, unknown>;
}

/** A Venus-class case's surface range in `sanityRanges.json`, R08.T12.b's 2–4%. */
function venusSanityRange(caseName: VenusCaseName): {
  readonly low: number;
  readonly high: number;
} {
  const range: unknown = Reflect.get(committedSanity, `${caseName}-surface`);
  const low: unknown = sectionOf(range, "low");
  const high: unknown = sectionOf(range, "high");
  if (typeof low !== "number" || typeof high !== "number") {
    throw new Error(`sanityRanges.json holds no range for ${caseName}`);
  }
  return { low, high };
}

/**
 * A Venus-class case's gate through its deck (the module's remarks): every geometry against the
 * scalar I, the ground's downwelling flux under each aggregate against the scalar flux (direct and
 * diffuse) by the tolerance test, and R08.T12.b's flux sanity, the twin's luminance fraction at the
 * probe's sun against `sanityRanges.json`'s range; with the bake's figures and the downwelling
 * values the geometries' rows do not hold.
 */
function venusSectionOf(
  caseName: VenusCaseName,
  values: TwinValues,
  figures: VenusFigures,
): Record<string, unknown> {
  const referenceCase = thickReferenceCases()[caseName];
  const reference = VENUS_REFERENCES[caseName];
  const section = caseGateSection({ referenceCase, reference }, values, "scalar");
  const asRecord = { cases: { [caseName]: section } };
  const splitM = figures.split.altitudeM;
  const valueOf = (aggregate: string): Rgb => downwellingOf(values, caseName, aggregate);
  const probe = reference.aggregates.find((a) => a.name === "flux-lsfr");
  if (probe === undefined) {
    throw new Error(`${caseName}'s reference has no probe aggregate`);
  }
  const atProbe = valueOf("flux-lsfr");
  const share = (c: 0 | 1 | 2): number =>
    recorded(atProbe[c] / (probe.incidentTop[c] ?? Number.NaN));
  const channels: Rgb = [share(0), share(1), share(2)];
  const fraction = recorded(luminanceOf(channels));
  const { low, high } = venusSanityRange(caseName);
  const names = (below: boolean): string[] =>
    referenceCase.geometries
      .filter((g) => g.observer.heightM < splitM === below)
      .map((g) => g.name);
  return {
    radiance: "scalar",
    route:
      "the deck's cloudDeck split (classifyRegime under its cover), the uncorrected twin: above the split the upper medium over the deck (R08.T15.a.2), below it the table below the deck (R08.T15.b.2)",
    cover: venusCover(referenceCase),
    split: figures.split,
    floorAltitudeM: recorded(figures.floorAltitudeM),
    aboveTheSplit: names(false),
    belowTheSplit: names(true),
    routed: recordImagesOf(asRecord, caseName),
    routedRatios: recordRatiosOf(asRecord, caseName),
    geometries: sectionOf(section, "geometries"),
    groundIrradiance: deckGroundIrradianceOf(reference, "scalar", valueOf),
    sanity: {
      aggregate: "flux-lsfr",
      channels,
      fraction,
      low,
      high,
      within: fraction >= low && fraction <= high,
    },
    tables: figures.tables,
    downwelling: referenceCase.aggregates.map((a) => {
      const key = twinValueKey({
        quantity: "downwelling",
        setting: "case",
        caseName,
        name: a.name,
      });
      const value = valueOf(a.name);
      return [key, value[0], value[1], value[2]];
    }),
  };
}

/** The Venus-class cases' part of the deck's section, from each case's values and figures. */
function venusClassSectionOf(
  values: ReadonlyMap<VenusCaseName, TwinValues>,
  figures: ReadonlyMap<VenusCaseName, VenusFigures>,
): Record<string, unknown> {
  return {
    ruling:
      "main, 2026-11-01: the Venus-class cases are gated by their references' scalar radiance in every view, their twin the uncorrected twin; their vector I is kept in the files, never gated; thick/polarisationBake.test.ts's guard holds the polarisation the scalar reading neglects to 1% of I",
    cases: Object.fromEntries(
      VENUS_CASES.map((name) => {
        const held = values.get(name);
        const figure = figures.get(name);
        if (held === undefined || figure === undefined) {
          throw new Error(`the Venus-class gate has no values or figures of ${name}`);
        }
        return [name, venusSectionOf(name, held, figure)];
      }),
    ),
  };
}

/**
 * A Venus-class case baked through its deck, by hand (the module's remarks): its deck's bake on the
 * twin's body at the split its cover gives, its uncorrected twin's values, and the bake's figures.
 */
function venusGateBaked(caseName: VenusCaseName): {
  readonly values: TwinValues;
  readonly figures: VenusFigures;
} {
  const referenceCase = thickReferenceCases()[caseName];
  const lights = referenceCaseBakeLights(referenceCase);
  const body = twinBakeBody(referenceCase.figure);
  const split = venusSplit(referenceCase);
  const kept = bakeKeptDeck(referenceCase, lights, body, split);
  const twin = caseTwin(referenceCase, { deck: { tables: kept.tables, polarisation: null } });
  const values = valuesOf(twin, referenceCase, caseSpecs(referenceCase));
  const [first] = kept.tables;
  const upper = upperMediumOf(referenceCase.medium, split.altitudeM);
  const below = packBelowDeckTable(kept.tables, kernelMedium(referenceCase.medium));
  const words = new Float32Array(below.texels.buffer);
  // The table below's f32 values under f32's least normal, 2⁻¹²⁶, and above 0.
  const subnormal = words.reduce(
    (n, x) => (x !== 0 && Math.abs(x) < F32_LEAST_NORMAL ? n + 1 : n),
    0,
  );
  return {
    values,
    figures: {
      split,
      floorAltitudeM: first?.floorAltitudeM ?? Number.NaN,
      tables: {
        bands: kept.tables.length,
        slots: first?.slots.length ?? 0,
        deckBytes: deckTableBytes(packDeckTable(kept.tables)),
        aboveBytes: thickTableBytes(
          packThickTable(deckAboveTables(kept.tables), kernelMedium(upper)),
        ),
        belowBytes: belowDeckTableBytes(below),
        belowF32BelowLeastNormal: subnormal,
        deckSolves: first?.slots[0]?.solves ?? 0,
        aboveSolves: first?.above.slots[0]?.solves ?? 0,
      },
    },
  };
}

/** The committed record's Venus-class section of `caseName`, read back. */
function committedVenusSection(caseName: VenusCaseName): unknown {
  return sectionOf(sectionOf(sectionOf(committedDeck(), "venusClass"), "cases"), caseName);
}

/**
 * The committed Venus-class section's twin values of `caseName`: its geometries' rows' radiance and
 * surface share, and its downwelling rows.
 */
function committedVenusValues(caseName: VenusCaseName): Map<string, Rgb> {
  const section = committedVenusSection(caseName);
  return twinValuesOfRecord({
    format: CASE_GATE_FORMAT,
    cases: { [caseName]: { geometries: sectionOf(section, "geometries") } },
    twinValues: sectionOf(section, "downwelling"),
  });
}

/**
 * The committed Venus-class section's bake's figures of `caseName`.
 *
 * @throws Error for a record whose section lacks them.
 */
function committedVenusFigures(caseName: VenusCaseName): VenusFigures {
  const section = committedVenusSection(caseName);
  const split: unknown = sectionOf(section, "split");
  const altitudeM: unknown = sectionOf(split, "altitudeM");
  const floorAltitudeM: unknown = sectionOf(section, "floorAltitudeM");
  const tables: unknown = sectionOf(section, "tables");
  if (
    typeof altitudeM !== "number" ||
    typeof floorAltitudeM !== "number" ||
    typeof tables !== "object" ||
    tables === null
  ) {
    throw new Error(`vectorGate.json's Venus-class section holds no figures of ${caseName}`);
  }
  return {
    split: { altitudeM, floor: "ground" },
    floorAltitudeM,
    tables: Object.fromEntries(Object.entries(tables)),
  };
}

// --- R08.T14.d.4: titan-class through its route -------------------------------------------------

/** The Titan-class gate's case: R08.T12.b.2's `titan-class`, re-traced deep by R08.T12.h. */
const TITAN_CASE = "titan-class";

/** `titan-class` and its committed reference. */
function titanGateCase(): GateCase {
  return { referenceCase: titanClassReferenceCase(), reference: titanClassReference };
}

/** What the Titan-class section takes beside its twin's values: its bakes' own figures and cuts. */
interface TitanFigures {
  /** The case's tables as packed, as {@link tablesOf} gives a kept case's. */
  readonly tables: Record<string, unknown>;
  /** The route's tallies at each of {@link CUT_MODES}, by the record's name. */
  readonly byModes: Record<string, unknown>;
}

/** `titan-class`'s tallies by image against the vector I, from a twin's values of its geometries. */
function titanTalliesOf(values: TwinValues): unknown {
  return recordImagesOf(
    { cases: { [TITAN_CASE]: caseGateSection(titanGateCase(), values, "vector") } },
    TITAN_CASE,
  );
}

/**
 * `titan-class`'s gate through its route (the module's remarks): the route's tallies by image and
 * its ratios against the vector I, at the cut modes too, every geometry's row, the ground's
 * downwelling flux under each aggregate against the vector flux by the tolerance test, the tables,
 * and the downwelling values the geometries' rows do not hold.
 */
function titanSectionOf(values: TwinValues, figures: TitanFigures): Record<string, unknown> {
  const gateCase = titanGateCase();
  const { referenceCase, reference } = gateCase;
  const { regime, path } = routeOf(referenceCase);
  const section = caseGateSection(gateCase, values, "vector");
  const asRecord = { cases: { [TITAN_CASE]: section } };
  const valueOf = (aggregate: string): Rgb => downwellingOf(values, TITAN_CASE, aggregate);
  return {
    case: TITAN_CASE,
    regime: regime.kind,
    route: path,
    radiance: "vector",
    rule: "thick/routing.ts's hazeGatePasses: every image above the horizon passes somewhere and none misses (the deck's rule, deckGatePasses), the sun's images below the horizon TWILIGHT_REASON's",
    routed: recordImagesOf(asRecord, TITAN_CASE),
    routedRatios: recordRatiosOf(asRecord, TITAN_CASE),
    routedByModes: figures.byModes,
    geometries: sectionOf(section, "geometries"),
    groundIrradiance: deckGroundIrradianceOf(reference, "vector", valueOf),
    tables: figures.tables,
    downwelling: referenceCase.aggregates.map((a) => {
      const key = twinValueKey({
        quantity: "downwelling",
        setting: "case",
        caseName: TITAN_CASE,
        name: a.name,
      });
      const value = valueOf(a.name);
      return [key, value[0], value[1], value[2]];
    }),
  };
}

/**
 * `titan-class` baked through its route, by hand (the module's remarks): its bake and correction on
 * the twin's body, the route's twin's values, and the bakes' figures, the route's tallies at the cut
 * modes among them.
 */
function titanGateBaked(): { readonly values: TwinValues; readonly figures: TitanFigures } {
  const { referenceCase } = titanGateCase();
  const bakes = bakesOf(referenceCase);
  const specs = caseSpecs(referenceCase);
  const byModes = Object.fromEntries(
    CUT_MODES.map(([name, modes]) => {
      const cut = { ...bakes, bands: cutModes(bakes.bands, modes) };
      return [
        name,
        titanTalliesOf(valuesOf(twinOn("routed", referenceCase, cut), referenceCase, specs)),
      ];
    }),
  );
  return {
    values: valuesOf(twinOn("routed", referenceCase, bakes), referenceCase, specs),
    figures: { tables: tablesOf(referenceCase, bakes), byModes },
  };
}

/** The committed record's Titan-class section, read back. */
function committedTitanSection(): unknown {
  return Reflect.get(committed, "titanClass");
}

/**
 * The committed Titan-class section's twin values: its geometries' rows' radiance and surface
 * share, and its downwelling rows.
 */
function committedTitanValues(): Map<string, Rgb> {
  const section = committedTitanSection();
  return twinValuesOfRecord({
    format: CASE_GATE_FORMAT,
    cases: { [TITAN_CASE]: { geometries: sectionOf(section, "geometries") } },
    twinValues: sectionOf(section, "downwelling"),
  });
}

/**
 * The committed Titan-class section's figures.
 *
 * @throws Error for a record whose section lacks them.
 */
function committedTitanFigures(): TitanFigures {
  const section = committedTitanSection();
  const tables: unknown = sectionOf(section, "tables");
  const byModes: unknown = sectionOf(section, "routedByModes");
  if (typeof tables !== "object" || tables === null) {
    throw new Error("vectorGate.json's Titan-class section holds no tables");
  }
  if (typeof byModes !== "object" || byModes === null) {
    throw new Error("vectorGate.json's Titan-class section holds no tallies at the cut modes");
  }
  return {
    tables: Object.fromEntries(Object.entries(tables)),
    byModes: Object.fromEntries(Object.entries(byModes)),
  };
}

/**
 * The record of both paths' values, the route at the cut modes and each case's tables (the module's
 * remarks), each case's last two by `modes` and `tables`, `giant-deck`'s gate through its deck,
 * `deck` (R08.T15.a.2), and `titan-class`'s through its route, `titanClass` (R08.T14.d.4).
 */
function vectorGateRecord(
  paths: Readonly<Record<PathName, TwinValues>>,
  modes: (caseName: string) => unknown,
  tables: (caseName: string) => unknown,
  deck: unknown,
  titanClass: unknown,
): Record<string, unknown> {
  const routed = vectorRecordOf(paths.routed);
  const diagnostic = vectorRecordOf(paths.diagnostic);
  const uncorrected = caseGateRecord(CASES, C1_VALUES, { writer: WRITER, radiance: "vector" });
  return {
    description:
      "R08.T14.f's vector gates: R08.T13.c1's case gate re-run against the four committed references' vector radiance I with the polarisation correction, each case through the path classifyRegime gives it under a clear sky (routed, the bake at m <= 2 from R08.T14.d.1), beside the route with the bake cut to m = 0 alone and to m <= 1 (routedByModes, R08.T14.d.1), the analytic path with the correction (diagnostic), the uncorrected twin against the vector I, and c1's own record, the uncorrected twin against the scalar I (caseGate.json); R08.T13's surface irradiance against the vector flux aggregate, uncorrected; the ground's irradiance under every aggregate's sun on both paths, its sky's irradiance built over the thick table as the kernels' is (groundIrradiance, R08.T14.d.4); each case's tables; and giant-deck through its deck (R08.T15.a.2): its route over the geometries above the split, the deck's multiple-scattering part cut to m < 3 and m < 6, its sky-lit radiance at the deck's modes, its floor white, the case split as an envelope over its black floor and its white, each variant's tallies and twin ÷ reference by image, the bake's plane albedo against the aggregates and the deck's share of each orbit image (deck); its route below the split, the table below the deck uncorrected against the vector I, and the ground's flux against the vector flux (deck.routedBelow, R08.T15.b.2); and the Venus-class cases through their deck against their references' scalar radiance, the uncorrected twin above the split and the table below the deck beneath it, with the ground's flux against the scalar flux and R08.T12.b's flux sanity (deck.venusClass, R08.T15.b.2); and titan-class through its route, its whole column baked at m <= 2 with its correction, against the vector I, its tallies by image at m <= 2 and at the cut modes, with the ground's flux under each aggregate against the vector flux (titanClass, R08.T14.d.4). Verdicts recorded, never gated. Written by vectorGate.test.ts; do not edit by hand.",
    format: VECTOR_GATE_FORMAT,
    cases: Object.fromEntries(
      CASES.map(({ referenceCase }) => {
        const { name } = referenceCase;
        const { regime, path } = routeOf(referenceCase);
        return [
          name,
          {
            regime: regime.kind,
            route: path,
            tables: tables(name),
            routed: recordImagesOf(routed, name),
            routedByModes: modes(name),
            diagnostic: recordImagesOf(diagnostic, name),
            uncorrectedVector: recordImagesOf(uncorrected, name),
            c1: recordImagesOf(committedGate, name),
            routedRatios: recordRatiosOf(routed, name),
            diagnosticRatios: recordRatiosOf(diagnostic, name),
          },
        ];
      }),
    ),
    irradiance: sectionOf(uncorrected, "irradiance"),
    groundIrradiance: groundIrradianceSection(paths),
    routedMarsAureole: sectionOf(routed, "marsAureole"),
    routedSaturnOblateness: sectionOf(routed, "saturnOblateness"),
    twinValues: { routed: rowsOf(paths.routed), diagnostic: rowsOf(paths.diagnostic) },
    deck,
    titanClass,
  };
}

/** The deck's section: `giant-deck`'s (R08.T15.a.2) and the Venus-class cases' (R08.T15.b.2). */
function deckRecordOf(
  giant: Record<string, unknown>,
  venusClass: Record<string, unknown>,
): Record<string, unknown> {
  return { ...giant, venusClass };
}

/**
 * `giant-deck`'s deck baked, by hand (the module's remarks): the route's bake and the variants'
 * (the sky-lit radiance at the deck's modes, the floor white, split as an envelope over its black
 * floor and its white), each with its correction over the deck, their twins over the gated
 * geometries (the route's below the split too, and its ground's flux), and the deck's section.
 */
function deckGateBaked(): Record<string, unknown> {
  const referenceCase = thickReferenceCases()[DECK_CASE];
  const lights = referenceCaseBakeLights(referenceCase);
  const body = twinBakeBody(referenceCase.figure);
  const split = giantSplit(referenceCase, false);
  const envelopeSplit = giantSplit(referenceCase, true);
  const route = bakeKeptDeck(referenceCase, lights, body, split);
  const wide = bakeKeptDeck(referenceCase, lights, body, split, {
    band: { diffuseModes: DECK_MODES },
  });
  const white = bakeKeptDeck(referenceCase, lights, body, split, { deck: { floorAlbedo: 1 } });
  const envelope = bakeKeptDeck(referenceCase, lights, body, envelopeSplit);
  const whiteEnvelope = bakeKeptDeck(referenceCase, lights, body, envelopeSplit, {
    deck: { floorAlbedo: 1 },
  });
  const gateCase = deckGateCase(referenceCase, split.altitudeM);
  const envelopeCase = deckGateCase(referenceCase, envelopeSplit.altitudeM);
  const twinOf = (
    tables: ReadonlyArray<DeckTables>,
    correction: ReadonlyArray<PolarisationTable> | null,
    at: GateCase = gateCase,
  ): Map<string, Rgb> =>
    deckValuesOf(caseTwin(referenceCase, { deck: { tables, polarisation: correction } }), at);
  // The route's twin below the split as well, and its ground's flux (R08.T15.b.2).
  const routeTwin = caseTwin(referenceCase, {
    deck: { tables: route.tables, polarisation: route.correction },
  });
  const routeValues = new Map([
    ...deckValuesOf(routeTwin, gateCase),
    ...deckValuesOf(routeTwin, deckGateCase(referenceCase, split.altitudeM, "below")),
    ...valuesOf(routeTwin, referenceCase, downwellingSpecs(referenceCase)),
  ]);
  const values = new Map<DeckVariant, TwinValues>([
    ["route", routeValues],
    ["m < 3", twinOf(cutDeckModes(route.tables, 3), route.correction)],
    ["m < 6", twinOf(cutDeckModes(route.tables, 6), route.correction)],
    ["diffuse at 8", twinOf(wide.tables, wide.correction)],
    ["floor at 1", twinOf(white.tables, white.correction)],
    ["as an envelope", twinOf(envelope.tables, envelope.correction, envelopeCase)],
    [
      "as an envelope, floor at 1",
      twinOf(whiteEnvelope.tables, whiteEnvelope.correction, envelopeCase),
    ],
    ["no deck", twinOf(darkDeck(route.tables), route.correction)],
  ]);
  const [first] = route.tables;
  const upper = upperMediumOf(referenceCase.medium, split.altitudeM);
  const above = packThickTable(
    deckAboveTables(route.tables),
    kernelMedium(upper),
    route.correction === null
      ? null
      : { bands: deckCorrectionAbove(route.correction, split.altitudeM), medium: upper },
  );
  return deckSectionOf(values, {
    split,
    envelopeSplit,
    floorAltitudeM: first?.floorAltitudeM ?? Number.NaN,
    floorCoAlbedo: floorCoAlbedoOf(route, body),
    planeAlbedo: deckPlaneAlbedoOf(route, body),
    tables: {
      bands: route.tables.length,
      slots: first?.slots.length ?? 0,
      modes: first?.modes ?? 0,
      diffuseModes: first?.diffuseModes ?? 0,
      deckBytes: deckTableBytes(packDeckTable(route.tables)),
      aboveBytes: thickTableBytes(above),
      deckSolves: first?.slots[0]?.solves ?? 0,
      aboveSolves: first?.above.slots[0]?.solves ?? 0,
      corrected: route.correction !== null,
      // The phase's least entry by channel, sr⁻¹: a moments term's series less its delta may ring.
      phaseLeast: [0, 1, 2].map((c) =>
        recorded(
          Math.min(
            ...(first?.slots ?? []).flatMap((slot) =>
              Array.from(slot.phase).filter((_, i) => i % 3 === c),
            ),
          ),
        ),
      ),
    },
  });
}

/** The committed record's deck section, read back. */
function committedDeck(): unknown {
  return Reflect.get(committed, "deck");
}

/** The committed deck section's twin values, by variant. */
function committedDeckValues(): Map<DeckVariant, TwinValues> {
  const held = sectionOf(committedDeck(), "twinValues");
  return new Map(
    DECK_VARIANTS.map((name) => {
      const rows: unknown = sectionOf(held, name);
      const values = new Map<string, Rgb>();
      if (Array.isArray(rows)) {
        for (const row of rows) {
          if (
            Array.isArray(row) &&
            typeof row[0] === "string" &&
            typeof row[1] === "number" &&
            typeof row[2] === "number" &&
            typeof row[3] === "number"
          ) {
            values.set(row[0], [row[1], row[2], row[3]]);
          }
        }
      }
      return [name, values];
    }),
  );
}

/**
 * Three numbers at `at` of a committed row.
 *
 * @throws Error for a row whose values there are not numbers.
 */
function rgbOfRow(row: ReadonlyArray<unknown>, at: number): Rgb {
  const [r, g, b] = [row[at], row[at + 1], row[at + 2]];
  if (typeof r !== "number" || typeof g !== "number" || typeof b !== "number") {
    throw new Error("vectorGate.json's deck section holds a value not a number");
  }
  return [r, g, b];
}

/**
 * The committed deck section's bakes' figures, read back as its JSON holds them.
 *
 * @throws Error for a record whose deck section lacks one.
 */
function committedDeckFigures(): DeckFigures {
  const deck = committedDeck();
  const splitOf = (value: unknown): DeckSplit => {
    const altitudeM: unknown = sectionOf(value, "altitudeM");
    const floor: unknown = sectionOf(value, "floor");
    if (typeof altitudeM !== "number" || (floor !== "ground" && floor !== "envelope")) {
      throw new Error("vectorGate.json's deck section holds no split");
    }
    return { altitudeM, floor };
  };
  const floorAltitudeM: unknown = sectionOf(deck, "floorAltitudeM");
  const coAlbedo: unknown = sectionOf(deck, "floorCoAlbedo");
  const planeAlbedo: unknown = sectionOf(deck, "planeAlbedo");
  const tables: unknown = sectionOf(deck, "tables");
  if (
    typeof floorAltitudeM !== "number" ||
    !Array.isArray(coAlbedo) ||
    !Array.isArray(planeAlbedo) ||
    typeof tables !== "object" ||
    tables === null
  ) {
    throw new Error("vectorGate.json's deck section holds no bakes' figures");
  }
  return {
    split: splitOf(sectionOf(deck, "split")),
    envelopeSplit: splitOf(sectionOf(sectionOf(deck, "asAnEnvelope"), "split")),
    floorAltitudeM,
    floorCoAlbedo: rgbOfRow(coAlbedo, 0),
    planeAlbedo: planeAlbedo.map((row: unknown) => {
      if (!Array.isArray(row) || typeof row[0] !== "string") {
        throw new Error("vectorGate.json's deck section holds a plane albedo row not one");
      }
      const [r, g, b] = rgbOfRow(row, 1);
      return [row[0], r, g, b] as const;
    }),
    tables: Object.fromEntries(Object.entries(tables)),
  };
}

describe.skipIf(!BAKES)("R08.T14.f's vector gates, baked (by hand)", () => {
  it(
    "bakes the four cases, titan-class and their corrections, gates both paths' twins against the vector I, and writes vectorGate.json",
    { timeout: LIMIT_MS.whole },
    async () => {
      const paths: Record<PathName, Map<string, Rgb>> = {
        routed: new Map(),
        diagnostic: new Map(),
      };
      const cuts = new Map(CUT_MODES.map(([name]) => [name, new Map<string, Rgb>()]));
      const tables = new Map<string, unknown>();
      for (const { referenceCase } of CASES) {
        const bakes = bakesOf(referenceCase);
        for (const path of ["routed", "diagnostic"] as const) {
          for (const [key, value] of valuesOf(
            twinOn(path, referenceCase, bakes),
            referenceCase,
            PATH_SPECS,
          )) {
            paths[path].set(key, value);
          }
        }
        for (const [name, modes] of CUT_MODES) {
          const cut = { ...bakes, bands: cutModes(bakes.bands, modes) };
          for (const [key, value] of valuesOf(
            twinOn("routed", referenceCase, cut),
            referenceCase,
            PATH_SPECS,
          )) {
            cuts.get(name)?.set(key, value);
          }
        }
        tables.set(referenceCase.name, tablesOf(referenceCase, bakes));
      }
      const venusValues = new Map<VenusCaseName, TwinValues>();
      const venusFigures = new Map<VenusCaseName, VenusFigures>();
      for (const name of VENUS_CASES) {
        const baked = venusGateBaked(name);
        venusValues.set(name, baked.values);
        venusFigures.set(name, baked.figures);
      }
      const titan = titanGateBaked();
      const record = vectorGateRecord(
        paths,
        cutModesRecord(cuts),
        (name) => tables.get(name) ?? null,
        deckRecordOf(deckGateBaked(), venusClassSectionOf(venusValues, venusFigures)),
        titanSectionOf(titan.values, titan.figures),
      );
      await expect(expectCommittedTable(record, committed, RECORD_FILE)).resolves.toBeUndefined();
    },
  );
});

describe.skipIf(BAKES)("R08.T14.f's vector gates' record", () => {
  it(
    "holds the route's verdicts its own values give against the committed references",
    { timeout: LIMIT_MS.record },
    () => {
      // Through JSON, as the record is committed.
      const formed: unknown = JSON.parse(
        JSON.stringify({
          ...routedSections(vectorRecordOf(committedValues("routed"))),
          groundIrradiance: groundIrradianceSection({
            routed: committedValues("routed"),
            diagnostic: committedValues("diagnostic"),
          }),
        }),
      );
      expect(formed).toEqual({
        ...committedRoutedSections(),
        groundIrradiance: sectionOf(committed, "groundIrradiance"),
      });
    },
  );

  it(
    "holds titan-class's tallies, rows and ground's flux its own values give against its committed reference",
    { timeout: LIMIT_MS.record },
    () => {
      // Through JSON, as the record is committed.
      const formed: unknown = JSON.parse(
        JSON.stringify(titanSectionOf(committedTitanValues(), committedTitanFigures())),
      );
      expect(formed).toEqual(committedTitanSection());
    },
  );

  it.skipIf(!SLOW)(
    "is the record its own values give against the committed references, byte for byte",
    { timeout: LIMIT_MS.record },
    async () => {
      const record = vectorGateRecord(
        { routed: committedValues("routed"), diagnostic: committedValues("diagnostic") },
        (name) => committedCaseSection(name, "routedByModes"),
        (name) => committedCaseSection(name, "tables"),
        deckRecordOf(
          deckSectionOf(committedDeckValues(), committedDeckFigures()),
          venusClassSectionOf(
            new Map(VENUS_CASES.map((name) => [name, committedVenusValues(name)])),
            new Map(VENUS_CASES.map((name) => [name, committedVenusFigures(name)])),
          ),
        ),
        titanSectionOf(committedTitanValues(), committedTitanFigures()),
      );
      await expect(expectCommittedTable(record, committed, RECORD_FILE)).resolves.toBeUndefined();
    },
  );

  it(
    "holds the values earth's route gives now, at its first ground views",
    { timeout: LIMIT_MS.record },
    () => {
      const twin = twinOn("routed", EARTH_CASE, bakesOf(EARTH_CASE));
      const now = valuesOf(
        twin,
        EARTH_CASE,
        PATH_SPECS.filter((spec) => SAMPLED.includes(twinValueKey(spec))),
      );
      const held = committedValues("routed");
      expect(SAMPLED.length).toBe(3);
      expect(SAMPLED.map((key) => now.get(key))).toEqual(SAMPLED.map((key) => held.get(key)));
    },
  );
});
