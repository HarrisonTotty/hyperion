/**
 * The store, `docs/measurements/rendering/runs.v1.jsonl` (plan R12, Design note 1): append-only, one
 * record a line, each ID unique.
 */

import { foldSpikeResult, SPIKE_RESULTS_SCHEMA } from "./fold.ts";
import {
  parsePerfRecords,
  perfRecordId,
  serialisePerfRecord,
  validatePerfRecord,
} from "./record.ts";
import type { PerfRunRecord } from "./record.ts";

/** The store's text with records appended, and the IDs they took. */
export interface Appended {
  readonly text: string;
  readonly ids: ReadonlyArray<string>;
}

function withoutId(record: PerfRunRecord): string {
  return serialisePerfRecord({ ...record, id: "" });
}

/**
 * Appends records to the store's text: each takes the first free number of its date, machine and
 * scene (`…-1`, `…-2`), and one whose content, its ID aside, is already in the store is refused.
 *
 * @throws {@link PerfRecordError} if the store's text is not a valid store.
 * @throws Error naming the record already in the store.
 */
export function appendRecords(store: string, records: ReadonlyArray<PerfRunRecord>): Appended {
  const existing = parsePerfRecords(store);
  const ids = new Set(existing.map((r) => r.id));
  const contents = new Map(existing.map((r) => [withoutId(r), r.id]));
  const lines: string[] = [];
  const added: string[] = [];
  for (const record of records) {
    const already = contents.get(withoutId(record));
    if (already !== undefined) throw new Error(`the record is already in the store, as ${already}`);
    let n = 1;
    let id = perfRecordId(record.date, record.machine.name, record.scene.id, n);
    while (ids.has(id)) {
      n += 1;
      id = perfRecordId(record.date, record.machine.name, record.scene.id, n);
    }
    const numbered: PerfRunRecord = { ...record, id };
    ids.add(id);
    contents.set(withoutId(numbered), id);
    lines.push(serialisePerfRecord(numbered));
    added.push(id);
  }
  const base = store.length === 0 || store.endsWith("\n") ? store : `${store}\n`;
  return { text: base + lines.map((line) => `${line}\n`).join(""), ids: added };
}

/**
 * The records a file given to `just perf-record` holds: one of R05's descent-spike results files,
 * folded; one record as JSON; or records as JSON lines, as perf mode's `--perf-out` appends them.
 *
 * @throws {@link PerfRecordError} naming the field at fault.
 */
export function recordsOfFile(text: string): ReadonlyArray<PerfRunRecord> {
  let whole: unknown;
  try {
    whole = JSON.parse(text);
  } catch {
    return parsePerfRecords(text);
  }
  if (
    typeof whole === "object" &&
    whole !== null &&
    "schema" in whole &&
    whole.schema === SPIKE_RESULTS_SCHEMA
  ) {
    return [foldSpikeResult(whole)];
  }
  return [validatePerfRecord(whole)];
}
