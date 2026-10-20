/**
 * The registry parity check (plan R08, R08.T19): the client's optics tables held to the sim's
 * substance registry through its public mirror, `packages/protocol/fixtures/substances.json`.
 *
 * @remarks
 * The fixture holds the registry's rows in index order, append-only, each with the client optics
 * the sim may ask of it (decision-composition §1.1 and §1.9). The client's optics are keyed by the
 * sim's strings, every look-up returns a provenance, and the fallback never drops a species. This
 * check reads each row's key, its molar mass, its three flags and its grain density: never
 * `heat_capacity_over_r_298k`, which is for display and cross-check only (science-r08-gases-2 §1.4,
 * which drops the c_p row). It fails if:
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
 *    (decision-p14-t54a-grain-density §2.6): condition 3's density analogue.
 *
 * The fifth row, the client's τ(550) against the record's within 1%, is held
 * (decision-bump-plan-after-22): see its tripwire below.
 *
 * The check grows with the registry: a row P14 appends with a flag set is checked once the fixture
 * is re-blessed.
 */
import type { BodySurfaceDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import planetary from "../../../../../../../packages/protocol/fixtures/planetary.json" with { type: "json" };
import registry from "../../../../../../../packages/protocol/fixtures/substances.json" with { type: "json" };
import { crossSectionAt } from "./absorbers";
import { committedLineSpecies } from "./absorbers/kDistribution";
import { NO_VISIBLE_ABSORPTION, PENDING_ABSORBERS } from "./absorbers/transparency";
import { type GrainDensityRow, grainDensityKgPerM3 } from "./grainDensity";
import { MATERIAL_FILES, type MaterialPhase, resolveMaterial } from "./materials/materials";
import { GAS_MOLAR_MASS_G_PER_MOL, GASES, rayleighOf } from "./rayleigh";

/** A fixture row as this check reads it: its key, its molar mass and its three optics flags. */
interface RegistryRow {
  readonly key: string;
  /** g mol⁻¹, or `null` for a row with no definite formula. */
  readonly molarMassGPerMol: number | null;
  readonly rayleigh: boolean;
  readonly absorber: boolean;
  readonly aerosolMaterial: boolean;
}

/** The fixture's rows, in its order. */
const ROWS: ReadonlyArray<RegistryRow> = registry.substances.map((row) => ({
  key: row.key,
  molarMassGPerMol: row.molar_mass_g_per_mol,
  rayleigh: row.rayleigh,
  absorber: row.absorber,
  aerosolMaterial: row.aerosol_material,
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
 * (`substance_aerosol_refuses_what_it_cannot_serve`; decision-p14-t54a-grain-density §2.6).
 */
const NO_GRAIN_DENSITY: ReadonlyArray<FileForm & { readonly reason: string }> = [
  { key: "H2SO4", phase: "liquid", variant: "84.5wt%", reason: "not the row's 75 wt% reference" },
  { key: "S8", phase: "liquid", variant: null, reason: "liquid sulphur's density not read" },
  { key: "H2S", phase: "solid", variant: null, reason: "solid H2S's density not read" },
  { key: "Ar", phase: "solid", variant: null, reason: "solid argon's density not read" },
  { key: "H2O", phase: "liquid", variant: "S35", reason: "seawater, not the row's pure water" },
];

/**
 * Whether a row has a grain density as a phase of a variant at some temperature: the reader gives
 * one at a node of the phase's member, of the file's variant.
 */
function hasGrainDensity(row: GrainDensityRow, form: FileForm): boolean {
  const pieces = form.phase === "solid" ? row.solid_grain_density : row.liquid_grain_density;
  return (pieces ?? []).some((piece) =>
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
 * rename in the generated type fails the type check, so that the hold cannot outlive its member.
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

/** What to do when the τ(550) hold's tripwire fires. */
const TAU_550_HOLD_LIFTED =
  "the protocol fixture now carries an aerosol inventory. If its modes carry the sim's τ(550) " +
  "(P14.T54.b, generator version 24: P14.T24.c's inventory with P14.T49.e's condensate " +
  "extinction, decision-bump-plan-after-22), build R08.T19's fifth row in this test's place: the " +
  "client's τ(550) for each of the fixture's modes within 1% of the record's. Otherwise restate " +
  "the hold's condition here and record why in plan R08 (R08.T19's records in 08-atmospheres.md)";

describe("τ(550), the record's against the client's", () => {
  /**
   * The hold on R08.T19's fifth row (decision-bump-plan-after-22, 2026-10-14; updated 2026-10-15).
   *
   * @remarks
   * P14.T54.b, at generator version 24, wires the aerosol inventory (P14.T24.c with T24.g) and the
   * vertical structure (T24.e). Its τ(550) is computed from R08's index files through P14.T49.e's
   * `condensate_extinction`, so that the record's τ(550) and the client's Mie agree; before 24
   * nothing on the record reads that table. The client has no generator version, so the hold
   * asserts what it can see: no message of the protocol fixture carries an `aerosols` member,
   * `BodySurfaceDto.aerosols`, which its doc says is absent until P14.T24.c.
   *
   * The Jupiter envelope's deck in the fixture's `body_detail_jupiter` carries a τ(550) of 8.0, but
   * the fixture's description calls the envelope's values illustrative. It is not the sim's, so the
   * row cannot run against it now.
   */
  it("is held until P14.T54.b's version 24 (decision-bump-plan-after-22): the protocol fixture carries no aerosols", () => {
    // The instruction rides with the members found, so that a failure prints it.
    expect({
      aerosolsMembers: aerosolsMembers(planetary, "planetary.json"),
      ifFound: TAU_550_HOLD_LIFTED,
    }).toEqual({ aerosolsMembers: [], ifFound: TAU_550_HOLD_LIFTED });
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
