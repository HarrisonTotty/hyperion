/**
 * `VIEW`'s room (plan R10, R10.T12.b; decision-r10-view-block-room §3): the order in which the
 * plates give way, the slots held, each part's return with a line to spare, the room to open a
 * slot and the yield lines, against measures given in CSS px at 16 px to the rem unless a test
 * says otherwise.
 *
 * @remarks
 * At 16 px to the rem the insets are 8 px, so the plates have the stage's height less 16 px, the
 * plates stand 8 px apart, a line to spare is 17.5 px, and a 16 rem slot stands in 8 + 256 + 8 px.
 * The hysteresis holds only at an unchanged stage measured there, so its tests change the plates'
 * or the slots' heights on one stage; a resize or a change of interface scale is decided afresh.
 */
import { describe, expect, it } from "vitest";

import {
  INSTRUMENT_SLOT_REM,
  LINE_SPARE_REM,
  lowerYieldLine,
  type PlateMeasures,
  type RoomInput,
  roomToOpen,
  sameRoom,
  sameStage,
  type SlotRoomInput,
  slotWidthFits,
  type StageRoom,
  STUB_HEIGHT_REM,
  type ViewRoom,
  viewRoom,
  wholeRoom,
} from "./viewRoom";

/**
 * The plates at the stage's full width: an upper plate 440 px whole whose eleven lines before
 * `STARS` are 20 px each but `CAMERA`'s, the sixth, 40 px (its rate on a second line), and a lower
 * plate 120 px whole, 60 px with its readout given way and 40 px with its survey line too.
 *
 * @remarks
 * Its steps, by the stage's height: whole from 584 px; the readout given way from 524 px; the
 * survey line and `DATUM` from 504 px; the lines before `STARS` scrolling in 266 px plus the lines
 * in view (10 lines from 502 px down to `FRAME`–`CAMERA`'s 6 from 422 px); below that to one line
 * from 302 px; then the whole body.
 */
const FULL: PlateMeasures = {
  upper: {
    wholePx: 440,
    middleLinesPx: [20, 20, 20, 20, 20, 40, 20, 20, 20, 20, 20],
    pinnedLines: 6,
    bodyPx: 420,
    positionPx: 18,
  },
  lower: { wholePx: 120, readoutYieldedPx: 60, surveyYieldedPx: 40 },
};

/**
 * The same plates beside a standing slot, narrower and so taller: every line before `STARS` 40 px
 * but `CAMERA`'s 60 px. Beside a slot they stand by step 3 only from 542 px, scrolling
 * `FRAME`–`CAMERA`.
 */
const BESIDE: PlateMeasures = {
  upper: {
    wholePx: 640,
    middleLinesPx: [40, 40, 40, 40, 40, 60, 40, 40, 40, 40, 40],
    pinnedLines: 6,
    bodyPx: 620,
    positionPx: 18,
  },
  lower: { wholePx: 160, readoutYieldedPx: 80, surveyYieldedPx: 60 },
};

/** {@link FULL} with its upper plate `wholePx` tall whole, its lines before `STARS` as they are. */
function fullWith(wholePx: number): PlateMeasures {
  return { ...FULL, upper: { ...FULL.upper, wholePx, bodyPx: wholePx - 20 } };
}

/** {@link BESIDE} with its upper plate `wholePx` tall whole, its lines before `STARS` as they are. */
function besideWith(wholePx: number): PlateMeasures {
  return { ...BESIDE, upper: { ...BESIDE.upper, wholePx, bodyPx: wholePx - 20 } };
}

/** Plates that stand whole on any stage here: the slots' tests' own. */
const TINY: PlateMeasures = {
  upper: { wholePx: 100, middleLinesPx: [], pinnedLines: 0, bodyPx: 80, positionPx: 18 },
  lower: null,
};

const CLOSED: SlotRoomInput = { open: false, heightRem: INSTRUMENT_SLOT_REM.heightRem };
const OPEN: SlotRoomInput = { open: true, heightRem: INSTRUMENT_SLOT_REM.heightRem };

/** A slot as tall as R07.T19 measured one at 1920 × 1080: 254 px at 16 px to the rem. */
const MEASURED: SlotRoomInput = { open: true, heightRem: 254 / 16 };

/** A stage `heightPx` tall, 1280 px wide at 16 px to the rem unless given otherwise. */
function stageOf(heightPx: number, widthPx = 1280, remPx = 16): StageRoom {
  return { widthPx, heightPx, remPx };
}

/**
 * The decision's input on `stage`: both slots closed and the plates at {@link FULL}, measured at
 * `stage`, by default.
 */
function inputOn(stage: StageRoom | null, overrides: Partial<RoomInput> = {}): RoomInput {
  return {
    stage,
    measuredStage: stage,
    slots: [CLOSED, CLOSED],
    stubHeightRem: STUB_HEIGHT_REM,
    full: FULL,
    beside: null,
    ...overrides,
  };
}

/** The room decided on a 1280 px stage `heightPx` tall. */
function roomAt(
  heightPx: number,
  overrides: Partial<RoomInput> = {},
  previous: ViewRoom | null = null,
): ViewRoom {
  return viewRoom(inputOn(stageOf(heightPx), overrides), previous);
}

/** The slots' input with both open at 16 rem, their plates {@link TINY}. */
const BOTH_OPEN: Partial<RoomInput> = { slots: [OPEN, OPEN], full: TINY, beside: TINY };

describe("VIEW's room: the plates' parts in the ruled order (R10.T12.b)", () => {
  it("gives nothing way where both plates stand whole", () => {
    const room = roomAt(600);
    expect([room.step, room.readout, room.survey, room.upper]).toEqual([
      0,
      "stands",
      "stands",
      { kind: "whole" },
    ]);
  });

  it("gives the readout way first", () => {
    const room = roomAt(530);
    expect([room.step, room.readout, room.survey, room.upper]).toEqual([
      1,
      "yields",
      "stands",
      { kind: "whole" },
    ]);
  });

  it("gives the survey line's values and DATUM way next, the upper plate still whole", () => {
    const room = roomAt(510);
    expect([room.step, room.readout, room.survey, room.upper]).toEqual([
      2,
      "yields",
      "yields",
      { kind: "whole" },
    ]);
  });

  it("then scrolls the lines before STARS in a box exactly as tall as the first lines that fit", () => {
    // 266 px and eight lines, 180 px, fill 446 of the 454 px; nine would take 466.
    expect(roomAt(470).upper).toEqual({ kind: "middle", heightPx: 180, lines: 8 });
  });

  it("scrolls down to FRAME–CAMERA at step 3, CAMERA's two lines included", () => {
    const room = roomAt(422);
    expect([room.step, room.upper]).toEqual([3, { kind: "middle", heightPx: 140, lines: 6 }]);
  });

  it("scrolls below FRAME–CAMERA only at step 5", () => {
    const room = roomAt(421);
    expect([room.step, room.upper]).toEqual([5, { kind: "middle", heightPx: 100, lines: 5 }]);
  });

  it("scrolls down to one line at step 5", () => {
    const room = roomAt(302);
    expect([room.step, room.upper]).toEqual([5, { kind: "middle", heightPx: 20, lines: 1 }]);
  });

  it("scrolls the whole upper plate under its VIEW line in what the lower plate leaves at step 6", () => {
    // 301 px less the insets, the VIEW line and position line (38 px), the gap and the lower plate.
    const room = roomAt(301);
    expect([room.step, room.upper]).toEqual([6, { kind: "body", heightPx: 301 - 16 - 38 - 48 }]);
  });

  it("never shrinks the whole plate's scroll below one label row", () => {
    expect(roomAt(100).upper).toEqual({ kind: "body", heightPx: LINE_SPARE_REM * 16 });
  });

  it("scrolls no lines with a plate that has none before STARS, the whole plate scrolling instead", () => {
    const full: PlateMeasures = { ...TINY, upper: { ...TINY.upper, wholePx: 400, bodyPx: 380 } };
    expect(roomAt(300, { full }).upper.kind).toBe("body");
  });
});

describe("VIEW's room: each part returns only with a line to spare at an unchanged stage (R10.T12.b)", () => {
  // A 600 px stage: 584 px for the plates, which grow and shrink as their statements come and go.
  const ON = 600;

  /** The room on the 600 px stage with the upper plate `wholePx` tall whole. */
  function withUpper(wholePx: number, previous: ViewRoom | null = null): ViewRoom {
    return roomAt(ON, { full: fullWith(wholePx) }, previous);
  }

  it("gives the readout way as the plates grow past the edge, asking no room to spare", () => {
    // 456.1 + 8 + 120 px.
    expect(withUpper(456.1, withUpper(440)).readout).toBe("yields");
  });

  it("keeps a readout that gave way, as the plates shrink back, until it would stand with a line to spare", () => {
    const yielded = withUpper(457);
    expect([withUpper(438.6, yielded).readout, withUpper(438.5, yielded).readout]).toEqual([
      "yields",
      "stands",
    ]);
  });

  it("keeps the survey line given way until it would stand with a line to spare", () => {
    const yielded = withUpper(530);
    expect([withUpper(498.6, yielded).survey, withUpper(498.5, yielded).survey]).toEqual([
      "yields",
      "stands",
    ]);
  });

  it("keeps the lines scrolling until the plate would stand whole with a line to spare", () => {
    const scrolling = withUpper(560);
    expect([
      withUpper(518.6, scrolling).upper.kind,
      withUpper(518.5, scrolling).upper.kind,
    ]).toEqual(["middle", "whole"]);
  });

  it("keeps the whole plate scrolling until one line would stand in the lines' scroll with a line to spare", () => {
    const body = withUpper(760);
    expect([withUpper(720.6, body).step, withUpper(720.5, body).step]).toEqual([6, 5]);
  });

  it("lets the lines in view follow the room at once within a step", () => {
    expect(withUpper(560, withUpper(580)).upper).toEqual({
      kind: "middle",
      heightPx: 180,
      lines: 8,
    });
  });

  it("keeps the hysteresis on a stage within half a pixel of the one decided at", () => {
    const yielded = withUpper(457);
    expect(roomAt(ON + 0.4, { full: fullWith(440) }, yielded).readout).toBe("yields");
  });
});

describe("VIEW's room decided afresh (R10.T12.b)", () => {
  it("returns a readout at once on a resize to a stage that holds it", () => {
    // Given way at 583 px; at 590 px the plates stand whole in 568 of 574 px.
    expect(roomAt(590, {}, roomAt(583)).readout).toBe("stands");
  });

  it("returns a readout at once on a stage half a pixel taller than the one decided at", () => {
    const yielded = roomAt(600, { full: fullWith(457) });
    expect(roomAt(600.5, { full: fullWith(440) }, yielded).readout).toBe("stands");
  });

  it("returns a readout at once on a change of interface scale", () => {
    // At 15.9 px to the rem the plates stand whole in 567.95 of 584.1 px, a line to spare 17.4 px.
    const yielded = roomAt(600, { full: fullWith(457) });
    const rescaled = viewRoom(inputOn(stageOf(600, 1280, 15.9), { full: fullWith(440) }), yielded);
    expect(rescaled.readout).toBe("stands");
  });

  it("takes the stage it decided at as its basis on measures taken there", () => {
    expect(roomAt(600).basis).toEqual({ widthPx: 1280, heightPx: 600, remPx: 16 });
  });

  it("takes no basis on measures taken at another stage", () => {
    expect(roomAt(600, { measuredStage: stageOf(541, 814) }).basis).toBeNull();
  });

  it("takes no hysteresis from the room before on measures taken at another stage", () => {
    const yielded = roomAt(600, { full: fullWith(457) });
    const stale = roomAt(600, { full: fullWith(440), measuredStage: stageOf(541, 814) }, yielded);
    expect(stale.readout).toBe("stands");
  });

  it("takes no hysteresis from a room decided on stale measures", () => {
    const stale = roomAt(600, { full: fullWith(457), measuredStage: stageOf(541, 814) });
    // 440 + 8 + 120 px stand in 584 px, though not with a line to spare.
    expect(roomAt(600, { full: fullWith(440) }, stale).readout).toBe("stands");
  });

  it("takes no basis where the plates' stage is not measured", () => {
    expect(roomAt(600, { measuredStage: null }).basis).toBeNull();
  });
});

describe("VIEW's room: the slots held for the plates (step 4, R10.T12.b)", () => {
  const ONE_OPEN: Partial<RoomInput> = { slots: [OPEN, CLOSED], beside: BESIDE };

  it("keeps an open slot where the plates beside it stand by step 3", () => {
    const room = roomAt(560, ONE_OPEN);
    expect([room.slots, room.heldForPlates, room.step]).toEqual([["stands", "closed"], false, 3]);
  });

  it("decides the plates beside the slot while it stands", () => {
    expect(roomAt(560, ONE_OPEN).upper).toEqual({ kind: "middle", heightPx: 260, lines: 6 });
  });

  it("holds the open slots where step 3 beside them does not hold the plates", () => {
    const room = roomAt(530, ONE_OPEN);
    expect([room.slots, room.ownSlots, room.heldForPlates]).toEqual([
      ["held", "closed"],
      ["stands", "closed"],
      true,
    ]);
  });

  it("decides the plates at the stage's full width once the slots are held for them", () => {
    // Beside the slot they would scroll; at the full width the readout's giving way holds them.
    const room = roomAt(530, ONE_OPEN);
    expect([room.step, room.readout, room.upper]).toEqual([1, "yields", { kind: "whole" }]);
  });

  it("returns the slots held for the plates only once the plates beside them stand with a line to spare", () => {
    // At 530 px the plates beside the slot stand by step 3 from 628 px whole, with a line to spare
    // from 610.5 px.
    const held = roomAt(530, ONE_OPEN);
    expect([
      roomAt(530, { ...ONE_OPEN, beside: besideWith(610.6) }, held).heldForPlates,
      roomAt(530, { ...ONE_OPEN, beside: besideWith(610.5) }, held).heldForPlates,
    ]).toEqual([true, false]);
  });

  it("stands the slot beside those same plates where it was not held for them before", () => {
    expect(roomAt(530, { ...ONE_OPEN, beside: besideWith(610.6) }).heldForPlates).toBe(false);
  });

  it("returns the slots held for the plates at once on a resize to a stage that holds them", () => {
    // At 545 px the plates beside the slot stand in 526 of 529 px, not with a line to spare.
    expect(roomAt(545, ONE_OPEN, roomAt(530, ONE_OPEN)).heldForPlates).toBe(false);
  });

  it("stands the plates whole beside the slots before they are measured there", () => {
    const room = roomAt(300, { slots: [OPEN, CLOSED], beside: null });
    expect([room.slots, room.step, room.upper]).toEqual([
      ["stands", "closed"],
      0,
      { kind: "whole" },
    ]);
  });
});

describe("VIEW's room: the slots by their own room (R10.T12.b)", () => {
  it("stands INSTRUMENT 2 as its stub while INSTRUMENT 1 stands", () => {
    // INSTRUMENT 1 takes 272 px; INSTRUMENT 2 would take 264 more, its stub 72.
    expect(roomAt(400, BOTH_OPEN).slots).toEqual(["stands", "stub"]);
  });

  it("holds INSTRUMENT 2 whole where its stub does not fit below INSTRUMENT 1", () => {
    expect(roomAt(343, BOTH_OPEN).slots).toEqual(["stands", "held"]);
  });

  it("reckons the stub at its own height as last laid out", () => {
    expect(roomAt(343, { ...BOTH_OPEN, stubHeightRem: 3 }).slots).toEqual(["stands", "stub"]);
  });

  it("holds both slots where the stage is too narrow beside the plates' least width", () => {
    const room = viewRoom(inputOn(stageOf(720, 807), BOTH_OPEN), null);
    expect(room.slots).toEqual(["held", "held"]);
  });

  it("decides the plates at the full width, not as held for them, where no slot has its own room", () => {
    const room = viewRoom(inputOn(stageOf(720, 807), BOTH_OPEN), null);
    expect(room.heldForPlates).toBe(false);
  });

  it("stands INSTRUMENT 2 alone in its corner when INSTRUMENT 1 is closed", () => {
    expect(roomAt(272, { ...BOTH_OPEN, slots: [CLOSED, OPEN] }).slots).toEqual([
      "closed",
      "stands",
    ]);
  });

  it("stands a slot just opened where it fits exactly", () => {
    const before = roomAt(272, { ...BOTH_OPEN, slots: [CLOSED, CLOSED] });
    expect(roomAt(272, { ...BOTH_OPEN, slots: [OPEN, CLOSED] }, before).slots[0]).toBe("stands");
  });

  it("returns a held slot at an unchanged stage only with a line to spare", () => {
    // On a 300 px stage an 18 rem slot is held; it stands again from 16.65625 rem, 266.5 px.
    const tall = (heightRem: number): Partial<RoomInput> => ({
      ...BOTH_OPEN,
      slots: [{ open: true, heightRem }, CLOSED],
    });
    const held = roomAt(300, tall(18));
    expect([
      roomAt(300, tall(16.7), held).slots[0],
      roomAt(300, tall(16.65625), held).slots[0],
    ]).toEqual(["held", "stands"]);
  });

  it("stands a slot at an unchanged stage where it fits, though not with a line to spare, if it was not held", () => {
    const ONE: Partial<RoomInput> = {
      ...BOTH_OPEN,
      slots: [{ open: true, heightRem: 16.7 }, CLOSED],
    };
    expect(roomAt(300, ONE, roomAt(300, ONE)).slots[0]).toBe("stands");
  });

  it("returns INSTRUMENT 2 from its stub at an unchanged stage only with a line to spare", () => {
    // On a 545 px stage INSTRUMENT 2 at 17 rem stands as its stub; it stands whole again from
    // 15.46875 rem, 247.5 px, though at 16 rem it fits.
    const second = (heightRem: number): Partial<RoomInput> => ({
      ...BOTH_OPEN,
      slots: [OPEN, { open: true, heightRem }],
    });
    const stub = roomAt(545, second(17));
    expect([
      roomAt(545, second(16), stub).slots[1],
      roomAt(545, second(15.46875), stub).slots[1],
    ]).toEqual(["stub", "stands"]);
  });

  /** Both slots open at 16 rem, INSTRUMENT 1 `heightRem` tall. */
  function firstAt(heightRem: number): Partial<RoomInput> {
    return { ...BOTH_OPEN, slots: [{ open: true, heightRem }, OPEN] };
  }

  it("returns INSTRUMENT 2 held whole as its stub at an unchanged stage only with a line to spare", () => {
    // On a 343 px stage the stub does not fit below a 16 rem INSTRUMENT 1 (344 px); as INSTRUMENT 1
    // shrinks, the stub stands again with 17.5 px to spare from 14.84375 rem, 237.5 px.
    const held = roomAt(343, firstAt(16));
    expect([
      held.slots[1],
      roomAt(343, firstAt(14.9), held).slots[1],
      roomAt(343, firstAt(14.84375), held).slots[1],
    ]).toEqual(["held", "held", "stub"]);
  });

  it("stands INSTRUMENT 2 as its stub where it fits, though not with a line to spare, if it was not held", () => {
    expect(roomAt(343, firstAt(14.9)).slots[1]).toBe("stub");
  });

  it("holds a stub whole at an unchanged stage as soon as it no longer fits, asking no line to spare", () => {
    // On a 400 px stage the stub fits below INSTRUMENT 1 up to 19.5 rem, 312 px, to the pixel.
    const stub = roomAt(400, firstAt(16));
    expect([
      stub.slots[1],
      roomAt(400, firstAt(19.5), stub).slots[1],
      roomAt(400, firstAt(19.6), stub).slots[1],
    ]).toEqual(["stub", "stub", "held"]);
  });

  it("reckons a slot at its own height as it last stood", () => {
    const tall: SlotRoomInput = { open: true, heightRem: 20 };
    // 8 + 320 + 8 px.
    expect([
      roomAt(335.9, { ...BOTH_OPEN, slots: [tall, CLOSED] }).slots[0],
      roomAt(336, { ...BOTH_OPEN, slots: [tall, CLOSED] }).slots[0],
    ]).toEqual(["held", "stands"]);
  });

  it("takes a slot, three insets and the plates' least width, 50.5 rem, as wide enough", () => {
    expect([slotWidthFits(stageOf(720, 807.9)), slotWidthFits(stageOf(720, 808))]).toEqual([
      false,
      true,
    ]);
  });
});

describe("VIEW's room across an interface scale change (R10.T12.b)", () => {
  // A 1280 × 720 window's stage at 100% and at 150%, CSS px.
  const AT_100 = stageOf(541, 814, 16);
  const AT_150 = stageOf(381, 393, 24);
  const ONE: Partial<RoomInput> = { slots: [MEASURED, CLOSED], full: TINY, beside: TINY };
  const TWO: Partial<RoomInput> = { slots: [MEASURED, MEASURED], full: TINY, beside: TINY };

  it("holds INSTRUMENT 1 at 150% and stands it again at 100%", () => {
    const before = viewRoom(inputOn(AT_100, ONE), null);
    const scaled = viewRoom(inputOn(AT_150, ONE), before);
    const back = viewRoom(inputOn(AT_100, ONE), scaled);
    expect([before.slots[0], scaled.slots[0], back.slots[0]]).toEqual(["stands", "held", "stands"]);
  });

  it("holds INSTRUMENT 1 at 24 px to the rem on the 100% stage's px", () => {
    const before = viewRoom(inputOn(AT_100, ONE), null);
    expect(viewRoom(inputOn(stageOf(541, 814, 24), ONE), before).slots[0]).toBe("held");
  });

  it("returns INSTRUMENT 2 whole at 100%, where both stood with less than a line to spare, the stage decided afresh", () => {
    // Both stand in 532 of 541 px; held at 150%, they return as the 100% stage is decided anew.
    const before = viewRoom(inputOn(AT_100, TWO), null);
    const scaled = viewRoom(inputOn(AT_150, TWO), before);
    expect([before.slots, viewRoom(inputOn(AT_100, TWO), scaled).slots]).toEqual([
      ["stands", "stands"],
      ["stands", "stands"],
    ]);
  });

  it("keeps INSTRUMENT 2 as its stub at the unchanged 100% stage once it gave way there", () => {
    // It grows to 16.5 rem (542 px for both) and gives way, then shrinks back to the 254 px that
    // stood, which leaves no line to spare.
    const before = viewRoom(inputOn(AT_100, TWO), null);
    const grown = viewRoom(
      inputOn(AT_100, { ...TWO, slots: [MEASURED, { open: true, heightRem: 16.5 }] }),
      before,
    );
    expect([grown.slots[1], viewRoom(inputOn(AT_100, TWO), grown).slots[1]]).toEqual([
      "stub",
      "stub",
    ]);
  });
});

describe("VIEW's room to open a slot (R10.T12.b)", () => {
  /** Whether a closed slot may open on a 1280 px stage `heightPx` tall. */
  function canOpen(heightPx: number, overrides: Partial<RoomInput> = {}): boolean {
    const input = inputOn(stageOf(heightPx), { full: TINY, ...overrides });
    return roomToOpen(input, viewRoom(input, null));
  }

  it("offers room before the stage is laid out", () => {
    const input = inputOn(null);
    expect(roomToOpen(input, viewRoom(input, null))).toBe(true);
  });

  it("offers none where the stage is too narrow", () => {
    const input = inputOn(stageOf(720, 807), { full: TINY });
    expect(roomToOpen(input, viewRoom(input, null))).toBe(false);
  });

  it("offers a first slot room where 16 rem and the insets fit", () => {
    expect([canOpen(271.9), canOpen(272)]).toEqual([false, true]);
  });

  it("reckons the new slot as tall as the one that stands", () => {
    const tall: SlotRoomInput = { open: true, heightRem: 20 };
    // 8 + 320 + 8 + 320 + 8 px.
    expect([
      canOpen(663.9, { slots: [tall, CLOSED], beside: TINY }),
      canOpen(664, { slots: [tall, CLOSED], beside: TINY }),
    ]).toEqual([false, true]);
  });

  it("counts a held slot as nothing", () => {
    const tall: SlotRoomInput = { open: true, heightRem: 20 };
    // INSTRUMENT 1 is held at 300 px, and INSTRUMENT 2 takes 272.
    expect(canOpen(300, { slots: [tall, CLOSED], beside: TINY })).toBe(true);
  });

  it("counts a stub as its stub's height, the new slot as tall as the mean of those standing", () => {
    // 8 + (256 + 8) + (64 + 8) + 160 + 8 px.
    const input = inputOn(stageOf(512), BOTH_OPEN);
    const room = viewRoom(input, null);
    expect([room.slots, roomToOpen(input, room)]).toEqual([["stands", "stub"], true]);
  });

  it("offers none where the plates beside a slot would not stand by step 3", () => {
    expect([
      canOpen(530, { full: FULL, beside: BESIDE }),
      canOpen(560, { full: FULL, beside: BESIDE }),
    ]).toEqual([false, true]);
  });
});

describe("the lower plate's yield line (R10.T12.b)", () => {
  it("names the readout alone", () => {
    expect(lowerYieldLine(["readout"])).toBe(
      "NOT AVAILABLE: no room for GROUND AT LINE OF SIGHT at this window size",
    );
  });

  it("names the survey line, DATUM and the readout in the plate's order", () => {
    expect(lowerYieldLine(["readout", "datum", "survey"])).toBe(
      "NOT AVAILABLE: no room for SURVEY COVERAGE, DATUM or GROUND AT LINE OF SIGHT at this window size",
    );
  });

  it("names the survey line alone where it stands alone", () => {
    expect(lowerYieldLine(["survey"])).toBe(
      "NOT AVAILABLE: no room for SURVEY COVERAGE at this window size",
    );
  });

  it("refuses a line that holds nothing back", () => {
    expect(() => lowerYieldLine([])).toThrow(RangeError);
  });
});

describe("VIEW's room before anything is laid out (R10.T12.b)", () => {
  const SLOTS: RoomInput["slots"] = [OPEN, CLOSED];

  it("stands every open slot and gives nothing way, at no stage", () => {
    expect(wholeRoom(SLOTS)).toEqual({
      slots: ["stands", "closed"],
      ownSlots: ["stands", "closed"],
      heldForPlates: false,
      step: 0,
      readout: "stands",
      survey: "stands",
      upper: { kind: "whole" },
      basis: null,
    });
  });

  it("decides nothing before the stage is laid out", () => {
    expect(viewRoom(inputOn(null, { slots: SLOTS }), null)).toEqual(wholeRoom(SLOTS));
  });

  it("decides nothing before the plates are measured, on a stage too small for anything", () => {
    expect(viewRoom(inputOn(stageOf(100, 100), { slots: SLOTS, full: null }), null)).toEqual(
      wholeRoom(SLOTS),
    );
  });

  it("decides nothing on a stage with no rem", () => {
    expect(viewRoom(inputOn(stageOf(100, 100, 0), { slots: SLOTS }), null)).toEqual(
      wholeRoom(SLOTS),
    );
  });
});

describe("two rooms decided alike (R10.T12.b)", () => {
  it("finds a room decided again the same", () => {
    expect(sameRoom(roomAt(470), roomAt(470))).toBe(true);
  });

  it("tells apart two scrolls of one height holding different line counts", () => {
    const room = roomAt(470);
    expect(
      sameRoom(
        { ...room, upper: { kind: "middle", heightPx: 180, lines: 8 } },
        { ...room, upper: { kind: "middle", heightPx: 180, lines: 7 } },
      ),
    ).toBe(false);
  });

  it("tells apart a scroll of the lines from one of the whole plate", () => {
    const room = roomAt(470);
    expect(
      sameRoom(
        { ...room, upper: { kind: "middle", heightPx: 180, lines: 8 } },
        { ...room, upper: { kind: "body", heightPx: 180 } },
      ),
    ).toBe(false);
  });

  it("tells apart rooms alike decided at different stages", () => {
    const room = roomAt(600);
    expect(sameRoom(room, { ...room, basis: stageOf(601) })).toBe(false);
  });

  it("finds rooms alike decided at stages within half a pixel the same", () => {
    const room = roomAt(600);
    expect(sameRoom(room, { ...room, basis: stageOf(600.4) })).toBe(true);
  });

  it("tells a room decided on fresh measures from one decided on stale ones", () => {
    const room = roomAt(600);
    expect(sameRoom(room, { ...room, basis: null })).toBe(false);
  });

  it("finds two rooms with no basis the same", () => {
    expect(sameRoom(wholeRoom([OPEN, CLOSED]), wholeRoom([OPEN, CLOSED]))).toBe(true);
  });
});

describe("one stage (R10.T12.b)", () => {
  it("takes stages within half a pixel each way at one interface scale as one", () => {
    expect(sameStage(stageOf(600, 1280), stageOf(600.49, 1279.51))).toBe(true);
  });

  it("tells stages half a pixel apart in height apart", () => {
    expect(sameStage(stageOf(600), stageOf(600.5))).toBe(false);
  });

  it("tells stages half a pixel apart in width apart", () => {
    expect(sameStage(stageOf(600, 1280), stageOf(600, 1280.5))).toBe(false);
  });

  it("tells one stage's px at two interface scales apart", () => {
    expect(sameStage(stageOf(600, 1280, 16), stageOf(600, 1280, 24))).toBe(false);
  });

  it("takes no stage as one with any", () => {
    expect([sameStage(null, null), sameStage(stageOf(600), null)]).toEqual([false, false]);
  });
});
