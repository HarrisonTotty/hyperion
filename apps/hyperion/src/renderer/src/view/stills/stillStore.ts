/**
 * The client's saved stills (plan R11, R11.T10.c, Design note 17), over the preload's `saveStill`,
 * `listStills` and `readStill`: a still is saved with its label embedded in its PNG and as its
 * sidecar, listed for `StillList`, and read back with the PNG's label checked against the sidecar.
 *
 * @remarks
 * An external store for `useSyncExternalStore`: its snapshot is the listing as it stands, which a
 * save refreshes. A still is the client's own artefact and never ship state, so nothing here
 * reaches the server.
 */

import type {
  SavedStill,
  StillFile,
  StillLabel,
  StillSaveAnswer,
  StillsApi,
} from "../../../../preload/api";
import { readStillLabel, withStillLabel } from "./pngLabel";
import { stillLabelText } from "./stillLabel";

/** The saved stills, as the store last listed them. */
export type StillListState =
  /** The first listing has not answered. */
  | { readonly kind: "listing" }
  /** Newest first. */
  | { readonly kind: "listed"; readonly stills: ReadonlyArray<SavedStill> }
  /** The listing failed: the main process's words. */
  | { readonly kind: "failed"; readonly reason: string };

/** What reading a saved still back gave. */
export type StillReadOutcome =
  /** The still, its PNG's label equal to its sidecar's, text for text. */
  | { readonly kind: "read"; readonly still: StillFile }
  /** No still of the ID is saved, or its files are gone. */
  | { readonly kind: "gone" }
  /** The PNG's `hyperion-still` chunk is missing, malformed, or not the sidecar's label. */
  | { readonly kind: "mismatch"; readonly reason: string };

/** A rejection's words. */
function reasonOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** The client's saved stills over the preload's functions. */
export class StillStore {
  readonly #api: StillsApi;
  readonly #listeners = new Set<() => void>();
  #state: StillListState = { kind: "listing" };
  /** Counts listings asked for, so that only the latest one's answer is kept. */
  #generation = 0;

  constructor(api: StillsApi) {
    this.#api = api;
  }

  /** Subscribes to the listing's changes; returns the unsubscription. */
  readonly subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  /** The listing as it stands. */
  readonly getSnapshot = (): StillListState => this.#state;

  /**
   * Lists the saved stills again; the latest listing asked for is the one kept. Never rejects: a
   * failed listing is the `failed` state.
   */
  async refresh(): Promise<void> {
    this.#generation += 1;
    const generation = this.#generation;
    let next: StillListState;
    try {
      next = { kind: "listed", stills: await this.#api.listStills() };
    } catch (error: unknown) {
      next = { kind: "failed", reason: reasonOf(error) };
    }
    if (generation === this.#generation) {
      this.#set(next);
    }
  }

  /**
   * Saves a still: `png`, as the encoder made it, with `label` embedded as its `hyperion-still`
   * chunk, and `label` as its sidecar; a saved still refreshes the listing.
   *
   * @throws RangeError where `png` is not a PNG of the label's size, or already holds a label
   *   (`withStillLabel`): the job's bug.
   * @throws Error, as a rejection, where the call itself fails.
   */
  async save(png: Uint8Array, label: StillLabel): Promise<StillSaveAnswer> {
    const answer = await this.#api.saveStill(withStillLabel(png, label), label);
    if (answer.kind === "saved") {
      await this.refresh();
    }
    return answer;
  }

  /**
   * Reads the still `id` back, its PNG's label checked against its sidecar's: they are equal, text
   * for text, or the outcome says why not.
   *
   * @throws Error, as a rejection, where the call itself fails.
   */
  async read(id: string): Promise<StillReadOutcome> {
    const still = await this.#api.readStill(id);
    if (still === null) {
      return { kind: "gone" };
    }
    const embedded = readStillLabel(still.png);
    let outcome: StillReadOutcome;
    switch (embedded.kind) {
      case "none":
        outcome = { kind: "mismatch", reason: "the PNG holds no label" };
        break;
      case "malformed":
        outcome = { kind: "mismatch", reason: embedded.reason };
        break;
      case "label":
        outcome =
          embedded.text === stillLabelText(still.label)
            ? { kind: "read", still }
            : { kind: "mismatch", reason: "the PNG's label is not the sidecar's" };
        break;
    }
    return outcome;
  }

  #set(state: StillListState): void {
    this.#state = state;
    for (const listener of this.#listeners) {
      listener();
    }
  }
}
