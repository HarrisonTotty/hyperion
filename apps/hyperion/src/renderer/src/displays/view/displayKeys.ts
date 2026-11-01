/**
 * `VIEW`'s display keys: single keys that change what the display shows, not a view's camera, and
 * so are not among the camera's `VIEW_SINGLE_KEYS` (the owner's request, 2026-11-01).
 */
import { isTextEntry } from "../../lib/textEntry";
import type { KeyPress } from "../../view/camera/keys";

/**
 * The `GRID` row's key, `5`, after the style's `4` on the same panel: it turns the reference grid of
 * the view it acts on `ON` or `OFF` while that view's row stands. A digit, as every single key of
 * the view is, the letters being the flight keys'.
 */
export const GRID_KEY = "5";

/** `MAXIMIZE VIEW`'s key, `6`, the next digit free in the view's key map. */
export const MAXIMIZE_KEY = "6";

/** What a display key asks: the `GRID` row's other setting, or `MAXIMIZE VIEW`'s. */
export type ViewDisplayKey = "grid" | "maximize";

/**
 * The display key of a press while `VIEW` is visible, or `null` where it has none or must pass
 * through: ignored in a text field, on a key repeat and with any modifier, Shift included, as the
 * camera's digits are (`viewKeyAction`).
 */
export function viewDisplayKey(press: KeyPress): ViewDisplayKey | null {
  if (
    press.repeat ||
    press.ctrlKey ||
    press.altKey ||
    press.metaKey ||
    press.shiftKey ||
    isTextEntry(press.target)
  ) {
    return null;
  }
  let key: ViewDisplayKey | null = null;
  if (press.key === GRID_KEY) {
    key = "grid";
  } else if (press.key === MAXIMIZE_KEY) {
    key = "maximize";
  }
  return key;
}

/**
 * The line under the key legend while `MAXIMIZE VIEW` is on, in the legend's form: where its key
 * acts, then the key and what it does, `MAXIMIZE VIEW: 6 SHOWS PANELS`.
 */
export const MAXIMIZED_KEYS = `MAXIMIZE VIEW: ${MAXIMIZE_KEY} SHOWS PANELS`;
