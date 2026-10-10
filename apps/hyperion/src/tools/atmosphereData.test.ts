import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it, onTestFinished, vi } from "vitest";

import committed from "../renderer/src/view/atmosphere/colourMatching.json" with { type: "json" };
import { main, MATCHING_OUTPUT, matchingText, readChecked, reduceMatching } from "./atmosphereData";
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

  it.each([[[]], [["matching"]], [["ozone", "--cie", "x"]]])(
    "refuses the command line %j",
    (args: string[]) => {
      quiet();
      expect(main(args)).toBe(1);
    },
  );
});
