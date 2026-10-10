import { describe, expect, it } from "vitest";

import { henyeyGreenstein } from "./aerosol";
import enstatitePhase from "./fixtures/enstatite-sample-phase.json" with { type: "json" };
import ga from "./fixtures/granada-amsterdam.json" with { type: "json" };
import mgsPhase from "./fixtures/mgs1m-phase.json" with { type: "json" };
import excerpt from "./fixtures/tamudust-excerpt.json" with { type: "json" };
import { MATERIAL_FILES, refractiveIndex } from "./materials/materials";
import { mieSphere } from "./mie";
import {
  FLOOR_ALBEDO_MIN,
  forwardPeakSplit,
  type NonSphericalParticles,
  nonSphericalModeOptics,
  opticsOfParticles,
  PHASE_FILES,
  parsePhaseFile,
  type PhaseFileTable,
  phaseFileOf,
  SMALL_GRAIN_SIZE_PARAMETER,
  tableModeOptics,
  tabulatedRadiusDistribution,
  waterIceTable,
} from "./nonSpherical";
import { sphereModeOptics } from "./sizeDistribution";

const cosine = (deg: number): number => Math.cos((deg * Math.PI) / 180);

/** A value computed on its first use, so that a describe block's setup runs inside its tests. */
function once<T>(make: () => T): () => T {
  let value: { readonly made: T } | undefined;
  return () => {
    value ??= { made: make() };
    return value.made;
  };
}

/** The angles of the measured ratios (science-r08-nonspherical.md §2.1–§2.2). */
const ANGLES = [30, 90, 120, 150, 165, 170] as const;
const MU = Float64Array.from(ANGLES, cosine);
const at = (deg: (typeof ANGLES)[number]): number => ANGLES.indexOf(deg);

/** A laser sizer's measured number distribution, n(r) per µm at each r. */
interface MeasuredSizes {
  readonly radiiUm: ReadonlyArray<number>;
  readonly numberDensity: ReadonlyArray<number>;
}

/**
 * A measured distribution as one in the tables' volume-to-area radius r_VA = 3V ÷ 4A, from the
 * sizer's projected-area-equivalent radius r_A (MGS-1 M, Martikainen et al. 2025 §2.2) or its
 * volume-equivalent radius r_V (enstatite, Frattin et al. 2019 §4.3.2): r_VA ÷ r_A is the table's
 * own (3Ṽ ÷ 4Ã) ÷ √(Ã ÷ π), 0.599 at sphericity 0.71, and r_VA ÷ r_V its (3Ṽ ÷ 4Ã) ÷ ∛(3Ṽ ÷ 4π),
 * 0.71, each the same at every size of the ensemble. With a wavelength, the points past the table's
 * largest size there are dropped.
 */
function inVolumeToArea(
  table: PhaseFileTable,
  sample: MeasuredSizes,
  radius: "projectedArea" | "volume" = "projectedArea",
  wavelengthNm?: number,
): ReturnType<typeof tabulatedRadiusDistribution> {
  const node = table.nodes[0];
  const v = node?.volumes[20] ?? Number.NaN;
  const a = node?.areas[20] ?? Number.NaN;
  const ratio =
    (0.75 * v) /
    a /
    (radius === "volume" ? Math.cbrt((3 * v) / (4 * Math.PI)) : Math.sqrt(a / Math.PI));
  const last = (node?.sizeParameters.length ?? 0) - 1;
  const topUm =
    wavelengthNm === undefined
      ? Infinity
      : (((0.75 * (node?.volumes[last] ?? Number.NaN)) / (node?.areas[last] ?? Number.NaN)) *
          wavelengthNm) /
        1000 /
        (2 * Math.PI);
  const kept = sample.radiiUm
    .map((r, i) => [r * ratio, (sample.numberDensity[i] ?? Number.NaN) / ratio] as const)
    .filter(([r]) => r <= topUm);
  return tabulatedRadiusDistribution(
    kept.map(([r]) => r),
    kept.map(([, n]) => n),
  );
}

/** The MgSiO₃ phase file, on the shortwave kernel's floor. */
function enstatiteTable(): PhaseFileTable {
  const table = PHASE_FILES.find((p) => p.key === "MgSiO3");
  if (table === undefined) {
    throw new Error("the MgSiO3 phase file is missing");
  }
  return table;
}

/** The ratios a₁(θ) ÷ a₁(30°) of a matrix at {@link ANGLES}. */
function normalisedAt30(a1: Float64Array): number[] {
  const at30 = a1[at(30)] ?? Number.NaN;
  return Array.from(a1, (v) => v / at30);
}

/**
 * a₁ ÷ a₁(30°) at {@link ANGLES} of spheres of radius r over a measured distribution, by Mie, 40
 * steps per interval in ln r.
 */
function sphereRatiosAt30(
  sample: MeasuredSizes,
  wavelengthNm: number,
  index: { readonly n: number; readonly k: number },
): number[] {
  const sums = new Float64Array(ANGLES.length);
  const { radiiUm, numberDensity } = sample;
  for (let i = 0; i + 1 < radiiUm.length; i += 1) {
    const r0 = radiiUm[i] ?? Number.NaN;
    const r1 = radiiUm[i + 1] ?? Number.NaN;
    const n0 = numberDensity[i] ?? Number.NaN;
    const n1 = numberDensity[i + 1] ?? Number.NaN;
    for (let s = 0; s < 40; s += 1) {
      const t = (s + 0.5) / 40;
      const r = r0 * (r1 / r0) ** t;
      const n = n0 * (n1 / n0) ** t;
      const x = (2 * Math.PI * r * 1000) / wavelengthNm;
      const mie = mieSphere(x, index, MU);
      const weight = (r * n * Math.log(r1 / r0) * r * r) / (40 * x * x);
      for (let j = 0; j < ANGLES.length; j += 1) {
        sums[j] =
          (sums[j] ?? 0) +
          weight *
            ((mie.s1[2 * j] ?? 0) ** 2 +
              (mie.s1[2 * j + 1] ?? 0) ** 2 +
              (mie.s2[2 * j] ?? 0) ** 2 +
              (mie.s2[2 * j + 1] ?? 0) ** 2);
      }
    }
  }
  return normalisedAt30(sums);
}

describe("the phase files", () => {
  it("parse, each with its model, source, licence and nodes", () => {
    expect(PHASE_FILES.length).toBeGreaterThanOrEqual(4);
    for (const file of PHASE_FILES) {
      expect(file.licence).toContain("CC BY 4.0");
      expect(file.source).toMatch(/MD5 [0-9a-f]{32}/u);
      expect(file.nodes.length).toBeGreaterThan(0);
    }
  });

  it("carry TAMUdust2020's required sentence in its tables", () => {
    for (const file of PHASE_FILES.filter((p) => p.model === "tamudust2020")) {
      expect(file.licence).toContain("The scattering properties are obtained from TAMUdust2020.");
    }
  });

  it("belong to material files, each covering 380–780 nm", () => {
    for (const file of PHASE_FILES) {
      const material = MATERIAL_FILES.find(
        (m) => m.key === file.key && m.phase === file.phase && m.variant === file.variant,
      );
      expect(material === undefined ? undefined : phaseFileOf(material)).toBe(file);
      expect(file.nodes[0]?.wavelengthNm).toBeLessThanOrEqual(380);
      expect(file.nodes.at(-1)?.wavelengthNm).toBeGreaterThanOrEqual(780);
    }
  });

  it("give Mars dust, the silicates and solid iron the hexahedra", () => {
    for (const key of ["mars_dust", "MgSiO3", "Mg2SiO4", "Fe"]) {
      expect(PHASE_FILES.find((p) => p.key === key)?.model).toBe("tamudust2020");
    }
  });

  it("refuse a broken file by name", () => {
    expect(() => parsePhaseFile({ model: "tamudust2020" }, "bad.json")).toThrow(/bad\.json/u);
  });
});

describe("MGS-1 M against Martikainen et al. (science-r08-nonspherical.md §2.1)", () => {
  const table = once(() => parsePhaseFile(mgsPhase, "fixtures/mgs1m-phase.json"));
  const distribution = once(() => inVolumeToArea(table(), ga.mgs1m));

  it("gives Martikainen et al. 2025's g 0.7111 ± 0.005 and ω 0.9905 ± 0.002 at 650 nm", () => {
    const optics = tableModeOptics(table(), distribution(), { n: 1.5, k: 4.33e-4 }, 650, MU);
    expect(Math.abs(optics.asymmetry - 0.7111)).toBeLessThan(0.005);
    const omega = optics.scatteringCrossSectionM2 / optics.extinctionCrossSectionM2;
    expect(Math.abs(omega - 0.9905)).toBeLessThan(0.002);
  });

  // The measured matrix at 640 nm (Martikainen et al. 2024, ApJS 273, 28; the database's
  // matrix_MGS1M_640nm.txt), its k from opticalPropertiesMGS1M_1.txt's 600 and 650 nm rows.
  const matrix = once(
    () => tableModeOptics(table(), distribution(), { n: 1.5, k: 4.29e-4 }, 640, MU).matrix,
  );

  it("puts a₁ ÷ a₁(30°) within 40% of the measured 0.141, 0.105, 0.099 and 0.119", () => {
    const ratios = normalisedAt30(matrix().a1);
    for (const [deg, value] of [
      [90, 0.141],
      [120, 0.105],
      [150, 0.099],
      [170, 0.119],
    ] as const) {
      expect(Math.abs((ratios[at(deg)] ?? Number.NaN) / value - 1)).toBeLessThan(0.4);
    }
  });

  it("polarises with the measured signs: −b₁ ÷ a₁ positive at 90° and negative at 165°", () => {
    const measured = matrix();
    expect(-(measured.b1[at(90)] ?? 0) / (measured.a1[at(90)] ?? 1)).toBeGreaterThan(0);
    expect(-(measured.b1[at(165)] ?? 0) / (measured.a1[at(165)] ?? 1)).toBeLessThan(0);
  });

  it("puts a₂ ÷ a₁ at 170° within 0.15 of the measured 0.54", () => {
    const measured = matrix();
    expect(Math.abs((measured.a2[at(170)] ?? 0) / (measured.a1[at(170)] ?? 1) - 0.54)).toBeLessThan(
      0.15,
    );
  });

  it("gives b₂ ÷ a₁ at 90° the sign of the measured F₃₄ ÷ F₁₁, +0.137", () => {
    const measured = matrix();
    expect((measured.b2[at(90)] ?? 0) / (measured.a1[at(90)] ?? 1)).toBeGreaterThan(0);
  });

  it("fails it with spheres of the same distribution", () => {
    const spheres = sphereRatiosAt30(ga.mgs1m, 640, { n: 1.5, k: 4.29e-4 });
    const measuredRatios = [
      [90, 0.141],
      [120, 0.105],
      [150, 0.099],
      [170, 0.119],
    ] as const;
    const misses = measuredRatios.filter(
      ([deg, value]) => Math.abs((spheres[at(deg)] ?? 0) / value - 1) >= 0.4,
    );
    expect(misses.length).toBeGreaterThan(0);
  });
});

describe("Mars dust (science-r08-nonspherical.md §2.1 (iii))", () => {
  // The test's r_eff 1.5 µm is in Wolff et al. 2009's convention, the surface-equivalent radius of
  // D/L = 1 cylinders (their ¶[26], ¶[32]); in the modes' volume-to-area radius it is
  // 1.5 × (2 ÷ 3)^½ = 1.2247 µm, the same v_eff (science-r08-mmf.md §2.3).
  const mode = {
    material: "mars_dust",
    sizes: { kind: "gamma", effectiveRadiusUm: 1.2247, effectiveVariance: 0.3 },
  } as const;
  const optics = once(() =>
    nonSphericalModeOptics(mode, 650, Float64Array.from([90, 150], cosine)),
  );

  it("is drawn on the hexahedra as a model", () => {
    expect(optics().phaseBasis).toBe("model");
    expect(optics().phaseModel).toBe("tamudust2020");
  });

  it("gives g within Chen-Chen et al. 2019's 0.673 ± 0.081 at 650 nm", () => {
    expect(Math.abs(optics().asymmetry - 0.673)).toBeLessThan(0.081);
  });

  it("gives a₁(150°) ÷ a₁(90°) within 0.5–0.85 (MSL's sky, 0.71)", () => {
    const ratio = (optics().matrix?.a1[1] ?? 0) / (optics().matrix?.a1[0] ?? 1);
    expect(ratio).toBeGreaterThan(0.5);
    expect(ratio).toBeLessThan(0.85);
  });

  it("gives ω within Wolff et al. 2009's 0.975 ± 0.015", () => {
    expect(Math.abs(optics().singleScatteringAlbedo - 0.975)).toBeLessThan(0.015);
  });
});

describe("enstatite against Frattin et al. 2019 (science-r08-nonspherical.md §2.2, §7)", () => {
  // The sample's own iron, Mg₀.₈₅Fe₀.₀₈Si₀.₉₉O₃: Dorschner et al. 1995's pyroxene glasses give
  // 1.594 + 4.5 × 10⁻⁴i at 520 nm, the fixture's index (test-only; the MgSiO₃ file stays iron-free).
  // Its sizes are the MIE section, read as volume-equivalent radii (Frattin et al. §4.3.2).
  const index = { n: 1.594, k: 4.5e-4 } as const;
  const table = once(() => parsePhaseFile(enstatitePhase, "fixtures/enstatite-sample-phase.json"));
  const optics = once(() =>
    tableModeOptics(table(), inVolumeToArea(table(), ga.enstatite, "volume", 520), index, 520, MU),
  );
  const measured = [
    [90, 0.149],
    [120, 0.11],
    [150, 0.102],
    [170, 0.115],
  ] as const;
  /** The measured angles at which ratios to 30° part from the measured by a factor of 2 or more. */
  const missesByFactor2 = (ratios: ReadonlyArray<number>): number[] =>
    measured
      .filter(([deg, value]) => {
        const r = (ratios[at(deg)] ?? Number.NaN) / value;
        return !(r > 0.5 && r < 2);
      })
      .map(([deg]) => deg);

  it("is drawn on the hexahedra at its own index, above the kernel's floor", () => {
    expect(optics().floored).toBe(false);
  });

  it("puts a₁(170°) ÷ a₁(90°) within 0.5–1.2 (measured 0.77)", () => {
    const { a1 } = optics().matrix;
    const ratio = (a1[at(170)] ?? Number.NaN) / (a1[at(90)] ?? Number.NaN);
    expect(ratio).toBeGreaterThan(0.5);
    expect(ratio).toBeLessThan(1.2);
  });

  it("polarises positively at 90° (measured +0.108)", () => {
    const { matrix } = optics();
    expect(-(matrix.b1[at(90)] ?? 0) / (matrix.a1[at(90)] ?? 1)).toBeGreaterThan(0);
  });

  it("puts a₁ ÷ a₁(30°) within a factor of 2 of the measured 0.149, 0.110, 0.102 and 0.115", () => {
    expect(missesByFactor2(normalisedAt30(optics().matrix.a1))).toEqual([]);
  });

  it("is a bound that volume-equivalent spheres of the distribution fail", () => {
    expect(missesByFactor2(sphereRatiosAt30(ga.enstatite, 520, index)).length).toBeGreaterThan(0);
  });

  it("is a bound that Henyey–Greenstein at the mode's g fails", () => {
    const g = optics().asymmetry;
    const hg = normalisedAt30(Float64Array.from(MU, (mu) => henyeyGreenstein(g, mu)));
    expect(missesByFactor2(hg).length).toBeGreaterThan(0);
  });
});

/** The excerpt's a₁ at one record, decoded (round(1000 ln a₁), size-major). */
function excerptA1(record: 0 | 1): Float64Array {
  const rec = excerpt.records[record];
  if (rec === undefined) {
    throw new Error(`the excerpt has no record ${record}`);
  }
  const bytes = atob(rec.a1);
  return Float64Array.from({ length: bytes.length / 2 }, (_, i) => {
    const v = bytes.charCodeAt(2 * i) | (bytes.charCodeAt(2 * i + 1) << 8);
    return Math.exp((v >= 0x8000 ? v - 0x10000 : v) / 1000);
  });
}

describe("the rules, on the raw excerpt (n 1.60, sphericity 0.712)", () => {
  const angles = excerpt.phaseAnglesDeg.length;

  it("holds the k-floor rule: a₁ at k 10⁻³ and 10⁻⁴ within 2% wherever ω at 10⁻³ ≥ 0.99", () => {
    const low = excerptA1(0);
    const high = excerptA1(1);
    expect(excerpt.records[0]?.k).toBeCloseTo(1e-4, 9);
    expect(excerpt.records[1]?.k).toBeCloseTo(1e-3, 9);
    let checked = 0;
    for (const [s, row] of (excerpt.records[1]?.isca ?? []).entries()) {
      const omega = (row[4] ?? 0) / (row[3] ?? 1);
      if (omega < FLOOR_ALBEDO_MIN) {
        continue;
      }
      checked += 1;
      for (let j = 0; j < angles; j += 1) {
        const ratio = (high[s * angles + j] ?? 0) / (low[s * angles + j] ?? 1);
        expect(Math.abs(ratio - 1)).toBeLessThan(0.02);
      }
    }
    expect(checked).toBeGreaterThan(50);
  });

  it("starts the small-grain rule where the hexahedra and spheres part by 5% in a₁", () => {
    const a1 = excerptA1(0);
    const mu = Float64Array.from(excerpt.phaseAnglesDeg, cosine);
    const worstAt = (row: ReadonlyArray<number>, s: number): { x: number; worst: number } => {
      const xVa = (0.75 * (row[2] ?? 0)) / (row[1] ?? 1);
      const mie = mieSphere(xVa, { n: 1.6, k: 1e-4 }, mu);
      let worst = 0;
      for (let j = 0; j < angles; j += 1) {
        const sq =
          (mie.s1[2 * j] ?? 0) ** 2 +
          (mie.s1[2 * j + 1] ?? 0) ** 2 +
          (mie.s2[2 * j] ?? 0) ** 2 +
          (mie.s2[2 * j + 1] ?? 0) ** 2;
        const sphere = (2 * sq) / (xVa * xVa * mie.qSca);
        worst = Math.max(worst, Math.abs((a1[s * angles + j] ?? 0) / sphere - 1));
      }
      return { x: xVa, worst };
    };
    const rows = excerpt.records[0]?.isca ?? [];
    const results = rows.slice(0, 60).map((row, s) => worstAt(row, s));
    const below = results.filter((r) => r.x < SMALL_GRAIN_SIZE_PARAMETER);
    const above = results.filter((r) => r.x >= SMALL_GRAIN_SIZE_PARAMETER);
    expect(below.every((r) => r.worst < 0.05)).toBe(true);
    // The next node past the constant parts by 4.9% and the one after by 5.3%: the constant is the
    // measured crossing, per particle (x_VA), applied to a mode's 2π r_eff ÷ λ.
    expect(above[0]?.worst ?? 0).toBeGreaterThan(0.048);
    expect(above[1]?.worst ?? 0).toBeGreaterThan(0.05);
  });
});

describe("the fallback", () => {
  /** Rutile's ordinary index by DeVore's Sellmeier fit (J. Opt. Soc. Am. 41, 416, 1951), k = 0. */
  const rutile: NonSphericalParticles = {
    material: "TiO2",
    shape: "nonSphericalMineral",
    index: (nm) => ({ n: Math.sqrt(5.913 + 0.2441 / ((nm / 1000) ** 2 - 0.0803)), k: 0 }),
    provenance: "measured",
    table: undefined,
  };

  it("draws a 3 µm TiO₂ mode, outside both kernels, with Henyey–Greenstein and no matrix", () => {
    const optics = opticsOfParticles(
      rutile,
      { kind: "gamma", effectiveRadiusUm: 3, effectiveVariance: 0.1 },
      550,
    );
    expect(optics.phaseBasis).toBe("fallback");
    expect(optics.phaseModel).toBe("henyeyGreenstein");
    expect(optics.matrix).toBeUndefined();
    expect(optics.phaseNote).toContain("below the longwave kernel's k floor");
  });

  it("draws a small TiO₂ grain with the sphere's own Mie matrix, still a fallback", () => {
    const radiusUm = (0.5 * SMALL_GRAIN_SIZE_PARAMETER * 0.55) / (2 * Math.PI);
    const optics = opticsOfParticles(
      rutile,
      { kind: "gamma", effectiveRadiusUm: radiusUm, effectiveVariance: 0.1 },
      550,
    );
    expect(optics.phaseBasis).toBe("fallback");
    expect(optics.phaseModel).toBe("mie");
    expect(optics.matrix).toBeDefined();
  });
});

describe("solid and liquid iron", () => {
  it("draws a solid iron grain on the longwave kernel, with a₂ ÷ a₁ below 0.95 at 170°", () => {
    const optics = nonSphericalModeOptics(
      { material: "Fe", sizes: { kind: "gamma", effectiveRadiusUm: 0.5, effectiveVariance: 0.1 } },
      550,
      Float64Array.from([cosine(170)]),
    );
    expect(optics.phaseModel).toBe("tamudust2020");
    expect((optics.matrix?.a2[0] ?? 1) / (optics.matrix?.a1[0] ?? 1)).toBeLessThan(0.95);
    expect(PHASE_FILES.find((p) => p.key === "Fe")?.nodes[0]?.kernel).toBe("longwave");
  });
});

describe("water ice on Yang et al. 2013's roughened 8-column aggregate (§2.3)", () => {
  /** Gamma r_eff 30 µm, v_eff 0.1, in Yang et al.'s own volume-to-area convention. */
  const cirrus = {
    material: "H2O",
    form: { phase: "solid" },
    sizes: { kind: "gamma", effectiveRadiusUm: 30, effectiveVariance: 0.1 },
  } as const;

  it("draws water ice as the model", () => {
    const optics = nonSphericalModeOptics(cirrus, 550);
    expect(optics.phaseBasis).toBe("model");
    expect(optics.phaseModel).toBe("yang2013");
    expect(optics.matrix).toBeDefined();
  });

  it("gives Järvinen et al. 2018's g 0.750 ± 0.010 at 532 nm and 0.754 ± 0.010 at 780 nm", () => {
    expect(Math.abs(nonSphericalModeOptics(cirrus, 532).asymmetry - 0.75)).toBeLessThan(0.01);
    expect(Math.abs(nonSphericalModeOptics(cirrus, 780).asymmetry - 0.754)).toBeLessThan(0.01);
  });

  it("has no local maximum of a₁ between 15° and 50°: no 22° or 46° halo", () => {
    const degrees = Array.from({ length: 36 }, (_, i) => 15 + i);
    const optics = nonSphericalModeOptics(cirrus, 550, Float64Array.from(degrees, cosine));
    const a1 = optics.matrix?.a1 ?? new Float64Array(0);
    expect(a1.length).toBe(degrees.length);
    const maxima = degrees.filter(
      (_, i) =>
        i > 0 &&
        i + 1 < degrees.length &&
        (a1[i] ?? 0) > (a1[i - 1] ?? 0) &&
        (a1[i] ?? 0) > (a1[i + 1] ?? 0),
    );
    expect(maxima).toEqual([]);
  });

  it("is an analogue when crystals below the table's 2 µm carry over 10% at 550 nm", () => {
    // The 8-column aggregate of D = 2 µm has r = 3V ÷ 4A of 0.30 µm, so the tail is r < 0.30 µm.
    const small = nonSphericalModeOptics(
      { ...cirrus, sizes: { kind: "gamma", effectiveRadiusUm: 0.25, effectiveVariance: 0.1 } },
      550,
    );
    expect(small.phaseBasis).toBe("analogue");
    expect(small.phaseNote).toContain("2 µm");
  });
});

describe("the other ices, on water ice's table as named analogues", () => {
  const sizes = { kind: "gamma", effectiveRadiusUm: 10, effectiveVariance: 0.1 } as const;

  it("draws NH₃ ice as an analogue with its own index's Mie cross-sections and ω", () => {
    const optics = nonSphericalModeOptics({ material: "NH3", sizes }, 550);
    expect(optics.phaseBasis).toBe("analogue");
    expect(optics.phaseModel).toBe("yang2013");
    expect(optics.phaseNote).toContain("-0.056");
    const water = nonSphericalModeOptics({ material: "H2O", form: { phase: "solid" }, sizes }, 550);
    expect(optics.singleScatteringAlbedo).toBeLessThan(water.singleScatteringAlbedo);
  });

  it("draws CH₄ and CO₂ ices as analogues too", () => {
    for (const material of ["CH4", "CO2"]) {
      const optics = nonSphericalModeOptics({ material, form: { phase: "solid" }, sizes }, 550);
      expect(optics.phaseBasis).toBe("analogue");
    }
  });
});

describe("the floor rule's two exits", () => {
  // Coarse MgSiO₃ grains on the shortwave floor (k 10⁻⁴) absorb along their paths, so the mode's
  // ω on the floor's table falls below 0.99.
  const coarse = { kind: "gamma", effectiveRadiusUm: 8, effectiveVariance: 0.1 } as const;

  it("makes a coarse mode past the floor rule an analogue on the floor's table", () => {
    const particles: NonSphericalParticles = {
      material: "MgSiO3",
      shape: "nonSphericalMineral",
      index: (nm) => refractiveIndex("MgSiO3", nm).index,
      provenance: "measured",
      table: enstatiteTable(),
    };
    const optics = opticsOfParticles(particles, coarse, 550);
    expect(optics.phaseBasis).toBe("analogue");
    expect(optics.phaseModel).toBe("tamudust2020");
    expect(optics.phaseNote).toContain("k-floor rule exceeded");
  });

  it("makes it a fallback instead above n 1.70", () => {
    const particles: NonSphericalParticles = {
      material: "a transparent high-index mineral",
      shape: "nonSphericalMineral",
      index: () => ({ n: 1.8, k: 2e-5 }),
      provenance: "measured",
      table: enstatiteTable(),
    };
    const optics = opticsOfParticles(particles, coarse, 550);
    expect(optics.phaseBasis).toBe("fallback");
    expect(optics.matrix).toBeUndefined();
  });
});

describe("the particles below a table's smallest size", () => {
  it("keep their own absorption, by Mie, unscaled by the table's k", () => {
    // Ice of r_eff 0.03 µm lies wholly below the 8-column table's r = 0.30 µm: it is the
    // volume-to-area spheres' Mie, whatever k ÷ k_node the table's absorption takes at 650 nm.
    const sizes = { kind: "gamma", effectiveRadiusUm: 0.03, effectiveVariance: 0.1 } as const;
    const ice = nonSphericalModeOptics({ material: "H2O", form: { phase: "solid" }, sizes }, 650);
    const index = refractiveIndex("H2O", 650, { phase: "solid" }).index;
    const spheres = sphereModeOptics(sizes, index, 650);
    const ratio = (1 - ice.singleScatteringAlbedo) / (1 - spheres.singleScatteringAlbedo);
    expect(Math.abs(ratio - 1)).toBeLessThan(0.02);
  });

  it("keep each ice crystal's area and volume, as Grenfell and Warren 1999's spheres", () => {
    // Each crystal below the table is Ã ÷ πr̃² spheres of its r_VA, at the table's smallest crystal.
    const sizes = { kind: "gamma", effectiveRadiusUm: 0.03, effectiveVariance: 0.1 } as const;
    const ice = nonSphericalModeOptics({ material: "H2O", form: { phase: "solid" }, sizes }, 650);
    const index = refractiveIndex("H2O", 650, { phase: "solid" }).index;
    const spheres = sphereModeOptics(sizes, index, 650);
    const node = waterIceTable()?.nodes[0];
    const area = node?.areas[0] ?? Number.NaN;
    const r = (0.75 * (node?.volumes[0] ?? Number.NaN)) / area;
    const copies = area / (Math.PI * r * r);
    expect(copies).toBeGreaterThan(4);
    for (const [crystals, sphere] of [
      [ice.geometricCrossSectionM2, spheres.geometricCrossSectionM2],
      [ice.volumeM3, spheres.volumeM3],
      [ice.scatteringCrossSectionM2, spheres.scatteringCrossSectionM2],
    ] as const) {
      expect(Math.abs(crystals / sphere / copies - 1)).toBeLessThan(1e-3);
    }
  });

  it("make a water-ice mode of r_eff 0.5 µm, v_eff 0.3, an analogue by its tail", () => {
    // Tail crystals keep their area, so they carry 14% of the scattering at 550 nm.
    const sizes = { kind: "gamma", effectiveRadiusUm: 0.5, effectiveVariance: 0.3 } as const;
    const ice = nonSphericalModeOptics({ material: "H2O", form: { phase: "solid" }, sizes }, 550);
    expect(ice.phaseBasis).toBe("analogue");
  });
});

describe("the forward peak the files' angles miss", () => {
  it("is sized so that the whole integrates to 1 with the source's g", () => {
    // A node of x_D 11,810: the interpolant holds 0.591 of the scattering, of mean cosine 0.772.
    const split = forwardPeakSplit(0.591, 0.591 * 0.772, 0.866);
    expect(split.scale * 0.591 + split.forwardPeak).toBeCloseTo(1, 12);
    expect(split.scale * 0.591 * 0.772 + split.forwardPeak).toBeCloseTo(0.866, 12);
    expect(split.forwardPeak).toBeGreaterThan(0.39);
    expect(Math.abs(split.scale - 1)).toBeLessThan(0.01);
  });

  it("is none where the resolved part's own g is above the source's: that part is normalised", () => {
    expect(forwardPeakSplit(0.98, 0.98 * 0.8, 0.79)).toEqual({ scale: 1 / 0.98, forwardPeak: 0 });
  });
});

describe("a non-spherical mode with no phase", () => {
  it("is drawn as its solid: a water crystal on Yang et al.'s table, not liquid water's index", () => {
    const optics = nonSphericalModeOptics(
      {
        material: "H2O",
        shape: "crystal",
        sizes: { kind: "gamma", effectiveRadiusUm: 30, effectiveVariance: 0.1 },
      },
      550,
    );
    expect(optics.phaseBasis).toBe("model");
    expect(optics.phaseModel).toBe("yang2013");
  });
});

describe("the distributions and files the tables read", () => {
  it("refuse a bad tabulated distribution", () => {
    expect(() => tabulatedRadiusDistribution([1], [1])).toThrow(RangeError);
    expect(() => tabulatedRadiusDistribution([1, 1], [1, 1])).toThrow(RangeError);
    expect(() => tabulatedRadiusDistribution([1, 2], [1, -1])).toThrow(RangeError);
  });

  it("refuse a phase file whose variant or sizes are bad", () => {
    expect(() => parsePhaseFile({ ...mgsPhase, variant: 5 }, "bad.json")).toThrow(/variant/u);
    const [node] = mgsPhase.nodes;
    const reversed = { ...node, volumes: (node?.volumes ?? []).toReversed() };
    expect(() => parsePhaseFile({ ...mgsPhase, nodes: [reversed] }, "bad.json")).toThrow(/ascend/u);
  });
});
