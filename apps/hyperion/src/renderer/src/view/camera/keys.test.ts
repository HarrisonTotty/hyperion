import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import {
  flightInput,
  flightKey,
  flightKeyAction,
  flightKeyReleased,
  type KeyPress,
  viewKeyAction,
} from "./keys";

function press(key: string, overrides: Partial<KeyPress> = {}): KeyPress {
  return {
    key,
    ctrlKey: false,
    altKey: false,
    metaKey: false,
    shiftKey: false,
    repeat: false,
    target: document.body,
    ...overrides,
  };
}

describe("the view's single keys", () => {
  it("select the presets, step targets and step the field of view", () => {
    expect(viewKeyAction(press("1"))).toEqual({ kind: "preset", preset: "seat" });
    expect(viewKeyAction(press("2"))).toEqual({ kind: "preset", preset: "chase" });
    expect(viewKeyAction(press("3"))).toEqual({ kind: "preset", preset: "free" });
    expect(viewKeyAction(press("]"))).toEqual({ kind: "target", step: 1 });
    expect(viewKeyAction(press("["))).toEqual({ kind: "target", step: -1 });
    expect(viewKeyAction(press("+"))).toEqual({ kind: "fov", step: -1 });
    expect(viewKeyAction(press("="))).toEqual({ kind: "fov", step: -1 });
    expect(viewKeyAction(press("-"))).toEqual({ kind: "fov", step: 1 });
  });

  it("ignore presses with a modifier", () => {
    const modified = [
      press("1", { ctrlKey: true }),
      press("=", { ctrlKey: true }),
      press("1", { altKey: true }),
      press("1", { metaKey: true }),
      press("1", { shiftKey: true }),
    ];
    expect(modified.map(viewKeyAction)).toEqual([null, null, null, null, null]);
  });

  it("ignore key repeats", () => {
    expect(viewKeyAction(press("1", { repeat: true }))).toBeNull();
  });

  it("ignore presses in a text field", () => {
    expect(viewKeyAction(press("1", { target: document.createElement("input") }))).toBeNull();
  });

  it("allow Shift with +, which some layouts type with it", () => {
    expect(viewKeyAction(press("+", { shiftKey: true }))).toEqual({ kind: "fov", step: -1 });
  });

  it("leave the flight letters to the canvas", () => {
    for (const key of ["w", "a", "s", "d", "q", "e", "r", "f", "ArrowUp"]) {
      expect(viewKeyAction(press(key))).toBeNull();
    }
  });
});

describe("the flight keys", () => {
  it("hold translation and rotation axes, whatever the letter's case", () => {
    expect(flightKey(press("w"))).toBe("w");
    expect(flightKey(press("W"))).toBe("w");
    expect(flightKey(press("ArrowLeft"))).toBe("ArrowLeft");
    expect(flightKey(press("1"))).toBeNull();
  });

  it("ignore presses with a modifier", () => {
    expect([
      flightKey(press("w", { ctrlKey: true })),
      flightKey(press("w", { shiftKey: true })),
      flightKeyAction(press("PageUp", { altKey: true })),
    ]).toEqual([null, null, null]);
  });

  it("ignore presses in a text field", () => {
    expect(flightKey(press("w", { target: document.createElement("textarea") }))).toBeNull();
  });

  it("release a held key whatever the modifiers at its release", () => {
    const held = new Set<string>();
    const down = flightKey(press("w"));
    if (down !== null) {
      held.add(down);
    }
    const up = flightKeyReleased(press("W", { shiftKey: true, ctrlKey: true }));
    if (up !== null) {
      held.delete(up);
    }
    expect([...held]).toEqual([]);
  });

  it("step the commanded rate", () => {
    expect(flightKeyAction(press("PageUp"))).toEqual({ kind: "rate", step: 1 });
    expect(flightKeyAction(press("PageDown"))).toEqual({ kind: "rate", step: -1 });
  });

  it("sum the held keys into the free camera's input, opposite keys cancelling", () => {
    expect(flightInput(new Set(["w", "d", "ArrowUp", "q"]))).toEqual({
      translate: vec3(1, 0, -1),
      rotate: vec3(1, 0, 1),
    });
    expect(flightInput(new Set(["w", "s", "ArrowLeft", "ArrowRight"]))).toEqual({
      translate: vec3(0, 0, 0),
      rotate: vec3(0, 0, 0),
    });
  });
});
