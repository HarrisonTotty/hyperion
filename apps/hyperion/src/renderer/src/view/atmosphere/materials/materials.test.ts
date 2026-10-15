import { describe, expect, it } from "vitest";

import registry from "../../../../../../../../packages/protocol/fixtures/substances.json" with { type: "json" };
import { substanceName, UNLISTED_MATERIAL } from "../../../lib/substances";
import { aerosolModeOptics } from "../aerosol";
import { mieSphere } from "../mie";
import { type Dispersion, GAS_DISPERSION } from "../rayleigh";
import { CLOUD_DECK_SPLIT_OPTICAL_DEPTH } from "../thick/regime";
import {
  GENERIC_STAND_IN_INDEX,
  MATERIAL_FILES,
  MATERIAL_WAVELENGTHS_NM,
  materialApproximations,
  materialLabels,
  parseMaterialFile,
  refractiveIndex,
  resolveMaterial,
  variantWords,
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
  ])("refuse %s", (_, broken) => {
    expect(() => parseMaterialFile({ ...waterFile, ...broken }, "broken.json")).toThrow(
      /broken\.json/u,
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
    const misstated = MATERIAL_FILES.filter((file) => {
      const kelvin = /(?:at|near) ([0-9]+) K\b/u.exec(file.name)?.[1];
      return (
        file.name.includes("°C") || (kelvin !== undefined && Number(kelvin) !== file.temperatureK)
      );
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
  if (fit === undefined) {
    throw new Error("CO₂ ice carries no fit");
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
      "a non-absorbing particle whose real index is a Lorentz–Lorenz estimate, since no visible optical constants of NH4SH are published: 1.674 at 380 nm, 1.638 at 550 nm and 1.623 at 780 nm, within +0.081 and -0.022",
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

  it("gives no label beneath a τ 30 deck", () => {
    expect(
      materialLabels([
        { material: "NH3", form: { phase: "solid" }, opticalDepthAbove550: 0 },
        { material: "NH4SH", opticalDepthAbove550: 30 },
      ]),
    ).toEqual([]);
  });
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

describe("the registry's aerosol materials (R08.T5.d)", () => {
  it("serve every aerosol-material row of the registry's fixture with a file of their own", () => {
    // A row the generic stand-in alone serves fails: its key would have no file.
    const rows = registry.substances.filter((row) => row.aerosol_material);
    const unserved = rows.filter((row) => resolveMaterial(row.key).file === undefined);
    expect(unserved.map((row) => row.key)).toEqual([]);
  });

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
      "an empirical absorber: H2SO4 droplets with an unidentified ultraviolet absorber, whose k is fitted to Venus's albedo spectrum (candidates FeCl3, S2O/OSSO, Sx, organics)",
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
      if (fit === undefined) {
        throw new Error(`liquid ${key} carries no fit`);
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
