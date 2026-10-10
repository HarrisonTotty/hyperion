import { describe, expect, it } from "vitest";

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

  it("cover 380–780 nm every 5 nm", () => {
    expect(MATERIAL_WAVELENGTHS_NM[0]).toBe(380);
    expect(MATERIAL_WAVELENGTHS_NM.at(-1)).toBe(780);
    for (const file of MATERIAL_FILES) {
      expect(file.n).toHaveLength(MATERIAL_WAVELENGTHS_NM.length);
      expect(file.k).toHaveLength(MATERIAL_WAVELENGTHS_NM.length);
    }
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
    for (const file of MATERIAL_FILES.filter((f) => f.provenance === "measured")) {
      expect(file.source).toMatch(/SHA-256 [0-9a-f]{64}/u);
      expect(file.source).toMatch(/fetched 2026-10-09/u);
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

  it.each([
    [
      "a grid other than 380–780 nm every 5 nm",
      { wavelengthsNm: waterFile.wavelengthsNm.slice(1) },
    ],
    ["n and k of other lengths", { n: waterFile.n.slice(1) }],
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
    const i550 = MATERIAL_WAVELENGTHS_NM.indexOf(550);
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
    expect(liquidIron.standIn).toMatch(/analogue/u);
    const strongAcid = resolveMaterial("H2SO4", { variant: "96wt%" });
    expect(strongAcid.provenance).toBe("standIn");
    expect(strongAcid.file?.variant).toBe("75wt%");
  });

  it("gives an unknown key the generic stand-in, a non-absorbing sphere of index 1.5", () => {
    expect(refractiveIndex("Xx9", 550)).toEqual({
      index: GENERIC_STAND_IN_INDEX,
      provenance: "standIn",
    });
    expect(GENERIC_STAND_IN_INDEX).toEqual({ n: 1.5, k: 0 });
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
    for (const [i, nm] of MATERIAL_WAVELENGTHS_NM.entries()) {
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

  it("has an estimated n of 1.644 ± 0.001 at 550 nm", () => {
    // The Lorentz–Lorenz estimate of science-r08-nonspherical.md §3.1, replacing the 1.80 stand-in,
    // with NH₃ at its theoretic density and NH₄Cl's increment re-derived (R08's Risks, "Deviations in
    // T3.b's follow-up, as built"); the ruling's 1.648 took NH₃'s real-gas Z.
    expect(Math.abs(refractiveIndex("NH4SH", 550).index.n - 1.644)).toBeLessThanOrEqual(0.001);
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
    expect(resolveMaterial("NH4SH").standIn).toMatch(/Lorentz–Lorenz/u);
  });

  it("takes rayleigh.ts's NH₃ and H₂S rows, stated at 0 °C and 760 mm, as its estimate does", () => {
    for (const gas of NH4SH_GASES) {
      expect([gas.referenceK, gas.referencePa]).toEqual([273.15, 101_325]);
    }
  });

  it("is the Lorentz–Lorenz index of rayleigh.ts's NH₃ and H₂S rows at West 1934's cell", () => {
    // science-r08-nonspherical.md §3.1: (n² − 1) ÷ (n² + 2) = 1.035 R ÷ V_m, with R the gases'
    // summed molar refraction as stated at 0 °C and 760 mm, each at its formula's Z, 1.035 NH₄Cl's
    // increment (the tool's NH4SH_IONIC_INCREMENT) and V_m from West's cell (COD 1010249:
    // a = 6.011 Å, c = 4.009 Å, two formula units). The tool holds its own copy of the rows; this
    // catches drift.
    const idealMolarVolumeCm3PerMol = 22_413.969_54;
    const cellMolarVolumeCm3PerMol = (6.022_140_76e23 * 6.011 ** 2 * 4.009 * 1e-24) / 2;
    for (const [i, nm] of MATERIAL_WAVELENGTHS_NM.entries()) {
      const molarRefraction = NH4SH_GASES.reduce(
        (sum, gas) =>
          sum + lorentz(1 + gas.nMinusOne(nm)) * gas.compressibility * idealMolarVolumeCm3PerMol,
        0,
      );
      const expected = (1.035 * molarRefraction) / cellMolarVolumeCm3PerMol;
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
