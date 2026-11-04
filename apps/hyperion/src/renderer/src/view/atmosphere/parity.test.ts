/**
 * The registry parity check (plan R08, R08.T19): the client's optics tables held to the sim's
 * substance registry through its public mirror, `packages/protocol/fixtures/substances.json`.
 *
 * @remarks
 * The fixture holds the registry's rows in index order, append-only, each with the client optics
 * the sim may ask of it (decision-composition §1.1 and §1.9). The client's optics are keyed by the
 * sim's strings, every look-up returns a provenance, and the fallback never drops a species. This
 * check reads each row's key, its molar mass, its three flags, its Rayleigh cross section at
 * 550 nm and its grain density: never `heat_capacity_over_r_298k`, which is for display and
 * cross-check only (science-r08-gases-2 §1.4, which drops the c_p row). It fails if:
 *
 * 1. a row flagged `rayleigh` has neither measured nor estimated Rayleigh optics;
 * 2. a row flagged `absorber` has no cross-sections (H⁻'s closed form among them), no committed
 *    line table (R08.T4.d's and R08.T4.g's), no pending entry with its cell means and no stated
 *    none with its reason;
 * 3. a row flagged `aerosol_material` has no index file of its own, a stated stand-in being a file
 *    whose provenance is `standIn` and whose text says what stands in (the generic stand-in alone
 *    fails); or a file's key is not a row (R08.T5.b's test, which T19 checks);
 * 4. `GAS_MOLAR_MASS_G_PER_MOL` differs from the fixture's molar mass by more than 10⁻¹² g mol⁻¹ for
 *    a key in both;
 * 5. an index file of an `aerosol_material` row has no grain density of its phase and variant at
 *    any temperature (`grainDensity.ts`), unless it is one of the sim's stated refusals
 *    (decision-p14-t54a-grain-density §2.6): condition 3's density analogue;
 * 6. a row's `rayleigh_cross_section_550nm_m2` (P14.T24.h.a, the sim's Rayleigh floor) is given
 *    for a gas this client does not measure, or differs from its `rayleighCrossSectionM2` at
 *    550 nm by more than 10⁻³ of it, or is null for a gas it measures (decision-bump-plan-after-22,
 *    "p14-gate-24's questions" Q1). The sim reduces each from `rayleigh.ts`'s formula, reference
 *    state and King factor, so the bound covers the constant's printed digits alone;
 * 7. `registryFlags.ts`' table, which the optics worker reads at run time (R08.T10.a,
 *    `decision-r08-t10a-leans.md` item 7), lacks a row, holds one the fixture does not, or differs
 *    from a row's `absorber` or `rayleigh` flag.
 *
 * R08.T19's fifth row holds the client's τ(550) to the record's within 1% for every mode of the
 * protocol fixture's one surface aerosol inventory, the generated `body_detail_temperate_world`'s
 * (`hyperion-server`'s `the_fixture_s_temperate_world_is_the_generator_s`): in-cloud, as the
 * record's, before the coverage `assemble.ts`'s `modesOf` applies, from the mode's column mass and
 * this client's mass extinction of its particles.
 *
 * The check grows with the registry: a row P14 appends with a flag set is checked once the fixture
 * is re-blessed.
 */
import type { AerosolTauBasisDto, BodySurfaceDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import planetary from "../../../../../../../packages/protocol/fixtures/planetary.json" with { type: "json" };
import registry from "../../../../../../../packages/protocol/fixtures/substances.json" with { type: "json" };
import type { AerosolMode as RecordAerosolMode } from "../../lib/system/model";
import { detailOf } from "../../test/atmosphereRecords";
import { TEMPERATE_WORLD_SYSTEM, temperateWorldDetail } from "../../test/planetaryFixture";
import { crossSectionAt } from "./absorbers";
import { committedLineSpecies } from "./absorbers/kDistribution";
import { aerosolModeInRange, aerosolModeOptics } from "./aerosol";
import { recordModeParticles } from "./assemble";
import { NO_VISIBLE_ABSORPTION, PENDING_ABSORBERS } from "./absorbers/transparency";
import { type GrainDensityRow, grainDensityKgPerM3, memberOf } from "./grainDensity";
import { MATERIAL_FILES, type MaterialPhase, resolveMaterial } from "./materials/materials";
import {
  GAS_MOLAR_MASS_G_PER_MOL,
  type Gas,
  GASES,
  rayleighCrossSectionM2,
  rayleighOf,
} from "./rayleigh";
import { REGISTRY_OPTICS_FLAGS } from "./registryFlags";
import { resolveModeMaterial } from "./sizeDistribution";

/**
 * A fixture row as this check reads it: its key, its molar mass, its three optics flags and its
 * Rayleigh cross section.
 */
interface RegistryRow {
  readonly key: string;
  /** g mol⁻¹, or `null` for a row with no definite formula. */
  readonly molarMassGPerMol: number | null;
  readonly rayleigh: boolean;
  readonly absorber: boolean;
  readonly aerosolMaterial: boolean;
  /** m² per molecule at 550 nm in vacuum, or `null` where the sim holds none (P14.T24.h.a). */
  readonly rayleighCrossSection550nmM2: number | null;
}

/** The fixture's rows, in its order. */
const ROWS: ReadonlyArray<RegistryRow> = registry.substances.map((row) => ({
  key: row.key,
  molarMassGPerMol: row.molar_mass_g_per_mol,
  rayleigh: row.rayleigh,
  absorber: row.absorber,
  aerosolMaterial: row.aerosol_material,
  rayleighCrossSection550nmM2: row.rayleigh_cross_section_550nm_m2,
}));

/** The keys of the rows with a flag set, in the fixture's order. */
function flagged(flag: "rayleigh" | "absorber" | "aerosolMaterial"): ReadonlyArray<string> {
  return ROWS.filter((row) => row[flag]).map((row) => row.key);
}

/**
 * A temperature, K, at which to ask for a species' cross-sections: whether a table or a closed form
 * answers does not depend on it.
 */
const ANY_TEMPERATURE_K = 250;

/** Whether the client holds cross-sections for a key: a table, or H⁻'s closed form. */
function hasCrossSections(key: string): boolean {
  return crossSectionAt(key, ANY_TEMPERATURE_K) !== undefined;
}

/** Whether a key is pending with its cell means at one node or more. */
function isPendingWithCellMeans(key: string): boolean {
  return PENDING_ABSORBERS.some(
    ({ key: pending, bound }) => pending === key && bound.temperaturesK.length > 0,
  );
}

/** Whether a key is stated to have no visible absorption, with its evidence. */
function isStatedNone(key: string): boolean {
  return NO_VISIBLE_ABSORPTION.some((n) => n.species === key && n.evidence.length > 0);
}

describe("the registry's Rayleigh rows (decision-composition §1.9)", () => {
  const keys = flagged("rayleigh");

  it("are found in the fixture", () => {
    expect(keys.length).toBeGreaterThan(0);
  });

  it.each(keys)("give %s measured or estimated optics", (species) => {
    expect(rayleighOf(species).provenance).not.toBe("none");
  });
});

/** The keys among those given with none of condition 2's answers. */
function unanswered(keys: ReadonlyArray<string>): ReadonlyArray<string> {
  const tables = committedLineSpecies();
  return keys.filter(
    (key) =>
      !hasCrossSections(key) &&
      !tables.includes(key) &&
      !isPendingWithCellMeans(key) &&
      !isStatedNone(key),
  );
}

describe("the registry's absorbers", () => {
  it("each have cross-sections, a line table, a pending entry with its cell means, or a stated none", () => {
    const keys = flagged("absorber");
    expect([keys.length > 0, unanswered(keys)]).toEqual([true, []]);
  });
});

describe("the client's table of the registry's optics flags (R08.T10.a)", () => {
  it("holds every row of the fixture, in its order, with its absorber and rayleigh flags", () => {
    expect([...REGISTRY_OPTICS_FLAGS]).toEqual(
      ROWS.map((row) => [row.key, { absorber: row.absorber, rayleigh: row.rayleigh }]),
    );
  });

  // 24's 115 and, from 27, P14.T52.e's twelve further carriers, none flagged.
  it("holds 127 rows at generator version 27, 54 of them absorbers and 43 Rayleigh scatterers", () => {
    const flags = [...REGISTRY_OPTICS_FLAGS.values()];
    expect([
      flags.length,
      flags.filter((f) => f.absorber).length,
      flags.filter((f) => f.rayleigh).length,
    ]).toEqual([127, 54, 43]);
  });
});

describe("the registry's aerosol materials (R08.T5.d)", () => {
  it("serve every aerosol-material row of the registry's fixture with a file of their own", () => {
    // A row the generic stand-in alone serves fails: its key would have no file.
    const keys = flagged("aerosolMaterial");
    const unserved = keys.filter((key) => resolveMaterial(key).file === undefined);
    expect([keys.length > 0, unserved]).toEqual([true, []]);
  });

  it("say what stands in for each row whose own file is a stated stand-in", () => {
    const unstated = flagged("aerosolMaterial").filter((key) => {
      const { provenance, standIn } = resolveMaterial(key);
      return provenance === "standIn" && (standIn === undefined || standIn.length === 0);
    });
    expect(unstated).toEqual([]);
  });

  it("key every index file by a row of the fixture (R08.T5.b)", () => {
    const keys = new Set(ROWS.map((row) => row.key));
    expect(MATERIAL_FILES.map((file) => file.key).filter((key) => !keys.has(key))).toEqual([]);
  });
});

/** An index file as the density check names it: its key, its phase and its variant. */
interface FileForm {
  readonly key: string;
  readonly phase: MaterialPhase;
  readonly variant: string | null;
}

/**
 * The index files of aerosol-material rows that have no grain density of their phase and variant
 * at any temperature, each with its reason: the sim's `NoDensity` refusals
 * (`substance_aerosol_refuses_what_it_cannot_serve`; decision-p14-t54a-grain-density §2.6). From
 * generator version 24 the fixture reads liquid sulphur's, solid H2S's and solid argon's densities
 * (P14.T49.n), which 23's refused.
 */
const NO_GRAIN_DENSITY: ReadonlyArray<FileForm & { readonly reason: string }> = [
  { key: "H2SO4", phase: "liquid", variant: "84.5wt%", reason: "not the row's 75 wt% reference" },
  { key: "H2O", phase: "liquid", variant: "S35", reason: "seawater, not the row's pure water" },
];

/**
 * Whether a row has a grain density as a phase of a variant at some temperature: the reader gives
 * one at a node of the phase's member, of the file's variant.
 */
function hasGrainDensity(row: GrainDensityRow, form: FileForm): boolean {
  return (memberOf(row, form.phase) ?? []).some((piece) =>
    piece.temperatures_k.some(
      (t) => grainDensityKgPerM3(row, form.phase, t, form.variant ?? undefined) !== undefined,
    ),
  );
}

/** A file's form as one string, for comparing lists in any order. */
function formName({ key, phase, variant }: FileForm): string {
  return `${phase} ${key} ${variant ?? ""}`;
}

describe("the registry's aerosol materials' grain densities (decision-p14-t54a-grain-density)", () => {
  /** The fixture's aerosol-material rows by key. */
  const rows = new Map(
    registry.substances.filter((row) => row.aerosol_material).map((row) => [row.key, row]),
  );
  /** The index files of those rows, as the density check names them. */
  const files: ReadonlyArray<FileForm> = MATERIAL_FILES.filter((file) => rows.has(file.key)).map(
    (file) => ({ key: file.key, phase: file.phase, variant: file.variant ?? null }),
  );

  it("are found for the rows' files", () => {
    expect(files.length).toBeGreaterThan(0);
  });

  it("give every file of an aerosol-material row a density of its phase and variant, or a stated refusal", () => {
    const undensed = files.filter((form) => {
      const row = rows.get(form.key);
      return row === undefined || !hasGrainDensity(row, form);
    });
    expect(undensed.map(formName).toSorted()).toEqual(NO_GRAIN_DENSITY.map(formName).toSorted());
  });

  it("state a reason for each refusal", () => {
    expect(NO_GRAIN_DENSITY.filter(({ reason }) => reason.length === 0)).toEqual([]);
  });
});

/** The bound on condition 6's relative difference: the sim's constants' digits. */
const RAYLEIGH_PARITY_TOLERANCE = 1e-3;

/** Whether a key is one of this client's measured gases. */
function isMeasuredGas(key: string): key is Gas {
  return GASES.some((gas) => gas === key);
}

describe("the registry's Rayleigh cross sections at 550 nm (P14.T24.h.a)", () => {
  /** The measured gases the fixture gives a cross section, each with it, m². */
  const given = ROWS.flatMap(({ key, rayleighCrossSection550nmM2: crossSectionM2 }) =>
    isMeasuredGas(key) && crossSectionM2 !== null ? [{ gas: key, crossSectionM2 }] : [],
  );

  it("are given for some rows", () => {
    expect(given.length).toBeGreaterThan(0);
  });

  it("are given only for this client's measured gases", () => {
    const unmeasured = ROWS.filter(
      ({ key, rayleighCrossSection550nmM2 }) =>
        rayleighCrossSection550nmM2 !== null && !isMeasuredGas(key),
    ).map(({ key }) => key);
    expect(unmeasured).toEqual([]);
  });

  it.each(given)("give $gas its cross section within 10⁻³", ({ gas, crossSectionM2 }) => {
    const clientM2 = rayleighCrossSectionM2(gas, 550);
    expect(Math.abs(crossSectionM2 - clientM2) / clientM2).toBeLessThanOrEqual(
      RAYLEIGH_PARITY_TOLERANCE,
    );
  });

  it("are given for every measured gas the fixture holds", () => {
    const missing = ROWS.filter(
      ({ key, rayleighCrossSection550nmM2 }) =>
        isMeasuredGas(key) && rayleighCrossSection550nmM2 === null,
    ).map(({ key }) => key);
    expect(missing).toEqual([]);
  });
});

describe("the molar masses", () => {
  /** The measured gases the fixture holds rows for. */
  const shared = GASES.filter((gas) => ROWS.some((row) => row.key === gas));

  it("are shared with the fixture", () => {
    expect(shared.length).toBeGreaterThan(0);
  });

  it.each(shared)("give %s the fixture's molar mass within 10⁻¹² g mol⁻¹", (gas) => {
    const row = ROWS.find((r) => r.key === gas);
    const difference = Math.abs(
      (row?.molarMassGPerMol ?? Number.NaN) - GAS_MOLAR_MASS_G_PER_MOL[gas],
    );
    expect(difference).toBeLessThanOrEqual(1e-12);
  });
});

/**
 * The wire's aerosol inventory, `BodySurfaceDto.aerosols` (P14.T24.c), by its member's name: a
 * rename in the generated type fails the type check, so that the census of the fixture's
 * inventories cannot outlive its member.
 */
const AEROSOLS_MEMBER = "aerosols" satisfies keyof BodySurfaceDto;

/** Every path in a JSON value at which an object has an `aerosols` member, appended to `found`. */
function collectAerosolsMembers(value: unknown, path: string, found: string[]): void {
  if (Array.isArray(value)) {
    value.forEach((item: unknown, i) => {
      collectAerosolsMembers(item, `${path}[${i}]`, found);
    });
    return;
  }
  if (typeof value !== "object" || value === null) {
    return;
  }
  for (const [key, member] of Object.entries(value)) {
    const at = `${path}.${key}`;
    if (key === AEROSOLS_MEMBER) {
      found.push(at);
    }
    collectAerosolsMembers(member, at, found);
  }
}

/** Every path in a JSON value at which an object has an `aerosols` member. */
function aerosolsMembers(value: unknown, path: string): ReadonlyArray<string> {
  const found: string[] = [];
  collectAerosolsMembers(value, path, found);
  return found;
}

/**
 * The fixture's message whose record is the generator's, a golden body's (`hyperion-server`'s
 * `the_fixture_s_temperate_world_is_the_generator_s`): the one message whose surface carries an
 * aerosol inventory.
 */
const GENERATED_SURFACE = "body_detail_temperate_world";

/** What to do when the census finds another inventory than the generated surface's. */
const ANOTHER_INVENTORY =
  "another protocol fixture message carries a surface's aerosol inventory. If it is a generated " +
  "surface's, pinned by a server test as body_detail_temperate_world is, run R08.T19's fifth row " +
  "over its modes too; otherwise its τ(550) is no record's, and the row does not read it";

/** The bound on the fifth row's relative difference, the client's τ(550) against the record's. */
const TAU_550_PARITY_TOLERANCE = 0.01;

/** A mode of the generated surface's inventory with its τ(550), the record's and the client's. */
interface ModeDepth {
  /** Its phase and material, as `liquid H2O`. */
  readonly name: string;
  readonly basis: AerosolTauBasisDto | undefined;
  readonly recordTau550: number | undefined;
  readonly clientTau550: number | undefined;
}

/** The fixture's rows by key, for their grain densities. */
const DENSITY_ROWS = new Map(registry.substances.map((row) => [row.key, row]));

/**
 * The client's τ(550) of a record's mode, in-cloud as the record's (its coverage, which
 * `assemble.ts`'s `modesOf` applies, left out): its column mass times its particles' mass
 * extinction, ⟨C_ext⟩ ÷ (ρ ⟨V⟩), from this client's optics of the particles it takes from the
 * record (`recordModeParticles`: the index file, the size quadrature and the large-particle limit)
 * and the fixture's grain density at the mode's phase and temperature, of the file's variant.
 *
 * @returns Undefined where the record leaves out the mode's sizes, column mass or temperature, the
 *   particles are an aggregate's or outside the optics at 550 nm, or the fixture gives no density.
 */
function clientTau550Of(mode: RecordAerosolMode): number | undefined {
  const { sizeDistribution: sizes, columnMassKgM2, temperatureK } = mode;
  if (sizes.kind !== "value" || columnMassKgM2.kind !== "value" || temperatureK.kind !== "value") {
    return undefined;
  }
  const particles = recordModeParticles(mode, sizes.value);
  const row = DENSITY_ROWS.get(mode.material);
  if (
    particles.kind !== "distribution" ||
    !aerosolModeInRange(particles, 550) ||
    row === undefined
  ) {
    return undefined;
  }
  const variant = resolveModeMaterial(particles.mode).file?.variant;
  const densityKgPerM3 = grainDensityKgPerM3(row, mode.phase, temperatureK.value, variant);
  if (densityKgPerM3 === undefined) {
    return undefined;
  }
  const optics = aerosolModeOptics(particles, 550);
  return (
    (columnMassKgM2.value * optics.extinctionCrossSectionM2) / (densityKgPerM3 * optics.volumeM3)
  );
}

/**
 * The generated surface's modes in the record's order, each with its τ(550); none where its surface
 * carries no inventory, which the census below then reports.
 */
function generatedModeDepths(): ReadonlyArray<ModeDepth> {
  const { surface } = detailOf(temperateWorldDetail(), TEMPERATE_WORLD_SYSTEM).record;
  if (surface.state !== "ok" || surface.value.aerosols.kind !== "value") {
    return [];
  }
  return surface.value.aerosols.value.modes.map((mode) => {
    const depth = mode.opticalDepth.kind === "value" ? mode.opticalDepth.value : undefined;
    return {
      name: `${mode.phase} ${mode.material}`,
      basis: depth?.basis,
      recordTau550: depth?.tau550,
      clientTau550: clientTau550Of(mode),
    };
  });
}

/** A mode's τ(550), the client's, relative to the record's, less 1. */
function relativeDifference({ recordTau550, clientTau550 }: ModeDepth): number {
  return (clientTau550 ?? Number.NaN) / (recordTau550 ?? Number.NaN) - 1;
}

describe("τ(550), the record's against the client's (R08.T19's fifth row)", () => {
  const depths = generatedModeDepths();

  it("finds one aerosol inventory in the fixture, the generated temperate world's", () => {
    // The instruction rides with the members found, so that a failure prints it.
    expect({
      aerosolsMembers: aerosolsMembers(planetary, "planetary.json"),
      ifAnother: ANOTHER_INVENTORY,
    }).toEqual({
      aerosolsMembers: [`planetary.json.${GENERATED_SURFACE}.body.record.surface.value.aerosols`],
      ifAnother: ANOTHER_INVENTORY,
    });
  });

  it("finds a water deck's mode and a sulphate layer's, each with its basis and the client's τ(550)", () => {
    // From generator version 27 the temperate world's water deck is ice: its base at 269.5 K, below
    // water's freezing point (275.8 K, liquid, at 26).
    expect(
      depths.map(({ name, basis, clientTau550 }) => [name, basis, clientTau550 !== undefined]),
    ).toEqual([
      ["solid H2O", "geometric_limit", true],
      ["liquid H2SO4", "sphere", true],
    ]);
  });

  it.each(depths)("gives the $name the record's τ(550) within 1%", (depth) => {
    expect(Math.abs(relativeDifference(depth))).toBeLessThanOrEqual(TAU_550_PARITY_TOLERANCE);
  });

  it("would find an aerosols member at any depth, null or not", () => {
    const message = {
      body: { record: [{ surface: { aerosols: null } }, { aerosols: { modes: [] } }] },
    };
    expect(aerosolsMembers(message, "m")).toEqual([
      "m.body.record[0].surface.aerosols",
      "m.body.record[1].aerosols",
    ]);
  });
});
