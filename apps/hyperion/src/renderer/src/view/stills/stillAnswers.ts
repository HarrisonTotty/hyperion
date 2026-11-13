/**
 * The main process's still answers, read back (plan R11, R11.T10.c): each call's answer arrives
 * across IPC as `unknown`, and is narrowed here, its labels through the schema (`stillLabel.ts`),
 * before anything typed is made of it.
 *
 * @remarks
 * Pure, with no platform API: the client's preload (`preload/stillsApi.ts`) reads its answers
 * through {@link stillsOver}, and so does the smoke page (`smoke/stills.ts`), over its own
 * preload's raw calls, which is why this file is in `tsconfig.node.json`'s list. The two preloads
 * share no module of their own, since a module both imported would be bundled as a chunk of its
 * own, which a sandboxed preload cannot `require`.
 */

import type {
  SavedStill,
  StillFile,
  StillLabel,
  StillSaveAnswer,
  StillsApi,
} from "../../../../preload/api";
import { parseStillLabel } from "./stillLabel";

/** `value`'s field `key` where `value` is an object, else `undefined`. */
function fieldOf(value: unknown, key: string): unknown {
  return typeof value === "object" && value !== null ? Reflect.get(value, key) : undefined;
}

/** `value` as a saved still, its label of the schema, or `null`. */
export function readSavedStill(value: unknown): SavedStill | null {
  const id = fieldOf(value, "id");
  const label = parseStillLabel(fieldOf(value, "label"));
  return typeof id === "string" && label !== null ? { id, label } : null;
}

/**
 * The main process's answer to a save.
 *
 * @throws Error where it is none of the answers a save has.
 */
export function readSaveAnswer(value: unknown): StillSaveAnswer {
  const kind = fieldOf(value, "kind");
  const reason = fieldOf(value, "reason");
  const still = kind === "saved" ? readSavedStill(fieldOf(value, "still")) : null;
  if (kind === "saved" && still !== null) {
    return { kind, still };
  }
  if (
    kind === "refused" &&
    (reason === "oversized" ||
      reason === "malformed-label" ||
      reason === "busy" ||
      reason === "not-png" ||
      reason === "size-mismatch" ||
      reason === "label-mismatch")
  ) {
    return { kind, reason };
  }
  if (kind === "failed" && typeof reason === "string") {
    return { kind, reason };
  }
  throw new Error("the main process gave no answer to a still's save");
}

/**
 * The main process's listing: each still whose label is of the schema, the rest passed over.
 *
 * @throws Error where it is not a list.
 */
export function readStillList(value: unknown): ReadonlyArray<SavedStill> {
  if (!Array.isArray(value)) {
    throw new Error("the main process listed no stills");
  }
  return value.map(readSavedStill).filter((still) => still !== null);
}

/**
 * The main process's still read back, or `null` where it has none.
 *
 * @throws Error where it is neither `null` nor a still with its PNG.
 */
export function readStillFile(value: unknown): StillFile | null {
  if (value === null) {
    return null;
  }
  const saved = readSavedStill(value);
  const png = fieldOf(value, "png");
  if (saved === null || !(png instanceof Uint8Array)) {
    throw new Error("the main process read back no still");
  }
  return { id: saved.id, png, label: saved.label };
}

/** The raw calls on the stills' three channels, each answering what the main process gave. */
export interface StillCalls {
  readonly save: (png: Uint8Array, label: StillLabel) => Promise<unknown>;
  readonly list: () => Promise<unknown>;
  readonly read: (id: string) => Promise<unknown>;
}

/** The still functions over `calls`, each answer read back. */
export function stillsOver(calls: StillCalls): StillsApi {
  return {
    saveStill: async (png, label) => readSaveAnswer(await calls.save(png, label)),
    listStills: async () => readStillList(await calls.list()),
    readStill: async (id) => readStillFile(await calls.read(id)),
  };
}
