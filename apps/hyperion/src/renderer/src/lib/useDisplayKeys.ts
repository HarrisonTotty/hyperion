import { useEffect } from "react";

import type { DisplayDefinition, DisplayId } from "./displays";

function hasModifier(event: KeyboardEvent): boolean {
  return event.ctrlKey || event.altKey || event.metaKey || event.shiftKey;
}

/**
 * Selects a display when its key is pressed, from any focus.
 *
 * @remarks
 * One `keydown` listener on the document, removed on cleanup. A display's key (a function key,
 * see {@link DisplayDefinition.key}) works even while a text field has focus, since typing never
 * produces it. Key repeats and presses with a modifier are ignored, so held keys and system
 * shortcuts pass through. A handled key's default action is prevented.
 *
 * @param onSelect - Called with the display whose key was pressed.
 */
export function useDisplayKeys(
  displays: ReadonlyArray<DisplayDefinition>,
  onSelect: (id: DisplayId) => void,
): void {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.repeat || hasModifier(event)) {
        return;
      }
      const display = displays.find((candidate) => candidate.key === event.key);
      if (display === undefined) {
        return;
      }
      event.preventDefault();
      onSelect(display.id);
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [displays, onSelect]);
}
