import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it, onTestFinished, vi } from "vitest";

import crossSections from "../renderer/src/view/atmosphere/absorbers/crossSections.json" with { type: "json" };
import committed from "../renderer/src/view/atmosphere/colourMatching.json" with { type: "json" };
import {
  binOzone,
  CROSS_SECTION_RANGE_NM,
  crossSectionsText,
  main,
  MATCHING_OUTPUT,
  matchingText,
  METHANE_SOURCE,
  METHANE_STEP_NM,
  methaneWavenumberPerCm,
  OZONE_SOURCE,
  parseMethane,
  parseOzone,
  readChecked,
  reduceMatching,
  resampleMethane,
  standardAirIndex,
} from "./atmosphereData";
import { CIE_CSV_SHA256, parseCie, XYZ_TO_REC709 } from "./solarFactors";

/** Three rows of a toy observer, 1 nm apart. */
const TOY = [
  { wavelengthNm: 500, x: 1, y: 0, z: 0 },
  { wavelengthNm: 501, x: 0, y: 1, z: 0 },
  { wavelengthNm: 502, x: 0, y: 0, z: 1 },
];

describe("the matching functions' reduction", () => {
  it("takes each row to Rec. 709 by the matrix's columns", () => {
    expect(reduceMatching(TOY).rows).toEqual([
      [500, XYZ_TO_REC709[0][0], XYZ_TO_REC709[1][0], XYZ_TO_REC709[2][0]],
      [501, XYZ_TO_REC709[0][1], XYZ_TO_REC709[1][1], XYZ_TO_REC709[2][1]],
      [502, XYZ_TO_REC709[0][2], XYZ_TO_REC709[1][2], XYZ_TO_REC709[2][2]],
    ]);
  });

  it("records the first row's wavelength", () => {
    expect(reduceMatching(TOY).firstNm).toBe(500);
  });

  it("records the CIE table's checksum", () => {
    expect(reduceMatching(TOY).sha256).toBe(CIE_CSV_SHA256);
  });

  it("refuses rows that are not 1 nm apart", () => {
    expect(() => reduceMatching(TOY.filter((_, i) => i !== 1))).toThrow(
      /row 2 is at 502 nm, not 501/u,
    );
  });

  it("refuses an empty table", () => {
    expect(() => reduceMatching([])).toThrow(/no rows/u);
  });

  it("writes JSON that reads back to the same values", () => {
    const reduced = reduceMatching(TOY);
    expect(JSON.parse(matchingText(reduced))).toEqual(JSON.parse(JSON.stringify(reduced)));
  });

  it("names the CIE table's checksum in the committed file", () => {
    expect(committed.sha256).toBe(CIE_CSV_SHA256);
  });

  it("names the matrix in the committed file", () => {
    expect(committed.xyzToRgb).toEqual(XYZ_TO_REC709);
  });

  it("holds the CIE table's 471 rows from 360 nm in the committed file", () => {
    expect([committed.firstNm, committed.rows.length]).toEqual([360, 471]);
  });

  it("gives the committed file byte for byte from R06's committed copy of the CIE table", () => {
    const cie = fileURLToPath(
      new URL(
        "../../../../crates/hyperion-fit/data/cie_cmf/CIE_xyz_1931_2deg.csv",
        import.meta.url,
      ),
    );
    const file = fileURLToPath(new URL(`../../${MATCHING_OUTPUT}`, import.meta.url));
    expect(matchingText(reduceMatching(parseCie(readChecked(cie, CIE_CSV_SHA256))))).toBe(
      readFileSync(file, "utf8"),
    );
  });
});

/** Silences the tool's failure reports on stderr: the tests assert the exit code instead. */
function quiet(): void {
  vi.spyOn(console, "error").mockImplementation(() => undefined);
}

/** A directory of its own for one test, removed when the test finishes. */
function scratchDirectory(): string {
  const directory = mkdtempSync(join(tmpdir(), "atmosphere-data-"));
  onTestFinished(() => {
    rmSync(directory, { recursive: true, force: true });
  });
  return directory;
}

describe("the command line", () => {
  it("refuses a file whose checksum is not the CIE table's", () => {
    const cie = join(scratchDirectory(), "cie.csv");
    writeFileSync(cie, "500,1,0,0\n");
    expect(() => readChecked(cie, CIE_CSV_SHA256)).toThrow(/SHA-256/u);
  });

  it("writes nothing for an input that fails its checksum", () => {
    quiet();
    const directory = scratchDirectory();
    const cie = join(directory, "cie.csv");
    const out = join(directory, "out.json");
    writeFileSync(cie, "500,1,0,0\n");
    expect(main(["matching", "--cie", cie, "--out", out])).toBe(1);
    expect(() => readFileSync(out)).toThrow(/ENOENT/u);
  });

  it("writes nothing for cross-sections whose files fail their checksums", () => {
    quiet();
    const directory = scratchDirectory();
    const ozone = join(directory, "o3.dat");
    const methane = join(directory, "ch4.txt");
    const out = join(directory, "out.json");
    writeFileSync(ozone, "380.00 1 1\n");
    writeFileSync(methane, "#TYPE:3\n");
    expect(main(["cross-sections", "--ozone", ozone, "--methane", methane, "--out", out])).toBe(1);
    expect(() => readFileSync(out)).toThrow(/ENOENT/u);
  });

  it.each([
    [[]],
    [["matching"]],
    [["ozone", "--cie", "x"]],
    [["cross-sections"]],
    [["cross-sections", "--ozone", "x"]],
  ])("refuses the command line %j", (args: string[]) => {
    quiet();
    expect(main(args)).toBe(1);
  });
});

/** A toy ozone file: two temperature columns, 293 K then 193 K, every 0.01 nm over 379.50–800.49. */
function toyOzone(value: (nm: number, column: number) => number, skip?: number): string {
  const lines = [
    " COLUMN 1:      vacuum wavelength (nm) ",
    " COLUMN 2:      cross section (cm^2/molecule)   @293K",
    " COLUMN 3:      cross section (cm^2/molecule)   @193K",
    "",
  ];
  for (let h = 37_950; h < 80_050; h += 1) {
    if (h !== skip) {
      const nm = h / 100;
      lines.push(
        `${nm.toFixed(3)} ${value(nm, 0).toExponential(5)} ${value(nm, 1).toExponential(5)}`,
      );
    }
  }
  return `${lines.join("\n")}\n`;
}

describe("the ozone file's reduction", () => {
  it("reads the temperatures from the header's COLUMN lines", () => {
    expect(parseOzone(toyOzone(() => 1e-21)).temperaturesK).toEqual([293, 193]);
  });

  it("bins each whole nm over [λ − 0.5, λ + 0.5), in m², the temperatures ascending", () => {
    const { reduced } = binOzone(parseOzone(toyOzone((nm, column) => (column + 1) * nm * 1e-20)));
    expect(reduced.temperaturesK).toEqual([193, 293]);
    expect(reduced.rows).toHaveLength(421);
    for (const [i, row] of reduced.rows.entries()) {
      const nm = CROSS_SECTION_RANGE_NM[0] + i;
      // The mean of nm − 0.50 … nm + 0.49 is nm − 0.005.
      expect(row[0]).toBe(nm);
      expect(row[1] ?? Number.NaN).toBeCloseTo(2 * (nm - 0.005) * 1e-24, 30);
      expect(row[2] ?? Number.NaN).toBeCloseTo((nm - 0.005) * 1e-24, 30);
    }
  });

  it("writes a negative mean as 0 and counts it", () => {
    const { reduced, negatives } = binOzone(
      parseOzone(toyOzone((nm, column) => (column === 1 && nm < 380.5 ? -1e-24 : 1e-21))),
    );
    expect(negatives).toBe(1);
    expect(reduced.rows[0]).toEqual([380, 0, 1e-25]);
  });

  it("refuses a file missing a bin's row", () => {
    expect(() => binOzone(parseOzone(toyOzone(() => 1e-21, 50_000)))).toThrow(/500 nm/u);
  });

  it("refuses a header that names no temperatures", () => {
    const text = toyOzone(() => 1e-21)
      .split("\n")
      .filter((line) => !line.includes("COLUMN"))
      .join("\n");
    expect(() => parseOzone(text)).toThrow(/names no temperature columns/u);
  });

  it("refuses a data row of the wrong width", () => {
    expect(() => parseOzone(`${toyOzone(() => 1e-21)}500.000 1e-21\n`)).toThrow(/values, not 3/u);
  });
});

/**
 * A toy PSG methane file: Karkoschka and Tomasko's wavenumbers from 25,000 down to 11,950 cm⁻¹,
 * each written as its air wavelength in µm to 5 decimals, with σ in cm² from a function of the
 * vacuum wavelength, per temperature.
 */
function toyMethane(sigma: (vacuumNm: number, column: number) => number): string {
  const lines = [
    "#MOLECULE:CH4",
    "#TYPE:3   ! 3=Cross_section[cm2/molecule]",
    "#TEMP: 100 198 296",
  ];
  lines.push("    0.152000  4.500000e-24  4.500000e-24  4.500000e-24");
  for (let nu = 25_000; nu >= 11_950; nu -= nu > 19_300 ? 25 : 5) {
    const vacuumUm = 1e4 / nu;
    let air = vacuumUm;
    for (let i = 0; i < 3; i += 1) {
      air = vacuumUm / standardAirIndex(air);
    }
    const values = [0, 1, 2].map((c) => sigma(vacuumUm * 1e3, c).toExponential(6));
    lines.push(`    ${air.toFixed(5)}  ${values.join("  ")}`);
  }
  return `${lines.join("\n")}\n`;
}

describe("the methane file's reduction", () => {
  it("recovers Karkoschka and Tomasko's wavenumbers from PSG's air wavelengths", () => {
    // PSG's first row, the row at the sampling's change and its last row.
    expect(methaneWavenumberPerCm(0.39989)).toBe(25_000);
    expect(methaneWavenumberPerCm(0.51799)).toBe(19_300);
    expect(methaneWavenumberPerCm(0.83624)).toBe(11_955);
  });

  it("puts standard air's index near 1.000 28 in the visible (Edlén 1966)", () => {
    expect(standardAirIndex(0.55)).toBeGreaterThan(1.000_277);
    expect(standardAirIndex(0.55)).toBeLessThan(1.000_279);
  });

  it("refuses a row off the sampling", () => {
    expect(() => methaneWavenumberPerCm(0.39995)).toThrow(/not on the 25 cm⁻¹ grid/u);
  });

  it("interpolates linearly on vacuum wavelengths, in m², and is 0 below the first row", () => {
    const reduced = resampleMethane(
      parseMethane(toyMethane((nm, column) => (column + 1) * nm * 1e-30)),
    );
    expect(reduced.temperaturesK).toEqual([100, 198, 296]);
    expect(reduced.rows).toHaveLength(1_681);
    for (const [k, row] of reduced.rows.entries()) {
      const nm = CROSS_SECTION_RANGE_NM[0] + k * METHANE_STEP_NM;
      expect(row[0]).toBe(nm);
      for (let c = 0; c < 3; c += 1) {
        const expected = nm < 400 ? 0 : (c + 1) * nm * 1e-34;
        expect(Math.abs((row[c + 1] ?? Number.NaN) - expected)).toBeLessThanOrEqual(
          1e-5 * expected,
        );
      }
    }
  });

  it("refuses rows that do not ascend", () => {
    const text = toyMethane(() => 1e-27).replace(
      "    0.40029",
      "    0.39989  1.000000e-27  1.000000e-27  1.000000e-27\n    0.40029",
    );
    expect(() => resampleMethane(parseMethane(text))).toThrow(/do not ascend in wavelength/u);
  });

  it("refuses temperatures that do not ascend", () => {
    const text = toyMethane(() => 1e-27).replace("#TEMP: 100 198 296", "#TEMP: 100 296 198");
    expect(() => resampleMethane(parseMethane(text))).toThrow(/temperatures do not ascend/u);
  });

  it("refuses a data row of the wrong width", () => {
    const text = toyMethane(() => 1e-27).replace(
      "    0.40029",
      "    0.40010  1.000000e-27  1.000000e-27\n    0.40029",
    );
    expect(() => parseMethane(text)).toThrow(/has 3 values, not 4/u);
  });

  it("refuses rows that end below 800 nm", () => {
    const text = toyMethane(() => 1e-27)
      .split("\n")
      .filter((line) => !(Number(line.trim().split(/\s+/u)[0]) > 0.79))
      .join("\n");
    expect(() => resampleMethane(parseMethane(text))).toThrow(/below 800 nm/u);
  });

  it("refuses a file that does not declare cross-sections", () => {
    expect(() => parseMethane(toyMethane(() => 1e-27).replace("#TYPE:3", "#TYPE:2"))).toThrow(
      /#TYPE:3/u,
    );
  });
});

describe("the cross-sections' file", () => {
  it("reads back to the same values", () => {
    const ozone = binOzone(parseOzone(toyOzone(() => 1e-21))).reduced;
    const methane = resampleMethane(parseMethane(toyMethane(() => 1e-27)));
    const file: unknown = JSON.parse(crossSectionsText([ozone, methane]));
    expect(file).toMatchObject({
      tables: [
        { species: "O3", rows: ozone.rows, temperaturesK: ozone.temperaturesK },
        { species: "CH4", rows: methane.rows, temperaturesK: methane.temperaturesK },
      ],
    });
  });

  it("names each source's URL and checksum in the committed file", () => {
    expect(crossSections.tables.map((t) => [t.species, t.url, t.sha256])).toEqual([
      ["O3", OZONE_SOURCE.url, OZONE_SOURCE.sha256],
      ["CH4", METHANE_SOURCE.url, METHANE_SOURCE.sha256],
    ]);
  });

  it("holds ozone's 421 rows at 1 nm and methane's 1,681 at 0.25 nm from 380 nm", () => {
    expect(crossSections.tables.map((t) => [t.firstNm, t.stepNm, t.rows.length])).toEqual([
      [380, 1, 421],
      [380, 0.25, 1_681],
    ]);
  });
});
