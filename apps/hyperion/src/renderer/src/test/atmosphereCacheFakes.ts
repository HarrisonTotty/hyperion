import type { BuiltAtmosphere, CachedTables } from "../view/atmosphere/AtmosphereCache";
import type { DiscReflectanceTable } from "../view/atmosphere/discTable";
import { type AtmosphereBody, type TablesPlan, tablesPlanOf } from "../view/atmosphere/tables";
import type { MsSourceTable } from "../view/atmosphere/thick/bake";
import type { DeckTables } from "../view/atmosphere/thick/deck";
import type { PolarisationTable } from "../view/atmosphere/thick/polarisation";

/**
 * A body's per-planet tables as a test's factory makes them for the atmosphere cache (plan R08,
 * R08.T10.a; R08.T14.c; R08.T14.f): what they were built from, the bytes they report, their plan
 * and body as `AtmosphereTables` gives them for an unwidened row, and every thick bake,
 * polarisation correction, disc table (R08.T16.b) and deck (R08.T15.a.2) given them.
 */
export class FakeTables implements CachedTables {
  readonly built: BuiltAtmosphere;
  readonly rowBytes: number;
  readonly plan: TablesPlan;
  readonly body: AtmosphereBody | null;
  readonly bottomRadiusM: number;
  /** Every bake {@link FakeTables.setThickTables} took, in order, `null` for one taken away. */
  readonly thick: Array<ReadonlyArray<MsSourceTable> | null> = [];
  /** What `setThickTables` throws while set, taking nothing. */
  thickThrows: Error | null = null;
  /**
   * Every correction {@link FakeTables.setPolarisationCorrection} took, in order, `null` for one
   * taken away.
   */
  readonly corrections: Array<ReadonlyArray<PolarisationTable> | null> = [];
  /** What `setPolarisationCorrection` throws while set, taking nothing. */
  correctionThrows: Error | null = null;
  /** Every disc table {@link FakeTables.setDiscReflectance} took, in order, `null` for one taken away. */
  readonly discs: Array<DiscReflectanceTable | null> = [];
  /** What `setDiscReflectance` throws while set, taking nothing. */
  discThrows: Error | null = null;
  /** Every deck {@link FakeTables.setDeckTables} took, in order, `null` for one taken away (R08.T15.a.2). */
  readonly decks: Array<ReadonlyArray<DeckTables> | null> = [];
  disposed = false;

  constructor(built: BuiltAtmosphere, rowBytes: number) {
    this.built = built;
    this.rowBytes = rowBytes;
    this.plan = tablesPlanOf(built.body, built.bottomRadiusM);
    this.body = built.body;
    this.bottomRadiusM = built.bottomRadiusM;
  }

  /** The bake the tables hold: the last given, or `null`. */
  get thickHeld(): ReadonlyArray<MsSourceTable> | null {
    return this.thick.at(-1) ?? null;
  }

  setThickTables(bands: ReadonlyArray<MsSourceTable> | null): void {
    if (this.thickThrows !== null) {
      throw this.thickThrows;
    }
    this.thick.push(bands);
  }

  /** The correction the tables hold: the last given, or `null`. */
  get correctionHeld(): ReadonlyArray<PolarisationTable> | null {
    return this.corrections.at(-1) ?? null;
  }

  setPolarisationCorrection(bands: ReadonlyArray<PolarisationTable> | null): void {
    if (this.correctionThrows !== null) {
      throw this.correctionThrows;
    }
    this.corrections.push(bands);
  }

  setDiscReflectance(table: DiscReflectanceTable | null): void {
    if (this.discThrows !== null) {
      throw this.discThrows;
    }
    this.discs.push(table);
  }

  setDeckTables(bands: ReadonlyArray<DeckTables> | null): void {
    this.decks.push(bands);
  }

  dispose(): void {
    this.disposed = true;
  }
}
