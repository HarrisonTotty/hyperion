import { describe, expect, it } from "vitest";

import { mieSphere } from "../mie";
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
  it("is a non-absorbing particle of real index 1.80 over 380–780 nm", () => {
    for (const nm of [380, 550, 780]) {
      expect(refractiveIndex("NH4SH", nm)).toEqual({
        index: { n: 1.8, k: 0 },
        provenance: "standIn",
      });
    }
    expect(resolveMaterial("NH4SH").standIn).toMatch(/1\.80/u);
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
