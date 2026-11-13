import { createContext, useContext, useId, useState } from "react";

/**
 * Which field of a panel holds the one entry refusal it shows, and how a field takes it: the
 * `PLANET VIEWER`'s `INPUTS` panel provides it, so that a refusal under one field drops the one
 * under another (R15.T6's review: nothing cut at 1280 × 720 by refusals standing at once).
 */
export interface EntryRefusals {
  /** The field whose refusal stands, by its {@link useEntryDraft} ID, or `null`. */
  readonly holder: string | null;
  /** Makes `field` the holder, or none. */
  readonly hold: (field: string | null) => void;
}

/** The panel's {@link EntryRefusals}; `null` outside such a panel, where each field keeps its own. */
export const EntryRefusalContext = createContext<EntryRefusals | null>(null);

/** A field's entry as it is typed, and its refusal, from {@link useEntryDraft}. */
export interface EntryDraft {
  /** What was typed and not yet entered, or `null`, the field showing its value. */
  readonly draft: string | null;
  /** Why what was typed was refused, as the field states it, or `null`. */
  readonly refusal: string | null;
  /** Takes what is typed, dropping the refusal. */
  readonly type: (text: string) => void;
  /** Drops what was typed and its refusal, as `Escape` does. */
  readonly drop: () => void;
  /** Refuses what was typed, which stays in the field. */
  readonly refuse: (refusal: string) => void;
}

/**
 * A text field's draft and refusal on the `PLANET VIEWER`, dropped, as `Escape` drops them, while
 * `folded` hides the field, and, under an {@link EntryRefusalContext}, once another field's entry
 * is refused, so that one refusal stands at a time.
 */
export function useEntryDraft(folded: boolean): EntryDraft {
  const field = useId();
  const refusals = useContext(EntryRefusalContext);
  const [draft, setDraft] = useState<string | null>(null);
  const [refusal, setRefusal] = useState<string | null>(null);
  const superseded = refusal !== null && refusals !== null && refusals.holder !== field;
  // Adjusted as it renders (React's pattern for state that follows a prop or a context), so that
  // what a fold hides, or a newer refusal supersedes, never stands again when it is shown.
  if ((folded && (draft !== null || refusal !== null)) || superseded) {
    setDraft(null);
    setRefusal(null);
  }
  const release = (): void => {
    if (refusals !== null && refusals.holder === field) {
      refusals.hold(null);
    }
  };
  return {
    draft,
    refusal,
    type: (text) => {
      setDraft(text);
      setRefusal(null);
      release();
    },
    drop: () => {
      setDraft(null);
      setRefusal(null);
      release();
    },
    refuse: (next) => {
      setRefusal(next);
      refusals?.hold(field);
    },
  };
}
