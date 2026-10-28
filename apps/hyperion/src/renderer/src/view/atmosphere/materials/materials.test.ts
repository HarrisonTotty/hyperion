import { describe, expect, it } from "vitest";

import registry from "../../../../../../../../packages/protocol/fixtures/substances.json" with { type: "json" };
import { SUBSTANCE_NAMES, substanceName, UNLISTED_MATERIAL } from "../../../lib/substances";
import { aerosolModeOptics } from "../aerosol";
import { mieSphere } from "../mie";
import { ICE_KEYS } from "../nonSpherical";
import { type Dispersion, ESTIMATED_RAYLEIGH, GAS_DISPERSION, GASES } from "../rayleigh";
import { CLOUD_DECK_SPLIT_OPTICAL_DEPTH } from "../thick/regime";
import {
  ATOMIC_SOLID_KEYS,
  GENERIC_STAND_IN_INDEX,
  MATERIAL_FILES,
  MATERIAL_WAVELENGTHS_NM,
  type MaterialForm,
  materialApproximations,
  materialLabels,
  parseMaterialFile,
  phasedName,
  refractiveIndex,
  resolveMaterial,
  type ScaledMolarRefractionFit,
  variantWords,
  VOLATILE_ICE_KEYS,
  WORDED_SOLID_KEYS,
} from "./materials";
import waterFile from "./water.json" with { type: "json" };

/** The keys T5.b's files carry (plan R08, R08.T5.b; decision-composition §1.9). */
const T5B_KEYS = [
  "H2O",
  "NH3",
  "CH4",
  "CO2",
  "H2SO4",
  "NH4SH",
  "Fe",
  "soot",
  "tholin",
  "mars_dust",
  "MgSiO3",
  "Mg2SiO4",
];

/**
 * The keys R08.T5.d adds: the plan's condensates and hazes, the fixture's O2 and Ar ices
 * (decision-r08-t5d ruling 3), and the lithologies with an open primary.
 */
const T5D_KEYS = [
  "KCl",
  "NaCl",
  "ZnS",
  "Na2S",
  "MnS",
  "Cr",
  "FeS",
  "SiO2",
  "Al2O3",
  "CaTiO3",
  "TiO2",
  "graphite",
  "S8",
  "H2S",
  "N2",
  "CO",
  "O2",
  "Ar",
  "chromophore",
  "ice_giant_haze",
  "acid_uv_absorber",
  "basalt",
  "basalt_fe",
  "lava_glass",
];

/**
 * The liquids R08.T5.g adds, each by its key, phase and variant, with its file's provenance: seawater
 * as liquid water's variant, the cryogenic liquids (C₂H₆ `derived` since R08.T5.h, levelled to a
 * measured index), and molten sulphur's stand-in.
 */
const T5G_LIQUIDS = [
  ["H2O", "S35", "measured"],
  ["N2", undefined, "derived"],
  ["NH3", undefined, "derived"],
  ["C2H6", undefined, "derived"],
  ["S8", undefined, "standIn"],
] as const;

/** The key R08.T5.g adds to the registry's files: liquid ethane (the others add a phase). */
const T5G_KEYS = ["C2H6"];

/**
 * The liquids R08.T5.k adds (decision-p14-t49h-gas-index §2.5 and A.4), each by its key with its
 * file's provenance, its temperature by §2.5's rule, K, and its registry row's triple point, K
 * (`crates/hyperion-sim/src/substance/rows/diagrams.rs` at 47ffd02a).
 */
const T5K_LIQUIDS = [
  ["O2", "derived", 90.19, 54.361],
  ["Ar", "measured", 83.81, 83.8058],
  ["Kr", "measured", 115.77, 115.77],
  ["Xe", "measured", 161.4, 161.4],
  ["CO", "standIn", 68.16, 68.16],
  ["CO2", "derived", 216.6, 216.592],
  ["N2O", "derived", 184.67, 182.33],
  ["SO2", "derived", 197.7, 197.7],
  ["H2S", "standIn", 187.7, 187.7],
] as const;

/** The keys R08.T5.k adds to the registry's files: each liquid is its key's only file. */
const T5K_KEYS = ["Kr", "Xe", "N2O", "SO2"];

/** A scaled molar refraction's fields, O₂'s, for the format's cases (decision-p14-t49h-gas-index A.1). */
const SCALED_FIT = {
  form: "scaledMolarRefraction",
  molarDensityMolPerL: 35.662_63,
  factor: 0.986_155,
  anchoredFromUm: 0.5461,
  anchoredToUm: 0.5461,
} as const;

/**
 * A scaled gas absorption's fields, liquid methane's, for the format's cases (R08.T5.j;
 * decision-p14-t49h-liquids §2.4).
 */
const GAS_ABSORPTION_FIT = {
  form: "scaledGasAbsorption",
  molarDensityMolPerL: 28.141_91,
  fromUm: 0.38,
  toUm: 0.555_556,
} as const;

/** The ideal gas's molar volume at a row's reference state, cm³ mol⁻¹, times its Z. */
function gasMolarVolumeCm3PerMol(gas: Dispersion): number {
  return (
    ((gas.compressibility * 6.022_140_76e23 * 1.380_649e-23 * gas.referenceK) / gas.referencePa) *
    1e6
  );
}

/** A gas row's Lorentz–Lorenz molar refraction at a wavelength, nm, cm³ mol⁻¹. */
function rowMolarRefractionCm3PerMol(gas: Dispersion, nm: number): number {
  return lorentz(1 + gas.nMinusOne(nm)) * gasMolarVolumeCm3PerMol(gas);
}

/** A liquid's index from its gas row's molar refraction scaled by c at ρ_m, mol/l: x = cRρ_m. */
function scaledIndex(gas: Dispersion, nm: number, molPerL: number, factor: number): number {
  const x = (factor * rowMolarRefractionCm3PerMol(gas, nm) * molPerL) / 1_000;
  return Math.sqrt((1 + 2 * x) / (1 - x));
}

/** A T5.k key's gas row in rayleigh.ts: its GAS_DISPERSION row, or SO₂'s ESTIMATED_RAYLEIGH dispersion. */
function gasRowOf(key: string): Dispersion {
  if (key === "SO2") {
    const so2 = ESTIMATED_RAYLEIGH.find((row) => row.species === "SO2");
    if (so2?.kind !== "dispersion") {
      throw new Error("rayleigh.ts holds no SO2 dispersion");
    }
    return so2.dispersion;
  }
  const row = GASES.find((gas) => gas === key);
  if (row === undefined) {
    throw new Error(`rayleigh.ts holds no gas row for ${key}`);
  }
  return GAS_DISPERSION[row];
}

/** A liquid's own file, by key and variant: never a named analogue. */
function liquidOf(key: string, variant?: string): (typeof MATERIAL_FILES)[number] {
  const file = MATERIAL_FILES.find(
    (f) => f.key === key && f.phase === "liquid" && f.variant === variant,
  );
  if (file === undefined) {
    throw new Error(`no liquid file for ${key}`);
  }
  return file;
}

/** A file's value at a wavelength on its grid, nm. */
function valueAt(values: Float64Array, file: (typeof MATERIAL_FILES)[number], nm: number): number {
  return values[file.wavelengthsNm.indexOf(nm)] ?? Number.NaN;
}

/** The imaginary index of an absorption coefficient a, m⁻¹, at a wavelength, nm: k = aλ ÷ 4π. */
function absorptionK(perM: number, nm: number): number {
  return (perM * nm * 1e-9) / (4 * Math.PI);
}

/** A file's grid fields, with n and k of the grid's own length. */
function onGrid(grid: ReadonlyArray<number>): object {
  return { wavelengthsNm: grid, n: grid.map(() => 1.33), k: grid.map(() => 0) };
}

/** Every 5 nm from one wavelength to at most another, nm. */
function every5(from: number, to: number): number[] {
  return Array.from({ length: Math.floor((to - from) / 5) + 1 }, (_, i) => from + 5 * i);
}

/**
 * Venus's ultraviolet absorber's k, k_UA, by science-r08-unowned-absorbers' formula at the
 * a-priori k₃₆₅ 1.2 × 10⁻³ and L 26.9 nm (decision-r08-t5d ruling 4).
 */
function absorberK(nm: number): number {
  if (nm >= 550) {
    return 0;
  }
  const decay = 1.2e-3 * Math.exp(-(nm - 365) / 26.9);
  return nm > 500 ? (decay * (550 - nm)) / 50 : decay;
}

/** A file's k from a wavelength on, nm. */
function kFrom(file: (typeof MATERIAL_FILES)[number], nm: number): number[] {
  return Array.from(file.k.slice(file.wavelengthsNm.indexOf(nm)));
}

function fileOf(key: string, phase?: "liquid" | "solid"): (typeof MATERIAL_FILES)[number] {
  const file = MATERIAL_FILES.find(
    (f) => f.key === key && (phase === undefined || f.phase === phase),
  );
  if (file === undefined) {
    throw new Error(`no file for ${key}`);
  }
  return file;
}

/** The Lorentz–Lorenz factor (n² − 1) ÷ (n² + 2). */
function lorentz(n: number): number {
  return (n * n - 1) / (n * n + 2);
}

/** The gas dispersions NH₄SH's estimate is built from: rayleigh.ts's NH₃ and H₂S rows. */
const NH4SH_GASES: ReadonlyArray<Dispersion> = [GAS_DISPERSION.NH3, GAS_DISPERSION.H2S];

/** Mars dust's k at a wavelength, nm. */
function marsDustK(wavelengthNm: number): number {
  return refractiveIndex("mars_dust", wavelengthNm).index.k;
}

/**
 * A citation in player text: an author's `et al`, or a year of 1900–2099 standing alone, perhaps
 * lettered (`2000b`) and perhaps ending a clause (`2020,`, `2020.`). A year's four digits are not
 * a decimal's or a grouped number's, and not a figure's with its unit after it (`1950 nm`,
 * `2000 K`, `1950nm`). No citation reaches the screen (`decision-r08-approximate-plate.md` §6;
 * R08.T10.e).
 */
const CITATION = /\bet al\b|(?<![\d.,])(?:19|20)\d{2}(?!\s?(?:nm|µm|K)\b)[a-z]?(?!\d|[.,]\d)/u;

/**
 * The terms a stand-in reason may hold that contain its own substance's name as a word, though
 * they name something else: liquid O₂'s pair absorption, `O2–O2` (R08.T10.f).
 */
const OTHER_TERMS_WITH_NAME: ReadonlyArray<string> = ["O2–O2"];

/** A stand-in reason that is a file's own text, with the head it should follow. */
interface OwnTextReason {
  readonly file: (typeof MATERIAL_FILES)[number];
  readonly head: string;
  readonly reason: string;
}

/**
 * Every reason a file gives as its own text, as `materialApproximations` gives it for the file's
 * own key and form at τ 0: the sixteen stand-in files' `standIn`, and liquid O₂'s
 * `absorptionStandIn` (R08.T10.f).
 */
function ownTextReasons(): ReadonlyArray<OwnTextReason> {
  return MATERIAL_FILES.filter(
    (file) => file.provenance === "standIn" || file.absorptionStandIn !== undefined,
  ).flatMap((file) =>
    materialApproximations([
      {
        material: file.key,
        form: {
          phase: file.phase,
          ...(file.variant === undefined ? {} : { variant: file.variant }),
        },
        opticalDepthAbove550: 0,
      },
    ]).map(({ reason }) => ({ file, head: `${phasedName(file.key, file.phase)}: `, reason })),
  );
}

/** Whether a text holds a name as a whole word: no letter or digit runs on at either end. */
function holdsWord(text: string, name: string): boolean {
  const escaped = name.replaceAll(/[$()*+.?[\\\]^{|}]/gu, String.raw`\$&`);
  return new RegExp(String.raw`(?<![\p{L}\p{N}])${escaped}(?![\p{L}\p{N}])`, "u").test(text);
}

describe("the material files", () => {
  it("hold every key of T5.b's list", () => {
    const keys = new Set(MATERIAL_FILES.map((f) => f.key));
    for (const key of T5B_KEYS) {
      expect(keys.has(key)).toBe(true);
    }
  });

  it("each end their grid with the 380–780 nm every 5 nm the renderer draws at", () => {
    // decision-r08-t5d ruling 1: a file's grid starts per file, at or below 380 nm.
    expect(MATERIAL_WAVELENGTHS_NM[0]).toBe(380);
    expect(MATERIAL_WAVELENGTHS_NM.at(-1)).toBe(780);
    expect(MATERIAL_WAVELENGTHS_NM).toHaveLength(81);
    for (const file of MATERIAL_FILES) {
      expect(Array.from(file.wavelengthsNm.slice(-MATERIAL_WAVELENGTHS_NM.length))).toEqual(
        Array.from(MATERIAL_WAVELENGTHS_NM),
      );
      expect(file.n).toHaveLength(file.wavelengthsNm.length);
      expect(file.k).toHaveLength(file.wavelengthsNm.length);
    }
  });

  it("start below 380 nm only for the two files Venus's ultraviolet fit reads, at 360 nm", () => {
    const early = MATERIAL_FILES.filter((file) => (file.wavelengthsNm[0] ?? 0) < 380);
    expect(early.map((file) => [file.key, file.variant, file.wavelengthsNm[0]])).toEqual([
      ["H2SO4", "75wt%", 360],
      ["acid_uv_absorber", undefined, 360],
    ]);
  });

  it("each name its paper, source, licence basis, reduction, provenance, phase and shape", () => {
    for (const file of MATERIAL_FILES) {
      expect(file.paper.length).toBeGreaterThan(0);
      expect(file.source.length).toBeGreaterThan(0);
      expect(file.licence.length).toBeGreaterThan(0);
      expect(file.reduction.length).toBeGreaterThan(0);
      expect(["measured", "derived", "standIn"]).toContain(file.provenance);
      expect(["liquid", "solid"]).toContain(file.phase);
      expect(["sphere", "nonSphericalMineral", "crystal", "aggregate"]).toContain(file.shape);
    }
  });

  it("give every fetched file its checksum and the date fetched", () => {
    // Each file carries its own fetch date: T5.b's 2026-10-09, T5.d's later.
    for (const file of MATERIAL_FILES.filter((f) => f.provenance === "measured")) {
      expect(file.source).toMatch(/SHA-256 [0-9a-f]{64}/u);
      expect(file.source).toMatch(/fetched \d{4}-\d{2}-\d{2}/u);
    }
  });

  it("are keyed as the wire is: ASCII, at most 16 bytes, one file per key, phase and variant", () => {
    const seen = new Set<string>();
    for (const file of MATERIAL_FILES) {
      expect(file.key).toMatch(/^[ -~]{1,16}$/u);
      const identity = `${file.key}/${file.phase}/${file.variant ?? ""}`;
      expect(seen.has(identity)).toBe(false);
      seen.add(identity);
    }
  });

  it("are refused when malformed", () => {
    expect(() => parseMaterialFile({ key: "X" }, "bad.json")).toThrow(/bad\.json/u);
  });

  it("accept a well-formed file", () => {
    expect(parseMaterialFile(waterFile, "water.json").key).toBe("H2O");
  });

  it("accept a derived file with a scaled molar refraction, and keep its four fields", () => {
    const parsed = parseMaterialFile(
      { ...waterFile, provenance: "derived", fit: SCALED_FIT },
      "scaled.json",
    );
    expect(parsed.fit).toEqual(SCALED_FIT);
  });

  it("accept a derived file with a scaled gas absorption, and keep its four fields", () => {
    const parsed = parseMaterialFile(
      { ...waterFile, provenance: "derived", fit: GAS_ABSORPTION_FIT },
      "absorption.json",
    );
    expect(parsed.fit).toEqual(GAS_ABSORPTION_FIT);
  });

  it("accept a grid that starts at 360 nm", () => {
    const longer = {
      ...waterFile,
      wavelengthsNm: [360, 365, 370, 375, ...waterFile.wavelengthsNm],
      n: [1.34, 1.34, 1.34, 1.34, ...waterFile.n],
      k: [0, 0, 0, 0, ...waterFile.k],
    };
    expect(Array.from(parseMaterialFile(longer, "longer.json").wavelengthsNm.slice(0, 5))).toEqual([
      360, 365, 370, 375, 380,
    ]);
  });

  // decision-r08-t5d ruling 1's refusals, each with n and k of the grid's own length.
  it.each([
    ["a grid starting at 385 nm", every5(385, 780)],
    ["a grid starting at 362 nm", every5(362, 782)],
    ["a grid with an uneven step", every5(380, 780).map((nm) => (nm === 550 ? 551 : nm))],
    ["a grid ending at 775 nm", every5(380, 775)],
    ["a grid ending at 785 nm", every5(380, 785)],
  ])("refuse %s, by the grid's rule", (_, grid) => {
    expect(() => parseMaterialFile({ ...waterFile, ...onGrid(grid) }, "grid.json")).toThrow(
      /"wavelengthsNm" must run every 5 nm from a multiple of 5 nm at or below 380 nm to 780 nm/u,
    );
  });

  it.each([
    ["a grid starting past 380 nm", { wavelengthsNm: waterFile.wavelengthsNm.slice(1) }],
    ["n and k of other lengths", { n: waterFile.n.slice(1) }],
    ["a k of another length", { k: waterFile.k.slice(1) }],
    ["an n that is not positive", { n: waterFile.n.map(() => 0) }],
    ["a negative k", { k: waterFile.k.map(() => -1e-9) }],
    ["a stand-in that states no stand-in", { provenance: "standIn" }],
    ["a measured file that states a stand-in", { standIn: "something" }],
    ["a derived file without its fit", { provenance: "derived" }],
    ["a bad shape class", { shape: "cube" }],
    // decision-p14-t49h-gas-index A.1: the fit is a union by `form`, and the second form's density
    // and factor are positive, its anchors in order.
    ["a fit of another form", { provenance: "derived", fit: { ...SCALED_FIT, form: "sellmeier" } }],
    [
      "a scaled fit whose density is not positive",
      { provenance: "derived", fit: { ...SCALED_FIT, molarDensityMolPerL: 0 } },
    ],
    [
      "a scaled fit whose factor is not positive",
      { provenance: "derived", fit: { ...SCALED_FIT, factor: -0.98 } },
    ],
    [
      "a scaled fit whose anchors run backwards",
      { provenance: "derived", fit: { ...SCALED_FIT, anchoredFromUm: 0.6, anchoredToUm: 0.5 } },
    ],
    ["a measured file that carries a scaled fit", { fit: SCALED_FIT }],
    // R08.T5.j: the third form's density is positive and its span runs from a positive wavelength
    // to a longer one.
    [
      "a gas absorption whose density is not positive",
      { provenance: "derived", fit: { ...GAS_ABSORPTION_FIT, molarDensityMolPerL: -1 } },
    ],
    [
      "a gas absorption whose span runs backwards",
      { provenance: "derived", fit: { ...GAS_ABSORPTION_FIT, fromUm: 0.6, toUm: 0.5 } },
    ],
    [
      "a gas absorption whose span starts at no wavelength",
      { provenance: "derived", fit: { ...GAS_ABSORPTION_FIT, fromUm: 0 } },
    ],
    [
      "a gas absorption missing its span's end",
      { provenance: "derived", fit: { ...GAS_ABSORPTION_FIT, toUm: undefined } },
    ],
    ["a measured file that carries a gas absorption", { fit: GAS_ABSORPTION_FIT }],
  ])("refuse %s", (_, broken) => {
    expect(() => parseMaterialFile({ ...waterFile, ...broken }, "broken.json")).toThrow(
      /broken\.json/u,
    );
  });

  it("cite no paper in a file's player text: its name, its stand-in or its absorption's", () => {
    // Each reaches the plate (`materialApproximations`, `resolveMaterial`'s named analogue), and
    // each file's `paper`, which no screen shows, holds its citation (R08.T10.e).
    expect(
      MATERIAL_FILES.flatMap((file) => [
        file.name,
        file.standIn ?? "",
        file.absorptionStandIn ?? "",
      ]).filter((text) => CITATION.test(text)),
    ).toEqual([]);
  });

  describe("the citation pattern", () => {
    it.each([
      "Martonchik et al.'s",
      "(Irwin et al. 2022)",
      "Khachai et al. 2009's",
      "(Smith 2020)",
      "Smith 2020,",
      "Smith 2020.",
    ])("takes %s for a citation", (text) => {
      expect(CITATION.test(text)).toBe(true);
    });

    it.each(["1.2015 at 514.5 nm", "at 1950 nm", "at 2000 K", "1,950 nm", "at 1950nm", "2000.5"])(
      "takes %s, a figure, for no citation",
      (text) => {
        expect(CITATION.test(text)).toBe(false);
      },
    );
  });

  it.each([
    ["NH3", "solid", "J. V. Martonchik", "(1984)"],
    ["CaTiO3", "solid", "K. Ueda", "(1998)"],
    ["ice_giant_haze", "solid", "P. G. J. Irwin", "(2022)"],
    ["chromophore", "solid", "A. S. Braude", "(2020)"],
    ["basalt_fe", "solid", "J. Martikainen", "(2023)"],
    ["Na2S", "solid", "H. Khachai", "(2009)"],
  ] as const)("keep the %s %s file's citation in its paper: %s %s", (key, phase, author, year) => {
    // The six whose stand-in text cited its paper until R08.T10.e.
    const { paper } = fileOf(key, phase);
    expect([paper.includes(author), paper.includes(year)]).toEqual([true, true]);
  });

  it("hold no colon in a name, which a named analogue's reason sets after one", () => {
    // `no liquid Jovian chromophore on file: <name> stands in, a named analogue` (R08.T10.f).
    expect(MATERIAL_FILES.map((file) => file.name).filter((name) => name.includes(":"))).toEqual(
      [],
    );
  });
});

describe("refractiveIndex", () => {
  it("gives a file's values at its wavelengths and interpolates between them", () => {
    const water = fileOf("H2O", "liquid");
    const i550 = water.wavelengthsNm.indexOf(550);
    expect(refractiveIndex("H2O", 550)).toEqual({
      index: { n: water.n[i550], k: water.k[i550] },
      provenance: "measured",
    });
    const between = refractiveIndex("H2O", 552.5).index.n;
    expect(between).toBeCloseTo(0.5 * ((water.n[i550] ?? 0) + (water.n[i550 + 1] ?? 0)), 12);
  });

  it("refuses a wavelength outside the files' range", () => {
    expect(() => refractiveIndex("H2O", 379)).toThrow(RangeError);
    expect(() => refractiveIndex("H2O", 781)).toThrow(RangeError);
  });

  // decision-r08-t5d ruling 1: a file is read on its own grid.
  describe.each([
    ["acid_uv_absorber", {}],
    ["H2SO4", { variant: "75wt%" }],
  ] as const)("on %s's 360–780 nm grid", (key, form) => {
    const file = resolveMaterial(key, form).file;
    const at = (i: number): { n: number; k: number } => ({
      n: file?.n[i] ?? Number.NaN,
      k: file?.k[i] ?? Number.NaN,
    });

    it("gives the file's values at 360, 365 and 550 nm", () => {
      expect(refractiveIndex(key, 360, form).index).toEqual(at(0));
      expect(refractiveIndex(key, 365, form).index).toEqual(at(1));
      expect(refractiveIndex(key, 550, form).index).toEqual(at(38));
    });

    it("gives n's linear and k's log-linear mean at 362.5 nm", () => {
      const { n, k } = refractiveIndex(key, 362.5, form).index;
      expect(n).toBeCloseTo(0.5 * (at(0).n + at(1).n), 14);
      expect(Math.abs(k / Math.sqrt(at(0).k * at(1).k) - 1)).toBeLessThan(1e-12);
    });

    it("refuses 359 nm", () => {
      expect(() => refractiveIndex(key, 359, form)).toThrow(RangeError);
    });
  });

  it("refuses 379 nm for the 84.5 wt% acid, every file on 380–780 nm and the generic stand-in", () => {
    expect(() => refractiveIndex("H2SO4", 379, { variant: "84.5wt%" })).toThrow(RangeError);
    for (const file of MATERIAL_FILES.filter((f) => f.wavelengthsNm[0] === 380)) {
      expect(() =>
        refractiveIndex(file.key, 379, {
          phase: file.phase,
          ...(file.variant === undefined ? {} : { variant: file.variant }),
        }),
      ).toThrow(RangeError);
    }
    expect(() => refractiveIndex("Xx9", 379)).toThrow(RangeError);
  });

  it("takes a key's first file by default and a phase's or variant's when asked", () => {
    expect(resolveMaterial("H2O").file?.phase).toBe("liquid");
    expect(resolveMaterial("H2O", { phase: "solid" }).file?.name).toMatch(/ice/u);
    expect(resolveMaterial("H2SO4").file?.variant).toBe("75wt%");
    expect(resolveMaterial("H2SO4", { variant: "84.5wt%" }).file?.variant).toBe("84.5wt%");
    expect(resolveMaterial("CH4", { phase: "solid" }).provenance).toBe("measured");
  });

  it("gives a named analogue, as a stand-in, for a phase or variant not on file", () => {
    const liquidIron = resolveMaterial("Fe", { phase: "liquid" });
    expect(liquidIron.provenance).toBe("standIn");
    expect(liquidIron.file?.key).toBe("Fe");
    expect(liquidIron.standIn).toBe(
      "no liquid Fe on file: metallic Fe films at room temperature stands in, a named analogue",
    );
    const strongAcid = resolveMaterial("H2SO4", { variant: "96wt%" });
    expect(strongAcid.provenance).toBe("standIn");
    expect(strongAcid.file?.variant).toBe("75wt%");
    expect(strongAcid.standIn).toBe(
      "no H2SO4, mass fraction 96 %, on file: aqueous H2SO4, mass fraction 75 %, at 300 K stands in, a named analogue",
    );
  });

  it("gives an unknown key the generic stand-in, a non-absorbing sphere of index 1.5", () => {
    expect(refractiveIndex("Xx9", 550)).toEqual({
      index: GENERIC_STAND_IN_INDEX,
      provenance: "standIn",
    });
    expect(GENERIC_STAND_IN_INDEX).toEqual({ n: 1.5, k: 0 });
  });

  it("names an unknown name key in its stand-in as an unlisted material, never as the key", () => {
    expect(resolveMaterial("brine_xyz").standIn).toBe(
      "no index file for unlisted material: the generic stand-in, a non-absorbing sphere of real index 1.5, a stated convention",
    );
  });

  it("names a name key's material in words where another phase stands in", () => {
    expect(resolveMaterial("mars_dust", { phase: "liquid" }).standIn).toBe(
      "no liquid Mars-type dust on file: Mars-type dust from Mars's atmosphere, indices retrieved from Mars Reconnaissance Orbiter spectrometer and Mars Exploration Rover observations stands in, a named analogue",
    );
  });

  it("names a name key's material in words where another variant stands in", () => {
    expect(resolveMaterial("mars_dust", { variant: "75wt%" }).standIn).toBe(
      "no Mars-type dust, mass fraction 75 %, on file: Mars-type dust from Mars's atmosphere, indices retrieved from Mars Reconnaissance Orbiter spectrometer and Mars Exploration Rover observations stands in, a named analogue",
    );
  });

  it("names each file's substance as every console names it", () => {
    const misnamed = MATERIAL_FILES.filter(
      (file) =>
        substanceName(file.key) === UNLISTED_MATERIAL ||
        !file.name.includes(substanceName(file.key)),
    ).map((file) => file.name);
    expect(misnamed).toEqual([]);
  });

  it("states a sample's temperature in its name in kelvin, as its file holds it", () => {
    // A name gives whole kelvins (decision-ux-numbers-names ruling 1); since R08.T5.k a file's
    // temperature may carry 0.01 K, so the name binds it rounded.
    const misstated = MATERIAL_FILES.filter((file) => {
      const kelvin = /(?:at|near) ([0-9]+) K\b/u.exec(file.name)?.[1];
      const temperature =
        file.temperatureK === undefined ? undefined : Math.round(file.temperatureK);
      return file.name.includes("°C") || (kelvin !== undefined && Number(kelvin) !== temperature);
    }).map((file) => file.name);
    expect(misstated).toEqual([]);
  });

  it("signs a number in a file's player text with the hyphen-minus, as the formatters do", () => {
    expect(
      MATERIAL_FILES.flatMap((file) => [file.name, file.standIn ?? ""]).filter((text) =>
        /−[0-9]/u.test(text),
      ),
    ).toEqual([]);
  });

  it("groups no number of four digits or fewer in a file's player text, as the guide writes them", () => {
    // ux-guidelines.md, "Numbers, units and time": digits group in threes from five digits.
    expect(
      MATERIAL_FILES.flatMap((file) => [file.name, file.standIn ?? ""]).filter((text) =>
        /(?<![0-9,])[0-9],[0-9]{3}(?![0-9,])/u.test(text),
      ),
    ).toEqual([]);
  });

  it("names the enstatite glass by its phase and formula, with no mineral name", () => {
    expect(MATERIAL_FILES.find((file) => file.key === "MgSiO3")?.name).toBe("amorphous MgSiO3");
  });

  it("writes a variant's mass fraction in words, and any other variant as another composition", () => {
    expect(["75wt%", "84.5wt%", "dilute", undefined].map(variantWords)).toEqual([
      "mass fraction 75 %",
      "mass fraction 84.5 %",
      "another composition",
      "another composition",
    ]);
  });

  it("writes a variant's practical salinity in words, with no unit (decision-r08-t5g ruling 4)", () => {
    expect(["S35", "S38.5", "S", "S-1"].map(variantWords)).toEqual([
      "practical salinity 35",
      "practical salinity 38.5",
      "another composition",
      "another composition",
    ]);
  });
});

describe("sulphuric acid", () => {
  it("has n at 550 nm inside Hansen and Hovenier 1974's 1.44 ± 0.015", () => {
    for (const variant of ["75wt%", "84.5wt%"]) {
      const { n } = refractiveIndex("H2SO4", 550, { variant }).index;
      expect(Math.abs(n - 1.44)).toBeLessThanOrEqual(0.015);
    }
  });
});

describe("Mars dust", () => {
  it("has k at 380 nm above its 440 nm row and at most the 440–500 nm rows' log-linear extrapolation", () => {
    // science-r08-nonspherical.md §3.2: the 321 nm row anchors 380–440 nm; the two attributed
    // alternatives bound it, holding the 440 nm row (0.00767) and extrapolating to 0.0107.
    const at440 = marsDustK(440);
    const extrapolated = at440 * (marsDustK(500) / at440) ** ((380 - 440) / (500 - 440));
    expect(at440).toBeCloseTo(0.00767, 5);
    expect(extrapolated).toBeCloseTo(0.0107, 4);
    expect(marsDustK(380)).toBeGreaterThan(at440);
    expect(marsDustK(380)).toBeLessThanOrEqual(extrapolated);
  });

  it("states that its 263 nm row is not used", () => {
    expect(fileOf("mars_dust").reduction).toMatch(
      /263 nm row lies outside the grid and is not used/u,
    );
  });
});

describe("CO₂ ice, a derived file", () => {
  const file = fileOf("CO2");
  const fit = file.fit;
  if (fit?.form !== "cauchy") {
    throw new Error("CO₂ ice carries no Cauchy fit");
  }
  const fitN = (wavelengthNm: number): number => fit.a + fit.bUm2 / (wavelengthNm / 1000) ** 2;

  it("is derived, a crystal, and holds its fit", () => {
    expect(file.provenance).toBe("derived");
    expect(file.shape).toBe("crystal");
    expect(fit.fittedFromUm).toBe(0.3);
    expect(fit.fittedToUm).toBe(1.1);
  });

  it("has Warren 1986's real index, 1.413 ± 0.001 at 553 nm and 1.404 ± 0.001 at 1,000 nm", () => {
    // Warren 1986, Appl. Opt. 25, 2650, Table I: asserted values.
    expect(Math.abs(fitN(553) - 1.413)).toBeLessThanOrEqual(0.001);
    expect(Math.abs(fitN(1000) - 1.404)).toBeLessThanOrEqual(0.001);
    expect(Math.abs(refractiveIndex("CO2", 553).index.n - 1.413)).toBeLessThanOrEqual(0.001);
  });

  it("gives the fit's values on its grid", () => {
    for (const [i, nm] of file.wavelengthsNm.entries()) {
      expect(Math.abs((file.n[i] ?? 0) - fitN(nm))).toBeLessThan(1e-5);
      const k = (fit.absorptionPerM * nm * 1e-9) / (4 * Math.PI);
      expect(Math.abs((file.k[i] ?? 0) / k - 1)).toBeLessThan(1e-5);
    }
  });

  it("has k between 0 and 2.2 × 10⁻⁶ over 380–780 nm", () => {
    for (const k of file.k) {
      expect(k).toBeGreaterThan(0);
      expect(k).toBeLessThan(2.2e-6);
    }
  });

  it("scatters a 2 µm sphere at 550 nm with a single-scattering albedo above 0.9999", () => {
    const x = (2 * Math.PI * 2) / 0.55;
    const result = mieSphere(x, refractiveIndex("CO2", 550).index, new Float64Array(0));
    expect(result.qSca / result.qExt).toBeGreaterThan(0.9999);
  });
});

describe("NH₄SH, a stated stand-in", () => {
  const file = fileOf("NH4SH");

  it("has an estimated n of 1.638 ± 0.001 at 550 nm", () => {
    // The Lorentz–Lorenz estimate of science-r08-nonspherical.md §3.1, replacing the 1.80 stand-in,
    // with NH₃ at its theoretic density and NH₄Cl's increment re-derived (R08's Risks, "Deviations in
    // T3.b's follow-up, as built") from NH₄Cl's X-ray cell and measured dispersion
    // (science-r08-gases-2.md §2); the ruling's 1.648 took NH₃'s real-gas Z.
    expect(Math.abs(refractiveIndex("NH4SH", 550).index.n - 1.638)).toBeLessThanOrEqual(0.001);
  });

  it("has n falling monotonically from 380 to 780 nm", () => {
    for (let i = 1; i < file.n.length; i += 1) {
      expect(file.n[i]).toBeLessThan(file.n[i - 1] ?? Number.NaN);
    }
  });

  it("is non-absorbing", () => {
    expect(file.k.every((k) => k === 0)).toBe(true);
  });

  it("stays a stand-in that states its estimate", () => {
    expect(refractiveIndex("NH4SH", 550).provenance).toBe("standIn");
    expect(resolveMaterial("NH4SH").standIn).toBe(
      "a non-absorbing particle whose real index is a Lorentz–Lorenz estimate, 1.674 at 380 nm, 1.638 at 550 nm and 1.623 at 780 nm, within +0.081 and -0.022, since no visible optical constants are published",
    );
    expect(fileOf("NH4SH").source).toContain(
      "1.80 would need a molar refraction 22% above the additive one.",
    );
  });

  it("takes rayleigh.ts's NH₃ and H₂S rows, stated at 0 °C and 760 mm, as its estimate does", () => {
    for (const gas of NH4SH_GASES) {
      expect([gas.referenceK, gas.referencePa]).toEqual([273.15, 101_325]);
    }
  });

  it("is the Lorentz–Lorenz index of rayleigh.ts's NH₃ and H₂S rows at West 1934's cell", () => {
    // science-r08-nonspherical.md §3.1: (n² − 1) ÷ (n² + 2) = 1.027 R ÷ V_m, with R the gases'
    // summed molar refraction as stated at 0 °C and 760 mm, each at its formula's Z, 1.027 NH₄Cl's
    // increment (the tool's NH4SH_IONIC_INCREMENT) and V_m from West's cell (COD 1010249:
    // a = 6.011 Å, c = 4.009 Å, two formula units). The tool holds its own copy of the rows; this
    // catches drift.
    const idealMolarVolumeCm3PerMol = 22_413.969_54;
    const cellMolarVolumeCm3PerMol = (6.022_140_76e23 * 6.011 ** 2 * 4.009 * 1e-24) / 2;
    for (const [i, nm] of file.wavelengthsNm.entries()) {
      const molarRefraction = NH4SH_GASES.reduce(
        (sum, gas) =>
          sum + lorentz(1 + gas.nMinusOne(nm)) * gas.compressibility * idealMolarVolumeCm3PerMol,
        0,
      );
      const expected = (1.027 * molarRefraction) / cellMolarVolumeCm3PerMol;
      expect(Math.abs(lorentz(file.n[i] ?? Number.NaN) / expected - 1)).toBeLessThan(2e-5);
    }
  });

  it("gives atmosphereApproximate with less than the split's optical depth above it", () => {
    const layers = [{ material: "NH4SH", opticalDepthAbove550: 2 }];
    expect(materialLabels(layers)).toEqual(["atmosphereApproximate"]);
    expect(materialApproximations(layers)).toHaveLength(1);
    expect(
      materialLabels([
        { material: "NH4SH", opticalDepthAbove550: CLOUD_DECK_SPLIT_OPTICAL_DEPTH * 0.999 },
      ]),
    ).toEqual(["atmosphereApproximate"]);
  });

  it("gives no reason beneath a τ 30 deck", () => {
    // Since R08.T5.i the NH₃-ice deck above is a stand-in too (decision-p14-t49c-split §5.3), so
    // the body takes the label from it; the NH₄SH mode under τ 30 adds no reason.
    expect(
      materialApproximations([
        { material: "NH3", form: { phase: "solid" }, opticalDepthAbove550: 0 },
        { material: "NH4SH", opticalDepthAbove550: 30 },
      ]).map((a) => a.material),
    ).toEqual(["NH3"]);
  });
});

describe("the solids' visible absorption (R08.T5.i, decision-p14-t49c-split §5.3)", () => {
  const ammonia = fileOf("NH3", "solid");
  const titanate = fileOf("CaTiO3");
  const sulphide = fileOf("Na2S");

  it("files NH3 ice, CaTiO3 and Na2S as stand-ins", () => {
    expect([ammonia, titanate, sulphide].map((file) => file.provenance)).toEqual([
      "standIn",
      "standIn",
      "standIn",
    ]);
  });

  it("keeps each one's fetched source with its checksum and the date fetched", () => {
    // "give every fetched file its checksum and the date fetched" reads the `measured` files, which
    // NH3 ice and CaTiO3 have left; all three are reduced from a fetched source.
    for (const file of [ammonia, titanate, sulphide]) {
      expect(file.source).toMatch(/SHA-256 [0-9a-f]{64}/u);
      expect(file.source).toMatch(/fetched \d{4}-\d{2}-\d{2}/u);
    }
  });

  it("draws NH3 ice with k = 0 at every grid point", () => {
    expect(Array.from(ammonia.k).filter((k) => k !== 0)).toEqual([]);
  });

  it("keeps Martonchik et al.'s n for NH3 ice: 1.43606 at 550 nm", () => {
    expect(valueAt(ammonia.n, ammonia, 550)).toBe(1.43606);
  });

  it("keeps CaTiO3's and Na2S's k as their bounds: 1.81089 × 10⁻³ and 1.46463 × 10⁻² at 550 nm", () => {
    expect([valueAt(titanate.k, titanate, 550), valueAt(sulphide.k, sulphide, 550)]).toEqual([
      1.81089e-3, 1.46463e-2,
    ]);
  });

  it("names each stand-in's reason: the fill, reflectance's Kramers–Kronig value, the computed spectrum", () => {
    expect(ammonia.standIn).toMatch(
      /a fill between the values at 1462 nm and 320 nm, not a measurement/u,
    );
    expect(titanate.standIn).toMatch(/a Kramers–Kronig value from normal-incidence reflectance/u);
    expect(sulphide.standIn).toMatch(/a computed spectrum's, not a colourless solid's absorption/u);
  });

  it.each([
    [
      "an NH3-ice",
      { material: "NH3", form: { phase: "solid" }, opticalDepthAbove550: 0 },
      ammonia,
      "NH3 ice",
    ],
    ["a CaTiO3", { material: "CaTiO3", opticalDepthAbove550: 0 }, titanate, "solid CaTiO3"],
  ] as const)(
    "gives atmosphereApproximate for %s deck at τ 0, its file's stand-in after its phase and name the reason",
    (_, layer, file, head) => {
      expect(materialLabels([layer])).toEqual(["atmosphereApproximate"]);
      expect(materialApproximations([layer])).toEqual([
        { material: layer.material, reason: `${head}: ${file.standIn ?? ""}` },
      ]);
    },
  );
});

describe("the stand-in label", () => {
  it("is given by an unknown material key the body shows", () => {
    expect(materialLabels([{ material: "Xx9", opticalDepthAbove550: 0 }])).toEqual([
      "atmosphereApproximate",
    ]);
    expect(
      materialApproximations([{ material: "Xx9", opticalDepthAbove550: 0 }])[0]?.reason,
    ).toMatch(/generic stand-in/u);
  });

  it("puts a file's own stand-in text after its phase and name: liquid CO's, the chromophore's", () => {
    // decision-r08-approximate-plate.md §8.3 (R08.T10.e): neither text names its material. The
    // chromophore, named in words and held as a solid alone, takes no phase word (R08.T10.f).
    expect(
      materialApproximations([
        { material: "CO", form: { phase: "liquid" }, opticalDepthAbove550: 0 },
        { material: "chromophore", opticalDepthAbove550: 0 },
      ]).map(({ reason }) => reason),
    ).toEqual([
      `liquid CO: ${liquidOf("CO").standIn ?? ""}`,
      `Jovian chromophore: ${fileOf("chromophore").standIn ?? ""}`,
    ]);
  });

  it("leaves the reasons resolveMaterial writes as written: a named analogue's, the generic stand-in's", () => {
    // The chromophore's liquid is drawn with the solid's file, a stand-in, whose text it does not
    // give.
    expect(
      materialApproximations([
        { material: "SO2", form: { phase: "solid" }, opticalDepthAbove550: 0 },
        { material: "chromophore", form: { phase: "liquid" }, opticalDepthAbove550: 0 },
        { material: "Xx9", opticalDepthAbove550: 0 },
      ]).map(({ reason }) => reason),
    ).toEqual([
      "no SO2 ice on file: liquid SO2 at 198 K stands in, a named analogue",
      "no liquid Jovian chromophore on file: Jovian chromophore, an effective index retrieved from Jupiter's spectrum stands in, a named analogue",
      "no index file for Xx9: the generic stand-in, a non-absorbing sphere of real index 1.5, a stated convention",
    ]);
  });

  // decision-r08-approximate-plate.md §8.3 (R08.T10.f): the head names the material, so the text
  // after it holds no colon that a comma or "since" would serve, and names the material nowhere.
  it("sets each file's own text after its head with no second colon", () => {
    const reasons = ownTextReasons();
    expect(reasons).toHaveLength(17);
    expect(
      reasons
        .filter(
          ({ head, reason }) => !reason.startsWith(head) || reason.slice(head.length).includes(":"),
        )
        .map(({ reason }) => reason),
    ).toEqual([]);
  });

  it("names no file's material again in its own text after its head", () => {
    const reasons = ownTextReasons();
    expect(reasons).toHaveLength(17);
    expect(
      reasons
        .filter(({ file, head, reason }) =>
          holdsWord(
            OTHER_TERMS_WITH_NAME.reduce(
              (text, term) => text.replaceAll(term, ""),
              reason.slice(head.length),
            ),
            substanceName(file.key),
          ),
        )
        .map(({ reason }) => reason),
    ).toEqual([]);
  });

  it("is not given by measured or derived materials", () => {
    expect(
      materialLabels([
        { material: "H2SO4", opticalDepthAbove550: 0 },
        { material: "CO2", opticalDepthAbove550: 0 },
        { material: "tholin", opticalDepthAbove550: 0.1 },
      ]),
    ).toEqual([]);
  });

  it("refuses an optical depth that is negative or not finite", () => {
    expect(() => materialLabels([{ material: "H2O", opticalDepthAbove550: -1 }])).toThrow(
      RangeError,
    );
    expect(() => materialLabels([{ material: "H2O", opticalDepthAbove550: Number.NaN }])).toThrow(
      RangeError,
    );
  });
});

describe("liquid O₂'s colour (signoff-12.md, item 38(c))", () => {
  const LIQUID: MaterialForm = { phase: "liquid" };

  it("gives its own reason where it shows: its gas's pair absorption, no measurement of the liquid's", () => {
    expect(
      materialApproximations([{ material: "O2", form: LIQUID, opticalDepthAbove550: 1 }]),
    ).toEqual([
      {
        material: "O2",
        reason:
          "liquid O2: its absorption is the gas's O2–O2 pair absorption at 203 K carried to the liquid's density; no measurement of the liquid's is read",
      },
    ]);
  });

  it("keeps its file derived, as ruled", () => {
    expect(resolveMaterial("O2", LIQUID).provenance).toBe("derived");
  });

  it("gives no reason beneath a deck at the split's depth", () => {
    expect(
      materialApproximations([
        { material: "O2", form: LIQUID, opticalDepthAbove550: CLOUD_DECK_SPLIT_OPTICAL_DEPTH },
      ]),
    ).toEqual([]);
  });

  it("is liquid O₂'s alone: liquid N₂ gives no reason", () => {
    expect(
      materialApproximations([{ material: "N2", form: LIQUID, opticalDepthAbove550: 1 }]),
    ).toEqual([]);
  });
});

describe("a named analogue's phase word (signoff-12.md, item 39(c))", () => {
  const SOLID: MaterialForm = { phase: "solid" };

  it("calls a volatile molecular solid an ice: no SO2 ice on file", () => {
    expect(resolveMaterial("SO2", SOLID).standIn).toBe(
      "no SO2 ice on file: liquid SO2 at 198 K stands in, a named analogue",
    );
  });

  it("calls an atomic volatile's solid a solid, not an ice: no solid Kr on file", () => {
    expect(resolveMaterial("Kr", SOLID).standIn).toMatch(/^no solid Kr on file: /u);
  });

  it("calls a refractory's solid a solid: no solid H2SO4 on file", () => {
    expect(resolveMaterial("H2SO4", SOLID).standIn).toMatch(/^no solid H2SO4 on file: /u);
  });

  it("knows every ice nonSpherical.ts draws on water ice's table as a volatile ice", () => {
    expect([...ICE_KEYS].filter((key) => !VOLATILE_ICE_KEYS.has(key))).toEqual([]);
  });

  it("names Ar, an ice of that table, an atomic solid, as nonSpherical.ts's phase notes do", () => {
    expect(ATOMIC_SOLID_KEYS.has("Ar")).toBe(true);
  });
});

describe("a material named in words, solid alone (R08.T10.f; decision-r08-approximate-plate.md §8.3)", () => {
  it("takes no phase word for its solid: Jovian chromophore, tholin, Fe-rich basalt", () => {
    expect(["chromophore", "tholin", "basalt_fe"].map((key) => phasedName(key, "solid"))).toEqual([
      "Jovian chromophore",
      "tholin",
      "Fe-rich basalt",
    ]);
  });

  it("keeps liquid for its liquid: liquid Jovian chromophore", () => {
    expect(phasedName("chromophore", "liquid")).toBe("liquid Jovian chromophore");
  });

  it("leaves the formulas' phase words and a liquid named in words as they are", () => {
    expect([
      phasedName("Fe", "solid"),
      phasedName("CaTiO3", "solid"),
      phasedName("NH3", "solid"),
      phasedName("acid_uv_absorber", "liquid"),
    ]).toEqual(["solid Fe", "solid CaTiO3", "NH3 ice", "liquid ultraviolet-absorbing H2SO4"]);
  });

  it("holds the registry's name keys with a solid grain density and no liquid one", () => {
    const solidAlone = registry.substances
      .filter(
        (row) =>
          Object.hasOwn(SUBSTANCE_NAMES, row.key) &&
          row.solid_grain_density !== null &&
          row.liquid_grain_density === null,
      )
      .map((row) => row.key);
    expect(solidAlone).toHaveLength(28);
    expect([...WORDED_SOLID_KEYS].toSorted()).toEqual(solidAlone.toSorted());
  });
});

describe("the registry's aerosol materials (R08.T5.d)", () => {
  // That every aerosol-material row has a file of its own, and every file's key is a row, is
  // R08.T19's parity check (`../parity.test.ts`).
  it("find O2 and Ar among the fixture's aerosol-material rows (decision-r08-t5d ruling 3)", () => {
    const keys = new Set(
      registry.substances.filter((row) => row.aerosol_material).map((row) => row.key),
    );
    expect(["O2", "Ar"].filter((key) => !keys.has(key))).toEqual([]);
  });

  it("name no material by a mineral name", () => {
    // decision-ux-numbers-names: "crystalline Al2O3", not corundum; "crystalline SiO2", not quartz.
    // Graphite is the drafted name of its key (decision-substance-names), so it is not listed.
    const minerals =
      /\b(?:quartz|corundum|sapphire|rutile|anatase|sylvite|halite|sphalerite|wurtzite|alabandite|troilite|pyrrhotite|perovskite|forsterite|enstatite|cristobalite|tridymite)\b/iu;
    expect(MATERIAL_FILES.map((f) => f.name).filter((name) => minerals.test(name))).toEqual([]);
  });

  it("hold every key of R08.T5.d's list", () => {
    const keys = new Set(MATERIAL_FILES.map((f) => f.key));
    for (const key of T5D_KEYS) {
      expect(keys.has(key)).toBe(true);
    }
  });

  it("give a 1 µm Mg2SiO4 sphere a single-scattering albedo above 0.99 at 550 nm, and iron less", () => {
    const x = (2 * Math.PI * 1) / 0.55;
    const albedo = (key: string): number => {
      const mie = mieSphere(x, refractiveIndex(key, 550).index, new Float64Array(0));
      return mie.qSca / mie.qExt;
    };
    expect(albedo("Mg2SiO4")).toBeGreaterThan(0.99);
    expect(albedo("Fe")).toBeLessThan(albedo("Mg2SiO4"));
  });
});

describe("acid_uv_absorber, Venus's empirical ultraviolet absorber (decision-r08-t5d ruling 4)", () => {
  const absorber = fileOf("acid_uv_absorber");
  const acid = resolveMaterial("H2SO4", { variant: "75wt%" }).file;
  if (acid === undefined) {
    throw new Error("the 75 wt% acid has no file");
  }

  it("is a liquid sphere and a stand-in that says what stands in", () => {
    expect([absorber.phase, absorber.shape, absorber.provenance]).toEqual([
      "liquid",
      "sphere",
      "standIn",
    ]);
    expect(absorber.standIn).toBe(
      "an empirical absorber, droplets that carry an unidentified ultraviolet absorber whose k is fitted to Venus's albedo spectrum and whose candidates are FeCl3, S2O, OSSO, Sx and organics",
    );
  });

  it("has the 75 wt% acid's grid and n exactly, 360–780 nm", () => {
    expect(Array.from(absorber.wavelengthsNm)).toEqual(Array.from(acid.wavelengthsNm));
    expect(absorber.wavelengthsNm[0]).toBe(360);
    expect(Array.from(absorber.n)).toEqual(Array.from(acid.n));
  });

  it("has the acid's own k exactly from 550 nm: k_UA is 0 there", () => {
    expect(kFrom(absorber, 550)).toEqual(kFrom(acid, 550));
    expect(refractiveIndex("acid_uv_absorber", 550).index).toEqual(
      refractiveIndex("H2SO4", 550).index,
    );
  });

  it("adds k_UA to the acid's k below 550 nm, to the files' six figures, and never less", () => {
    // Six significant figures hold a value to half a unit in the sixth, at most 5 × 10⁻⁶ of it.
    for (const [i, nm] of absorber.wavelengthsNm.entries()) {
      const k = absorber.k[i] ?? Number.NaN;
      const own = acid.k[i] ?? Number.NaN;
      expect(k).toBeGreaterThanOrEqual(own);
      expect(Math.abs(k - own - absorberK(nm))).toBeLessThanOrEqual(5e-6 * k);
    }
  });

  it("gives atmosphereApproximate as a Venus-class deck's mode 2 under less than τ 10", () => {
    const deck = [
      { material: "H2SO4", opticalDepthAbove550: 0 },
      { material: "acid_uv_absorber", opticalDepthAbove550: 8 },
      { material: "H2SO4", opticalDepthAbove550: 9.5 },
    ];
    expect(materialLabels(deck)).toEqual(["atmosphereApproximate"]);
    expect(materialApproximations(deck).map((a) => a.material)).toEqual(["acid_uv_absorber"]);
    expect(materialLabels([{ material: "acid_uv_absorber", opticalDepthAbove550: 10 }])).toEqual(
      [],
    );
  });

  it("is drawn by Mie as Venus's mode 2, a model", () => {
    // Venus's mode-2 droplets, r_eff 1.05 µm and v_eff 0.07 (Hansen and Hovenier 1974).
    const optics = aerosolModeOptics(
      {
        kind: "distribution",
        mode: {
          material: "acid_uv_absorber",
          sizes: { kind: "gamma", effectiveRadiusUm: 1.05, effectiveVariance: 0.07 },
        },
      },
      550,
    );
    expect([optics.shape, optics.phaseModel, optics.phaseBasis]).toEqual([
      "sphere",
      "mie",
      "model",
    ]);
  });
});

/**
 * The molecular ices' Lorentz–Lorenz estimates (decision-r08-t5d rulings 3 and 5): each crystal's
 * cell as its paper gives it through the Crystallography Open Database, Å³, and its molecules.
 */
const ICE_CELLS = [
  // β-N2 at 46 K (Schuch and Mills 1970, COD 9012476), hexagonal, a = 4.050 Å, c = 6.604 Å.
  ["N2", (Math.sqrt(3) / 2) * 4.05 ** 2 * 6.604, 2],
  // γ-O2 at 46 K (Cox, Samuelsen and Beckurts 1973, COD 9011647).
  ["O2", 6.78 ** 3, 8],
  // α-CO (Vegard 1930, COD 1010305).
  ["CO", 5.63 ** 3, 4],
  // H2S phase I (Vegard and Sinding-Oserød 1942, COD 1539470).
  ["H2S", 5.797 ** 3, 4],
] as const;

describe("the molecular ices' stand-ins", () => {
  it.each(ICE_CELLS)(
    "give %s ice the Lorentz–Lorenz index of rayleigh.ts's row at its cell",
    (key, cellA3, molecules) => {
      // The tool holds its own copy of each row; this catches drift. R = (n² − 1) ÷ (n² + 2) × Z V₀
      // at the row's own reference state, and x = R ÷ V_m.
      const gas = GAS_DISPERSION[key];
      const file = fileOf(key, "solid");
      const cellMolarVolume = (6.022_140_76e23 * cellA3 * 1e-24) / molecules;
      const idealMolarVolume =
        ((6.022_140_76e23 * 1.380_649e-23 * gas.referenceK) / gas.referencePa) * 1e6;
      for (const [i, nm] of file.wavelengthsNm.entries()) {
        const expected =
          (lorentz(1 + gas.nMinusOne(nm)) * gas.compressibility * idealMolarVolume) /
          cellMolarVolume;
        expect(Math.abs(lorentz(file.n[i] ?? Number.NaN) / expected - 1)).toBeLessThan(2e-5);
      }
      expect([file.shape, file.provenance]).toEqual(["crystal", "standIn"]);
      expect(file.k.every((k) => k === 0)).toBe(true);
    },
  );

  it("states that solid O2's weak visible bands are not drawn", () => {
    expect(fileOf("O2", "solid").source).toContain("pale blue, are not drawn");
  });
});

describe("Ar ice, measured (decision-r08-t5d ruling 3)", () => {
  const file = fileOf("Ar", "solid");

  it("is Sinnock and Smith 1969's solid at 80 K, a measured crystal", () => {
    expect([file.temperatureK, file.shape, file.provenance]).toEqual([80, "crystal", "measured"]);
  });

  it("has their 1.2708 at 546.1 nm and holds their 643.9 nm row's 1.2693 to 780 nm", () => {
    expect(Math.abs(refractiveIndex("Ar", 546.1).index.n - 1.2708)).toBeLessThan(2e-5);
    const held = Array.from(file.n.slice(file.wavelengthsNm.indexOf(645)));
    expect(new Set(held)).toEqual(new Set([1.2693]));
    expect(file.k.every((k) => k === 0)).toBe(true);
  });
});

describe("the liquids (R08.T5.g)", () => {
  it.each(T5G_LIQUIDS)(
    "file liquid %s, variant %s, as a sphere, %s",
    (key, variant, provenance) => {
      const file = liquidOf(key, variant);
      expect([file.shape, file.provenance]).toEqual(["sphere", provenance]);
      const resolved = resolveMaterial(key, {
        phase: "liquid",
        ...(variant === undefined ? {} : { variant }),
      });
      expect(resolved.file).toBe(file);
      expect(resolved.provenance).toBe(provenance);
    },
  );

  it("leave every key's default file as it was", () => {
    expect(resolveMaterial("H2O").file?.name).toBe("liquid H2O at 298 K");
    expect(resolveMaterial("H2O", { phase: "liquid" }).file?.variant).toBeUndefined();
    expect(["N2", "NH3", "S8"].map((key) => resolveMaterial(key).file?.phase)).toEqual([
      "solid",
      "solid",
      "solid",
    ]);
    expect(resolveMaterial("C2H6").file?.phase).toBe("liquid");
    // R08.T5.k: the keys with an ice keep it as their default; the four new keys' liquid is their
    // only file.
    expect(["O2", "Ar", "CO", "CO2", "H2S"].map((key) => resolveMaterial(key).file?.phase)).toEqual(
      ["solid", "solid", "solid", "solid", "solid"],
    );
    expect(T5K_KEYS.map((key) => resolveMaterial(key).file?.phase)).toEqual([
      "liquid",
      "liquid",
      "liquid",
      "liquid",
    ]);
  });

  it("hold every key of R08.T5.g's list", () => {
    const keys = new Set(MATERIAL_FILES.map((f) => f.key));
    expect(T5G_KEYS.filter((key) => !keys.has(key))).toEqual([]);
  });

  it.each([
    [380, 0.00143],
    [400, 0.00222],
    [420, 0.00312],
    [440, 0.00522],
  ])("give water's k at %d nm as Mason, Cone and Fry 2016's a, %f m⁻¹ (R08.T5.h)", (nm, perM) => {
    // Their Table 2, as WOPP v3 and the journal give it: k = aλ ÷ 4π, to six figures.
    const water = liquidOf("H2O");
    expect(Math.abs(valueAt(water.k, water, nm) / absorptionK(perM, nm) - 1)).toBeLessThan(5e-6);
  });

  it.each([
    [550, 0.0565],
    [725, 1.489],
    [740, 2.78],
    [750, 2.85],
    [780, 2.69],
  ])(
    "give water's k at %d nm as Pope and Fry's to 725 nm and Kou et al.'s from 730 nm, %f m⁻¹ (R08.T5.h)",
    (nm, perM) => {
      // Pope and Fry 1997 over 500–725 nm; Kou et al. 1993 over 730–780 nm, as IOCCG 2018's Table 1.1
      // lists them.
      const water = liquidOf("H2O");
      expect(Math.abs(valueAt(water.k, water, nm) / absorptionK(perM, nm) - 1)).toBeLessThan(5e-6);
    },
  );

  it("meet Pope and Fry at the seam: water's a at 495 nm within 3% of their 0.0173 m⁻¹", () => {
    const water = liquidOf("H2O");
    const perM = (valueAt(water.k, water, 495) * 4 * Math.PI) / 495e-9;
    expect(Math.abs(perM / 0.0173 - 1)).toBeLessThan(0.03);
  });

  it("take water's n from IAPWS R9-97 at 25 °C: 1.34511 at 380 nm, 1.33419 at 550 nm, 1.32848 at 780 nm", () => {
    // decision-r08-t5g ruling 1; the tool's test holds every grid point to the release's eq. 1.
    const water = liquidOf("H2O");
    expect([380, 550, 780].map((nm) => valueAt(water.n, water, nm))).toEqual([
      1.34511, 1.33419, 1.32848,
    ]);
    expect(water.n.every((n, i) => i === 0 || n <= (water.n[i - 1] ?? 0))).toBe(true);
  });

  it("keep pure liquid water measured: R9-97 is a reference correlation fitted to measurements", () => {
    expect(liquidOf("H2O").provenance).toBe("measured");
  });

  it("give seawater water's k, and n raised by Quan and Fry's salinity terms at S = 35", () => {
    const water = liquidOf("H2O");
    const sea = liquidOf("H2O", "S35");
    expect(Array.from(sea.k)).toEqual(Array.from(water.k));
    const rise = Array.from(sea.n, (n, i) => n - (water.n[i] ?? Number.NaN));
    expect(Math.min(...rise)).toBeGreaterThan(0.0061);
    expect(Math.max(...rise)).toBeLessThan(0.0068);
    expect(rise.every((r, i) => i === 0 || r <= (rise[i - 1] ?? 0) + 1e-5)).toBe(true);
  });

  it("level liquid C2H6 to Hagermann et al.'s 1.3887 at 670 nm, its slope from rayleigh.ts's row at the liquid's density", () => {
    // The tool holds its own copy of the row; this catches drift. 21.53485 mol/l at 94 K is the
    // NIST Chemistry WebBook's (Bücker and Wagner 2006). R08.T5.h levels the estimate to the measured
    // 1.3887 (decision-r08-t5g ruling 3), and the file's Cauchy line follows the shifted estimate.
    const gas = GAS_DISPERSION.C2H6;
    const file = liquidOf("C2H6");
    const idealMolarVolume =
      ((6.022_140_76e23 * 1.380_649e-23 * gas.referenceK) / gas.referencePa) * 1e6;
    const estimate = (nm: number): number => {
      const x =
        (lorentz(1 + gas.nMinusOne(nm)) * gas.compressibility * idealMolarVolume) /
        (1_000 / 21.534_85);
      return Math.sqrt((1 + 2 * x) / (1 - x));
    };
    const shift = estimate(670) - 1.3887;
    for (const [i, nm] of file.wavelengthsNm.entries()) {
      expect(Math.abs((file.n[i] ?? Number.NaN) - (estimate(nm) - shift))).toBeLessThan(5e-4);
    }
    expect(Math.abs(refractiveIndex("C2H6", 670).index.n - 1.3887)).toBeLessThan(1e-5);
    expect([file.temperatureK, file.standIn]).toEqual([94, undefined]);
  });

  it("draw a liquid C2H6 sea with no label, its index levelled to a measurement", () => {
    expect(
      materialLabels([{ material: "C2H6", form: { phase: "liquid" }, opticalDepthAbove550: 0 }]),
    ).toEqual([]);
  });

  it("fit liquid N2 to Augustin 1915's four absolute indices at 77.32 K within 3 × 10⁻⁴", () => {
    const augustin = [
      [656.3, 1.19841],
      [579.0, 1.19873],
      [546.1, 1.19917],
      [435.9, 1.20141],
    ] as const;
    for (const [nm, measured] of augustin) {
      const { n } = refractiveIndex("N2", nm, { phase: "liquid" }).index;
      expect(Math.abs(n - measured)).toBeLessThan(3e-4);
    }
  });

  it("meet Wong and Anderson 1972's 1.360 at 632.8 nm for liquid NH3 at 239.8 K", () => {
    expect(refractiveIndex("NH3", 632.8, { phase: "liquid" }).index.n).toBeCloseTo(1.36, 4);
  });

  it("give each derived liquid its fit's n on its grid, and no absorption", () => {
    for (const key of ["N2", "NH3", "C2H6"]) {
      const file = liquidOf(key);
      const fit = file.fit;
      if (fit?.form !== "cauchy") {
        throw new Error(`liquid ${key} carries no Cauchy fit`);
      }
      for (const [i, nm] of file.wavelengthsNm.entries()) {
        const fitN = fit.a + fit.bUm2 / (nm / 1000) ** 2;
        expect(Math.abs((file.n[i] ?? Number.NaN) - fitN)).toBeLessThan(1e-5);
      }
      expect([fit.absorptionPerM, file.k.every((k) => k === 0)]).toEqual([0, true]);
    }
  });

  it("give molten S8 Becquerel 1877's 1.929, a non-absorbing stand-in that says its edge is missing", () => {
    const melt = liquidOf("S8");
    expect(new Set(melt.n)).toEqual(new Set([1.929]));
    expect(melt.k.every((k) => k === 0)).toBe(true);
    expect(melt.standIn).toContain("its blue absorption edge is not drawn");
    expect(
      materialLabels([{ material: "S8", form: { phase: "liquid" }, opticalDepthAbove550: 0 }]),
    ).toEqual(["atmosphereApproximate"]);
  });
});

/**
 * Sinnock and Smith 1969's liquid tables at their melting points (Phys. Rev. 181, 1297, through
 * refractiveindex.info's CC0 copies, `main/{Ar,Kr,Xe}/nk/Sinnock-liquid-*.yml`), λ in nm and n,
 * cited as facts: the measured inputs of R08.T5.k's three noble-gas files.
 */
const SINNOCK_LIQUID_ROWS = {
  Ar: [
    [361.2, 1.2395],
    [365, 1.2392],
    [406.3, 1.2372],
    [435.8, 1.2361],
    [475.3, 1.2349],
    [508.6, 1.2341],
    [546.1, 1.2334],
    [578, 1.2328],
    [643.9, 1.2321],
  ],
  Kr: [
    [363.1, 1.312],
    [406.3, 1.309],
    [435.8, 1.3074],
    [475.3, 1.3056],
    [508.6, 1.3042],
    [546.1, 1.3032],
    [578, 1.3024],
    [643.9, 1.3011],
  ],
  Xe: [
    [361.2, 1.4111],
    [365, 1.4103],
    [406.3, 1.4041],
    [435.8, 1.4001],
    [470, 1.3967],
    [480.6, 1.3957],
    [508.6, 1.3937],
    [546.1, 1.3918],
    [578, 1.39],
    [643.9, 1.3876],
  ],
} as const;

/** The rows' linear interpolation at a wavelength, nm, between the rows. */
function rowsAt(rows: ReadonlyArray<readonly [number, number]>, nm: number): number {
  const upper = rows.findIndex(([at]) => at >= nm);
  const hi = rows[upper];
  const lo = rows[upper - 1];
  if (hi === undefined || lo === undefined) {
    throw new Error(`no rows bracket ${nm} nm`);
  }
  return lo[1] + ((nm - lo[0]) / (hi[0] - lo[0])) * (hi[1] - lo[1]);
}

/** The factor c minimising the squared misses in n of a scaled gas row, by ternary search. */
function leastSquaresC(
  gas: Dispersion,
  molPerL: number,
  points: ReadonlyArray<readonly [nm: number, n: number]>,
): number {
  const misses = (c: number): number =>
    points.reduce((sum, [nm, n]) => sum + (scaledIndex(gas, nm, molPerL, c) - n) ** 2, 0);
  let low = 0.9;
  let high = 1.1;
  for (let step = 0; step < 200; step += 1) {
    const a = low + (high - low) / 3;
    const b = high - (high - low) / 3;
    if (misses(a) < misses(b)) {
      high = b;
    } else {
      low = a;
    }
  }
  return (low + high) / 2;
}

/** A T5.k liquid's scaled molar refraction, which each of its derived files must carry. */
function scaledFitOf(key: string): ScaledMolarRefractionFit {
  const fit = liquidOf(key).fit;
  if (fit?.form !== "scaledMolarRefraction") {
    throw new Error(`liquid ${key} carries no scaled molar refraction`);
  }
  return fit;
}

describe("the liquids from their gases and their measurements (R08.T5.k)", () => {
  it.each(T5K_LIQUIDS)(
    "file liquid %s as a sphere, %s, at %f K, not below its row's triple point, %f K",
    (key, provenance, temperatureK, tripleK) => {
      const file = liquidOf(key);
      expect([file.shape, file.provenance, file.temperatureK]).toEqual([
        "sphere",
        provenance,
        temperatureK,
      ]);
      expect(file.temperatureK).toBeGreaterThanOrEqual(tripleK);
    },
  );

  it.each(T5K_LIQUIDS)("resolve a liquid-phase %s mode to its own file", (key) => {
    expect(resolveMaterial(key, { phase: "liquid" }).file).toBe(liquidOf(key));
  });

  it.each(T5K_LIQUIDS)(
    "give liquid %s, %s, a scaled fit exactly when derived and a stand-in exactly when a stand-in",
    (key, provenance) => {
      // A.1 and A.4: the four anchored liquids carry the second form; the measured and the
      // stand-ins carry none, and a stand-in, and only one, states its stand-in.
      const file = liquidOf(key);
      expect([file.fit?.form, file.standIn !== undefined]).toEqual([
        provenance === "derived" ? "scaledMolarRefraction" : undefined,
        provenance === "standIn",
      ]);
    },
  );

  it("hold every key of R08.T5.k's list", () => {
    const keys = new Set(MATERIAL_FILES.map((f) => f.key));
    expect(T5K_KEYS.filter((key) => !keys.has(key))).toEqual([]);
  });

  it("give the eight colourless liquids no absorption", () => {
    const absorbing = T5K_LIQUIDS.filter(([key]) => key !== "O2").filter(([key]) =>
      liquidOf(key).k.some((k) => k !== 0),
    );
    expect(absorbing).toEqual([]);
  });

  it.each(["O2", "N2O", "SO2", "CO2"])(
    "rebuild %s's n at every grid point from its fit's density and factor and rayleigh.ts's row, to 10⁻⁵",
    (key) => {
      // decision-p14-t49h-gas-index A.1: the tool holds its own copies of the rows; this catches
      // drift. n is held, not the Lorentz–Lorenz function: at six figures, n ≈ 1.22's rounding
      // alone moves (n² − 1) ÷ (n² + 2) by up to 2.3 × 10⁻⁵ of itself.
      const file = liquidOf(key);
      const fit = scaledFitOf(key);
      const gas = gasRowOf(key);
      for (const [i, nm] of file.wavelengthsNm.entries()) {
        const n = file.n[i] ?? Number.NaN;
        const rebuilt = scaledIndex(gas, nm, fit.molarDensityMolPerL, fit.factor);
        expect(Math.abs(n - rebuilt)).toBeLessThan(1e-5);
      }
    },
  );

  it.each([
    // Ar's c is the least-squares fit to Sinnock and Smith's nine rows at the WebBook's 35.46464
    // mol/l at 83.81 K (decision-p14-t49h-gas-index A.2; the file's source); CO's and H2S's are 1,
    // at the WebBook's 30.32971 mol/l at 68.16 K and 29.11633 mol/l at 187.7 K (A.4).
    ["Ar", 35.464_64, 0.990_053],
    ["CO", 30.329_71, 1],
    ["H2S", 29.116_33, 1],
  ])(
    "give %s the scaled index of rayleigh.ts's row at %f mol/l and c = %f, to 10⁻⁵",
    (key, molPerL, factor) => {
      const file = liquidOf(key);
      const gas = gasRowOf(key);
      for (const [i, nm] of file.wavelengthsNm.entries()) {
        const n = file.n[i] ?? Number.NaN;
        expect(Math.abs(n - scaledIndex(gas, nm, molPerL, factor))).toBeLessThan(1e-5);
      }
      expect(file.source).toContain(`${molPerL} mol/l`);
    },
  );

  it.each([
    // The method's record rows of the two files whose n is their table, not the method's: the
    // WebBook's 29.18115 mol/l at 115.95 K and 22.59209 mol/l at 161.405 K, against Sinnock and
    // Smith's 1.3032 and 1.3918 at 546.1 nm. The tool holds its own copies of the gas rows; this
    // catches drift in the record the files state.
    ["Kr", 29.181_15, 1.3032, "liquid Kr at 115.95 K against Sinnock and Smith 1969"],
    [
      "Xe",
      22.592_09,
      1.3918,
      "liquid Xe at 161.35 K (the WebBook's 161.405 K) against Sinnock and Smith 1969",
    ],
  ])(
    "state liquid %s's record row from rayleigh.ts's row at the WebBook's %f mol/l",
    (key, molPerL, measured, what) => {
      const excess = scaledIndex(gasRowOf(key), 546.1, molPerL, 1) - measured;
      const stated = `${excess < 0 ? "−" : "+"}${Math.abs(excess).toFixed(4)}`;
      expect(liquidOf(key).source).toContain(`${what}, ${stated} at 546.1 nm`);
    },
  );

  it("meet Johns and Wilhelm 1937's 1.2242 at 546.1 nm for liquid O2, to 10⁻⁵", () => {
    // Can. J. Res. 15a, 101, their abstract: the normal boiling point, by a Wollaston cell.
    const { n } = refractiveIndex("O2", 546.1, { phase: "liquid" }).index;
    expect(Math.abs(n - 1.2242)).toBeLessThan(1e-5);
  });

  it("fit liquid N2O's factor by least squares to Liveing and Dewar 1892's five lines, to 10⁻⁵", () => {
    // Phil. Mag. (5) 34, 205, p. 209: the C, D, F and G lines and indium's 4509.6 Å, at the boiling
    // point; the lithium line is left out (its printed index contradicts its deviation).
    const lines = [
      [656.3, 1.329],
      [589.3, 1.3305],
      [486.1, 1.3345],
      [430.8, 1.3378],
      [450.96, 1.3368],
    ] as const;
    const fit = scaledFitOf("N2O");
    expect(fit.molarDensityMolPerL).toBe(27.957_49);
    const c = leastSquaresC(gasRowOf("N2O"), fit.molarDensityMolPerL, lines);
    expect(Math.abs(c - fit.factor)).toBeLessThan(1e-5);
  });

  it("set liquid SO2's factor to the mean of Ketteler's, Bichat's and Bleekrode's molar refractions over the gas's, to 10⁻⁵", () => {
    // Bleekrode 1884 (Proc. R. Soc. Lond. 37, 339, pp. 349–350): Ketteler's 1.3384 at 24 °C,
    // Bichat's 1.344 at 18.5 °C and his own 1.350 at 15 °C, at the sodium lines, each at the
    // WebBook's saturated-liquid density at its temperature, mol/l.
    const gas = gasRowOf("SO2");
    const measurements = [
      [1.3384, 21.379_59],
      [1.344, 21.619_56],
      [1.35, 21.770_4],
    ] as const;
    const ratios = measurements.map(
      ([n, molPerL]) => lorentz(n) / ((rowMolarRefractionCm3PerMol(gas, 589.3) * molPerL) / 1_000),
    );
    const mean = ratios.reduce((sum, r) => sum + r, 0) / ratios.length;
    expect(Math.abs(mean - scaledFitOf("SO2").factor)).toBeLessThan(1e-5);
  });

  it("set liquid CO2's factor by Phillips 1920's law at its density, 1 ÷ (1 + 0.0172ρ²), to 10⁻⁵", () => {
    // Proc. R. Soc. Lond. A 97, 225, p. 236, ρ in g cm⁻³: the WebBook's mol/l × 44.0095 ÷ 1,000.
    const fit = scaledFitOf("CO2");
    const density = (fit.molarDensityMolPerL * 44.0095) / 1_000;
    expect(Math.abs(1 / (1 + 0.0172 * density ** 2) - fit.factor)).toBeLessThan(1e-5);
    expect(fit.molarDensityMolPerL).toBe(26.776_63);
  });

  it.each([
    ["O2", 0.5461, 0.5461],
    ["N2O", 0.4308, 0.6563],
    ["SO2", 0.5893, 0.5893],
    ["CO2", 0.5461, 0.5461],
  ])("state %s's anchors as the measurements' %f–%f µm (A.1)", (key, from, to) => {
    const fit = scaledFitOf(key);
    expect([fit.anchoredFromUm, fit.anchoredToUm]).toEqual([from, to]);
  });

  it("meet each of Sinnock and Smith's liquid Ar rows from 380 nm within 2 × 10⁻⁴", () => {
    const rows = SINNOCK_LIQUID_ROWS.Ar.filter(([nm]) => nm >= 380);
    expect(rows).toHaveLength(7);
    for (const [nm, n] of rows) {
      const { index } = refractiveIndex("Ar", nm, { phase: "liquid" });
      expect(Math.abs(index.n - n)).toBeLessThan(2e-4);
    }
  });

  it.each([
    // The red tails, n = a + b ÷ λ², fitted to the rows from 435.8 nm (A.3), as each file's
    // reduction states them.
    ["Kr", 1.295_77, 0.002_208_01],
    ["Xe", 1.377_05, 0.004_346_56],
  ] as const)(
    "give liquid %s its rows' linear interpolation over 380–640 nm and its red tail from 645 nm, to 10⁻⁵",
    (key, a, b) => {
      const file = liquidOf(key);
      const rows = SINNOCK_LIQUID_ROWS[key];
      for (const [i, nm] of file.wavelengthsNm.entries()) {
        const expected = nm <= 640 ? rowsAt(rows, nm) : a + b / (nm / 1_000) ** 2;
        expect(Math.abs((file.n[i] ?? Number.NaN) - expected)).toBeLessThan(1e-5);
      }
      expect(file.reduction).toContain(`n = ${a} + ${b} µm² ÷ λ²`);
    },
  );

  it.each([
    ["Ar", 3e-4],
    ["Kr", 5e-4],
    ["Xe", 5e-4],
  ] as const)("keep %s's rows from 380 nm within its stated band, ±%f", (key, band) => {
    // decision-p14-t49h-gas-index §2.5; the rows at 361.2, 363.1 and 365.0 nm lie below the grid.
    for (const [nm, n] of SINNOCK_LIQUID_ROWS[key].filter(([at]) => at >= 380)) {
      const { index } = refractiveIndex(key, nm, { phase: "liquid" });
      expect(Math.abs(index.n - n)).toBeLessThanOrEqual(band);
    }
  });

  it.each([
    // Thalman and Volkamer 2013's O2–O2 σ at 203 K, their coldest, as HITRAN2024's O2-O2_2024.cia
    // gives it (linear in wavenumber between its points), cm⁵ a molecule pair: oxygen-liquid.json's
    // reduction states both (decision-p14-t49h-gas-index §2.6, path 2).
    [575, 1.191_78e-45],
    [630, 8.642_97e-46],
  ])(
    "give liquid O2 at %d nm the pairs' absorption at its density squared, σ %f cm⁵, as k = aλ ÷ 4π to the file's six figures",
    (nm, sigma) => {
      const oxygen = liquidOf("O2");
      const perCm3 = (scaledFitOf("O2").molarDensityMolPerL / 1_000) * 6.022_140_76e23;
      const expected = absorptionK(100 * sigma * perCm3 ** 2, nm);
      const k = valueAt(oxygen.k, oxygen, nm);
      expect(k).toBeGreaterThan(0);
      expect(Math.abs(k / expected - 1)).toBeLessThan(1e-5);
    },
  );

  it("give liquid O2 no k from 655 nm, past Thalman and Volkamer's sets, as its file says", () => {
    const oxygen = liquidOf("O2");
    expect(kFrom(oxygen, 655).filter((k) => k !== 0)).toEqual([]);
    expect(oxygen.reduction).toContain("k = 0 from 655 nm");
  });

  it("draw a deck of each of the seven measured or derived liquids at τ 0 with no label but liquid O2's", () => {
    // Liquid O2's colour raises its own reason, its file staying derived (signoff-12.md, 38(c)).
    const decks = T5K_LIQUIDS.filter(([, provenance]) => provenance !== "standIn").map(([key]) => ({
      material: key,
      form: { phase: "liquid" as const },
      opticalDepthAbove550: 0,
    }));
    expect(decks).toHaveLength(7);
    expect(materialApproximations(decks).map(({ material }) => material)).toEqual(["O2"]);
  });

  it.each(["CO", "H2S"])(
    "give atmosphereApproximate for a liquid-%s deck at τ 0, its file's stand-in after its phase and name the reason",
    (key) => {
      const layer = { material: key, form: { phase: "liquid" as const }, opticalDepthAbove550: 0 };
      const file = liquidOf(key);
      expect(materialLabels([layer])).toEqual(["atmosphereApproximate"]);
      expect(materialApproximations([layer])).toEqual([
        { material: key, reason: `liquid ${key}: ${file.standIn ?? ""}` },
      ]);
      expect(file.standIn).toMatch(
        /^a non-absorbing liquid whose real index is a Lorentz–Lorenz estimate from the gas's dispersion at the liquid's density, since its measured values[^;]*, contradict each other and lie /u,
      );
      expect(file.name).toMatch(/ \(an estimated index\)$/u);
    },
  );
});

describe("liquid methane's visible k, re-reduced (R08.T5.j, decision-p14-t49h-liquids §2.4)", () => {
  it("file liquid CH4 at 90 K as a derived sphere whose fit is a gas absorption over 380–555.6 nm", () => {
    const methane = liquidOf("CH4");
    expect([methane.shape, methane.provenance, methane.temperatureK]).toEqual([
      "sphere",
      "derived",
      90,
    ]);
    expect(methane.fit).toEqual(GAS_ABSORPTION_FIT);
    expect(methane.standIn).toBeUndefined();
  });

  it("keep liquid CH4's n, Martonchik and Orton's: 1.3 at 380 nm, 1.29345 at 550 nm, 1.28909 at 780 nm", () => {
    const methane = liquidOf("CH4");
    expect([380, 550, 780].map((nm) => valueAt(methane.n, methane, nm))).toEqual([
      1.3, 1.29345, 1.28909,
    ]);
  });

  it("give liquid CH4 its gas's 543 nm band in the blue, and k = 0 at 380–395 nm", () => {
    const methane = liquidOf("CH4");
    const blue = [380, 385, 390, 395].map((nm) => valueAt(methane.k, methane, nm));
    expect(blue).toEqual([0, 0, 0, 0]);
    const band = valueAt(methane.k, methane, 545);
    for (const nm of [440, 460, 485, 510, 530, 555]) {
      expect(valueAt(methane.k, methane, nm)).toBeLessThan(band);
    }
  });

  it("give liquid CH4's red bands over 10⁻⁸ and its trough at 765 nm at Fig. 3's axis", () => {
    const methane = liquidOf("CH4");
    expect(valueAt(methane.k, methane, 620)).toBeGreaterThan(1e-8);
    expect(valueAt(methane.k, methane, 730)).toBeGreaterThan(1e-8);
    expect(valueAt(methane.k, methane, 765)).toBeLessThan(2e-10);
    expect(valueAt(methane.k, methane, 640)).toBeLessThan(1e-9);
  });

  it("resolve liquid CH4 as its own derived file", () => {
    // Whether a liquid-CH4 deck raises ATMOSPHERE: APPROXIMATE, its blue k being its gas's carried
    // to the liquid (the guide's signoff-12 item 38(a)), is R08.T10.a's, routed with liquid O2's.
    const resolved = resolveMaterial("CH4", { phase: "liquid" });
    expect([resolved.file, resolved.provenance]).toEqual([liquidOf("CH4"), "derived"]);
  });
});
