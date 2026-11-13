/**
 * Which notes' reasons a view's plate shows opened (`LabelStatements`; the owner's request,
 * 2026-11-01): held above the plate, by the stage for the `PRIMARY` view and in its slot's state for
 * an instrument, so that the room is reckoned from the plates as they stand and an instrument held
 * for its room returns as it was.
 */
import { useCallback, useState } from "react";

/** Which notes' reasons stand opened on one view's plate, and how a press opens or folds them. */
export interface ReasonFolds {
  /** The notes whose reasons stand opened, by their text: none as the view opens. */
  readonly opened: ReadonlySet<string>;
  /** Opens a note's reasons, or folds them where they stand opened: its control's press. */
  readonly toggle: (note: string) => void;
}

/** No note's reasons opened. */
export const NONE_OPENED: ReadonlySet<string> = new Set();

/** `opened` with `note` opened, or folded where it was opened. */
export function toggledNote(opened: ReadonlySet<string>, note: string): ReadonlySet<string> {
  const next = new Set(opened);
  if (!next.delete(note)) {
    next.add(note);
  }
  return next;
}

/** One view's folds, every note's reasons folded as the caller mounts and kept while it stays. */
export function useReasonFolds(): ReasonFolds {
  const [opened, setOpened] = useState(NONE_OPENED);
  const toggle = useCallback((note: string): void => {
    setOpened((was) => toggledNote(was, note));
  }, []);
  return { opened, toggle };
}
