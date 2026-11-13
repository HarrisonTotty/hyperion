import { describe, expect, it } from "vitest";

import {
  renderedTauScale,
  SELECTION_SCALE_HOLD_FRAMES,
  SELECTION_SCALE_STEPS_PER_OCTAVE,
  SelectionScaleHold,
  selectionScaleStep,
} from "./selectionScale";

/** The M5's low window's floor at high: 720 of its 886 rows (perf-bodies-near-ground). */
const M5_FLOOR = [720 / 886, 1] as const;

describe("the floor's steps the lit selection's scale snaps to (perf-terrain-selection-scale)", () => {
  it("takes the bounds' max at or above it and the min, the lowest step, at or below it", () => {
    expect([
      selectionScaleStep(1, M5_FLOOR),
      selectionScaleStep(1.2, M5_FLOOR),
      selectionScaleStep(720 / 886, M5_FLOOR),
      selectionScaleStep(0.5, M5_FLOOR),
    ]).toEqual([1, 1, 720 / 886, 720 / 886]);
  });

  it("snaps a scale between them up to the next of the max over 2^(i/8)", () => {
    const steps = SELECTION_SCALE_STEPS_PER_OCTAVE;
    expect([
      selectionScaleStep(0.99, M5_FLOOR),
      selectionScaleStep(0.9, M5_FLOOR),
      selectionScaleStep(0.83, M5_FLOOR),
    ]).toEqual([1, 2 ** (-1 / steps), 2 ** (-2 / steps)]);
  });

  it("is never under the scale it snaps, nor more than a step over it, nor outside the bounds", () => {
    const bounds = [0.5, 1] as const;
    for (let scale = 0.5; scale <= 1; scale += 0.0037) {
      const step = selectionScaleStep(scale, bounds);
      expect(step).toBeGreaterThanOrEqual(scale);
      expect(step).toBeLessThanOrEqual(Math.min(1, scale * 2 ** (1 / 8) * (1 + 1e-12)));
      expect(step).toBeGreaterThanOrEqual(0.5);
    }
  });

  it("gives the steps of the bounds it is given: a narrower floor's lowest step is its own min", () => {
    expect(selectionScaleStep(0.85, [0.85, 1])).toBe(0.85);
    expect(selectionScaleStep(0.86, [0.85, 1])).toBe(2 ** (-1 / 8));
  });
});

/** `hold`'s steps over `steps`, one update each. */
function play(hold: SelectionScaleHold, steps: ReadonlyArray<number>): number[] {
  return steps.map((step) => hold.update(step));
}

describe("the lit selection's held scale (perf-terrain-selection-scale)", () => {
  it("takes its first step at once", () => {
    const hold = new SelectionScaleHold();
    expect([hold.held, hold.update(0.9), hold.held]).toEqual([null, 0.9, 0.9]);
  });

  it("takes a new step once it has stood for the hold's frames in a row, and not a frame before", () => {
    const hold = new SelectionScaleHold();
    hold.update(1);
    const held = play(hold, Array<number>(SELECTION_SCALE_HOLD_FRAMES).fill(0.8));
    expect(held.slice(0, -1).every((step) => step === 1)).toBe(true);
    expect(held.at(-1)).toBe(0.8);
  });

  it("starts the count again when the step moves on or comes back before the hold ends", () => {
    const hold = new SelectionScaleHold();
    hold.update(1);
    const short = SELECTION_SCALE_HOLD_FRAMES - 1;
    play(hold, Array<number>(short).fill(0.9));
    // Another step: its own count from its first frame.
    play(hold, Array<number>(short).fill(0.8));
    // Back to the held step: no count stands.
    play(hold, [1]);
    expect(play(hold, Array<number>(short).fill(0.8)).at(-1)).toBe(1);
    expect(hold.update(0.8)).toBe(0.8);
  });

  it("holds through a controller that moves its scale back and forth across a step", () => {
    const hold = new SelectionScaleHold();
    hold.update(1);
    const dither = Array.from({ length: 10 * SELECTION_SCALE_HOLD_FRAMES }, (_, n) =>
      Math.floor(n / 10) % 2 === 0 ? 0.917 : 1,
    );
    expect(new Set(play(hold, dither))).toEqual(new Set([1]));
  });

  it("takes the next update's step at once after a reset", () => {
    const hold = new SelectionScaleHold();
    hold.update(1);
    hold.reset();
    expect([hold.held, hold.update(0.7)]).toEqual([null, 0.7]);
  });
});

describe("the lit τ's factor from the rendered pixel (perf-terrain-selection-scale)", () => {
  it("is the CSS width over the rendered width where the rendered pixel is the larger", () => {
    // The M5's low window at DPR 1: 886 rows of CSS px rendered at 720 at the floor's lowest step.
    expect(
      renderedTauScale({ widthPx: 1640, heightPx: 886 }, { widthPx: 1333, heightPx: 720 }),
    ).toBe(1640 / 1333);
  });

  it("is 1 where the rendered pixel is the smaller, a Retina display's at its full scale", () => {
    expect([
      renderedTauScale({ widthPx: 1640, heightPx: 886 }, { widthPx: 1999, heightPx: 1080 }),
      renderedTauScale({ widthPx: 1640, heightPx: 886 }, { widthPx: 1640, heightPx: 886 }),
    ]).toEqual([1, 1]);
  });
});
