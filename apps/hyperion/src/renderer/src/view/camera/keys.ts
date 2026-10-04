import { vec3 } from "../../geometry/vec3";
import { isTextEntry } from "../../lib/textEntry";
import type { FreeCameraInput } from "./freeCamera";
import { STYLE_TOGGLE_KEY } from "../photoreal/style";
import type { CameraPreset, RenderStyle } from "./state";

/**
 * What the view's keys ask of its camera (plan R02, R02.T9.c).
 *
 * @remarks
 * `fov` −1 narrows the field of view a step and +1 widens it; `rate` +1 raises the free camera's
 * commanded rate a step.
 */
export type ViewKeyAction =
  | { readonly kind: "preset"; readonly preset: CameraPreset }
  | { readonly kind: "target"; readonly step: -1 | 1 }
  | { readonly kind: "fov"; readonly step: -1 | 1 }
  | { readonly kind: "rate"; readonly step: -1 | 1 }
  /** The view's style: one of R07's styles, or `toggle` to the other (the key `4`). */
  | { readonly kind: "style"; readonly style: RenderStyle | "toggle" };

/** The parts of a `KeyboardEvent` the bindings read. */
export interface KeyPress {
  /** `KeyboardEvent.key`. */
  readonly key: string;
  /** Whether Ctrl was held. */
  readonly ctrlKey: boolean;
  /** Whether Alt was held. */
  readonly altKey: boolean;
  /** Whether Meta was held. */
  readonly metaKey: boolean;
  /** Whether Shift was held. */
  readonly shiftKey: boolean;
  /** Whether the press is a key repeat. */
  readonly repeat: boolean;
  /** The element the event was aimed at. */
  readonly target: EventTarget | null;
}

/**
 * The view's single keys, which act while `VIEW` is visible from any focus but a text field: `1`
 * `SEAT`, `2` `CHASE`, `3` `FREE`, `4` the other style (R07.T8.a, `STYLE_TOGGLE_KEY`), `]` and
 * `[` the next and previous target, `+` (or `=`) a narrower field of view and `-` a wider one.
 *
 * @remarks
 * Split as plan 05's design note D3 splits the chart's. Digits and brackets leave the letters to
 * the flight keys, which act only on the focused canvas.
 */
export const VIEW_SINGLE_KEYS: Readonly<Record<string, ViewKeyAction>> = {
  "1": { kind: "preset", preset: "seat" },
  "2": { kind: "preset", preset: "chase" },
  "3": { kind: "preset", preset: "free" },
  [STYLE_TOGGLE_KEY]: { kind: "style", style: "toggle" },
  "]": { kind: "target", step: 1 },
  "[": { kind: "target", step: -1 },
  "+": { kind: "fov", step: -1 },
  "=": { kind: "fov", step: -1 },
  "-": { kind: "fov", step: 1 },
};

/** The flight keys' discrete actions, on the focused canvas: `PageUp` and `PageDown` step the rate. */
export const FLIGHT_ACTION_KEYS: Readonly<Record<string, ViewKeyAction>> = {
  PageUp: { kind: "rate", step: 1 },
  PageDown: { kind: "rate", step: -1 },
};

/** One held flight key's command: an axis of translation or rotation and its sense. */
interface FlightAxis {
  readonly motion: "translate" | "rotate";
  readonly axis: "x" | "y" | "z";
  readonly sense: -1 | 1;
}

/**
 * The held flight keys, on the focused canvas: `W` and `S` forward and back, `A` and `D` left and
 * right, `R` and `F` up and down; the arrows pitch and yaw; `Q` and `E` roll left and right.
 */
export const FLIGHT_AXIS_KEYS: Readonly<Record<string, FlightAxis>> = {
  w: { motion: "translate", axis: "z", sense: -1 },
  s: { motion: "translate", axis: "z", sense: 1 },
  a: { motion: "translate", axis: "x", sense: -1 },
  d: { motion: "translate", axis: "x", sense: 1 },
  r: { motion: "translate", axis: "y", sense: 1 },
  f: { motion: "translate", axis: "y", sense: -1 },
  ArrowUp: { motion: "rotate", axis: "x", sense: 1 },
  ArrowDown: { motion: "rotate", axis: "x", sense: -1 },
  ArrowLeft: { motion: "rotate", axis: "y", sense: 1 },
  ArrowRight: { motion: "rotate", axis: "y", sense: -1 },
  q: { motion: "rotate", axis: "z", sense: 1 },
  e: { motion: "rotate", axis: "z", sense: -1 },
};

/** A letter key's binding name: the letter in lower case, so that Caps Lock does not matter. */
function bindingKey(key: string): string {
  return key.length === 1 ? key.toLowerCase() : key;
}

/**
 * The single-key action of a press while `VIEW` is visible, or `null` where it has none or must
 * pass through.
 *
 * @remarks
 * Ignored in a text field, on a key repeat, and with Ctrl, Alt or Meta (so that Ctrl with `=` or
 * `-` stays the interface scale's); Shift is allowed only with `+`, which some layouts type with
 * it, as plan 05's chart allows it.
 */
export function viewKeyAction(press: KeyPress): ViewKeyAction | null {
  if (press.repeat || press.ctrlKey || press.altKey || press.metaKey || isTextEntry(press.target)) {
    return null;
  }
  if (press.shiftKey && press.key !== "+") {
    return null;
  }
  return Object.hasOwn(VIEW_SINGLE_KEYS, press.key) ? (VIEW_SINGLE_KEYS[press.key] ?? null) : null;
}

/**
 * The flight key a `keydown` on the focused canvas holds, or `null` where it is not one, a modifier
 * is held or the press lands in a text field.
 *
 * @returns The binding name to add to the held set.
 */
export function flightKey(press: KeyPress): string | null {
  if (
    press.ctrlKey ||
    press.altKey ||
    press.metaKey ||
    press.shiftKey ||
    isTextEntry(press.target)
  ) {
    return null;
  }
  return flightKeyReleased(press);
}

/**
 * The flight key a `keyup` releases, or `null` where it is not one.
 *
 * @remarks
 * A release is honoured whatever the modifiers and focus, so that a key pressed before Shift or a
 * change of focus is never left held. The display also clears the held set when the canvas loses
 * focus, since the `keyup` then goes elsewhere.
 *
 * @returns The binding name to remove from the held set.
 */
export function flightKeyReleased(press: Pick<KeyPress, "key">): string | null {
  const key = bindingKey(press.key);
  return Object.hasOwn(FLIGHT_AXIS_KEYS, key) ? key : null;
}

/** The discrete flight action of a press on the focused canvas (the rate keys), or `null`. */
export function flightKeyAction(press: KeyPress): ViewKeyAction | null {
  if (
    press.ctrlKey ||
    press.altKey ||
    press.metaKey ||
    press.shiftKey ||
    isTextEntry(press.target)
  ) {
    return null;
  }
  return Object.hasOwn(FLIGHT_ACTION_KEYS, press.key)
    ? (FLIGHT_ACTION_KEYS[press.key] ?? null)
    : null;
}

/**
 * The free camera's input from the flight keys held: each axis the sum of its keys' senses, so that
 * opposite keys cancel.
 */
export function flightInput(held: ReadonlySet<string>): FreeCameraInput {
  const sums = {
    translate: { x: 0, y: 0, z: 0 },
    rotate: { x: 0, y: 0, z: 0 },
  };
  for (const key of held) {
    const binding = Object.hasOwn(FLIGHT_AXIS_KEYS, key) ? FLIGHT_AXIS_KEYS[key] : undefined;
    if (binding !== undefined) {
      sums[binding.motion][binding.axis] += binding.sense;
    }
  }
  return {
    translate: vec3(sums.translate.x, sums.translate.y, sums.translate.z),
    rotate: vec3(sums.rotate.x, sums.rotate.y, sums.rotate.z),
  };
}
