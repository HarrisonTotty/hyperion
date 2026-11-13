import type {
  SavedStill,
  StillFile,
  StillLabel,
  StillSaveAnswer,
  StillsApi,
} from "../../../preload/api";
import { readStillLabel } from "../view/stills/pngLabel";
import { parseStillLabel } from "../view/stills/stillLabel";

/**
 * The preload's still functions over memory, for tests (plan R11, R11.T10.c): it keeps what is
 * saved, as the main process's folder does, refusing a label not of the schema or a PNG whose
 * chunk is not its label, and lists newest first.
 *
 * @remarks
 * Each save takes the next ID, `20261113T000000000Z-` and the save's count in eight hex digits, so
 * that IDs sort as saves do. `answer`, where set, is what every later save answers instead.
 */
export class FakeStillsApi implements StillsApi {
  readonly saved: StillFile[] = [];
  /** Every call made, by name, in order. */
  readonly calls: string[] = [];
  /** What each save answers, instead of saving, where set. */
  answer: StillSaveAnswer | null = null;
  /** Whether a listing rejects, as a main process that cannot read its folder does. */
  failListing = false;

  saveStill(png: Uint8Array, label: StillLabel): Promise<StillSaveAnswer> {
    this.calls.push("saveStill");
    if (this.answer !== null) {
      return Promise.resolve(this.answer);
    }
    const checked = parseStillLabel(label);
    if (checked === null) {
      return Promise.resolve({ kind: "refused", reason: "malformed-label" });
    }
    const embedded = readStillLabel(png);
    if (embedded.kind !== "label") {
      return Promise.resolve({ kind: "refused", reason: "label-mismatch" });
    }
    const id = `20261113T000000000Z-${(this.saved.length + 1).toString(16).padStart(8, "0")}`;
    this.saved.push({ id, png: png.slice(), label: checked });
    return Promise.resolve({ kind: "saved", still: { id, label: checked } });
  }

  listStills(): Promise<ReadonlyArray<SavedStill>> {
    this.calls.push("listStills");
    if (this.failListing) {
      return Promise.reject(new Error("EACCES"));
    }
    return Promise.resolve(this.saved.map(({ id, label }) => ({ id, label })).toReversed());
  }

  readStill(id: string): Promise<StillFile | null> {
    this.calls.push("readStill");
    return Promise.resolve(this.saved.find((still) => still.id === id) ?? null);
  }
}
