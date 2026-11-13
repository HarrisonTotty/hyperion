import { describe, expect, it } from "vitest";

import {
  angstromHalfWidthPerCm,
  type BalmerStarkLine,
  balmerStarkLine,
  parseStarkHeader,
  starkHalfWidthA,
} from "./balmerStark";

/** A Stehlé and Hutcheon profile header in their layout (the catalogue's ReadMe), its values made up. */
function starkHeader(
  upper: number,
  density: number,
  f0: number,
  widths: ReadonlyArray<number>,
): string {
  return [
    `           N LOWER =  2  N UPPER=  ${upper}  WAVELENGTH (angstroms)=  6564.70`,
    `                electronic density(1/cm**3)       ${density.toExponential(3)}`,
    `                conversion factor for wlengths=f0 ${f0.toExponential(3)}`,
    "                wings factor                  =   2.564E-03",
    "",
    "T(K)                2.500E+03                5.000E+03                1.000E+04",
    "RO/Debye            8.338E-02                5.896E-02                4.169E-02",
    `width Stark         ${widths.map((w) => w.toExponential(3)).join("                ")}`,
    "width incl.Dop.     2.047E+01                2.906E+01                4.068E+01",
  ].join("\n");
}

/** The toy Hα headers: 10¹⁰ and 10¹² cm⁻³, their f0 and Stark half widths in units of α. */
const LOW = { density: 1e10, f0: 5.808e-3, widths: [4.35e-3, 3.463e-3, 2.717e-3] } as const;
const HIGH = { density: 1e12, f0: 0.1251, widths: [1.165e-2, 9.793e-3, 8.382e-3] } as const;

/** The toy Hα line over its two densities. */
function toyAlpha(): BalmerStarkLine {
  return balmerStarkLine([
    parseStarkHeader(starkHeader(3, HIGH.density, HIGH.f0, HIGH.widths)),
    parseStarkHeader(starkHeader(3, LOW.density, LOW.f0, LOW.widths)),
  ]);
}

/** A toy header's half width at its k-th temperature, Å: α × f0. */
function halfWidthA(header: typeof LOW | typeof HIGH, k: number): number {
  return (header.widths[k] ?? Number.NaN) * header.f0;
}

describe("Stehlé and Hutcheon's profile headers", () => {
  it("read a header's line, density, f0 and Stark half widths", () => {
    expect(parseStarkHeader(starkHeader(3, 1e10, 5.808e-3, [4.35e-3, 3.463e-3, 2.717e-3]))).toEqual(
      {
        lowerN: 2,
        upperN: 3,
        wavelengthA: 6_564.7,
        electronsPerCm3: 1e10,
        f0A: 5.808e-3,
        temperaturesK: [2_500, 5_000, 10_000],
        halfWidthsAlpha: [4.35e-3, 3.463e-3, 2.717e-3],
      },
    );
  });

  it("refuse a header whose half widths are not one a temperature", () => {
    expect(() => parseStarkHeader(starkHeader(3, 1e10, 5.808e-3, [4.35e-3, 3.463e-3]))).toThrow(
      "header",
    );
  });
});

describe("a Balmer line's Stark table", () => {
  it("orders its densities and takes each half width in Å, α × f0", () => {
    const line = toyAlpha();
    expect([line.electronsPerCm3, line.halfWidthsA[0]?.[1]]).toEqual([
      [1e10, 1e12],
      expect.closeTo(halfWidthA(LOW, 1), 15),
    ]);
  });

  it("refuses a single density, which no interpolation in ln n_e can take", () => {
    const one = parseStarkHeader(starkHeader(3, LOW.density, LOW.f0, LOW.widths));
    expect(() => balmerStarkLine([one])).toThrow("two densities");
  });

  it("refuses headers of two lines", () => {
    expect(() =>
      balmerStarkLine([
        parseStarkHeader(starkHeader(3, LOW.density, LOW.f0, LOW.widths)),
        parseStarkHeader(starkHeader(4, HIGH.density, HIGH.f0, HIGH.widths)),
      ]),
    ).toThrow("one Balmer line");
  });
});

describe("a Balmer line's Stark half width", () => {
  const line = toyAlpha();

  it("is the table's at a node", () => {
    expect(starkHalfWidthA(line, 1e12, 5_000)).toBeCloseTo(halfWidthA(HIGH, 1), 15);
  });

  it("takes ln w linear in ln n_e between the densities", () => {
    const expected = Math.sqrt(halfWidthA(LOW, 1) * halfWidthA(HIGH, 1));
    expect(starkHalfWidthA(line, 1e11, 5_000) / expected).toBeCloseTo(1, 12);
  });

  it("extrapolates below 10¹⁰ cm⁻³ on the first interval's slope", () => {
    const low = halfWidthA(LOW, 1);
    const expected = low * Math.sqrt(low / halfWidthA(HIGH, 1));
    expect(starkHalfWidthA(line, 1e9, 5_000) / expected).toBeCloseTo(1, 12);
  });

  it("takes w linear in T between the temperatures", () => {
    const expected = (halfWidthA(LOW, 0) + halfWidthA(LOW, 1)) / 2;
    expect(starkHalfWidthA(line, 1e10, 3_750) / expected).toBeCloseTo(1, 12);
  });

  it("holds the end temperatures' half widths beyond them", () => {
    expect([starkHalfWidthA(line, 1e10, 1_000), starkHalfWidthA(line, 1e10, 20_000)]).toEqual([
      expect.closeTo(halfWidthA(LOW, 0), 15),
      expect.closeTo(halfWidthA(LOW, 2), 15),
    ]);
  });
});

describe("a half width in wavenumber", () => {
  it("is Δλ ÷ λ² × 10⁸ cm⁻¹ for Å: 1 Å at 5,000 Å is 4 cm⁻¹", () => {
    expect(angstromHalfWidthPerCm(1, 5_000)).toBeCloseTo(4, 12);
  });
});
