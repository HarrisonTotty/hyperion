import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it, onTestFinished, vi } from "vitest";

import crossSections from "../renderer/src/view/atmosphere/absorbers/crossSections.json" with { type: "json" };
import committed from "../renderer/src/view/atmosphere/colourMatching.json" with { type: "json" };
import {
  binOzone,
  binSulphurDioxide,
  BORYSOW_AMAGAT_PER_CM3,
  type CiaSet,
  CROSS_SECTION_RANGE_NM,
  crossSectionsText,
  HYDROGEN_HELIUM_PAIR_SOURCE,
  HYDROGEN_HELIUM_TEMPERATURES_K,
  HYDROGEN_PAIR_FILES,
  HYDROGEN_PAIR_SOURCE,
  main,
  MATCHING_OUTPUT,
  matchingText,
  METHANE_SOURCE,
  METHANE_STEP_NM,
  methaneWavenumberPerCm,
  OXYGEN_CARBON_DIOXIDE_PAIR_SOURCE,
  OXYGEN_NITROGEN_PAIR_SOURCE,
  OXYGEN_PAIR_SOURCE,
  OZONE_SOURCE,
  type PairReduction,
  parseBorysow,
  parseCia,
  parseMethane,
  parseOzone,
  parseTwoColumns,
  readChecked,
  reduceMatching,
  reduceHydrogenPair,
  reducePair,
  resampleMethane,
  standardAirIndex,
  SULPHUR_DIOXIDE_MEMBERS,
  SULPHUR_DIOXIDE_SOURCE,
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
    [["cross-sections", "--ozone", "x", "--methane", "y", "--so2", "z", "--o2o2", "w"]],
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

/** A toy two-column spectrum every 0.005 nm from 379 nm to a last wavelength, σ from a function. */
function toySpectrum(lastNm: number, sigma: (nm: number) => number): string {
  const lines: string[] = [];
  for (let k = 0; 379 + k * 0.005 <= lastNm + 1e-9; k += 1) {
    const nm = 379 + k * 0.005;
    lines.push(`${nm.toFixed(3)}   ${sigma(nm).toExponential(3)}`);
  }
  return `${lines.join("\n")}\n`;
}

describe("the sulphur dioxide files' reduction", () => {
  const tables = SULPHUR_DIOXIDE_MEMBERS.map((m, i) => ({
    temperatureK: m.temperatureK,
    rows: parseTwoColumns(
      toySpectrum(416.658, (nm) => (nm < 400 ? 1e-22 * (i + 1) : -1e-26)),
      m.name,
    ),
  }));

  it("takes the four temperatures, measured over 298–358 K, at 421 rows", () => {
    const { reduced } = binSulphurDioxide(tables);
    expect([reduced.temperaturesK, reduced.measuredRangeK, reduced.rows.length]).toEqual([
      [298, 318, 338, 358],
      [298, 358],
      421,
    ]);
  });

  it("bins each whole nm the source covers over [λ − 0.5, λ + 0.5), in m²", () => {
    const { reduced } = binSulphurDioxide(tables);
    expect(reduced.rows[390 - 380]).toEqual([390, 1e-26, 2e-26, 3e-26, 4e-26]);
  });

  it("writes 0 for a bin past the source's end", () => {
    const { reduced } = binSulphurDioxide(tables);
    expect(reduced.rows[417 - 380]).toEqual([417, 0, 0, 0, 0]);
  });

  it("writes a negative mean as 0 and counts it", () => {
    const { reduced, negatives } = binSulphurDioxide(tables);
    expect([reduced.rows[410 - 380], negatives]).toEqual([[410, 0, 0, 0, 0], 4 * 16]);
  });

  it("refuses a missing temperature", () => {
    expect(() => binSulphurDioxide(tables.slice(1))).toThrow(/298 K/u);
  });

  it("refuses rows that start past the range", () => {
    const late = tables.map((t) => ({
      temperatureK: t.temperatureK,
      rows: t.rows.filter(([nm]) => nm > 381),
    }));
    expect(() => binSulphurDioxide(late)).toThrow(/start above 380 nm/u);
  });

  it("refuses a row that is not two numbers", () => {
    expect(() => parseTwoColumns("380.0 1e-22 3\n", "toy")).toThrow(/line 1/u);
  });

  it("refuses rows that do not ascend", () => {
    expect(() => parseTwoColumns("380.0 1e-22\n379.9 1e-22\n", "toy")).toThrow(/ascend/u);
  });

  it("names the snapshot's four files by their checksums", () => {
    expect(SULPHUR_DIOXIDE_MEMBERS.map((m) => [m.temperatureK, m.sha256.length])).toEqual([
      [298, 64],
      [318, 64],
      [338, 64],
      [358, 64],
    ]);
  });
});

/** A HITRAN CIA set's header: the pair, limits, count, temperature, maximum, resolution, comment, reference. */
function ciaHeader(
  pair: string,
  low: number,
  high: number,
  count: number,
  t: number,
  ref: string,
): string {
  return (
    pair.padStart(20) +
    low.toFixed(3).padStart(10) +
    high.toFixed(3).padStart(10) +
    String(count).padStart(7) +
    t.toFixed(1).padStart(7) +
    "1.000E-45".padStart(10) +
    "-.999".padStart(6) +
    "".padStart(21) +
    ref.padStart(3)
  );
}

/** A toy CIA set every 1 cm⁻¹ from `low` to `high` cm⁻¹, its coefficient from a function. */
function toyCia(
  low: number,
  high: number,
  t: number,
  ref: string,
  value: (nu: number) => number,
): string {
  const lines = [ciaHeader("O2-O2", low, high, high - low + 1, t, ref)];
  for (let nu = low; nu <= high; nu += 1) {
    lines.push(`${nu.toFixed(4)} ${value(nu).toExponential(4)}`);
  }
  return lines.join("\n");
}

describe("the collision-induced absorption files' reduction", () => {
  it("reads each set's header and points", () => {
    const sets = parseCia(`${toyCia(20_000, 20_010, 223, "54", () => 1e-46)}\n`);
    expect(
      sets.map((s) => [
        s.pair,
        s.minPerCm,
        s.maxPerCm,
        s.temperatureK,
        s.reference,
        s.points.length,
      ]),
    ).toEqual([["O2-O2", 20_000, 20_010, 223, "54", 11]]);
  });

  it("refuses a header that does not read", () => {
    expect(() => parseCia("not a header\n")).toThrow(/header/u);
  });

  it("refuses a set cut short", () => {
    expect(() =>
      parseCia(
        toyCia(20_000, 20_010, 223, "54", () => 1e-46)
          .split("\n")
          .slice(0, 5)
          .join("\n"),
      ),
    ).toThrow(/line 6/u);
  });

  /** Two references over 380–760 nm: a short one (10) to 500 nm, a long one (20) under it. */
  const sets: ReadonlyArray<CiaSet> = parseCia(
    [
      toyCia(20_000, 26_400, 250, "10", () => 2e-46),
      toyCia(20_000, 26_400, 290, "10", () => 4e-46),
      toyCia(13_100, 26_400, 200, "20", () => 1e-46),
      toyCia(13_100, 26_400, 300, "20", (nu) => (nu > 15_000 ? 3e-46 : -1e-46)),
    ].join("\n"),
  );
  const plan: PairReduction = {
    pair: "O2+O2",
    members: ["O2", "O2"],
    source: OXYGEN_PAIR_SOURCE,
    regions: [
      { reference: "10", colderFrom: "20" },
      { reference: "20", colderFrom: undefined },
    ],
    setTemperaturesK: undefined,
    temperaturesK: [200, 250, 290, 300],
    temperatureLaw: "linear",
    measuredRangeK: [200, 300],
    reduction: "a test",
  };

  it("takes each bin from the first region covering it, extended colder by its second reference", () => {
    const { reduced } = reducePair(sets, plan);
    const row = (nm: number) => reduced.rows[nm - 380] ?? [];
    // 450 nm: reference 10 at 250 and 290 K, with reference 20's 200 K below; held at 300 K.
    expect(row(450)).toEqual([450, 1e-56, 2e-56, 4e-56, 4e-56]);
    // 600 nm: reference 20 alone, linear in T between its 200 and 300 K sets.
    expect(row(600)).toEqual([600, 1e-56, 2e-56, 2.8e-56, 3e-56]);
  });

  it("writes 0 for a bin no set covers", () => {
    const { reduced } = reducePair(sets, plan);
    // Reference 20 ends at 13,100 cm⁻¹, 763.4 nm, so no set covers 770 nm.
    expect(reduced.rows[770 - 380]).toEqual([770, 0, 0, 0, 0]);
  });

  it("writes a negative mean as 0 and counts it", () => {
    // Reference 20's 300 K set is negative below 15,000 cm⁻¹, from 667 nm.
    const { reduced, negatives } = reducePair(sets, plan);
    expect([reduced.rows[700 - 380]?.[4], negatives > 0]).toEqual([0, true]);
  });

  it("refuses a region's reference the file does not hold", () => {
    expect(() =>
      reducePair(sets, { ...plan, regions: [{ reference: "99", colderFrom: undefined }] }),
    ).toThrow(/reference 99/u);
  });

  it("refuses a chosen temperature the file lacks", () => {
    expect(() => reducePair(sets, { ...plan, setTemperaturesK: [200, 250, 275] })).toThrow(
      /275 K/u,
    );
  });

  it("takes H₂–He's sets at 26 temperatures, none of 950–1,175 K", () => {
    expect([
      HYDROGEN_HELIUM_TEMPERATURES_K.length,
      HYDROGEN_HELIUM_TEMPERATURES_K.some((t) => t >= 950 && t <= 1_175),
    ]).toEqual([26, false]);
  });
});

/**
 * A toy Borysow table: a header naming its temperatures, then rows every `step` cm⁻¹ from 10 to
 * `last`, a wavenumber in 8 characters and each value right-aligned in 14, k from a function.
 */
function toyBorysow(
  temperaturesK: readonly number[],
  step: number,
  last: number,
  value: (nu: number, t: number) => number,
): string {
  const lines = [
    "     CIA spectra of H2-H2, in cm^{-1}/amagat^{2}",
    `     freq ${temperaturesK.map((t) => `${t}K`.padStart(9)).join("")}`,
    "   (cm^-1)",
  ];
  for (let nu = 10; nu <= last; nu += step) {
    lines.push(
      nu.toFixed(1).padStart(8) +
        temperaturesK.map((t) => value(nu, t).toExponential(6).padStart(14)).join(""),
    );
  }
  return `${lines.join("\n")}\n`;
}

describe("Borysow's H₂–H₂ tables' reduction", () => {
  it("reads the temperatures from the header line that names the most", () => {
    expect(
      parseBorysow(
        toyBorysow([60, 100], 10, 100, () => 1e-12),
        "toy",
      ).temperaturesK,
    ).toEqual([60, 100]);
  });

  it("places a shorter row's values by their columns' right edges, tabs expanded", () => {
    // 600 K's column blank on the last two rows, as final_CIA_HT.dat's from 16,800 cm⁻¹, and
    // a tab before 700 K's on the last, as its 16,820 cm⁻¹ row.
    const text = [
      "   freq  400K     500K      600K      700K",
      "    10.0 1.000E-09 2.000E-09 3.000E-09 4.000E-09",
      "    20.0 1.000E-09 2.000E-09           4.000E-09",
      "    30.0 1.000E-09 2.000E-09 \t    4.000E-09",
    ].join("\n");
    const table = parseBorysow(text, "toy");
    expect(table.sets.map((set) => set.length)).toEqual([3, 3, 1, 3]);
  });

  it("refuses a table that names no temperatures", () => {
    expect(() => parseBorysow("    10.0 1.000E-09\n", "toy")).toThrow(/names no temperatures/u);
  });

  it("refuses a value that lies under no column", () => {
    const text = [
      "   freq  400K     500K",
      "    10.0 1.000E-09 2.000E-09",
      "    20.0        1.0E-09",
    ].join("\n");
    expect(() => parseBorysow(text, "toy")).toThrow(/under no column/u);
  });

  it("refuses a shorter row before any full one", () => {
    const text = ["   freq  400K     500K", "    10.0 1.000E-09"].join("\n");
    expect(() => parseBorysow(text, "toy")).toThrow(/does not fit the table's columns/u);
  });

  it("refuses a coefficient below 0", () => {
    const text = ["   freq  400K     500K", "    10.0 1.000E-09 -2.00E-09"].join("\n");
    expect(() => parseBorysow(text, "toy")).toThrow(/coefficient below 0/u);
  });

  it("refuses wavenumbers that do not ascend", () => {
    const text = [
      "   freq  400K     500K",
      "    20.0 1.000E-09 2.000E-09",
      "    10.0 1.000E-09 2.000E-09",
    ].join("\n");
    expect(() => parseBorysow(text, "toy")).toThrow(/does not ascend/u);
  });

  /** Three toy tables: 60 and 350 K to 16,490 cm⁻¹, 400 and 1,000 K to 17,000, 1,000 and 2,000 K to 20,000. */
  const tables = [
    parseBorysow(
      toyBorysow([60, 350], 10, 16_490, (nu) => 1e-12 + nu * 1e-17),
      "low",
    ),
    parseBorysow(
      toyBorysow([400, 1_000], 10, 17_000, () => 2e-12),
      "high",
    ),
    parseBorysow(
      toyBorysow([1_000, 2_000], 20, 20_000, (_, t) => t * 1e-15),
      "hot",
    ),
  ] as const;
  const reduced = reduceHydrogenPair(tables);
  const row = (nm: number) => reduced.rows[nm - 380] ?? [];

  it("takes the 2002 tables' temperatures, then the 2001 table's above 1,000 K", () => {
    expect([reduced.temperaturesK, reduced.measuredRangeK, reduced.temperatureLaw]).toEqual([
      [60, 350, 400, 1_000, 2_000],
      [60, 2_000],
      "lnLinear",
    ]);
  });

  it("gives σ = k ÷ n₀², in m⁵, a constant k's", () => {
    // Within the 6 significant figures the file is written to.
    const sigma = (2e-12 / BORYSOW_AMAGAT_PER_CM3 ** 2) * 1e-10;
    expect(Math.abs((row(700)[3] ?? 0) / sigma - 1)).toBeLessThan(1e-6);
  });

  it("gives a bin the mean of k linear in wavenumber over it", () => {
    // k = 1e-12 + 1e-17 ν: the mean over a bin is k at its middle wavenumber.
    const middle = 0.5 * (1e7 / 700.5 + 1e7 / 699.5);
    const sigma = ((1e-12 + middle * 1e-17) / BORYSOW_AMAGAT_PER_CM3 ** 2) * 1e-10;
    expect(Math.abs((row(700)[1] ?? 0) / sigma - 1)).toBeLessThan(1e-5);
  });

  it("writes 0 for a bin past each table's end", () => {
    // 600 nm (16,667 cm⁻¹) lies past 16,490 and within 17,000; 550 nm only within 20,000.
    expect([
      row(600).slice(1),
      row(550)
        .slice(1)
        .map((v) => v > 0),
    ]).toEqual([
      [0, 0, expect.any(Number), expect.any(Number), expect.any(Number)],
      [false, false, false, false, true],
    ]);
  });

  it("refuses 2002 tables given out of order", () => {
    const [low, high, hot] = tables;
    expect(() => reduceHydrogenPair([high, low, hot])).toThrow(/from one to the other/u);
  });

  it("refuses a 2001 table whose temperatures do not ascend", () => {
    const [low, high] = tables;
    const falling = parseBorysow(
      toyBorysow([3_000, 2_000], 20, 20_000, () => 1e-12),
      "falling",
    );
    expect(() => reduceHydrogenPair([low, high, falling])).toThrow(/temperatures do not ascend/u);
  });

  it("names Borysow's three tables by their checksums, coldest first", () => {
    expect(HYDROGEN_PAIR_FILES.map((f) => [f.name, f.sha256.length])).toEqual([
      ["final_CIA_LT.dat", 64],
      ["final_CIA_HT.dat", 64],
      ["CIA.H2H2.Yi", 64],
    ]);
  });
});

describe("the cross-sections' file", () => {
  it("reads back to the same values", () => {
    const ozone = binOzone(parseOzone(toyOzone(() => 1e-21))).reduced;
    const methane = resampleMethane(parseMethane(toyMethane(() => 1e-27)));
    const file: unknown = JSON.parse(crossSectionsText([ozone, methane], []));
    expect(file).toMatchObject({
      tables: [
        { species: "O3", rows: ozone.rows, temperaturesK: ozone.temperaturesK },
        { species: "CH4", rows: methane.rows, temperaturesK: methane.temperaturesK },
      ],
      pairs: [],
    });
  });

  it("names each source's URL and checksum in the committed file", () => {
    expect(crossSections.tables.map((t) => [t.species, t.url, t.sha256])).toEqual([
      ["O3", OZONE_SOURCE.url, OZONE_SOURCE.sha256],
      ["CH4", METHANE_SOURCE.url, METHANE_SOURCE.sha256],
      ["SO2", SULPHUR_DIOXIDE_SOURCE.url, SULPHUR_DIOXIDE_SOURCE.sha256],
    ]);
    expect(crossSections.pairs.map((p) => [p.pair, p.members, p.url, p.sha256])).toEqual([
      ["O2+O2", ["O2", "O2"], OXYGEN_PAIR_SOURCE.url, OXYGEN_PAIR_SOURCE.sha256],
      ["O2+N2", ["O2", "N2"], OXYGEN_NITROGEN_PAIR_SOURCE.url, OXYGEN_NITROGEN_PAIR_SOURCE.sha256],
      [
        "O2+CO2",
        ["O2", "CO2"],
        OXYGEN_CARBON_DIOXIDE_PAIR_SOURCE.url,
        OXYGEN_CARBON_DIOXIDE_PAIR_SOURCE.sha256,
      ],
      ["H2+He", ["H2", "He"], HYDROGEN_HELIUM_PAIR_SOURCE.url, HYDROGEN_HELIUM_PAIR_SOURCE.sha256],
      ["H2+H2", ["H2", "H2"], HYDROGEN_PAIR_SOURCE.url, HYDROGEN_PAIR_SOURCE.sha256],
    ]);
  });

  it("holds the tables' rows from 380 nm: ozone and SO₂ at 1 nm, methane at 0.25 nm, the pairs at 1 nm", () => {
    expect(crossSections.tables.map((t) => [t.firstNm, t.stepNm, t.rows.length])).toEqual([
      [380, 1, 421],
      [380, 0.25, 1_681],
      [380, 1, 421],
    ]);
    expect(crossSections.pairs.map((t) => [t.firstNm, t.stepNm, t.rows.length])).toEqual([
      [380, 1, 421],
      [380, 1, 421],
      [380, 1, 421],
      [380, 1, 421],
      [380, 1, 421],
    ]);
  });
});
