import { describe, expect, it } from "vitest";

import type { HandoverRegime } from "../../../../../preload/api";
import {
  HANDOVER_IN_WINDOW,
  HANDOVER_OUT_WINDOW,
  LockWatch,
  pairAdjacency,
  type PairedFrame,
  readBackWanted,
  SettleWatch,
  SwitchPairing,
} from "./handoverPairing";

/** Frames drawn in `regimes`, in order, each 1/60 s apart. */
function drawn(regimes: ReadonlyArray<HandoverRegime>): PairedFrame[] {
  return regimes.map((regime, index) => ({ index, regime, scriptS: 100 + index / 60 }));
}

/** The pairs `read` (frames taken in order) makes, by name and the indices of its frames. */
function pairsOf(read: ReadonlyArray<PairedFrame>): Array<[string, number, number]> {
  const pairing = new SwitchPairing<PairedFrame>();
  return read.flatMap((frame) =>
    pairing
      .take(frame)
      .map((pair): [string, number, number] => [pair.name, pair.before.index, pair.after.index]),
  );
}

/** The frame of `frames` at `index`, which must be there. */
function at(frames: ReadonlyArray<PairedFrame>, index: number): PairedFrame {
  const frame = frames[index];
  if (frame === undefined) {
    throw new Error(`no frame ${String(index)}`);
  }
  return frame;
}

describe("a look's pairs", () => {
  it("are the three switches of disc, figure, terrain and disc again", () => {
    const frames = drawn(["disc", "disc", "figure", "figure", "terrain", "terrain", "disc"]);
    expect(pairsOf(frames)).toEqual([
      ["disc-to-figure", 1, 2],
      ["disc-to-terrain", 1, 4],
      ["terrain-to-disc", 5, 6],
    ]);
  });

  it("take the terrain drawn at the regime's switch for both the switch and the first draw", () => {
    expect(pairsOf(drawn(["disc", "terrain"]))).toEqual([
      ["disc-to-figure", 0, 1],
      ["disc-to-terrain", 0, 1],
    ]);
  });

  it("pair the frames read, across a read-back skipped", () => {
    const frames = drawn(["disc", "disc", "figure"]);
    expect(pairsOf([at(frames, 0), at(frames, 2)])).toEqual([["disc-to-figure", 0, 2]]);
  });

  it("pair the hand-back with the terrain's last frame read, past a figure frame", () => {
    const frames = drawn(["disc", "terrain", "terrain", "figure", "disc"]);
    expect(pairsOf(frames).at(-1)).toEqual(["terrain-to-disc", 2, 4]);
  });

  it("pair nothing for a disc that never handed over", () => {
    expect(pairsOf(drawn(["disc", "disc", "disc"]))).toEqual([]);
  });

  it("fail a look whose first frame read is not the disc's", () => {
    const pairing = new SwitchPairing<PairedFrame>();
    pairing.take({ index: 0, regime: "figure", scriptS: 116.25 });
    expect(pairing.failures).toEqual([
      "the first frame read on the way in, at 116.25 s of the script, drew the figure, not the disc",
    ]);
  });
});

describe("a pair's adjacency", () => {
  const frames = drawn(["disc", "disc", "figure", "figure", "terrain"]);

  it("states consecutive frames of their regimes' last and first", () => {
    expect(pairAdjacency(frames, at(frames, 1), at(frames, 2))).toEqual({
      framesApart: 1,
      beforeWasLast: true,
      afterWasFirst: true,
    });
  });

  it("states a read-back skipped: the frames apart, and the disc's last not read", () => {
    expect(pairAdjacency(frames, at(frames, 0), at(frames, 2))).toEqual({
      framesApart: 2,
      beforeWasLast: false,
      afterWasFirst: true,
    });
  });

  it("states the terrain's first draw some frames after the disc's last", () => {
    expect(pairAdjacency(frames, at(frames, 1), at(frames, 4))).toEqual({
      framesApart: 3,
      beforeWasLast: true,
      afterWasFirst: true,
    });
  });
});

describe("the read-backs' windows", () => {
  it.each([
    ["in", HANDOVER_IN_WINDOW + 0.01, false],
    ["in", HANDOVER_IN_WINDOW, true],
    ["hold", 0.85, true],
    ["out", HANDOVER_OUT_WINDOW + 0.01, false],
  ] as const)(
    "on the %s part at %f of the range read back %s before any pair",
    (part, ratio, wanted) => {
      expect(readBackWanted(part, ratio, new Set())).toBe(wanted);
    },
  );

  it("close on the way in once the terrain's first draw is paired", () => {
    expect(readBackWanted("hold", 0.85, new Set(["disc-to-figure", "disc-to-terrain"]))).toBe(
      false,
    );
  });

  it("open on the way out only after the terrain's first draw is paired", () => {
    expect(readBackWanted("out", 1.1, new Set(["disc-to-figure", "disc-to-terrain"]))).toBe(true);
  });

  it("stay shut on the way out below their start", () => {
    expect(readBackWanted("out", HANDOVER_OUT_WINDOW - 0.01, new Set(["disc-to-terrain"]))).toBe(
      false,
    );
  });

  it("close on the way out once the hand-back is paired", () => {
    expect(
      readBackWanted("out", 1.1, new Set(["disc-to-figure", "disc-to-terrain", "terrain-to-disc"])),
    ).toBe(false);
  });
});

describe("the exposure's settling", () => {
  it("settles once AUTO stands within a step for the while", () => {
    const watch = new SettleWatch(3_000);
    watch.note({ ev100: 15.1, level: "AUTO" }, 0);
    watch.note({ ev100: 15.2, level: "AUTO" }, 1_000);
    expect(watch.note({ ev100: 15.1, level: "AUTO" }, 3_000)).toBe(true);
  });

  it("starts again when AUTO moves by more than a step", () => {
    const watch = new SettleWatch(3_000);
    watch.note({ ev100: 15.1, level: "AUTO" }, 0);
    watch.note({ ev100: 15.4, level: "AUTO" }, 1_000);
    expect(watch.note({ ev100: 15.4, level: "AUTO" }, 3_000)).toBe(false);
  });

  it("starts again on a reading that is not AUTO's", () => {
    const watch = new SettleWatch(3_000);
    watch.note({ ev100: 15.1, level: "AUTO" }, 0);
    watch.note({ ev100: 15.1, level: "MAN" }, 1_000);
    expect(watch.note({ ev100: 15.1, level: "AUTO" }, 3_000)).toBe(false);
  });
});

describe("the exposure's lock", () => {
  const LOCKED = "EV100 13.5 INHIBITED · OPERATOR";

  it("holds while every reading is the locked one", () => {
    const watch = new LockWatch(LOCKED);
    watch.note(LOCKED);
    watch.note(LOCKED);
    expect([watch.held, watch.endReading]).toEqual([true, LOCKED]);
  });

  it("does not hold once a reading drifts, even back", () => {
    const watch = new LockWatch(LOCKED);
    watch.note("EV100 13.6 AUTO");
    watch.note(LOCKED);
    expect(watch.held).toBe(false);
  });

  it("keeps the last reading as the end's", () => {
    const watch = new LockWatch(LOCKED);
    watch.note("EV100 13.6 AUTO");
    expect(watch.endReading).toBe("EV100 13.6 AUTO");
  });

  it("never holds a lock never taken", () => {
    const watch = new LockWatch(null);
    watch.note(LOCKED);
    expect([watch.held, watch.endReading]).toEqual([false, null]);
  });
});
