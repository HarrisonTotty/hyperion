import { describe, expect, it } from "vitest";

import { type AtomicLineSet, atomicLineSet } from "./atomicKDistributions";
import {
  ATOMIC_SOURCES,
  firstNodeK,
  forbiddenIronIonLines,
  hydrogenLevelN,
  hydrogenLineSet,
  type LadderShares,
  LINE_FILE_BYTES,
  lineFiles,
  POTASSIUM_SATURATION,
  resonanceDoublet,
  tableNodesOf,
  THINNED_PRESSURES_PA,
  visibilityFloorK,
  withSodiumH2Widths,
} from "./atomicKDistributionsCli";
import { type KuruczLine } from "./atomicLines";
import { lineAt, toySahaData } from "./atomicLinesToys";
import { balmerStarkLine, parseStarkHeader } from "./balmerStark";
import { saturationPressurePa } from "./kDistributions";

/** A toy stage's line set: Fe I's mass and limit unless given, Q 25 at every temperature. */
function toySet(lines: ReadonlyArray<KuruczLine>, key = "Fe", limitEv = 7.902): AtomicLineSet {
  const [symbol = "", ion] = key.split("+");
  return atomicLineSet(
    { key, symbol, stage: ion === undefined ? 0 : 1 },
    26,
    55.845,
    limitEv,
    lines,
    [],
    () => 25,
  );
}

/** A drawn stage's source by its key, or a test failure. */
function sourceOfKey(key: string): (typeof ATOMIC_SOURCES)[number] {
  const s = ATOMIC_SOURCES.find((x) => x.key === key);
  if (s === undefined) {
    throw new Error(`no source ${key}`);
  }
  return s;
}

/** A stage's ladder shares with a floor, the rest as a floors record writes them. */
function sharesWithFloor(floorK: number | undefined): LadderShares {
  return {
    key: "Fe",
    temperaturesK: [1_000, 1_100],
    shares: [1e-4, 1e-2],
    sharesAt100: [1e-2, 1e-1],
    floorK,
    statedNone: false,
    reachesAt100K: 1_000,
  };
}

/** A line file's rows. */
function linesOfFile(text: string): unknown[] {
  const file: unknown = JSON.parse(text);
  const lines: unknown =
    typeof file === "object" && file !== null ? Reflect.get(file, "lines") : undefined;
  return Array.isArray(lines) ? lines : [];
}

/** A line file row's vacuum wavelength, nm: its first column. */
function vacuumNmOf(row: unknown): number {
  return Array.isArray(row) ? Number(row[0]) : Number.NaN;
}

/** A Stehlé and Hutcheon profile header in their layout (the catalogue's ReadMe), its values made up. */
function starkHeader(density: number, f0: number, widths: ReadonlyArray<number>): string {
  return [
    "           N LOWER =  2  N UPPER=  3  WAVELENGTH (angstroms)=  6564.70",
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

describe("a drawn stage's table nodes", () => {
  it("take Mg⁺'s and Si⁺'s ruled nodes, 4,400–5,000 K, whatever the ladder", () => {
    expect([
      tableNodesOf(sourceOfKey("Mg+"), undefined, "E1prime"),
      tableNodesOf(sourceOfKey("Si+"), undefined, "E1"),
    ]).toEqual([
      [4_400, 4_600, 4_800, 5_000],
      [4_400, 4_600, 4_800, 5_000],
    ]);
  });

  it("start at a ruled floor before a visibility floor", () => {
    const ruled = { ...sourceOfKey("Fe"), ruledFloorK: 4_600 };
    expect(firstNodeK(ruled, sharesWithFloor(3_300))).toBe(4_600);
  });

  it("start at the visibility floor where none is ruled", () => {
    expect(firstNodeK(sourceOfKey("Fe"), sharesWithFloor(3_300))).toBe(3_300);
  });

  it("start at the range's first node where the stage has no floor", () => {
    expect([
      firstNodeK(sourceOfKey("Fe"), sharesWithFloor(undefined)),
      firstNodeK(sourceOfKey("Na"), undefined),
    ]).toEqual([1_000, 500]);
  });
});

describe("a stage's visibility floor", () => {
  it("is the highest node at and below which every share stays under 10⁻³", () => {
    expect(visibilityFloorK([3_000, 3_300, 3_600, 3_950], [1e-5, 9e-4, 1.2e-3, 5e-4])).toBe(3_300);
  });

  it("is none where the first node's share reaches 10⁻³", () => {
    expect(visibilityFloorK([1_000, 1_100], [1e-3, 1e-2])).toBeUndefined();
  });
});

describe("E3's thinned pressures", () => {
  it("keep 20 of the 30 stored pressures, every other one below 1 MPa", () => {
    expect([THINNED_PRESSURES_PA.length, THINNED_PRESSURES_PA.slice(0, 3)]).toEqual([
      20,
      [10, expect.closeTo(31.622_776_6, 6), expect.closeTo(100, 9)],
    ]);
  });
});

describe("K's condensing cap (R08.T4.e's residual protocol, item (ii))", () => {
  it("takes NIST's Antoine fit for liquid K: 952 Pa at 700 K", () => {
    // 1e5 × 10^(4.45718 − 4691.58 ÷ 724.195) = 952.45 Pa.
    expect(saturationPressurePa(POTASSIUM_SATURATION, 700) / 952.45).toBeCloseTo(1, 5);
  });

  it("takes no cap below its 650 K node or above the fit's 1,033 K", () => {
    expect([
      saturationPressurePa(POTASSIUM_SATURATION, 600),
      saturationPressurePa(POTASSIUM_SATURATION, 1_100),
    ]).toEqual([Infinity, Infinity]);
  });
});

describe("Na D's widths", () => {
  const d2 = { ...lineAt(589.158, 0.108, 0), upperJ: 1.5 };
  const d1 = { ...lineAt(589.756, -0.194, 0), upperJ: 0.5 };
  const base = toySet([d2, d1], "Na");
  const set: AtomicLineSet = {
    ...base,
    vanDerWaals: base.vanDerWaals.map(() => ({
      kind: "abo" as const,
      logGammaPerHydrogen: -7.526,
      exponent: 0.3635,
    })),
  };

  it("take Allard et al.'s H₂ widths for both D lines", () => {
    expect(withSodiumH2Widths(set).vanDerWaals.map((v) => v.kind)).toEqual([
      "allardH2",
      "allardH2",
    ]);
  });

  it("are the doublet's two lines from the ground state", () => {
    expect(resonanceDoublet(set)).toEqual({
      d2: set.lines.indexOf(d2),
      d1: set.lines.indexOf(d1),
    });
  });

  it("refuse a list without both D lines", () => {
    expect(() => withSodiumH2Widths({ ...set, lines: set.lines.slice(0, 1) })).toThrow("D lines");
  });
});

describe("H I's Balmer lines", () => {
  const alpha = { ...lineAt(656.461, 0.71, 82_259.105, 97_492.302), upperJ: 2.5, lowerJ: 1.5 };
  const metal = lineAt(500, 0, 0);
  const stark = balmerStarkLine([
    parseStarkHeader(starkHeader(1e10, 5.808e-3, [4.35e-3, 3.463e-3, 2.717e-3])),
    parseStarkHeader(starkHeader(1e12, 0.1251, [1.165e-2, 9.793e-3, 8.382e-3])),
  ]);
  const set = hydrogenLineSet(toySet([alpha, metal], "H", 13.598), toySahaData(), [stark]);

  it("find each line's levels from their energies: n = 2 at 82,259 cm⁻¹, 3 at 97,492", () => {
    expect([82_259.105, 97_492.302, 102_823.893].map(hydrogenLevelN)).toEqual([2, 3, 4]);
  });

  it("take Barklem et al. 2000b's self-broadening for H perturbers", () => {
    expect(set.vanDerWaals[set.lines.indexOf(alpha)]?.kind).toBe("selfBroadening");
  });

  it("take a Stark width at the node's electron density", () => {
    expect(set.starkHalfWidthPerCm?.(set.lines.indexOf(alpha), 4_000, 1e4)).toBeGreaterThan(0);
  });

  it("give a line that is not a Balmer line no Stark width", () => {
    expect(set.starkHalfWidthPerCm?.(set.lines.indexOf(metal), 4_000, 1e4)).toBe(0);
  });
});

describe("[Fe II]'s forbidden lines (Deb and Hibbert 2011)", () => {
  // Table 5's levels: index 1–3, the configuration 5–21, the term 23–27, J 29–31, E 33–42 cm⁻¹;
  // the A-value tables: lower 1–3, upper 5–7, A 40–48 s⁻¹ (their ReadMe's columns, values made up).
  const levels = [
    `  1 ${"3d6.4s".padEnd(17)} ${"a6D".padEnd(5)} 4.5 ${"0.000".padStart(10)}`,
    `  2 ${"3d7".padEnd(17)} ${"a4F".padEnd(5)} 4.5 ${"20000.000".padStart(10)}`,
  ].join("\n");
  const aValues = `  1   2${" ".repeat(32)}1.000E-02`;
  const [line] = forbiddenIronIonLines(levels, aValues);

  it("take gf = g_u A λ² ÷ (8π² r_e c): 1.4992 × 10⁻¹⁶ g_u A λ² in Å", () => {
    // g_u 10, A 10⁻² s⁻¹ and 5,000 Å: gf = 1.4992e-16 × 10 × 1e-2 × 5000² = 3.748 × 10⁻¹⁰.
    expect(line?.logGf).toBeCloseTo(Math.log10(1.499_2e-16 * 10 * 1e-2 * 5_000 ** 2), 4);
  });

  it("take the vacuum wavelength of the levels' difference", () => {
    expect([line?.vacuumNm, line?.lowerPerCm, line?.upperPerCm]).toEqual([
      expect.closeTo(500, 9),
      0,
      20_000,
    ]);
  });
});

describe("the line files", () => {
  const many = Array.from({ length: 14_000 }, (_, i) => lineAt(370 + i * 0.028, -1, 1_000 + i));
  const files = lineFiles(sourceOfKey("Fe"), toySet(many));

  it("keep each file under the hook, splitting a long list into parts", () => {
    expect([
      files.length > 1,
      files.every((f) => Buffer.byteLength(f.text) <= LINE_FILE_BYTES),
    ]).toEqual([true, true]);
  });

  it("name the parts <key>-<n>.json", () => {
    expect(files.map((f) => f.file)).toEqual(files.map((_, k) => `Fe-${k + 1}.json`));
  });

  it("hold every line once across the parts", () => {
    expect(files.flatMap((f) => linesOfFile(f.text)).length).toBe(14_000);
  });

  it("split in wavenumber order, each part's first line beyond the last part's last", () => {
    const rows = files.map((f) => linesOfFile(f.text).map(vacuumNmOf));
    expect(
      rows.every((r, k) => k === 0 || (r[0] ?? Infinity) < (rows[k - 1]?.at(-1) ?? -Infinity)),
    ).toBe(true);
  });
});
