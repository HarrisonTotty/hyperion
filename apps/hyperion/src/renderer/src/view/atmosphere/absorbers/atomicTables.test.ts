/**
 * R08.T4.e's atoms' and ions' tables (plan R08, R08.T4.e; decision-r08-t4ef-storage and
 * decision-r08-t4e-budget): the stages drawn and deferred, their grids and recorded tiers, their
 * bytes against the budget and the cap, their line files, their curves, and a class IV giant's
 * geometric albedo at 550 nm.
 */
import { beforeAll, describe, expect, it } from "vitest";

import { earthLikeColumn, planckBakeSpectrum } from "../../../test/atmosphereFixtures";
import { inlinePart } from "../../../test/kDistributionFixtures";
import {
  absorberCurve,
  absorberSpecs,
  type ColumnAbsorberSpec,
  pairCrossSectionAt,
} from "../absorbers";
import { rayleighCrossSectionM2 } from "../rayleigh";
import {
  cellSigmasAt,
  cellTransmittance,
  curveRowsPa,
  type KDistributionFile,
  type KDistributionTable,
  kDistributionAt,
  loadKDistributions,
  PRESSURE_LATTICE_PA,
  STANDARD_PRESSURES_PA,
} from "./kDistribution";
import { PENDING_ABSORBERS } from "./transparency";

/** The stages R08.T4.e draws, in the ruled order (decision-r08-t4e-budget §4). */
const DRAWN = ["Na", "K", "Fe", "H", "Zn", "Mg+", "Si+", "Mg", "Cr"] as const;

/** The drawn stages' keys, for a sidecar's species. */
const DRAWN_KEYS: ReadonlySet<string> = new Set(DRAWN);

/**
 * The stages it defers: those of the order that would pass 60,000,000 bytes, and the five the
 * ruling defers whatever room is left (Si, V⁺, Cr⁺, Mn⁺ and Ni⁺).
 */
const DEFERRED = [
  "Ca",
  "Ca+",
  "Al",
  "V",
  "Ti",
  "Fe+",
  "Ti+",
  "Mn",
  "Ni",
  "Si",
  "V+",
  "Cr+",
  "Mn+",
  "Ni+",
] as const;

/** A sidecar with the records the reduction writes beside the grid the client reads. */
interface RecordedSidecar extends KDistributionFile {
  readonly worstNodeError: number;
  readonly worstChainError: number;
  readonly worstQuantisationError: number;
  readonly linearCells: number;
}

/**
 * Every committed sidecar, by file. Typing them as {@link RecordedSidecar} is a cast at the
 * committed-data boundary, as `kDistribution.ts`' loader's is: the files are the tools' own writing
 * (`src/tools/kDistributions.ts`' `kDistributionMetaText`), and each test reads only the fields it
 * names.
 */
const SIDECARS = import.meta.glob<RecordedSidecar>(
  ["./kDistributions/*.json", "./exomol/kDistributions/*.json"],
  { import: "default", eager: true },
);

/** Every committed sidecar's text, by file, for its bytes. */
const SIDECAR_TEXTS = import.meta.glob<string>(
  ["./kDistributions/*.json", "./exomol/kDistributions/*.json"],
  { query: "?raw", import: "default", eager: true },
);

/** The committed line files' texts, by file (`absorbers/lines/`). */
const LINE_FILES = import.meta.glob<string>("./lines/*.json", {
  query: "?raw",
  import: "default",
  eager: true,
});

/** A species' committed sidecar, or a test failure. */
function sidecarOf(species: string): RecordedSidecar {
  const found = Object.values(SIDECARS).find((s) => s.species === species);
  if (found === undefined) {
    throw new Error(`no committed table for ${species}`);
  }
  return found;
}

/** A file's bytes, UTF-8. */
function bytesOf(text: string): number {
  return new TextEncoder().encode(text).length;
}

/** A species' line files: `<key>.json`, or its parts `<key>-<n>.json`. */
function lineFilesOf(species: string): ReadonlyArray<string> {
  const escaped = species.replace("+", "\\+");
  const pattern = new RegExp(`^\\./lines/${escaped}(-\\d+)?\\.json$`, "u");
  return Object.keys(LINE_FILES).filter((path) => pattern.test(path));
}

/** A sidecar's pressure, written to 6 figures, as its index on the lattice, or −1 off it. */
function latticeIndexOf(pressurePa: number): number {
  return PRESSURE_LATTICE_PA.findIndex((s) => Math.abs(s / pressurePa - 1) < 1e-5);
}

/**
 * Whether a table's pressures meet the lattice rule (decision-r08-t4e-unsold's follow-up): on the
 * lattice, holding E3's base nodes (2 a decade below 1 MPa, every node from it), and a midpoint
 * (odd j below 1 MPa) only where both its 4-a-decade neighbours are held.
 */
function meetsLatticeRule(pressuresPa: ReadonlyArray<number>): boolean {
  const held = new Set(pressuresPa.map(latticeIndexOf));
  if (held.has(-1)) {
    return false;
  }
  return PRESSURE_LATTICE_PA.every((_, j) => {
    const base = j >= 40 || j % 4 === 0;
    const midpoint = j < 40 && j % 2 === 1;
    return (
      (!base || held.has(j)) && (!midpoint || !held.has(j) || (held.has(j - 1) && held.has(j + 1)))
    );
  });
}

/** The table's committed bytes, its parts and its sidecar. */
function tableBytes(file: string, sidecar: KDistributionFile): number {
  return sidecar.parts.reduce((sum, p) => sum + p.bytes, 0) + bytesOf(SIDECAR_TEXTS[file] ?? "");
}

describe("the stages R08.T4.e draws and defers (decision-r08-t4e-budget)", () => {
  it("commit a table for each drawn stage and none for a deferred one", () => {
    const species = new Set(Object.values(SIDECARS).map((s) => s.species));
    expect([
      DRAWN.filter((key) => !species.has(key)),
      DEFERRED.filter((key) => species.has(key)),
    ]).toEqual([[], []]);
  });

  it("keep each deferred stage's pending entry, saying why", () => {
    expect(
      DEFERRED.filter(
        (key) =>
          !PENDING_ABSORBERS.some(
            (p) =>
              p.key === key &&
              p.task === "R08.T4.e" &&
              p.what.includes("deferred past T4.e's byte budget (main, 2026-10-19)"),
          ),
      ),
    ).toEqual([]);
  });

  /** The committed line files' bytes, all T4.e's. */
  const lineBytes = (): number =>
    Object.values(LINE_FILES).reduce((sum, text) => sum + bytesOf(text), 0);

  it("stay within T4.e's 60,000,000 bytes, line files included", () => {
    const drawn = Object.entries(SIDECARS).filter(([, s]) => DRAWN_KEYS.has(s.species));
    const t4e = drawn.reduce((sum, [file, s]) => sum + tableBytes(file, s), 0) + lineBytes();
    expect(t4e).toBeLessThanOrEqual(60_000_000);
  });

  it("keep every committed table, line files included, under the 150,000,000-byte cap", () => {
    const all =
      Object.entries(SIDECARS).reduce((sum, [file, s]) => sum + tableBytes(file, s), 0) +
      lineBytes();
    expect(all).toBeLessThanOrEqual(150_000_000);
  });
});

describe("the drawn stages' grids", () => {
  it.each(DRAWN)(
    "start %s at its range's first node, its visibility floor or its ruled node, and run to 5,000 K",
    (key) => {
      const first: Readonly<Record<string, number>> = {
        Na: 500,
        K: 500,
        H: 3_300,
        Zn: 3_300,
        "Mg+": 4_400,
        "Si+": 4_400,
      };
      const temperaturesK = sidecarOf(key).temperaturesK;
      expect([temperaturesK[0], temperaturesK.at(-1)]).toEqual([first[key] ?? 1_000, 5_000]);
    },
  );

  it.each(DRAWN)(
    "take %s's E1′ nodes from its first, refined where the between-node test asked",
    (key) => {
      // E1′ (decision-r08-t4e-budget §3): from 1,000 K each step the largest multiple of 50 K at most
      // 10% of its lower node; Mg⁺'s and Si⁺'s ruled nodes are E1's 4,400–5,000 K.
      const ladder: number[] = [];
      for (let t = 50; t < 5_000; t += t < 1_000 ? 50 : Math.floor((0.1 * t) / 50 + 1e-9) * 50) {
        ladder.push(t);
      }
      ladder.push(5_000);
      const nodes = sidecarOf(key).temperaturesK;
      const own =
        key === "Mg+" || key === "Si+"
          ? [4_400, 4_600, 4_800, 5_000]
          : ladder.filter((t) => t >= (nodes[0] ?? Number.NaN));
      expect(own.filter((t) => !nodes.includes(t))).toEqual([]);
    },
  );

  it.each(DRAWN)(
    "store %s on E3's thinned pressures and any midpoints, read at the standard rows",
    (key) => {
      const pressures = sidecarOf(key).pressuresPa;
      const standard = pressures.filter(
        (p) => latticeIndexOf(p) >= 40 || latticeIndexOf(p) % 2 === 0,
      );
      expect([
        meetsLatticeRule(pressures),
        standard.length < STANDARD_PRESSURES_PA.length,
        curveRowsPa(pressures).length,
      ]).toEqual([true, true, 30]);
    },
  );
});

describe("the drawn stages' recorded tiers (T4.d's)", () => {
  it.each(DRAWN)("pass %s's node tier, 10⁻³", (key) => {
    expect(sidecarOf(key).worstNodeError).toBeLessThanOrEqual(1e-3);
  });

  it.each(DRAWN)("pass %s's 16-bit test, 2 × 10⁻⁴", (key) => {
    expect(sidecarOf(key).worstQuantisationError).toBeLessThanOrEqual(2e-4);
  });

  it.each(DRAWN)("pass %s's chain tier, 2 × 10⁻³", (key) => {
    expect(sidecarOf(key).worstChainError).toBeLessThanOrEqual(2e-3);
  });

  it.each(DRAWN)("record %s's 1 Pa check", (key) => {
    expect(typeof sidecarOf(key).measuredBelowGrid).toBe("boolean");
  });

  it.each(DRAWN)("record %s's G = 1 cells", (key) => {
    expect(Number.isInteger(sidecarOf(key).linearCells)).toBe(true);
  });

  it.each(DRAWN)("take H₂ as %s's reference broadener", (key) => {
    expect(sidecarOf(key).broadening["H2"]).toBe(1);
  });

  it.each(DRAWN)("widen no pure-gas cap for %s: every ratio at least 0.40", (key) => {
    // decision-r08-t4ef-storage §3: a ratio under 0.40 would widen the cap by 0.40 ÷ r_min.
    expect(Object.values(sidecarOf(key).broadening).every((r) => r >= 0.4)).toBe(true);
  });

  it("are measured, but H I's, whose Stark broadening rests on an electron density that is no axis", () => {
    expect(DRAWN.filter((key) => sidecarOf(key).basis !== "measured")).toEqual(["H"]);
  });
});

describe("the drawn stages' line files", () => {
  it("hold one file a spectrum, or its parts, for each drawn stage and none for a deferred one", () => {
    expect([
      DRAWN.filter((key) => lineFilesOf(key).length === 0),
      DEFERRED.filter((key) => lineFilesOf(key).length > 0),
      Object.keys(LINE_FILES).length,
    ]).toEqual([[], [], DRAWN.reduce((n, key) => n + lineFilesOf(key).length, 0)]);
  });

  it.each(Object.entries(LINE_FILES))(
    "keep %s under the hook, naming its source and selection, its lines in 360–780 nm",
    (_, text) => {
      const file: unknown = JSON.parse(text);
      const field = (name: string): unknown =>
        typeof file === "object" && file !== null ? Reflect.get(file, name) : undefined;
      const lines = field("lines");
      const wavelengths = Array.isArray(lines)
        ? lines.map((row: unknown) => (Array.isArray(row) ? Number(row[0]) : Number.NaN))
        : [];
      expect([
        bytesOf(text) < 500_000,
        ["source", "url", "fetched", "citation", "selection", "columns"].every(
          (name) => field(name) !== undefined,
        ),
        wavelengths.length > 0 && wavelengths.every((nm) => nm >= 360 && nm <= 780),
        wavelengths.every((nm, i) => i === 0 || nm <= (wavelengths[i - 1] ?? Infinity)),
      ]).toEqual([true, true, true, true]);
    },
  );
});

/** The drawn stages' tables, loaded once. */
let tables: ReadonlyArray<KDistributionTable> = [];

beforeAll(async () => {
  tables = await loadKDistributions([...DRAWN], inlinePart);
}, 300_000); // Fe's and Cr's parts the largest

/** A loaded table, or a test failure. */
function tableOf(species: string): KDistributionTable {
  const table = tables.find((t) => t.species === species);
  if (table === undefined) {
    throw new Error(`${species}'s committed table did not load`);
  }
  return table;
}

/** A temperature each drawn stage's table covers: mid-range, above its floor. */
function testTemperatureK(species: string): number {
  const nodes = tableOf(species).temperaturesK;
  return nodes[Math.floor(nodes.length / 2)] ?? Number.NaN;
}

describe("the drawn stages' curves", () => {
  it("deepen Na D's cell monotonically with column, at every stored pressure", () => {
    const kd = kDistributionAt(tableOf("Na"), 1_500);
    const cell = kd.cells.findIndex((c) => c.cellNm === 589);
    for (const p of kd.pressuresPa) {
      const sigmas = cellSigmasAt(kd, p)[cell] ?? new Float64Array(0);
      const weights = kd.cells[cell]?.weights ?? new Float64Array(0);
      const depths = Array.from(
        { length: 60 },
        (_, k) => -Math.log(cellTransmittance(weights, sigmas, 10 ** (14 + k / 4))),
      );
      expect(depths.every((d, k) => k === 0 || d >= (depths[k - 1] ?? Infinity))).toBe(true);
    }
  });

  it.each(DRAWN)(
    "keep %s's T_c in [10⁻⁶, 1], non-increasing in u along every row",
    (key) => {
      const column = earthLikeColumn();
      const temperatureK = testTemperatureK(key);
      const specs = absorberSpecs(
        [{ species: key, columnPerM2: 1e22, basePa: 101_325, topPa: 0 }],
        { ...column, temperaturesK: column.temperaturesK.map(() => temperatureK) },
        [],
        [],
        {
          kDistributions: tables,
          bulkMoleFractions: new Map([
            ["H2", 0.86],
            ["He", 0.14],
          ]),
        },
      );
      const spec = specs.specs.find((s): s is ColumnAbsorberSpec => s.kind === "column");
      if (spec === undefined) {
        throw new Error(`no column spec for ${key}`);
      }
      const curve = absorberCurve(spec, planckBakeSpectrum(5_772));
      const nodes = curve.logColumns.length;
      let least = Infinity;
      let rise = 0;
      for (const channel of curve.transmittance) {
        for (let r = 0; r < curve.logPressures.length; r += 1) {
          for (let j = 0; j < nodes; j += 1) {
            const t = channel[r * nodes + j] ?? Number.NaN;
            least = Math.min(least, t);
            rise = Math.max(rise, j > 0 ? t - (channel[r * nodes + j - 1] ?? Number.NaN) : 0);
            expect(t).toBeLessThanOrEqual(1);
          }
        }
      }
      expect([least >= 1e-6, rise]).toEqual([true, 0]);
    },
    60_000,
  );
});

/**
 * A class IV giant's geometric albedo at 550 nm (Sudarsky, Burrows and Pinto 2000, Astrophys. J.
 * 538, 885: §7's "the geometric albedo at 0.48 µm is only 0.03", and its Fig. 12a's flux ratios
 * about a G2V star at 0.05 AU, p about 0.055 at 0.45 µm and 0.0055 at 0.65 µm, beside Table 1a's
 * class IV Bond albedo 0.03; as the science check read them on arXiv, astro-ph/9910504v2, not
 * re-read here), the R08.T4.e test's model: an isothermal H₂–He
 * column (x 0.84 and 0.16 by number, μ 2.33 u, g 25 m s⁻²) with Asplund et al. 2009's solar Na and
 * K as free atoms, scattering by H₂'s and He's Rayleigh cross-sections (the client's) and absorbing
 * by the alkali pairs' far wings and the atoms' own tables' 550 nm cells where they hold one.
 * Single scattering at full phase, p₁ = (P(π) ÷ 2) ∫₀¹ dμ ∫ e^(−2τ ÷ μ) dτ_sca with Rayleigh's
 * P(π) = 3/2, and the higher orders by p₁ ÷ (1 − ω_max), ω_max the largest single-scattering albedo
 * over the levels that give 99% of p₁, plus 0.01 p₁ for the levels left out: a heuristic bound, not
 * a theorem, holding where each order is at most ω_max times the last, as here, where ω falls with
 * depth.
 */
function classIvAlbedo(temperatureK: number): { readonly single: number; readonly bound: number } {
  const k = 1.380_649e-23;
  const mass = 2.33 * 1.660_539_066_6e-27;
  const g = 25;
  const [xH2, xHe] = [0.84, 0.16];
  const xNa = 2 * xH2 * 10 ** (6.24 - 12);
  const xK = 2 * xH2 * 10 ** (5.03 - 12);
  const nm = 550;
  const pair = (key: string): number => {
    const p = pairCrossSectionAt(key, temperatureK);
    return p?.crossSectionsM5[nm - p.firstNm] ?? 0;
  };
  const scattering =
    xH2 * rayleighCrossSectionM2("H2", nm) + xHe * rayleighCrossSectionM2("He", nm);
  const pairs =
    xNa * (xH2 * pair("Na+H2") + xHe * pair("Na+He")) +
    xK * (xH2 * pair("K+H2") + xHe * pair("K+He"));
  // The atoms' own 550 nm cells, their mean σ at a pressure, where their tables hold one.
  const cells = (["Na", "K"] as const).map((key) => {
    const kd = kDistributionAt(tableOf(key), temperatureK);
    return { kd, c: kd.cells.findIndex((cell) => cell.cellNm === nm), x: key === "Na" ? xNa : xK };
  });
  const own = (pressurePa: number): number =>
    cells.reduce((sum, { kd, c, x }) => {
      const weights = kd.cells[c]?.weights;
      const sigmas = c < 0 ? undefined : cellSigmasAt(kd, pressurePa)[c];
      const mean =
        weights === undefined || sigmas === undefined
          ? 0
          : weights.reduce((s, w, i) => s + w * (sigmas[i] ?? 0), 0);
      return sum + x * mean;
    }, 0);
  // Levels in ln p, 10 mPa to 100 MPa.
  const levels = 4_000;
  const logLow = Math.log(1e-2);
  const logHigh = Math.log(1e8);
  const steps: { tauSca: number; tauExt: number; dSca: number; omega: number }[] = [];
  let tauSca = 0;
  let tauAbs = 0;
  for (let i = 0; i < levels; i += 1) {
    const p0 = Math.exp(logLow + ((logHigh - logLow) * i) / levels);
    const p1 = Math.exp(logLow + ((logHigh - logLow) * (i + 1)) / levels);
    const pm = Math.sqrt(p0 * p1);
    const dColumn = (p1 - p0) / (mass * g);
    const dSca = scattering * dColumn;
    const dAbs = (pairs * (pm / (k * temperatureK)) + own(pm)) * dColumn;
    tauSca += dSca;
    tauAbs += dAbs;
    steps.push({ tauSca, tauExt: tauSca + tauAbs, dSca, omega: dSca / (dSca + dAbs) });
  }
  const angles = 200;
  const contributions = steps.map((s) => {
    let sum = 0;
    for (let m = 0; m < angles; m += 1) {
      const mu = (m + 0.5) / angles;
      sum += Math.exp((-2 * s.tauExt) / mu) / angles;
    }
    return 0.75 * sum * s.dSca;
  });
  const single = contributions.reduce((a, b) => a + b, 0);
  // The smallest set of levels, by their share, that gives 99% of p₁, and its largest ω.
  const order = contributions.map((c, i) => [c, i] as const).toSorted((a, b) => b[0] - a[0]);
  let held = 0;
  let omegaMax = 0;
  for (const [c, i] of order) {
    if (held >= 0.99 * single) {
      break;
    }
    held += c;
    omegaMax = Math.max(omegaMax, steps[i]?.omega ?? 1);
  }
  return { single, bound: single / (1 - omegaMax) + 0.01 * single };
}

describe("a class IV giant's geometric albedo at 550 nm (Sudarsky et al. 2000)", () => {
  it.each([900, 1_200, 1_500])(
    "is under 0.1 at %i K, the alkali wings absorbing what H₂ and He scatter",
    (t) => {
      const { single, bound } = classIvAlbedo(t);
      expect([single > 0, bound < 0.1]).toEqual([true, true]);
    },
  );
});
