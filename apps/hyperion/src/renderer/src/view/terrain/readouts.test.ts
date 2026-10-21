import type { PaletteEntryDto, SurfaceHeaderDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import classSource from "../../../../../../../crates/hyperion-surface/src/material/class.rs?raw";
import readingSource from "../../../../../../../crates/hyperion-surface/src/reading.rs?raw";
import wasmSource from "../../../../../../../crates/hyperion-surface/src/wasm.rs?raw";
import { add, norm, scale, sub, vec3, type Vec3 } from "../../geometry/vec3";
import { substanceName } from "../../lib/substances";
import { moonLikeHeader } from "../../test/coarseFieldFixtures";
import { aBody, aViewScene, FIXTURE_PLANET } from "../../test/viewFixtures";
import type { CameraPose } from "../camera/pose";
import { project } from "../camera/projection";
import { lookAlong, quaternionFromAxisAngle, rotationRows } from "../camera/quaternion";
import { relativeToCamera } from "../coords/relative";
import { rotateToBody, rotation3FromRows } from "../coords/rotation";
import { sceneOrigins } from "../scene/model";
import { TERRAIN_COMPUTATION_FAILED, TERRAIN_NOT_AVAILABLE } from "./annunciation";
import type { FieldFailure, FieldStatus } from "./coarseFieldStore";
import { CoverageView, fieldCellCount } from "./coverage";
import {
  CAPACITY_FIELD_CH,
  COVERAGE_FIELD_CH,
  datumLine,
  ELEVATION_FIELD_CH,
  ELEVATION_UNCERTAINTY_CH,
  formatCoverage,
  formatElevationReading,
  formatSlantRange,
  formatSlopeReading,
  formatSurveyResolution,
  formatWithUncertainty,
  FRICTION_FIELD_CH,
  groundReadoutLines,
  lineOfSightOf,
  LONGEST_BASELINE_M,
  materialFormName,
  MAX_SIGHT_M,
  MIN_SIGHT_ORIGIN_M,
  NO_BREAK_SPACE,
  NO_READOUT_UNITS,
  NO_SIGHT_CENTRE,
  NO_SIGHT_FAR,
  NO_SIGHT_ROTATION,
  NO_SIGHT_ROTATION_NOT_RESOLVED,
  type ReadoutContext,
  type ReadoutLine,
  readoutLineText,
  type ReadoutSegment,
  type ReadoutUnits,
  RESOLUTION_FIELD_CH,
  type SightGap,
  sightGapLine,
  sightGapOf,
  SLANT_RANGE_FIELD_CH,
  SLANT_RANGE_UNCERTAINTY_CH,
  SLOPE_BASELINES_M,
  SLOPE_FIELD_CH,
  SLOPE_UNCERTAINTY_CH,
  STALE_MARK_CH,
  surveyLines,
  tallestTerrainLines,
  TEMPERATURE_FIELD_CH,
  type TerrainLabelInput,
  terrainLabelModel,
  terrainReadoutStands,
} from "./readouts";
import type { TerrainSurvey } from "./session";
import type {
  SurfaceGroundProperties,
  SurfaceGroundReadout,
  SurfaceReading,
  SurfaceSlantRange,
} from "./workers/messages";

/** `±` with its no-break spaces, as every Estimated reading writes it. */
const PM = `${NO_BREAK_SPACE}±${NO_BREAK_SPACE}`;

/** Radians from degrees. */
function rad(deg: number): number {
  return (deg * Math.PI) / 180;
}

/** A palette entry of `substance` as `role`, its photometry stand-ins, since no test reads them. */
function entry(substance: string, role: PaletteEntryDto["role"]): PaletteEntryDto {
  return { substance, role, normal_albedo_bvr: [0.1, 0.12, 0.14], phase_row: 1, liquid: null };
}

/**
 * The Moon-like test header with a palette of three: an Fe-rich basalt crust, water ice, and a
 * substance the console's list does not name.
 */
const HEADER: SurfaceHeaderDto = {
  ...moonLikeHeader(FIXTURE_PLANET),
  palette: [
    entry("basalt_fe", "primary_crust"),
    entry("H2O", "ice"),
    entry("not_a_listed_substance", "deposit"),
  ],
};

/** The test header on a world with a sea at -2.4 m, its main liquid water. */
const SEA_HEADER: SurfaceHeaderDto = {
  ...moonLikeHeader(FIXTURE_PLANET),
  equatorial_radius_m: 6_378_137,
  polar_radius_m: 6_356_752,
  sea_level_m: -2.4,
  palette: [entry("basalt", "primary_crust"), { ...entry("H2O", "liquid"), liquid: null }],
  main_liquid: 1,
};

/** A reading at the point: the guide's Moon-like example by default, unresolved under 410 m. */
function aReading(overrides: Partial<SurfaceReading> = {}): SurfaceReading {
  return {
    elevationM: 2_143.7,
    elevationSigmaM: 180,
    slopeRmsRad: rad(12),
    slopeSigmaRad: rad(4),
    slopeBaselineM: 2,
    resolutionM: 410,
    resolved: false,
    ...overrides,
  };
}

/** A slant range: `~41.2 km ± 0.3 km` by default. */
function aSight(overrides: Partial<SurfaceSlantRange> = {}): SurfaceSlantRange {
  return {
    rangeM: 41_234,
    sigmaM: 290,
    resolved: false,
    hitM: [1_738_000, 0, 0],
    at: { face: 0, u: 0, v: 0 },
    ...overrides,
  };
}

/** The ground's properties: lunar regolith's on a 12° slope, its friction NAVFAC's 0.20. */
function someProperties(overrides: Partial<SurfaceGroundProperties> = {}): SurfaceGroundProperties {
  return {
    bearingCapacityPa: 3_432_000,
    capacitySlopeRad: rad(12),
    padFriction: 0.2,
    frictionOnTemperature: false,
    temperatureK: 250,
    ...overrides,
  };
}

/** A readout where the line of sight meets surveyed regolith of the Fe-rich basalt. */
function groundAt(
  options: {
    readonly reading?: Partial<SurfaceReading>;
    readonly sight?: Partial<SurfaceSlantRange>;
    readonly classCode?: number;
    readonly entry?: number | null;
    readonly properties?: Partial<SurfaceGroundProperties>;
  } = {},
): SurfaceGroundReadout {
  return {
    kind: "ground",
    sight: aSight(options.sight),
    reading: aReading(options.reading),
    material: {
      classCode: options.classCode ?? 1,
      entry: options.entry === undefined ? 0 : options.entry,
    },
    properties: someProperties(options.properties),
  };
}

/** The readout's context: live, from the hull, under the reference pad. */
function context(overrides: Partial<ReadoutContext> = {}): ReadoutContext {
  return { stale: false, slantFromCamera: false, padWidthM: 1, ...overrides };
}

/** The readout's lines' texts. */
function readoutTexts(
  readout: SurfaceGroundReadout,
  overrides: Partial<ReadoutContext> = {},
  header: SurfaceHeaderDto = HEADER,
): string[] {
  return groundReadoutLines(readout, header, context(overrides), NO_READOUT_UNITS).lines.map(
    readoutLineText,
  );
}

/** The readout's line labelled `label`. */
function lineOf(lines: ReadonlyArray<ReadoutLine>, label: string): ReadoutLine {
  const line = lines.find((each) => each.kind === "reading" && each.label === label);
  if (line === undefined) {
    throw new Error(`no line ${label}`);
  }
  return line;
}

/** The text of the readout's line labelled `label`. */
function textOfLine(
  readout: SurfaceGroundReadout,
  label: string,
  overrides: Partial<ReadoutContext> = {},
): string {
  const { lines } = groundReadoutLines(readout, HEADER, context(overrides), NO_READOUT_UNITS);
  return readoutLineText(lineOf(lines, label));
}

/**
 * A segment as B612 Mono sets it: each field padded to its width, its `S` inside it after its text
 * where it holds one, and each kept place 2 ch.
 */
function laidOut(segment: ReadoutSegment): string {
  return segment
    .map((piece) => {
      let shown: string;
      switch (piece.kind) {
        case "field": {
          const text = piece.staleMark ? `${piece.text} S` : piece.text;
          shown = piece.align === "end" ? text.padStart(piece.widthCh) : text.padEnd(piece.widthCh);
          break;
        }
        case "mark":
          shown = piece.stale ? " S" : "  ";
          break;
        case "text":
        case "status":
          shown = piece.text;
          break;
      }
      return shown;
    })
    .join("");
}

/** A reading line as B612 Mono sets it, its segments joined by the middle dot. */
function laidOutLine(line: ReadoutLine): string {
  return line.kind === "reading"
    ? `${line.label} ${line.segments.map(laidOut).join(" · ")}`
    : line.text;
}

/** A coverage of level 5 with `cells` surveyed at code `code` from cell 0, or none. */
function coverageOf(cells: number, code = 62): CoverageView {
  const ranges = cells === 0 ? [] : [{ start: 0, end: cells, resolution: code }];
  const merged = CoverageView.empty(5).merged({ ranges }, 1);
  if (!merged.ok) {
    throw new Error(merged.message);
  }
  return merged.coverage;
}

/** A field's standing: wanted, answered, nothing failed. */
function standing(overrides: Partial<FieldStatus> = {}): FieldStatus {
  return { reason: "local-body", pending: false, notModelled: null, failure: null, ...overrides };
}

/** A survey of the test body: held, live, its first cells surveyed at about 2 m by default. */
function aSurvey(overrides: Partial<TerrainSurvey> = {}): TerrainSurvey {
  return {
    status: standing(),
    coverage: coverageOf(77),
    header: HEADER,
    stale: false,
    ...overrides,
  };
}

/** The survey line's texts and colours. */
function surveyShown(survey: TerrainSurvey | null): Array<[string, string]> {
  return surveyLines(survey, NO_READOUT_UNITS).lines.map((line) => [
    readoutLineText(line),
    line.kind === "status" ? line.colour : "reading",
  ]);
}

/** Every line a test makes, for the checks that hold of all of them. */
function everyLine(): ReadoutLine[] {
  const rejected: FieldFailure = {
    kind: "rejected",
    code: "internal",
    message: "the field could not be computed",
    refusal: false,
  };
  return [
    ...groundReadoutLines(groundAt(), HEADER, context(), NO_READOUT_UNITS).lines,
    ...groundReadoutLines(groundAt(), HEADER, context({ stale: true }), NO_READOUT_UNITS).lines,
    ...surveyLines(aSurvey({ status: standing({ failure: rejected }) }), NO_READOUT_UNITS).lines,
    datumLine(SEA_HEADER, true),
  ];
}

/**
 * The place of an uncertainty's last digit kept, by the rule taken apart from the module: its
 * three highest digits from `toExponential`'s mantissa, floored, two kept for 100 to 354, else
 * one, or the two of 1000; never finer than a step of 0.01's place, -2.
 */
function lastPlace(sigma: number): number {
  const [mantissa, exponent] = sigma.toExponential().split("e");
  const digits = Math.floor(Number(mantissa) * 100 * (1 + 1e-9));
  return Math.max(-2, digits <= 354 ? Number(exponent) - 1 : Number(exponent));
}

/** An Estimated slant range of `rangeM` ± `sigmaM`, as its value and uncertainty read. */
function estimatedRange(rangeM: number, sigmaM: number): readonly [string, string | null] {
  const range = formatSlantRange({ rangeM, sigmaM, resolved: false }, null);
  return [range.value, range.uncertainty];
}

/** The value of the `f64` constant `name` in the surface crate's `wasm.rs`. */
function wasmConstant(name: string): number {
  const found = new RegExp(String.raw`^const ${name}: f64 = ([\d.e]+);$`, "mu").exec(wasmSource);
  if (found?.[1] === undefined) {
    throw new Error(`wasm.rs defines no ${name}`);
  }
  return Number(found[1]);
}

/** The digits after a number's decimal point. */
function decimals(shown: string): number {
  return shown.split(".")[1]?.length ?? 0;
}

describe("formatWithUncertainty, the Particle Data Group's rule (Design note 6)", () => {
  it.each([
    // The guide's three examples: two figures, one, and up to 1000 with two.
    [2_143.7, 180, "2140", "180"],
    [2_143.7, 480, "2100", "500"],
    [2_143.7, 960, "2100", "1000"],
  ])("rounds %f ± %f m to %s ± %s", (value, sigma, shownValue, shownSigma) => {
    expect(formatWithUncertainty(value, sigma, 0.1)).toEqual({
      value: shownValue,
      uncertainty: shownSigma,
    });
  });

  it.each([
    // 354 keeps two figures, 355 one; 949 keeps one, 950 rounds up to 1000 and keeps two. The
    // digits are the three highest as they stand: 354.9's are 354.
    [354, "21,240", "350"],
    [354.9, "21,240", "350"],
    [355, "21,200", "400"],
    [949, "21,200", "900"],
    [949.9, "21,200", "900"],
    [950, "21,200", "1000"],
  ])("decides at the breakpoint %f by its three highest digits", (sigma, value, uncertainty) => {
    expect(formatWithUncertainty(21_236.4, sigma, 0.1)).toEqual({ value, uncertainty });
  });

  it.each([
    // The same breakpoints as binary fractions, whose floats lie a hair below 355 and 950 of the
    // third digit: read through the tolerance, not truncated.
    [0.354, "1.23", "0.35"],
    [0.3549, "1.23", "0.35"],
    [0.355, "1.2", "0.4"],
    [0.949, "1.2", "0.9"],
    [0.95, "1.2", "1.0"],
  ])("takes %f's three highest digits at their decimal value", (sigma, value, unc) => {
    expect(formatWithUncertainty(1.234_56, sigma, 0.001)).toEqual({ value, uncertainty: unc });
  });

  it.each([
    // Finer than an elevation's 0.1 m: the rule's figure up to whole steps, never down.
    [0.12, "2143.7", "0.2"],
    [0.04, "2143.7", "0.1"],
    [0.149, "2143.7", "0.2"],
    // The rule's 0.10, already one step: not the raw σ's 1.004 steps, which would read 0.2.
    [0.1004, "2143.7", "0.1"],
    [0.101, "2143.7", "0.1"],
    [0.0951, "2143.7", "0.1"],
    [0.1, "2143.7", "0.1"],
    [0, "2143.7", "0.1"],
  ])("rounds an uncertainty of %f m up to one step of 0.1 m", (sigma, value, uncertainty) => {
    expect(formatWithUncertainty(2_143.73, sigma, 0.1)).toEqual({ value, uncertainty });
  });

  it("holds a slope's 1.02°, the rule's 1.0, to one whole degree", () => {
    expect(formatWithUncertainty(12.3, 1.02, 1)).toEqual({ value: "12", uncertainty: "1" });
  });

  it("rounds the value to the nearest step where the uncertainty is held to it", () => {
    expect(formatWithUncertainty(2_143.76, 0.04, 0.1).value).toBe("2143.8");
  });

  it("never shows the value finer than its uncertainty", () => {
    for (const value of [-410.04, 0.37, 7.5, 2_143.7, 41_234.56, 1.234e6]) {
      for (const sigma of [0.012, 0.37, 3.54, 35.5, 96, 1_234, 98_765]) {
        const shown = formatWithUncertainty(value, sigma, 0.01);
        const place = lastPlace(sigma);
        expect([decimals(shown.value), decimals(shown.uncertainty)]).toEqual([
          Math.max(0, -place),
          Math.max(0, -place),
        ]);
        const steps = Number(shown.value.replaceAll(",", "")) / 10 ** place;
        expect(Math.abs(steps - Math.round(steps))).toBeLessThan(1e-6);
        expect(Math.abs(steps * 10 ** place - value)).toBeLessThanOrEqual(10 ** place / 2 + 1e-9);
      }
    }
  });

  it("writes a negative value with -, groups from five digits, and drops the sign of a zero", () => {
    expect(formatWithUncertainty(-410.04, 180, 0.1).value).toBe("-410");
    expect(formatWithUncertainty(12_486, 230, 0.1).value).toBe("12,490");
    expect(formatWithUncertainty(-40, 960, 0.1).value).toBe("0");
  });

  it("refuses a step that is not a power of ten, and an uncertainty that is not a number", () => {
    expect(() => formatWithUncertainty(1, 0.2, 0.2)).toThrow(RangeError);
    expect(() => formatWithUncertainty(1, Number.NaN, 0.1)).toThrow(RangeError);
    expect(() => formatWithUncertainty(1, -1, 0.1)).toThrow(RangeError);
  });
});

describe("the readings' formats", () => {
  it("writes a resolved ELEVATION to 0.1 m, - below the datum, grouped from five digits", () => {
    expect(formatElevationReading(2_143.7, null)).toEqual({ value: "2143.7 m", uncertainty: null });
    expect(formatElevationReading(-410, null).value).toBe("-410.0 m");
    expect(formatElevationReading(12_480, null).value).toBe("12,480.0 m");
  });

  it("writes an Estimated ELEVATION with its uncertainty, the unit on both", () => {
    expect(formatElevationReading(2_143.7, 180)).toEqual({
      value: "~2140 m",
      uncertainty: `±${NO_BREAK_SPACE}180 m`,
    });
    expect(formatElevationReading(2_143.73, 0.12).uncertainty).toBe(`±${NO_BREAK_SPACE}0.2 m`);
  });

  it("writes a SLOPE in whole degrees, and Estimated with its uncertainty", () => {
    expect(formatSlopeReading(rad(7.2), null)).toEqual({ value: "7°", uncertainty: null });
    expect(formatSlopeReading(rad(12.3), rad(3.6))).toEqual({
      value: "~12°",
      uncertainty: `±${NO_BREAK_SPACE}4°`,
    });
    expect(formatSlopeReading(rad(12.3), rad(1.02)).uncertainty).toBe(`±${NO_BREAK_SPACE}1°`);
  });

  it("writes a resolved SLANT RANGE at its three figures and units", () => {
    expect(formatSlantRange({ rangeM: 412.3, sigmaM: null, resolved: true }, null)).toEqual({
      value: "412 m",
      uncertainty: null,
      grazing: false,
      unit: "m",
    });
  });

  it("rounds an Estimated SLANT RANGE by its uncertainty in its unit, the third figure its step", () => {
    const shown = formatSlantRange({ rangeM: 41_234, sigmaM: 290, resolved: false }, null);
    expect([shown.value, shown.uncertainty]).toEqual(["~41.2 km", `±${NO_BREAK_SPACE}0.3 km`]);
    // 0.31 km is finer than the range's 0.1 km step: up to whole steps, never down.
    const up = formatSlantRange({ rangeM: 41_234, sigmaM: 312, resolved: false }, null);
    expect(up.uncertainty).toBe(`±${NO_BREAK_SPACE}0.4 km`);
    const fine = formatSlantRange({ rangeM: 41_234, sigmaM: 4, resolved: false }, null);
    expect([fine.value, fine.uncertainty]).toEqual(["~41.2 km", `±${NO_BREAK_SPACE}0.1 km`]);
    const coarse = formatSlantRange({ rangeM: 41_234, sigmaM: 12_300, resolved: false }, null);
    expect([coarse.value, coarse.uncertainty]).toEqual(["~41 km", `±${NO_BREAK_SPACE}12 km`]);
  });

  it("writes an Estimated SLANT RANGE below 1 cm in E notation where plain digits would run past four decimals", () => {
    expect(estimatedRange(5.0e-3, 3.4e-4)).toEqual(["~5.00E-3 m", `±${NO_BREAK_SPACE}3.4E-4 m`]);
    // The rule's 3.4E-6 is finer than the range's third figure, 1E-5: up to one step.
    expect(estimatedRange(5.0e-3, 3.4e-6)).toEqual(["~5.00E-3 m", `±${NO_BREAK_SPACE}1E-5 m`]);
    expect(estimatedRange(5.0e-4, 0)).toEqual(["~5.00E-4 m", `±${NO_BREAK_SPACE}1E-6 m`]);
    // An uncertainty that holds the range to four decimals or fewer keeps plain digits.
    expect(estimatedRange(5.0e-3, 3e-3)).toEqual(["~0.0050 m", `±${NO_BREAK_SPACE}0.0030 m`]);
    expect(estimatedRange(5.0e-3, 0.4)).toEqual(["~0.0 m", `±${NO_BREAK_SPACE}0.4 m`]);
  });

  it("writes a grazing SLANT RANGE Estimated at its own precision, with no uncertainty", () => {
    expect(formatSlantRange({ rangeM: 41_234, sigmaM: null, resolved: false }, null)).toEqual({
      value: "~41.2 km",
      uncertainty: null,
      grazing: true,
      unit: "km",
    });
  });

  it("never reads GRAZING on a resolved range", () => {
    expect(formatSlantRange({ rangeM: 41_234, sigmaM: null, resolved: true }, null).grazing).toBe(
      false,
    );
  });

  it("keeps the range's unit within its hysteresis from the one passed in", () => {
    expect(formatSlantRange({ rangeM: 1_020, sigmaM: null, resolved: true }, "m")).toMatchObject({
      value: "1020 m",
      unit: "m",
    });
    expect(formatSlantRange({ rangeM: 1_020, sigmaM: null, resolved: true }, null).unit).toBe("km");
  });

  it.each<[number, "m" | "km" | null, string, "m" | "km"]>([
    [1.974, null, "2.0", "m"],
    [410.3, null, "410", "m"],
    [1_490, null, "1.5", "km"],
    [0.01, null, "0.010", "m"],
    // Rounds to 1 km, which reads in km; within 5% of 1 km the unit shown is kept.
    [999.6, null, "1.0", "km"],
    [1_040, "m", "1000", "m"],
    [1_060, "m", "1.1", "km"],
    [960, "km", "0.96", "km"],
    [940, "km", "940", "m"],
  ])(
    "writes a survey resolution of %f m, shown before in %s, as %s %s",
    (m, previous, value, unit) => {
      expect(formatSurveyResolution(m, previous)).toEqual({ value, unit });
    },
  );
});

describe("the material forms", () => {
  /** `MaterialClass`'s variants as the guide names their forms. */
  const FORM_OF_VARIANT: Readonly<Record<string, string>> = {
    Bedrock: "BEDROCK",
    Regolith: "REGOLITH",
    Soil: "SOIL",
    Sand: "SAND",
    Sediment: "SEDIMENT",
    VolcanicRock: "VOLCANIC ROCK",
    Melt: "MELT",
    Glacier: "ICE",
    Snow: "SNOW",
    SeaIce: "SEA ICE",
    Frost: "FROST",
    OrganicSediment: "ORGANIC SEDIMENT",
    Seabed: "SEABED",
    Evaporite: "EVAPORITE",
    Dust: "DUST",
  };

  it("names every MaterialClass code of class.rs by the guide's form, and no other code", () => {
    const codes = [...classSource.matchAll(/^\s*(\w+) = (\d+),$/gmu)].map(
      ([, variant, code]) => [variant ?? "", Number(code)] as const,
    );
    expect(codes).toHaveLength(15);
    for (const [variant, code] of codes) {
      expect([variant, materialFormName(code)]).toEqual([variant, FORM_OF_VARIANT[variant]]);
    }
    expect(materialFormName(codes.length)).toBeNull();
    expect(materialFormName(1.5)).toBeNull();
  });
});

describe("the survey line (decision-r10-t16-leans §12)", () => {
  it("reads the coverage to three figures and the finest resolution to two", () => {
    const coverage = coverageOf(77);
    expect(surveyShown(aSurvey({ coverage }))).toEqual([
      [`SURVEY COVERAGE ${formatCoverage(coverage)} · FINEST 2.0 m`, "reading"],
    ]);
  });

  it.each([
    [0.124, "12.4 %"],
    [2e-7, "2.00E-5 %"],
    [0.000_012_3, "0.00123 %"],
    [0.999_5, "ABOVE 99.9 %"],
    [0.999_4, "99.9 %"],
    [1, "100 %"],
  ])("writes a share of %f as %s", (fraction, shown) => {
    expect(formatCoverage({ surveyedFraction: () => fraction, bestResolutionM: () => 2 })).toBe(
      shown,
    );
  });

  it("reads 100 % only for the whole body, and ABOVE 99.9 % for all but one cell", () => {
    expect(formatCoverage(coverageOf(fieldCellCount(5)))).toBe("100 %");
    const allButOne = CoverageView.empty(5).merged(
      { ranges: [{ start: 1, end: fieldCellCount(5), resolution: 62 }] },
      1,
    );
    expect(allButOne.ok ? formatCoverage(allButOne.coverage) : null).toBe("ABOVE 99.9 %");
  });

  it("reads NONE with no FINEST part where no cell is surveyed, and NONE S stale", () => {
    expect(surveyShown(aSurvey({ coverage: coverageOf(0) }))).toEqual([
      ["SURVEY COVERAGE NONE", "reading"],
    ]);
    expect(surveyShown(aSurvey({ coverage: coverageOf(0), stale: true }))).toEqual([
      ["SURVEY COVERAGE NONE S", "reading"],
    ]);
  });

  it("marks each value stale with its S while the link is down", () => {
    const coverage = coverageOf(77);
    expect(surveyShown(aSurvey({ coverage, stale: true }))).toEqual([
      [`SURVEY COVERAGE ${formatCoverage(coverage)} S · FINEST 2.0 m S`, "reading"],
    ]);
  });

  it("reads PENDING until the survey first arrives, and while a busy server is asked again", () => {
    const pending: Array<[string, string]> = [["SURVEY COVERAGE: PENDING", "text"]];
    expect(surveyShown(null)).toEqual(pending);
    const none = { coverage: null, header: null };
    expect(surveyShown(aSurvey({ ...none, status: standing({ pending: true }) }))).toEqual(pending);
    expect(surveyShown(aSurvey(none))).toEqual(pending);
  });

  it("leaves a held survey's line as it stood while a busy server is asked again", () => {
    const coverage = coverageOf(77);
    expect(surveyShown(aSurvey({ coverage, status: standing({ pending: true }) }))).toEqual([
      [`SURVEY COVERAGE ${formatCoverage(coverage)} · FINEST 2.0 m`, "reading"],
    ]);
  });

  it("reads TERRAIN: NOT YET MODELLED in the line's place for a body not modelled", () => {
    const notModelled = aSurvey({
      status: standing({ notModelled: "surface" }),
      coverage: null,
      header: null,
    });
    expect(surveyShown(notModelled)).toEqual([["TERRAIN: NOT YET MODELLED", "text"]]);
  });

  it("reads the server's refusal in --text and its own failure in --status-caution", () => {
    const refused = standing({
      failure: { kind: "rejected", code: "unknown_body", message: "no such body", refusal: true },
    });
    const failed = standing({
      failure: { kind: "rejected", code: "internal", message: "out of memory", refusal: false },
    });
    const none = { coverage: null, header: null };
    expect(surveyShown(aSurvey({ ...none, status: refused }))).toEqual([
      ["SURVEY COVERAGE REJECTED: no such body, reopen VIEW to retry", "text"],
    ]);
    expect(surveyShown(aSurvey({ ...none, status: failed }))).toEqual([
      ["SURVEY COVERAGE REJECTED: out of memory, reopen VIEW to retry", "status-caution"],
    ]);
  });

  it("reads a check that failed as SURVEY DATA INVALID in --status-caution", () => {
    const invalid = standing({ failure: { kind: "invalid", check: "payload cut short" } });
    expect(surveyShown(aSurvey({ coverage: null, header: null, status: invalid }))).toEqual([
      ["SURVEY DATA INVALID: payload cut short, reopen VIEW to retry", "status-caution"],
    ]);
  });

  it("keeps a held survey's values stale and states the failure on a line of its own after it", () => {
    const coverage = coverageOf(77);
    const failure: FieldFailure = { kind: "invalid", check: "coverage unusable" };
    expect(surveyShown(aSurvey({ coverage, stale: true, status: standing({ failure }) }))).toEqual([
      [`SURVEY COVERAGE ${formatCoverage(coverage)} S · FINEST 2.0 m S`, "reading"],
      ["SURVEY DATA INVALID: coverage unusable, reopen VIEW to retry", "status-caution"],
    ]);
  });

  it("keeps FINEST's unit within its hysteresis between publishes", () => {
    // Code 134: 1 cm × 2^(133 ÷ 8), about 1011 m, within 5% of 1 km.
    const fresh = surveyLines(aSurvey({ coverage: coverageOf(5, 134) }), NO_READOUT_UNITS);
    expect(fresh.units.finest).toBe("km");
    expect(fresh.lines.map(readoutLineText)[0]?.endsWith("FINEST 1.0 km")).toBe(true);
    const kept: ReadoutUnits = { ...NO_READOUT_UNITS, finest: "m" };
    const held = surveyLines(aSurvey({ coverage: coverageOf(5, 134) }), kept);
    expect(held.units.finest).toBe("m");
    expect(held.lines.map(readoutLineText)[0]?.endsWith("FINEST 1000 m")).toBe(true);
  });

  it("sets the coverage and FINEST in fixed fields, so that FINEST keeps its place", () => {
    const line = (coverage: CoverageView, stale: boolean): string => {
      const [first] = surveyLines(aSurvey({ coverage, stale }), NO_READOUT_UNITS).lines;
      return first === undefined ? "" : laidOutLine(first);
    };
    const places = [coverageOf(77), coverageOf(fieldCellCount(5) - 3), coverageOf(1)].flatMap(
      (coverage) => [line(coverage, false), line(coverage, true)].map((l) => l.indexOf("· FINEST")),
    );
    expect(new Set(places)).toEqual(new Set([`SURVEY COVERAGE `.length + COVERAGE_FIELD_CH + 3]));
  });
});

/** A `DATUM` line's figure: its first segment. */
function figureOf(line: ReadoutLine): ReadoutSegment {
  const figure = line.kind === "reading" ? line.segments[0] : undefined;
  if (figure === undefined) {
    throw new Error("the line has no figure");
  }
  return figure;
}

describe("the DATUM line", () => {
  it("reads the reference spheroid's two radii to 0.1 km, the unit on both", () => {
    expect(readoutLineText(datumLine(HEADER, false))).toBe(
      "DATUM REFERENCE SPHEROID 1738.1 km × 1736.0 km",
    );
  });

  it("reads a sphere by its one radius", () => {
    const sphere = { ...HEADER, equatorial_radius_m: 1_737_400, polar_radius_m: 1_737_400 };
    expect(readoutLineText(datumLine(sphere, false))).toBe("DATUM REFERENCE SPHERE 1737.4 km");
    expect(readoutLineText(datumLine(sphere, true))).toBe("DATUM REFERENCE SPHERE 1737.4 km S");
  });

  it("follows it with the SEA LEVEL in ELEVATION's form only on a world with a sea", () => {
    expect(readoutLineText(datumLine(SEA_HEADER, false))).toBe(
      "DATUM REFERENCE SPHEROID 6378.1 km × 6356.8 km · SEA LEVEL -2.4 m",
    );
    expect(readoutLineText(datumLine({ ...SEA_HEADER, main_liquid: null }, false))).toBe(
      "DATUM REFERENCE SPHEROID 6378.1 km × 6356.8 km",
    );
  });

  it("takes one S after the figure's second radius and one after the sea level", () => {
    expect(readoutLineText(datumLine(SEA_HEADER, true))).toBe(
      "DATUM REFERENCE SPHEROID 6378.1 km × 6356.8 km S · SEA LEVEL -2.4 m S",
    );
  });

  it("sets a spheroid's figure as two runs, breakable after its ×, with the figure's one S after the second (R10.T12.a)", () => {
    expect(figureOf(datumLine(SEA_HEADER, true))).toEqual([
      { kind: "text", text: "REFERENCE SPHEROID " },
      {
        kind: "field",
        text: "6378.1 km ×",
        widthCh: 0,
        align: "end",
        tone: "stale",
        staleMark: false,
      },
      { kind: "text", text: " " },
      {
        kind: "field",
        text: "6356.8 km",
        widthCh: 0,
        align: "end",
        tone: "stale",
        staleMark: false,
      },
      { kind: "mark", stale: true },
    ]);
  });

  it("sets a sphere's one radius as one run, as before (R10.T12.a)", () => {
    const sphere = { ...HEADER, equatorial_radius_m: 1_737_400, polar_radius_m: 1_737_400 };
    expect(figureOf(datumLine(sphere, true))).toEqual([
      { kind: "text", text: "REFERENCE SPHERE " },
      {
        kind: "field",
        text: "1737.4 km",
        widthCh: 0,
        align: "end",
        tone: "stale",
        staleMark: false,
      },
      { kind: "mark", stale: true },
    ]);
  });

  it("sets the tallest DATUM's figure as the same two runs, its text unchanged (R10.T12.a)", () => {
    const tallest = tallestTerrainLines(SEA_HEADER, 1).datum;
    expect({
      figure: figureOf(tallest),
      text: readoutLineText(tallest),
    }).toEqual({
      figure: figureOf(datumLine(SEA_HEADER, true)),
      text: "DATUM REFERENCE SPHEROID 6378.1 km × 6356.8 km S · SEA LEVEL -2.4 m S",
    });
  });
});

describe("the terrain readout", () => {
  it("reads the guide's Moon-like example, Estimated under a 410 m survey", () => {
    expect(readoutTexts(groundAt())).toEqual([
      "GROUND AT LINE OF SIGHT",
      `ELEVATION ~2140 m${PM}180 m`,
      `SLOPE ~12°${PM}4° · BASELINE 2 m`,
      `SLANT RANGE ~41.2 km${PM}0.3 km`,
      "MATERIAL REGOLITH · Fe-RICH BASALT",
      "ULTIMATE BEARING CAPACITY ~3.4 MPa · PAD WIDTH 1.0 m",
      "PAD FRICTION ~0.20",
      "RESOLUTION 410 m",
    ]);
  });

  it("reads every value at its own precision once the survey resolves the point", () => {
    const resolved = groundAt({
      reading: { resolved: true, elevationSigmaM: 0, slopeSigmaRad: 0, slopeRmsRad: rad(7.2) },
      sight: { resolved: true, sigmaM: 0 },
      properties: { bearingCapacityPa: 5_368_600 },
    });
    expect(readoutTexts(resolved)).toEqual([
      "GROUND AT LINE OF SIGHT",
      "ELEVATION 2143.7 m",
      "SLOPE 7° · BASELINE 2 m",
      "SLANT RANGE 41.2 km",
      "MATERIAL REGOLITH · Fe-RICH BASALT",
      "ULTIMATE BEARING CAPACITY 5.4 MPa · PAD WIDTH 1.0 m",
      "PAD FRICTION 0.20",
      "RESOLUTION 410 m",
    ]);
  });

  it("takes one S after each value of the server's data, and none after a status or a constant", () => {
    expect(readoutTexts(groundAt(), { stale: true })).toEqual([
      "GROUND AT LINE OF SIGHT",
      `ELEVATION ~2140 m${PM}180 m S`,
      `SLOPE ~12°${PM}4° S · BASELINE 2 m`,
      `SLANT RANGE ~41.2 km${PM}0.3 km S`,
      "MATERIAL REGOLITH S · Fe-RICH BASALT S",
      "ULTIMATE BEARING CAPACITY ~3.4 MPa S · PAD WIDTH 1.0 m",
      "PAD FRICTION ~0.20 S",
      "RESOLUTION 410 m S",
    ]);
  });

  it("marks the stale values muted and the console's own text and statuses not", () => {
    const { lines } = groundReadoutLines(
      groundAt({ sight: { sigmaM: null } }),
      HEADER,
      context({ stale: true }),
      NO_READOUT_UNITS,
    );
    const pieces = lines.flatMap((line) => (line.kind === "reading" ? line.segments.flat() : []));
    const tones = new Set(pieces.flatMap((piece) => (piece.kind === "field" ? [piece.tone] : [])));
    expect(tones).toEqual(new Set(["stale"]));
    expect(pieces.filter((piece) => piece.kind === "status").map((piece) => piece.text)).toEqual([
      "GRAZING",
    ]);
  });

  it("reads GRAZING in the uncertainty's place, stale after the range alone", () => {
    const grazing = groundAt({ sight: { sigmaM: null } });
    expect(textOfLine(grazing, "SLANT RANGE")).toBe("SLANT RANGE ~41.2 km · GRAZING");
    expect(textOfLine(grazing, "SLANT RANGE", { stale: true })).toBe(
      "SLANT RANGE ~41.2 km S · GRAZING",
    );
  });

  it("reads SLANT RANGE FROM CAMERA where the range is the camera's off the hull", () => {
    expect(textOfLine(groundAt(), "SLANT RANGE FROM CAMERA", { slantFromCamera: true })).toBe(
      `SLANT RANGE FROM CAMERA ~41.2 km${PM}0.3 km`,
    );
  });

  it("reads NOT SURVEYED or NO GROUND once in place of the readings, with no S", () => {
    for (const stale of [false, true]) {
      expect(readoutTexts({ kind: "not-surveyed" }, { stale })).toEqual([
        "GROUND AT LINE OF SIGHT",
        "NOT SURVEYED",
      ]);
      expect(readoutTexts({ kind: "no-ground" }, { stale })).toEqual([
        "GROUND AT LINE OF SIGHT",
        "NO GROUND",
      ]);
    }
  });

  it("reads a material whose substance is none as the em dash, which takes no S", () => {
    expect(textOfLine(groundAt({ entry: null }), "MATERIAL")).toBe("MATERIAL REGOLITH · —");
    expect(textOfLine(groundAt({ entry: null }), "MATERIAL", { stale: true })).toBe(
      "MATERIAL REGOLITH S · —",
    );
  });

  it("reads a substance the console's list does not name as UNLISTED MATERIAL", () => {
    expect(textOfLine(groundAt({ classCode: 14, entry: 2 }), "MATERIAL")).toBe(
      "MATERIAL DUST · UNLISTED MATERIAL",
    );
  });

  it("reads the capacity's em dash with no PAD WIDTH part where no source gives it", () => {
    const missing = groundAt({ classCode: 3, properties: { bearingCapacityPa: null } });
    expect(textOfLine(missing, "ULTIMATE BEARING CAPACITY")).toBe("ULTIMATE BEARING CAPACITY —");
    expect(textOfLine(missing, "ULTIMATE BEARING CAPACITY", { stale: true })).toBe(
      "ULTIMATE BEARING CAPACITY —",
    );
  });

  it("reads the friction with AT and its temperature where it depends on it", () => {
    const ice = groundAt({
      classCode: 7,
      entry: 1,
      reading: { resolved: true },
      sight: { resolved: true },
      properties: { padFriction: 0.08, frictionOnTemperature: true, temperatureK: 243.4 },
    });
    expect(textOfLine(ice, "PAD FRICTION")).toBe("PAD FRICTION 0.08 · AT 243 K");
    const estimated = groundAt({
      classCode: 7,
      entry: 1,
      properties: { padFriction: 0.08, frictionOnTemperature: true, temperatureK: 243 },
    });
    expect(textOfLine(estimated, "PAD FRICTION")).toBe("PAD FRICTION ~0.08 · AT 243 K");
    expect(textOfLine(estimated, "PAD FRICTION", { stale: true })).toBe(
      "PAD FRICTION ~0.08 S · AT 243 K S",
    );
  });

  it("reads PAD FRICTION — · AT 102 K on glacier ice of H2O at 102 K", () => {
    const europa = groundAt({
      classCode: 7,
      entry: 1,
      properties: {
        bearingCapacityPa: null,
        padFriction: null,
        frictionOnTemperature: true,
        temperatureK: 102,
      },
    });
    expect(textOfLine(europa, "MATERIAL")).toBe("MATERIAL ICE · H2O");
    expect(textOfLine(europa, "PAD FRICTION")).toBe("PAD FRICTION — · AT 102 K");
    expect(textOfLine(europa, "PAD FRICTION", { stale: true })).toBe("PAD FRICTION — · AT 102 K S");
  });

  it("reads PAD FRICTION — on snow, whose friction no source gives", () => {
    const snow = groundAt({
      classCode: 8,
      entry: 1,
      properties: { padFriction: null, frictionOnTemperature: false, temperatureK: 102 },
    });
    expect(textOfLine(snow, "PAD FRICTION")).toBe("PAD FRICTION —");
  });

  it("reads plain — with no AT where the temperature the friction depends on is not computed", () => {
    const unknown = groundAt({
      classCode: 7,
      entry: 1,
      properties: { padFriction: null, frictionOnTemperature: true, temperatureK: null },
    });
    expect(textOfLine(unknown, "PAD FRICTION")).toBe("PAD FRICTION —");
  });

  it("keeps the capacity's unit within its hysteresis, and the range's and resolution's", () => {
    const near = groundAt({ properties: { bearingCapacityPa: 960_000 } });
    const kept: ReadoutUnits = { ...NO_READOUT_UNITS, capacity: "MPa", slantRange: "km" };
    const shown = groundReadoutLines(near, HEADER, context(), kept);
    expect(readoutLineText(lineOf(shown.lines, "ULTIMATE BEARING CAPACITY"))).toBe(
      "ULTIMATE BEARING CAPACITY ~0.96 MPa · PAD WIDTH 1.0 m",
    );
    expect(shown.units).toEqual({
      finest: null,
      resolution: "m",
      slantRange: "km",
      capacity: "MPa",
    });
    const fresh = groundReadoutLines(near, HEADER, context(), NO_READOUT_UNITS);
    expect(fresh.units.capacity).toBe("kPa");
  });

  it("keeps a unit passed in while its value is not shown", () => {
    const kept: ReadoutUnits = { finest: "m", resolution: "km", slantRange: "Mm", capacity: "Pa" };
    expect(groundReadoutLines({ kind: "no-ground" }, HEADER, context(), kept).units).toEqual(kept);
  });

  it("never shows σ, and writes every ± with a no-break space either side", () => {
    for (const line of everyLine()) {
      const shown = readoutLineText(line);
      expect(shown).not.toContain("σ");
      for (const at of [...shown.matchAll(/±/gu)].map((match) => match.index)) {
        expect([shown[at - 1], shown[at + 1]]).toEqual([NO_BREAK_SPACE, NO_BREAK_SPACE]);
      }
    }
  });
});

describe("the readout's BASELINE (R10.T12.a; decision-r10-t12a-scope §3.1)", () => {
  it("offers the crate's three baselines, 2 m, 5 m and 100 m, shortest first", () => {
    const line = readingSource
      .split("\n")
      .find((each) => each.startsWith("pub const SLOPE_BASELINES"));
    const crate = [...(line ?? "").matchAll(/Metres::new\(([\d.]+)\)/gu)].map((found) =>
      Number(found[1]),
    );
    expect([SLOPE_BASELINES_M, crate]).toEqual([
      [2, 5, 100],
      [2, 5, 100],
    ]);
  });

  it("reckons the tallest SLOPE at the longest baseline, 100 m", () => {
    const slope = lineOf(tallestTerrainLines(HEADER, 1).readout, "SLOPE");
    expect([LONGEST_BASELINE_M, readoutLineText(slope).endsWith("· BASELINE 100 m")]).toEqual([
      100,
      true,
    ]);
  });

  it.each(SLOPE_BASELINES_M)(
    "states the answer's own baseline, %d m, on SLOPE, the capacity's line the same at each",
    (baselineM) => {
      const readout = groundAt({ reading: { slopeBaselineM: baselineM } });
      expect({
        slope: textOfLine(readout, "SLOPE"),
        capacity: textOfLine(readout, "ULTIMATE BEARING CAPACITY"),
      }).toEqual({
        slope: `SLOPE ~12°${PM}4° · BASELINE ${String(baselineM)} m`,
        capacity: textOfLine(groundAt(), "ULTIMATE BEARING CAPACITY"),
      });
    },
  );
});

describe("the readout's fixed fields", () => {
  /** Each `ELEVATION` state: Estimated, resolved, and each stale. */
  function elevationLines(): string[] {
    return [false, true].flatMap((stale) =>
      [aReading(), aReading({ resolved: true, elevationM: -410 }), aReading({ elevationM: 12_480 })]
        .map((reading) =>
          groundReadoutLines(groundAt({ reading }), HEADER, context({ stale }), NO_READOUT_UNITS),
        )
        .map(({ lines }) => laidOutLine(lineOf(lines, "ELEVATION"))),
    );
  }

  it("keeps ELEVATION's width and its value's unit in place from Estimated to resolved and stale", () => {
    const lines = elevationLines();
    const valueEnd = "ELEVATION ".length + ELEVATION_FIELD_CH;
    expect(new Set(lines.map((line) => line.length)).size).toBe(1);
    for (const line of lines) {
      expect(line.slice(valueEnd - 2, valueEnd)).toBe(" m");
    }
  });

  it("keeps SLOPE's BASELINE in place from Estimated to resolved and stale", () => {
    const places = [false, true].flatMap((stale) =>
      [aReading(), aReading({ resolved: true })].map((reading) => {
        const { lines } = groundReadoutLines(
          groundAt({ reading }),
          HEADER,
          context({ stale }),
          NO_READOUT_UNITS,
        );
        const line = laidOutLine(lineOf(lines, "SLOPE"));
        expect(line.slice(0, "SLOPE ".length + SLOPE_FIELD_CH).endsWith("°")).toBe(true);
        return line.indexOf("· BASELINE");
      }),
    );
    expect(new Set(places).size).toBe(1);
  });

  it("keeps the SLANT RANGE's unit in place from Estimated to resolved and stale", () => {
    const ends = [false, true].flatMap((stale) =>
      [aSight(), aSight({ resolved: true }), aSight({ sigmaM: null })].map((sight) => {
        const { lines } = groundReadoutLines(
          groundAt({ sight }),
          HEADER,
          context({ stale }),
          NO_READOUT_UNITS,
        );
        return laidOutLine(lineOf(lines, "SLANT RANGE")).slice(
          0,
          "SLANT RANGE ".length + SLANT_RANGE_FIELD_CH,
        );
      }),
    );
    for (const end of ends) {
      expect(end.endsWith(" km")).toBe(true);
    }
  });

  it("keeps the capacity's unit and PAD WIDTH in place from Estimated to resolved and stale", () => {
    const lines = [false, true].flatMap((stale) =>
      [false, true].map((resolved) => {
        const { lines: shown } = groundReadoutLines(
          groundAt({ reading: { resolved } }),
          HEADER,
          context({ stale }),
          NO_READOUT_UNITS,
        );
        return laidOutLine(lineOf(shown, "ULTIMATE BEARING CAPACITY"));
      }),
    );
    const unitEnd = "ULTIMATE BEARING CAPACITY ".length + CAPACITY_FIELD_CH;
    expect(new Set(lines.map((line) => line.slice(unitEnd - 4, unitEnd)))).toEqual(
      new Set([" MPa"]),
    );
    expect(new Set(lines.map((line) => line.indexOf("· PAD WIDTH"))).size).toBe(1);
  });

  it("keeps PAD FRICTION's AT in place, the em dash's included, from Estimated to stale", () => {
    const frictions = [0.08, null];
    const lines = [false, true].flatMap((stale) =>
      [false, true].flatMap((resolved) =>
        frictions.map((padFriction) => {
          const { lines: shown } = groundReadoutLines(
            groundAt({
              reading: { resolved },
              properties: { padFriction, frictionOnTemperature: true, temperatureK: 243 },
            }),
            HEADER,
            context({ stale }),
            NO_READOUT_UNITS,
          );
          return laidOutLine(lineOf(shown, "PAD FRICTION"));
        }),
      ),
    );
    const at = "PAD FRICTION ".length + FRICTION_FIELD_CH + STALE_MARK_CH;
    for (const line of lines) {
      expect(line.slice(at, at + " · AT ".length)).toBe(" · AT ");
      expect(line.slice(at + " · AT ".length, at + " · AT ".length + TEMPERATURE_FIELD_CH)).toBe(
        "243 K",
      );
    }
  });

  it("keeps RESOLUTION's unit in place", () => {
    const ends = [0.01, 410, 9_700].map((resolutionM) => {
      const { lines } = groundReadoutLines(
        groundAt({ reading: { resolutionM } }),
        HEADER,
        context(),
        NO_READOUT_UNITS,
      );
      return laidOutLine(lineOf(lines, "RESOLUTION")).length;
    });
    expect(ends).toEqual(Array<number>(3).fill("RESOLUTION ".length + RESOLUTION_FIELD_CH + 2));
  });

  it("sizes every value field for the longest reading its basis allows", () => {
    expect([
      COVERAGE_FIELD_CH,
      RESOLUTION_FIELD_CH,
      ELEVATION_FIELD_CH,
      SLOPE_FIELD_CH,
      SLANT_RANGE_FIELD_CH,
      CAPACITY_FIELD_CH,
      FRICTION_FIELD_CH,
      TEMPERATURE_FIELD_CH,
    ]).toEqual([12, 9, 12, 4, 10, 9, 5, 5]);
    expect(formatElevationReading(-99_999.94, 0.04).value).toHaveLength(ELEVATION_FIELD_CH);
    expect(formatSurveyResolution(36_063_000, null)).toEqual({ value: "36,000", unit: "km" });
  });

  it("sizes the uncertainties for the longest the rounding gives within their bounds", () => {
    expect([ELEVATION_UNCERTAINTY_CH, SLOPE_UNCERTAINTY_CH, SLANT_RANGE_UNCERTAINTY_CH]).toEqual([
      12, 6, 12,
    ]);
    // An omitted relief's σ just under 100 km, and a range's just under 10,000 of its unit.
    const elevation = formatElevationReading(2_143.7, 99_000).uncertainty;
    expect(`${NO_BREAK_SPACE}${elevation ?? ""}`).toBe(`${PM}100,000 m`);
    const range = formatSlantRange({ rangeM: 900e3, sigmaM: 9_960e3, resolved: false }, null);
    expect(`${NO_BREAK_SPACE}${range.uncertainty ?? ""}`).toBe(`${PM}10,000 km`);
  });

  it("sets a stale uncertainty's S straight after it, inside its field, the line's width kept", () => {
    const at = (stale: boolean): string => {
      const { lines } = groundReadoutLines(
        groundAt(),
        HEADER,
        context({ stale }),
        NO_READOUT_UNITS,
      );
      return laidOutLine(lineOf(lines, "ELEVATION"));
    };
    const value = "~2140 m".padStart(ELEVATION_FIELD_CH);
    const uncertaintyCh = ELEVATION_UNCERTAINTY_CH + STALE_MARK_CH;
    expect([at(false), at(true)]).toEqual([
      `ELEVATION ${value}${`${PM}180 m`.padEnd(uncertaintyCh)}`,
      `ELEVATION ${value}${`${PM}180 m S`.padEnd(uncertaintyCh)}`,
    ]);
  });
});

describe("whether the readout stands (decision-r10-t12-leans §5 and §7)", () => {
  it("stands once a header is held and the readout's first answer is, NONE included", () => {
    expect(terrainReadoutStands(aSurvey(), [], { kind: "not-surveyed" })).toBe(true);
    expect(terrainReadoutStands(aSurvey({ coverage: coverageOf(0) }), [], groundAt())).toBe(true);
  });

  it("does not stand before the readout's first answer", () => {
    expect(terrainReadoutStands(aSurvey(), [], null)).toBe(false);
  });

  it("does not stand with no survey held, a body not modelled, or a failure that dropped it", () => {
    expect(terrainReadoutStands(null, [], groundAt())).toBe(false);
    expect(terrainReadoutStands(aSurvey({ header: null, coverage: null }), [], groundAt())).toBe(
      false,
    );
    const notModelled = aSurvey({ status: standing({ notModelled: "surface" }) });
    expect(terrainReadoutStands(notModelled, [], groundAt())).toBe(false);
  });

  it("does not stand under the terrain's fault, of either cause", () => {
    for (const fault of [TERRAIN_NOT_AVAILABLE, TERRAIN_COMPUTATION_FAILED]) {
      expect(terrainReadoutStands(aSurvey(), [fault], groundAt())).toBe(false);
    }
    expect(terrainReadoutStands(aSurvey(), ["GRAPHICS DEVICE LOST: re-creating"], groundAt())).toBe(
      true,
    );
  });

  it("keeps the readout stale beside a failure while the survey is held", () => {
    const failure: FieldFailure = {
      kind: "rejected",
      code: "storage_failed",
      message: "the survey log could not be read",
      refusal: false,
    };
    const model = terrainLabelModel(
      {
        survey: aSurvey({ stale: true, status: standing({ failure }) }),
        readout: groundAt(),
        faults: [],
        slantFromCamera: false,
        padWidthM: 1,
      },
      NO_READOUT_UNITS,
    );
    expect(model.survey.map(readoutLineText).at(-1)).toBe(
      "SURVEY COVERAGE REJECTED: the survey log could not be read, reopen VIEW to retry",
    );
    expect(model.datum === null ? null : readoutLineText(model.datum)).toBe(
      "DATUM REFERENCE SPHEROID 1738.1 km × 1736.0 km S",
    );
    expect(model.readout.map(readoutLineText)[1]).toBe(`ELEVATION ~2140 m${PM}180 m S`);
  });

  it("states the survey line alone, with no DATUM or readout, under the terrain's fault", () => {
    const model = terrainLabelModel(
      {
        survey: aSurvey(),
        readout: groundAt(),
        faults: [TERRAIN_NOT_AVAILABLE],
        slantFromCamera: false,
        padWidthM: 1,
      },
      NO_READOUT_UNITS,
    );
    expect([model.survey.length, model.datum, model.readout]).toEqual([1, null, []]);
  });
});

describe("the readout where no line of sight forms (R10.T12.a; decision-r10-t12a-scope §3.2)", () => {
  /** The terrain lines of a held survey with no answer and the cause `sightGap`. */
  function gapModel(sightGap: SightGap, input: Partial<TerrainLabelInput> = {}) {
    return terrainLabelModel(
      {
        survey: aSurvey(),
        readout: null,
        faults: [],
        slantFromCamera: false,
        padWidthM: 1,
        sightGap,
        ...input,
      },
      NO_READOUT_UNITS,
    );
  }

  it.each<readonly [SightGap, string]>([
    ["rotationNotModelled", "NOT AVAILABLE: ROTATION not yet modelled"],
    ["rotationNotResolved", "NOT AVAILABLE: ROTATION not resolved"],
    ["far", `NOT AVAILABLE: camera more than 6.68${NO_BREAK_SPACE}AU from the body`],
    ["centre", "NOT AVAILABLE: camera at the body's centre"],
  ])("reads the heading, then %s's NOT AVAILABLE line in --text, with DATUM", (gap, line) => {
    const model = gapModel(gap);
    expect({
      readout: model.readout.map((each) => [
        each.kind,
        readoutLineText(each),
        each.kind === "status" ? each.colour : null,
      ]),
      datum: model.datum === null ? null : readoutLineText(model.datum),
      survey: model.survey.map(readoutLineText),
    }).toEqual({
      readout: [
        ["heading", "GROUND AT LINE OF SIGHT", null],
        ["status", line, "text"],
      ],
      datum: "DATUM REFERENCE SPHEROID 1738.1 km × 1736.0 km",
      survey: [expect.stringMatching(/^SURVEY COVERAGE /u)],
    });
  });

  it.each<SightGap>(["rotationNotModelled", "rotationNotResolved", "far", "centre"])(
    "keeps %s's line unchanged with no S on loss of the link, DATUM taking its S",
    (gap) => {
      const stale = gapModel(gap, { survey: aSurvey({ stale: true }) });
      expect({
        readout: stale.readout.map(readoutLineText),
        datum: stale.datum === null ? null : readoutLineText(stale.datum),
      }).toEqual({
        readout: gapModel(gap).readout.map(readoutLineText),
        datum: "DATUM REFERENCE SPHEROID 1738.1 km × 1736.0 km S",
      });
    },
  );

  it.each<readonly [string, Partial<TerrainLabelInput>]>([
    ["under the terrain's fault", { faults: [TERRAIN_COMPUTATION_FAILED] }],
    ["with no survey held", { survey: null }],
    ["before a header is held", { survey: aSurvey({ header: null, coverage: null }) }],
    [
      "for a body not modelled",
      { survey: aSurvey({ status: standing({ notModelled: "surface" }) }) },
    ],
    ["where a kept scene leaves the surface unset", { surfaceUnset: true }],
  ])("states no readout or DATUM %s", (_, input) => {
    const model = gapModel("rotationNotModelled", input);
    expect([model.datum, model.readout]).toEqual([null, []]);
  });

  it("states the survey line alone with no cause, before the first answer", () => {
    const model = terrainLabelModel(
      { survey: aSurvey(), readout: null, faults: [], slantFromCamera: false, padWidthM: 1 },
      NO_READOUT_UNITS,
    );
    expect([model.survey.length, model.datum, model.readout]).toEqual([1, null, []]);
  });

  it("formats the far line's figure from MAX_SIGHT_M, 10¹² m in AU to three figures, kept whole with its unit", () => {
    const au = (MAX_SIGHT_M / 149_597_870_700).toPrecision(3);
    expect([au, NO_SIGHT_FAR]).toEqual([
      "6.68",
      `NOT AVAILABLE: camera more than ${au}${NO_BREAK_SPACE}AU from the body`,
    ]);
  });

  it("names each cause's line", () => {
    expect([
      sightGapLine("rotationNotModelled"),
      sightGapLine("rotationNotResolved"),
      sightGapLine("far"),
      sightGapLine("centre"),
    ]).toEqual([NO_SIGHT_ROTATION, NO_SIGHT_ROTATION_NOT_RESOLVED, NO_SIGHT_FAR, NO_SIGHT_CENTRE]);
  });
});

/**
 * A segment's set width, ch of B612 Mono: a field its fixed width, or its text and any `S` it holds
 * where that is longer; a kept place 2 ch; the console's text and a status their length.
 */
function setWidthCh(segment: ReadoutSegment): number {
  let widthCh = 0;
  for (const piece of segment) {
    switch (piece.kind) {
      case "field":
        widthCh += Math.max(piece.widthCh, piece.text.length + (piece.staleMark ? 2 : 0));
        break;
      case "mark":
        widthCh += 2;
        break;
      case "text":
      case "status":
        widthCh += piece.text.length;
        break;
    }
  }
  return widthCh;
}

/** A whole line's set width, ch: a reading's label, its segments and their middle dots. */
function lineWidthCh(line: ReadoutLine): number {
  if (line.kind !== "reading") {
    return line.text.length;
  }
  const segmentsCh = line.segments.reduce((sum, segment) => sum + setWidthCh(segment), 0);
  return line.label.length + 1 + segmentsCh + " · ".length * (line.segments.length - 1);
}

/**
 * Where a reading is wider than `tallest`, its tallest form: a longer label, more segments, or a
 * segment wider than the tallest's in its place; none where it is no wider.
 */
function widerThan(line: ReadoutLine, tallest: ReadoutLine): string[] {
  if (line.kind !== "reading" || tallest.kind !== "reading") {
    return lineWidthCh(line) > lineWidthCh(tallest) ? [readoutLineText(line)] : [];
  }
  const wider: string[] = [];
  if (line.label.length > tallest.label.length) {
    wider.push(`the label ${line.label}`);
  }
  if (line.segments.length > tallest.segments.length) {
    wider.push(`${String(line.segments.length)} segments in ${readoutLineText(line)}`);
  }
  line.segments.forEach((segment, i) => {
    const room = tallest.segments[i];
    if (room !== undefined && setWidthCh(segment) > setWidthCh(room)) {
      wider.push(`segment ${String(i)} of ${readoutLineText(line)}`);
    }
  });
  return wider;
}

/** Where a readout's lines are wider than the tallest's, each against the line in its place. */
function readoutWiderThan(
  lines: ReadonlyArray<ReadoutLine>,
  tallest: ReadonlyArray<ReadoutLine>,
): string[] {
  const wider = lines.length > tallest.length ? [`${String(lines.length)} lines`] : [];
  lines.forEach((line, i) => {
    const room = tallest[i];
    if (room === undefined) {
      return;
    }
    wider.push(...widerThan(line, room));
  });
  return wider;
}

/**
 * The values of a line that stand without their `S`: each field not stale, or with no `S` after
 * it in its segment, a `±` reading's one `S` after its uncertainty serving its value too
 * (decision-r10-t12-leans §4), and a spheroid's one `S` after its second radius its first
 * (R10.T12.a).
 */
function valuesWithoutS(line: ReadoutLine): string[] {
  if (line.kind !== "reading") {
    return [];
  }
  return line.segments.flatMap((segment) =>
    segment.flatMap((piece, i) => {
      if (piece.kind !== "field") {
        return [];
      }
      const marked = segment
        .slice(i)
        .some(
          (after) =>
            (after.kind === "field" && after.staleMark) || (after.kind === "mark" && after.stale),
        );
      return piece.tone === "stale" && marked ? [] : [piece.text];
    }),
  );
}

/** The guide's Moon-like example once the survey resolves the point, its range with it. */
function resolvedGround(): SurfaceGroundReadout {
  return groundAt({ reading: { resolved: true }, sight: { resolved: true, sigmaM: 0 } });
}

describe("the tallest forms (decision-r10-view-block-room §3)", () => {
  const TALLEST = tallestTerrainLines(HEADER, 1);

  it.each<readonly [string, SurfaceGroundReadout, Partial<ReadoutContext>, ReadoutUnits]>([
    ["Estimated", groundAt(), {}, NO_READOUT_UNITS],
    ["Estimated and stale", groundAt(), { stale: true }, NO_READOUT_UNITS],
    ["resolved", resolvedGround(), {}, NO_READOUT_UNITS],
    ["resolved and stale", resolvedGround(), { stale: true }, NO_READOUT_UNITS],
    ["grazing", groundAt({ sight: { sigmaM: null } }), {}, NO_READOUT_UNITS],
    ["grazing and stale", groundAt({ sight: { sigmaM: null } }), { stale: true }, NO_READOUT_UNITS],
    ["from the camera", groundAt(), { slantFromCamera: true }, NO_READOUT_UNITS],
    [
      "grazing from the camera and stale",
      groundAt({ sight: { sigmaM: null } }),
      { slantFromCamera: true, stale: true },
      NO_READOUT_UNITS,
    ],
    [
      "at its widest Estimated readings",
      groundAt({
        reading: {
          elevationM: -99_999.94,
          elevationSigmaM: 99_000,
          slopeRmsRad: rad(89.6),
          slopeSigmaRad: rad(89),
        },
        sight: { rangeM: 900e3, sigmaM: 9_960e3 },
      }),
      { stale: true, slantFromCamera: true },
      NO_READOUT_UNITS,
    ],
    [
      "resolved at its lowest elevation",
      groundAt({ reading: { resolved: true, elevationM: -99_999.94 }, sight: { resolved: true } }),
      { stale: true },
      NO_READOUT_UNITS,
    ],
    [
      "below 1 cm in E notation",
      groundAt({ sight: { rangeM: 5e-3, sigmaM: 3.4e-4 } }),
      { stale: true, slantFromCamera: true },
      NO_READOUT_UNITS,
    ],
    [
      "grazing below 1 cm",
      groundAt({ sight: { rangeM: 5e-3, sigmaM: null } }),
      { stale: true },
      NO_READOUT_UNITS,
    ],
    [
      "with its em dashes and AT",
      groundAt({
        entry: null,
        properties: {
          bearingCapacityPa: null,
          padFriction: null,
          frictionOnTemperature: true,
          temperatureK: 102,
        },
      }),
      { stale: true },
      NO_READOUT_UNITS,
    ],
    [
      "with its plain em dashes and no AT",
      groundAt({
        classCode: 8,
        entry: null,
        properties: { bearingCapacityPa: null, padFriction: null },
      }),
      {},
      NO_READOUT_UNITS,
    ],
    [
      "with AT",
      groundAt({
        classCode: 7,
        entry: 1,
        properties: { padFriction: 0.08, frictionOnTemperature: true, temperatureK: 243.4 },
      }),
      { stale: true },
      NO_READOUT_UNITS,
    ],
    [
      "on organic sediment of an unlisted substance",
      groundAt({ classCode: 11, entry: 2 }),
      { stale: true },
      NO_READOUT_UNITS,
    ],
    [
      "with its units held past their switch",
      groundAt({
        reading: { resolutionM: 1_040 },
        sight: { rangeM: 1_040, sigmaM: 4 },
        properties: { bearingCapacityPa: 1_040_000 },
      }),
      { stale: true },
      { finest: "m", resolution: "m", slantRange: "m", capacity: "kPa" },
    ],
    ["NOT SURVEYED", { kind: "not-surveyed" }, { stale: true }, NO_READOUT_UNITS],
    ["NO GROUND", { kind: "no-ground" }, {}, NO_READOUT_UNITS],
  ])("holds the readout %s within its tallest form", (_, readout, overrides, units) => {
    const { lines } = groundReadoutLines(readout, HEADER, context(overrides), units);
    expect(readoutWiderThan(lines, TALLEST.readout)).toEqual([]);
  });

  it.each<readonly [string, TerrainSurvey, ReadoutUnits]>([
    ["at NONE", aSurvey({ coverage: coverageOf(0) }), NO_READOUT_UNITS],
    ["at NONE, stale", aSurvey({ coverage: coverageOf(0), stale: true }), NO_READOUT_UNITS],
    ["with its figures", aSurvey(), NO_READOUT_UNITS],
    ["with its figures, stale", aSurvey({ stale: true }), NO_READOUT_UNITS],
    [
      "at ABOVE 99.9 % and its coarsest FINEST, stale",
      aSurvey({ coverage: coverageOf(fieldCellCount(5) - 1, 255), stale: true }),
      NO_READOUT_UNITS,
    ],
    [
      "with FINEST held in m past 1 km",
      aSurvey({ coverage: coverageOf(5, 134) }),
      { ...NO_READOUT_UNITS, finest: "m" },
    ],
    [
      "stale beside a failure",
      aSurvey({
        stale: true,
        status: standing({ failure: { kind: "invalid", check: "coverage unusable" } }),
      }),
      NO_READOUT_UNITS,
    ],
  ])("holds the survey line %s within its tallest form", (_, survey, units) => {
    const readings = surveyLines(survey, units).lines.filter((line) => line.kind === "reading");
    expect(readings.map((line) => widerThan(line, TALLEST.survey))).toEqual([[]]);
  });

  /** The test header's body made a sphere of the Moon's mean radius. */
  const SPHERE_HEADER: SurfaceHeaderDto = {
    ...HEADER,
    equatorial_radius_m: 1_737_400,
    polar_radius_m: 1_737_400,
  };

  it.each<readonly [string, SurfaceHeaderDto, boolean]>([
    ["of a sphere", SPHERE_HEADER, false],
    ["of a sphere, stale", SPHERE_HEADER, true],
    ["of a sphere with a sea", { ...SPHERE_HEADER, main_liquid: 1, sea_level_m: -2.4 }, true],
    ["of a spheroid", HEADER, false],
    ["of a spheroid with a sea", SEA_HEADER, false],
    ["of a spheroid with a sea, stale", SEA_HEADER, true],
  ])("holds DATUM %s within its tallest form", (_, header, stale) => {
    expect(widerThan(datumLine(header, stale), tallestTerrainLines(header, 1).datum)).toEqual([]);
  });

  it("states DATUM as a spheroid with its SEA LEVEL, for a sphere with no sea too", () => {
    expect(readoutLineText(tallestTerrainLines(SPHERE_HEADER, 1).datum)).toBe(
      "DATUM REFERENCE SPHEROID 1737.4 km × 1737.4 km S · SEA LEVEL -2000.0 m S",
    );
  });

  /** The longest material form the guide names. */
  const LONGEST_FORM = Array.from({ length: 15 }, (_, code) => materialFormName(code) ?? "").reduce(
    (longest, form) => (form.length > longest.length ? form : longest),
  );

  it("takes the longest form and UNLISTED MATERIAL where the palette names none longer", () => {
    expect(readoutLineText(lineOf(TALLEST.readout, "MATERIAL"))).toBe(
      `MATERIAL ${LONGEST_FORM} S · UNLISTED MATERIAL S`,
    );
  });

  it("takes the longest substance the palette names where it is longer than UNLISTED MATERIAL", () => {
    const longest = substanceName("acid_uv_absorber", "upper");
    const header: SurfaceHeaderDto = {
      ...HEADER,
      palette: [entry("basalt_fe", "primary_crust"), entry("acid_uv_absorber", "deposit")],
    };
    expect([
      longest.length > "UNLISTED MATERIAL".length,
      readoutLineText(lineOf(tallestTerrainLines(header, 1).readout, "MATERIAL")),
    ]).toEqual([true, `MATERIAL ${LONGEST_FORM} S · ${longest} S`]);
  });

  it("marks every value of every line stale, each with its S", () => {
    const lines = [TALLEST.survey, TALLEST.datum, ...TALLEST.readout];
    expect(lines.flatMap(valuesWithoutS)).toEqual([]);
  });

  it("reads the survey line and DATUM at their longest values, each with its S", () => {
    expect([TALLEST.survey, TALLEST.datum].map(readoutLineText)).toEqual([
      "SURVEY COVERAGE ABOVE 99.9 % S · FINEST 36,000 km S",
      "DATUM REFERENCE SPHEROID 1738.1 km × 1736.0 km S · SEA LEVEL -2000.0 m S",
    ]);
  });

  it("states the heading and the seven readings in the guide's order", () => {
    expect(
      TALLEST.readout.map((line) =>
        line.kind === "reading" ? line.label : `${line.kind} ${line.text}`,
      ),
    ).toEqual([
      "heading GROUND AT LINE OF SIGHT",
      "ELEVATION",
      "SLOPE",
      "SLANT RANGE FROM CAMERA",
      "MATERIAL",
      "ULTIMATE BEARING CAPACITY",
      "PAD FRICTION",
      "RESOLUTION",
    ]);
  });

  it("holds both of SLANT RANGE's forms at once: its widest uncertainty, then GRAZING", () => {
    expect(readoutLineText(lineOf(TALLEST.readout, "SLANT RANGE FROM CAMERA"))).toBe(
      `SLANT RANGE FROM CAMERA ~5.00E-3 m${PM}10,000 km S · GRAZING`,
    );
  });
});

/** A camera `offsetM` off the planet, about 2 × 10⁷ m, looking 2° off its centre, so off-axis. */
function poseOffBody(offsetM: Vec3 = vec3(1.4e7, -9e6, 1.1e7)): CameraPose {
  const toCentre = scale(offsetM, -1 / norm(offsetM));
  const aside = vec3(0.035, 0.012, 0);
  return {
    frame: { kind: "body", body: FIXTURE_PLANET },
    positionM: offsetM,
    orientation: lookAlong(add(toCentre, aside), vec3(0, 0, 1)),
  };
}

describe("lineOfSightOf", () => {
  const AXIS = vec3(0.3, -0.5, 0.81);
  const ROTATION = rotation3FromRows(
    rotationRows(quaternionFromAxisAngle(scale(AXIS, 1 / norm(AXIS)), 0.7)),
  );
  const BODY = aBody({ rotation: ROTATION, radiusM: HEADER.equatorial_radius_m });
  const SCENE = aViewScene({ bodies: [BODY] });
  const VIEWPORT = { widthPx: 1920, heightPx: 1080 };
  const FOV_X_RAD = Math.PI / 3;

  it("runs through the picture's centre: its hit on the datum projects to the centre pixel", () => {
    const pose = poseOffBody();
    const sight = lineOfSightOf(pose, SCENE, BODY, HEADER);
    if (sight === null) {
      throw new Error("the sight is null");
    }
    // The ray's first meeting with a sphere of the equatorial radius, body-fixed.
    const d = scale(sight.direction, 1 / norm(sight.direction));
    const o = sight.originM;
    const b = o.x * d.x + o.y * d.y + o.z * d.z;
    const radiusM = HEADER.equatorial_radius_m;
    const t = -b - Math.sqrt(b * b - (norm(o) ** 2 - radiusM ** 2));
    const hitBodyFixed = add(o, scale(d, t));
    const centreM = relativeToCamera(
      { kind: "body", body: BODY.id, m: vec3(0, 0, 0) },
      pose,
      sceneOrigins(SCENE),
    );
    const hit = add(centreM, rotateToBody(ROTATION, hitBodyFixed));
    const shown = project(hit, { orientation: pose.orientation, fovXRad: FOV_X_RAD }, VIEWPORT);
    expect(shown.inFront).toBe(true);
    expect(shown.xPx).toBeCloseTo(VIEWPORT.widthPx / 2, 6);
    expect(shown.yPx).toBeCloseTo(VIEWPORT.heightPx / 2, 6);
  });

  it("starts at the camera, body-fixed, and reaches past the body's far side", () => {
    const pose = poseOffBody();
    const sight = lineOfSightOf(pose, SCENE, BODY, HEADER);
    const distanceM = norm(pose.positionM);
    expect(sight === null ? null : norm(sight.originM)).toBeCloseTo(distanceM, 3);
    expect(sight?.maxM).toBeCloseTo(
      distanceM + HEADER.equatorial_radius_m + HEADER.realised_relief_m,
      3,
    );
    const back = sight === null ? vec3(0, 0, 0) : rotateToBody(ROTATION, sight.originM);
    expect(norm(sub(back, pose.positionM))).toBeLessThan(1e-6);
  });

  it("gives null where the body's rotation is not known", () => {
    const still = aBody({ rotation: null });
    expect(lineOfSightOf(poseOffBody(), aViewScene({ bodies: [still] }), still, HEADER)).toBeNull();
  });

  it("gives null at the body's centre and beyond the binding's reach", () => {
    expect(lineOfSightOf(poseOffBody(vec3(0.5, 0, 0)), SCENE, BODY, HEADER)).toBeNull();
    expect(lineOfSightOf(poseOffBody(vec3(2 * MAX_SIGHT_M, 0, 0)), SCENE, BODY, HEADER)).toBeNull();
  });

  it("holds the binding's limits equal to the Rust constants in wasm.rs", () => {
    expect([MIN_SIGHT_ORIGIN_M, MAX_SIGHT_M]).toEqual([
      wasmConstant("MIN_SIGHT_ORIGIN_M"),
      wasmConstant("MAX_SIGHT_M"),
    ]);
  });

  it("holds its reach to the binding's longest", () => {
    const sight = lineOfSightOf(poseOffBody(vec3(MAX_SIGHT_M - 1e5, 0, 0)), SCENE, BODY, HEADER);
    expect(sight?.maxM).toBe(MAX_SIGHT_M);
  });
});

/** A camera `offsetM` from the planet's centre, looking along a fixed direction off its axes. */
function poseAt(offsetM: Vec3): CameraPose {
  return {
    frame: { kind: "body", body: FIXTURE_PLANET },
    positionM: offsetM,
    orientation: lookAlong(vec3(-0.8, 0.3, 0.52), vec3(0, 0, 1)),
  };
}

describe("sightGapOf (R10.T12.a; decision-r10-t12a-scope §3.2)", () => {
  const AXIS = vec3(0.3, -0.5, 0.81);
  const ROTATION = rotation3FromRows(
    rotationRows(quaternionFromAxisAngle(scale(AXIS, 1 / norm(AXIS)), 0.7)),
  );
  const TURNING = aBody({ rotation: ROTATION, radiusM: HEADER.equatorial_radius_m });
  const STILL = aBody({ rotation: null, radiusM: HEADER.equatorial_radius_m });
  // A hand-built body: no server sends a radius beside a withheld rotation today, since the
  // rotation is withheld with the bulk (rotation-not-resolved).
  const WITHHELD = aBody({
    rotation: null,
    rotationNotResolved: true,
    radiusM: HEADER.equatorial_radius_m,
  });

  /** Distances from the centre either side of the reach's two limits, m, along a fixed axis. */
  const DISTANCES_M = [
    0,
    0.25,
    0.999_999,
    1,
    1.000_001,
    2,
    1.738e6,
    2e7,
    MAX_SIGHT_M * (1 - 1e-9),
    MAX_SIGHT_M,
    MAX_SIGHT_M * (1 + 1e-9),
    3 * MAX_SIGHT_M,
  ];
  const DIRECTION = scale(vec3(0.6, -0.48, 0.64), 1 / norm(vec3(0.6, -0.48, 0.64)));

  it("gives a cause exactly where lineOfSightOf forms no sight, over a grid of poses", () => {
    const disagree = [TURNING, STILL, WITHHELD].flatMap((body) =>
      DISTANCES_M.flatMap((distanceM) => {
        const pose = poseAt(scale(DIRECTION, distanceM));
        const scene = aViewScene({ bodies: [body] });
        const formed = lineOfSightOf(pose, scene, body, HEADER) !== null;
        const gap = sightGapOf(pose, scene, body);
        return formed === (gap === null) ? [] : [`${String(distanceM)} m: ${String(gap)}`];
      }),
    );
    expect(disagree).toEqual([]);
  });

  it("names the body's rotation first, at every distance", () => {
    const scene = aViewScene({ bodies: [STILL] });
    const gaps = DISTANCES_M.map((distanceM) =>
      sightGapOf(poseAt(scale(DIRECTION, distanceM)), scene, STILL),
    );
    expect(new Set(gaps)).toEqual(new Set(["rotationNotModelled"]));
  });

  it("names the body's rotation first where the granted detail level withholds it, at every distance", () => {
    const scene = aViewScene({ bodies: [WITHHELD] });
    const gaps = DISTANCES_M.map((distanceM) =>
      sightGapOf(poseAt(scale(DIRECTION, distanceM)), scene, WITHHELD),
    );
    expect(new Set(gaps)).toEqual(new Set(["rotationNotResolved"]));
  });

  it.each<readonly [SightGap | null, string, number]>([
    ["centre", "at the body's centre", 0],
    ["centre", "0.25 m from it", 0.25],
    ["centre", "just inside 1 m of it", 0.999_999],
    [null, "just past 1 m from it", 1.000_001],
    [null, "20,000 km from it", 2e7],
    [null, "just inside 10¹² m of it", MAX_SIGHT_M * (1 - 1e-9)],
    ["far", "just past 10¹² m from it", MAX_SIGHT_M * (1 + 1e-9)],
    ["far", "three times 10¹² m from it", 3 * MAX_SIGHT_M],
  ])("gives %s for a camera %s", (gap, _, distanceM) => {
    const scene = aViewScene({ bodies: [TURNING] });
    expect(sightGapOf(poseAt(scale(DIRECTION, distanceM)), scene, TURNING)).toBe(gap);
  });

  it.each([Number.NaN, Number.POSITIVE_INFINITY])(
    "throws on a camera at a distance that is not finite, %d m, naming it",
    (distanceM) => {
      const scene = aViewScene({ bodies: [TURNING] });
      expect(() => sightGapOf(poseAt(vec3(distanceM, 0, 0)), scene, TURNING)).toThrow(
        /distance from .* centre is not finite/u,
      );
    },
  );
});
