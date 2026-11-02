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
  columnsFit,
  INSTRUMENT_SLOT_REM,
  leastStepOf,
  LINE_SPARE_REM,
  type LowerPlateMeasures,
  lowerYieldLine,
  type PlateArrangement,
  type PlateMeasures,
  PLATES_COVER_CAP,
  type RoomInput,
  roomToOpen,
  sameRoom,
  sameStage,
  type SlotRoomInput,
  slotWidthFits,
  type StageRoom,
  STUB_HEIGHT_REM,
  type UpperPlateMeasures,
  type ViewRoom,
  viewRoom,
  wholeRoom,
  type YieldingPart,
} from "./viewRoom";

/**
 * The plates' width in the tests that are not the cap's, 300 px: under it no test's plates but the
 * floor's cover half the 1280 px stage, so that the cap leaves them as the room's own steps do.
 */
const PLATE_WIDTH_PX = 300;

/** A lower plate's widths, every form {@link PLATE_WIDTH_PX} wide. */
const LOWER_WIDTHS = {
  wholeWidthPx: PLATE_WIDTH_PX,
  readoutYieldedWidthPx: PLATE_WIDTH_PX,
  surveyYieldedWidthPx: PLATE_WIDTH_PX,
} as const;

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
    widthPx: PLATE_WIDTH_PX,
  },
  lower: { wholePx: 120, readoutYieldedPx: 60, surveyYieldedPx: 40, passing: [], ...LOWER_WIDTHS },
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
    widthPx: PLATE_WIDTH_PX,
  },
  lower: { wholePx: 160, readoutYieldedPx: 80, surveyYieldedPx: 60, passing: [], ...LOWER_WIDTHS },
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
  upper: {
    wholePx: 100,
    middleLinesPx: [],
    pinnedLines: 0,
    bodyPx: 80,
    positionPx: 18,
    widthPx: PLATE_WIDTH_PX,
  },
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
 * `stage`, stacked, by default.
 */
function inputOn(stage: StageRoom | null, overrides: Partial<RoomInput> = {}): RoomInput {
  return {
    stage,
    measuredStage: stage,
    slots: [CLOSED, CLOSED],
    stubHeightRem: STUB_HEIGHT_REM,
    full: FULL,
    beside: null,
    columns: false,
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

/** `plates` with these lower parts' widest runs passing their plate (R10.T12.c). */
function passing(plates: PlateMeasures, parts: ReadonlyArray<YieldingPart>): PlateMeasures {
  const { lower } = plates;
  if (lower === null) {
    throw new Error("the plates hold no lower plate");
  }
  return { ...plates, lower: { ...lower, passing: parts } };
}

/** The room decided on a 1280 px stage `heightPx` tall, the readout's widest run passing its plate. */
function readoutPassingAt(heightPx: number): ViewRoom {
  return roomAt(heightPx, { full: passing(FULL, ["readout"]) });
}

describe("VIEW's room: the width test (R10.T12.c; decision-r10-t12a-scope §7 item 4)", () => {
  it("gives the readout way where its widest run passes the plate, on a stage tall enough for it", () => {
    const room = readoutPassingAt(600);
    expect([room.step, room.readout, room.survey, room.upper]).toEqual([
      1,
      "yields",
      "stands",
      { kind: "whole" },
    ]);
  });

  it.each<readonly [YieldingPart]>([["survey"], ["datum"]])(
    "gives the survey line's values and DATUM way, the readout with them, where %s's run passes the plate",
    (part) => {
      const room = roomAt(600, { full: passing(FULL, [part]) });
      expect([room.step, room.readout, room.survey]).toEqual([2, "yields", "yields"]);
    },
  );

  it("still scrolls the upper plate where the heights call for it past the width's step", () => {
    const room = readoutPassingAt(470);
    expect([room.step, room.upper]).toEqual([3, { kind: "middle", heightPx: 180, lines: 8 }]);
  });

  it("gives the readout way beside a slot it passes, the slot standing, rather than holding the slot", () => {
    const room = roomAt(700, {
      slots: [OPEN, CLOSED],
      beside: passing(besideWith(400), ["readout"]),
    });
    expect([room.slots, room.heldForPlates, room.step, room.readout]).toEqual([
      ["stands", "closed"],
      false,
      1,
      "yields",
    ]);
  });

  it("stands the readout again once its run is within the plate, with a line to spare as ever", () => {
    // The plates whole take 568 px: with a line to spare, 585.5 px of the room.
    expect([
      roomAt(600, {}, readoutPassingAt(600)).readout,
      roomAt(602, {}, readoutPassingAt(602)).readout,
    ]).toEqual(["yields", "stands"]);
  });

  it("is 0 with no lower plate", () => {
    expect(leastStepOf(null)).toBe(0);
  });

  it("ignores a run of a part the plate does not hold", () => {
    const readoutOnly: LowerPlateMeasures = {
      wholePx: 120,
      readoutYieldedPx: 60,
      surveyYieldedPx: null,
      passing: ["survey"],
      ...LOWER_WIDTHS,
      surveyYieldedWidthPx: null,
    };
    expect(leastStepOf(readoutOnly)).toBe(0);
  });
});

/** A plate's width in columns at the floor, px: its 24 rem column, as the owner's captures at 25 lay it out. */
const FLOOR_COLUMN_PX = 384;

/**
 * The floor's upper plate as measured (R10.T12.c): `wholePx` tall, ten 19 px lines before `STARS`,
 * {@link FLOOR_COLUMN_PX} wide.
 */
function floorUpper(wholePx: number): UpperPlateMeasures {
  return {
    wholePx,
    middleLinesPx: Array.from({ length: 10 }, () => 19),
    pinnedLines: 5,
    bodyPx: wholePx - 22,
    positionPx: 18,
    widthPx: FLOOR_COLUMN_PX,
  };
}

/**
 * The floor's lower plate at its tallest (R10.T12.c), `wholePx` whole and, unless given, 77 and 40
 * px given way, each form {@link FLOOR_COLUMN_PX} wide.
 */
function floorLower(
  wholePx: number,
  readoutYieldedPx = 77,
  surveyYieldedPx = 40,
): LowerPlateMeasures {
  return {
    wholePx,
    readoutYieldedPx,
    surveyYieldedPx,
    passing: [],
    wholeWidthPx: FLOOR_COLUMN_PX,
    readoutYieldedWidthPx: FLOOR_COLUMN_PX,
    surveyYieldedWidthPx: FLOOR_COLUMN_PX,
  };
}

describe("VIEW's room: the plates in columns (R10.T12.c)", () => {
  /** In columns on the stage: no slot standing, the stage fitting them. */
  const COLUMNS: Partial<RoomInput> = { columns: true };

  it("stands the readout at the floor by its height in its tallest state beside a photorealistic CHASE plate, where the cap gives it way", () => {
    // TERRAIN TEST at 1280 × 720, 100% (pod, SwiftShader): the upper plate 401 px whole stacked,
    // 438 px capped short of the centre (its STARS line and one note on a second row); the lower
    // plate at its tallest 210 px at the full width and, by arithmetic, 324 px in its 24 rem column
    // (16 rows of 19 px, the heading's margin and the padding), 77 and 40 px given way.
    const stage = stageOf(536, 816);
    const stacked = viewRoom(
      inputOn(stage, { full: { upper: floorUpper(401), lower: floorLower(210) } }),
      null,
    );
    const columns = viewRoom(
      inputOn(stage, {
        full: { upper: floorUpper(438), lower: floorLower(324) },
        columns: columnsFit(stage),
      }),
      null,
    );
    // In columns both stand by their heights (step 0); together they would cover 66.9% of the
    // picture, so the cap scrolls the upper plate's lines and gives the readout way (the owner's
    // delegated call, 2026-11-01).
    expect({
      fits: columnsFit(stage),
      stacked: stacked.readout,
      columns: [columns.arrangement, columns.step, columns.readout, columns.survey],
      upper: columns.upper.kind,
      capped: columns.cover?.capped,
    }).toEqual({
      fits: true,
      stacked: "yields",
      columns: ["columns", 0, "yields", "stands"],
      upper: "middle",
      capped: "readout",
    });
  });

  it("stands both plates whole in columns where stacked the readout would give way", () => {
    const room = roomAt(530, COLUMNS);
    expect([
      roomAt(530).readout,
      room.arrangement,
      room.step,
      room.readout,
      room.survey,
      room.upper,
    ]).toEqual(["yields", "columns", 0, "stands", "stands", { kind: "whole" }]);
  });

  it("gives the lower plate's parts way only for its own height, the upper plate whole", () => {
    const tallLower: PlateMeasures = {
      ...FULL,
      lower: {
        wholePx: 520,
        readoutYieldedPx: 60,
        surveyYieldedPx: 40,
        passing: [],
        ...LOWER_WIDTHS,
      },
    };
    const room = roomAt(530, { ...COLUMNS, full: tallLower });
    expect([room.step, room.readout, room.survey, room.upper]).toEqual([
      1,
      "yields",
      "stands",
      { kind: "whole" },
    ]);
  });

  it("scrolls the upper plate's lines only for its own height, the readout standing", () => {
    const room = roomAt(400, COLUMNS);
    expect([room.readout, room.survey, room.upper.kind, room.step]).toEqual([
      "stands",
      "stands",
      "middle",
      3,
    ]);
  });

  it("scrolls the whole upper plate in the stage's room less nothing for the lower plate", () => {
    const room = roomAt(200, COLUMNS);
    // 200 px less the insets and the VIEW and position lines (38 px).
    expect([room.readout, room.upper]).toEqual([
      "stands",
      { kind: "body", heightPx: 200 - 16 - 38 },
    ]);
  });

  it("gives the readout way where its widest run passes the plate, in columns too", () => {
    const room = roomAt(600, { ...COLUMNS, full: passing(FULL, ["readout"]) });
    expect([room.arrangement, room.step, room.readout]).toEqual(["columns", 1, "yields"]);
  });

  it("returns a lower part in columns only with a line to spare", () => {
    // The lower plate whole takes 520 px of a 514 px room at 530, and 530.5 with its spare.
    const tall = (wholePx: number): PlateMeasures => ({
      ...FULL,
      lower: { wholePx, readoutYieldedPx: 60, surveyYieldedPx: 40, passing: [], ...LOWER_WIDTHS },
    });
    const yielded = roomAt(530, { ...COLUMNS, full: tall(520) });
    expect([
      roomAt(530, { ...COLUMNS, full: tall(497) }, yielded).readout,
      roomAt(530, { ...COLUMNS, full: tall(496.5) }, yielded).readout,
    ]).toEqual(["yields", "stands"]);
  });

  it("stands the plates stacked beside a slot that stands, whatever the stage fits", () => {
    const room = roomAt(700, { ...COLUMNS, slots: [OPEN, CLOSED], beside: besideWith(400) });
    expect([room.slots, room.arrangement]).toEqual([["stands", "closed"], "stacked"]);
  });

  it("stands the plates in columns once the slots are held for them", () => {
    const room = roomAt(530, { ...COLUMNS, slots: [OPEN, CLOSED], beside: BESIDE });
    expect([room.heldForPlates, room.arrangement, room.readout]).toEqual([
      true,
      "columns",
      "stands",
    ]);
  });

  it.each<readonly [string, () => ViewRoom, PlateArrangement]>([
    ["no slot open, the stage fitting them", () => wholeRoom([CLOSED, CLOSED], true), "columns"],
    ["a slot open", () => wholeRoom([OPEN, CLOSED], true), "stacked"],
    ["the stage not fitting them", () => wholeRoom([CLOSED, CLOSED]), "stacked"],
    [
      "the plates not yet measured",
      () => viewRoom(inputOn(stageOf(530), { ...COLUMNS, full: null }), null),
      "columns",
    ],
  ])("first lays the plates out, with %s, %s", (_, room, arrangement) => {
    expect(room().arrangement).toBe(arrangement);
  });

  it.each<readonly [string, StageRoom, boolean]>([
    ["the floor's, 51 × 33.5 rem", stageOf(536, 816), true],
    ["1920 × 1080's at 150%, 51 × 33.54 rem", stageOf(805, 1224, 24), true],
    ["1920 × 1080's at 125%, 67 × 42.38 rem", stageOf(847.5, 1340, 20), false],
    ["1600 × 900's at 100%, about 71 × 44.8 rem", stageOf(716, 1136), false],
    ["1920 × 1080's at 100%, 70 × 56 rem", stageOf(896, 1120), false],
    ["a tall narrow window's, 51 × 62.5 rem", stageOf(1000, 816), false],
    ["1280 × 720's at 125%, about 35 × 24.6 rem, under the floor", stageOf(492, 708, 20), false],
  ])("fits the plates in columns on %s: %s", (_, stage, fits) => {
    expect(columnsFit(stage)).toBe(fits);
  });

  it("tells two rooms in other arrangements apart", () => {
    expect(sameRoom(roomAt(600), roomAt(600, COLUMNS))).toBe(false);
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

/**
 * The stage at 1280 × 720 in the owner's `VIEW BODY` captures at version 25 (low): 816 × 500 px,
 * 51 × 31.25 rem, where the plates stand in columns.
 */
const FLOOR_STAGE = stageOf(500, 816);

/**
 * The upper plate of `.0705`'s close-in in those captures, 444 px tall and 384 px wide: `FRAME` to
 * `METER` its ten 19 px lines before `STARS`, `TERRAIN: DETAIL LIMITED`, two atmosphere notes and
 * `› 10 REASONS` among its statements.
 */
const CLOSE_IN_UPPER = floorUpper(444);

/** The upper plate of `.0400`'s orbit in those captures, 366 px tall: one statement, `ATMOSPHERE: PENDING`. */
const ORBIT_UPPER = floorUpper(366);

/**
 * The lower plate of both in those captures, 267 px tall and 384 px wide whole: the survey line and
 * `DATUM` on two rows each and the readout's nine; given way, by its rows, 134 px (the readout's
 * yield line on two rows) and 73 px (the one yield line on three).
 */
const FLOOR_LOWER = floorLower(267, 134, 73);

describe("VIEW's room: the plates' cover capped at half the picture (the owner's delegated call, 2026-11-01)", () => {
  /** In columns on the floor's stage, as the captures stand them. */
  const atFloor = (upper: UpperPlateMeasures, previous: ViewRoom | null = null): ViewRoom =>
    viewRoom(
      inputOn(FLOOR_STAGE, { full: { upper, lower: FLOOR_LOWER }, columns: true }),
      previous,
    );

  /** The share of the floor's stage the plates cover at their heights alone, as measured. */
  const coverOf = (upper: UpperPlateMeasures): number =>
    (FLOOR_COLUMN_PX * (upper.wholePx + FLOOR_LOWER.wholePx)) / (816 * 500);

  it("holds .0705's close-in, 66.9% at its heights, to 47.2%: the lines to FRAME–CAMERA, then the readout", () => {
    const room = atFloor(CLOSE_IN_UPPER);
    expect({
      before: Number(coverOf(CLOSE_IN_UPPER).toFixed(3)),
      share: Number(room.cover?.share.toFixed(3)),
      capped: room.cover?.capped,
      parts: [room.arrangement, room.step, room.readout, room.survey],
      upper: room.upper,
    }).toEqual({
      before: 0.669,
      share: 0.472,
      capped: "readout",
      parts: ["columns", 0, "yields", "stands"],
      upper: { kind: "middle", heightPx: 5 * 19, lines: 5 },
    });
  });

  it("holds .0400's orbit, 59.6% at its heights, to 39.8% by the same step", () => {
    const room = atFloor(ORBIT_UPPER);
    expect({
      before: Number(coverOf(ORBIT_UPPER).toFixed(3)),
      share: Number(room.cover?.share.toFixed(3)),
      capped: room.cover?.capped,
      parts: [room.readout, room.survey, room.upper.kind],
    }).toEqual({
      before: 0.596,
      share: 0.398,
      capped: "readout",
      parts: ["yields", "stands", "middle"],
    });
  });

  it("scrolls the upper plate's lines first, only as far as the cap needs", () => {
    // 384 × (300 + 267) px is 53.4% of the picture: three lines fewer, with the position line under
    // them, is 49.7%.
    const room = atFloor(floorUpper(300));
    expect([room.cover?.capped, room.readout, room.upper]).toEqual([
      "scroll",
      "stands",
      { kind: "middle", heightPx: 7 * 19, lines: 7 },
    ]);
    expect(room.cover?.share).toBeLessThanOrEqual(PLATES_COVER_CAP);
  });

  it("gives the survey line's values and DATUM way last, the readout given way before them", () => {
    // With a lower plate that, its readout given way, is still 260 px tall.
    const room = viewRoom(
      inputOn(FLOOR_STAGE, {
        full: { upper: CLOSE_IN_UPPER, lower: floorLower(400, 260, 73) },
        columns: true,
      }),
      null,
    );
    expect([room.cover?.capped, room.readout, room.survey, room.upper.kind]).toEqual([
      "survey",
      "yields",
      "yields",
      "middle",
    ]);
  });

  it("gives nothing way where the plates cover less than half", () => {
    const room = atFloor(floorUpper(200));
    expect([room.cover?.capped, room.readout, room.upper.kind]).toEqual([
      "none",
      "stands",
      "whole",
    ]);
    expect(room.cover?.share).toBeCloseTo((384 * (200 + 267)) / (816 * 500), 9);
  });

  it("returns a part only once the plates would stay under half with a line to spare", () => {
    // Capped by its scroll at 300 px: whole, 260 px is 49.6% of the picture, and 51.2% with a line
    // of 17.5 px across 384 px to spare; at 240 px, 47.7% and 49.4% with it.
    const capped = atFloor(floorUpper(300));
    expect([
      atFloor(floorUpper(260)).cover?.capped,
      atFloor(floorUpper(260), capped).cover?.capped,
      atFloor(floorUpper(240), capped).cover?.capped,
    ]).toEqual(["none", "scroll", "none"]);
  });

  it("caps the plates stacked too, past the room's own steps", () => {
    // .0705's close-in stacked on a 700 × 560 px stage: by their heights the survey line and DATUM
    // give way (444 + 8 + 73 of 544 px), and 384 × (444 + 73) px is 50.6% of the picture, so the
    // lines before STARS scroll too, two fewer.
    const room = viewRoom(
      inputOn(stageOf(560, 700), { full: { upper: CLOSE_IN_UPPER, lower: FLOOR_LOWER } }),
      null,
    );
    expect([room.arrangement, room.step, room.survey, room.cover?.capped, room.upper]).toEqual([
      "stacked",
      2,
      "yields",
      "scroll",
      { kind: "middle", heightPx: 8 * 19, lines: 8 },
    ]);
  });

  it("returns a readout the cap gave way only once the plates would stay under half with a line to spare", () => {
    // With the lines at FRAME–CAMERA and the readout standing, 340 px is 49.9% of the picture and
    // 51.5% with the line to spare; 320 px, 48.0% and 49.6%.
    const capped = atFloor(CLOSE_IN_UPPER);
    expect([
      atFloor(floorUpper(340)).cover?.capped,
      atFloor(floorUpper(340), capped).cover?.capped,
      atFloor(floorUpper(320), capped).cover?.capped,
    ]).toEqual(["scroll", "readout", "scroll"]);
  });

  it("caps the plates beside a slot that stands, the slot standing", () => {
    // 690 px wide beside the slot on a 1280 × 700 px stage: 690 × (600 + 60) px is 50.8% of it, and
    // the lines before STARS scroll two fewer.
    const upper: UpperPlateMeasures = { ...floorUpper(600), widthPx: 690 };
    const lower: LowerPlateMeasures = {
      ...floorLower(60, 40, 30),
      wholeWidthPx: 690,
      readoutYieldedWidthPx: 690,
      surveyYieldedWidthPx: 690,
    };
    const room = roomAt(700, { slots: [OPEN, CLOSED], beside: { upper, lower } });
    expect([room.slots[0], room.heldForPlates, room.cover?.capped, room.upper]).toEqual([
      "stands",
      false,
      "scroll",
      { kind: "middle", heightPx: 8 * 19, lines: 8 },
    ]);
  });

  it("gives nothing way under MAXIMIZE VIEW's bare upper plate, the readout standing", () => {
    // Folded to its faults alone, 60 px: with the lower plate whole, 30.8% of the picture.
    const bare: UpperPlateMeasures = {
      wholePx: 60,
      middleLinesPx: [],
      pinnedLines: 0,
      bodyPx: 40,
      positionPx: 18,
      widthPx: FLOOR_COLUMN_PX,
    };
    const room = atFloor(bare);
    expect([room.cover?.capped, room.readout, room.survey]).toEqual(["none", "stands", "stands"]);
  });

  it("stays over half where what never gives way alone holds the plates there", () => {
    // Stacked on a 600 × 560 px stage the survey line gives way by the heights, and with the lines
    // at FRAME–CAMERA the plates still cover 50.3%: nothing more gives way.
    const room = viewRoom(
      inputOn(stageOf(560, 600), { full: { upper: CLOSE_IN_UPPER, lower: FLOOR_LOWER } }),
      null,
    );
    expect([room.cover?.capped, room.upper, room.survey]).toEqual([
      "scroll",
      { kind: "middle", heightPx: 5 * 19, lines: 5 },
      "yields",
    ]);
    expect(room.cover?.share).toBeGreaterThan(PLATES_COVER_CAP);
  });

  it("tells two rooms capped apart", () => {
    expect(sameRoom(atFloor(CLOSE_IN_UPPER), atFloor(floorUpper(200)))).toBe(false);
  });

  it("tells two rooms apart that differ in their cover alone", () => {
    const room = atFloor(CLOSE_IN_UPPER);
    const cover = room.cover;
    if (cover === null) {
      throw new Error("a room decided on measures has a cover");
    }
    expect([
      sameRoom(room, { ...room, cover: { ...cover, share: cover.share - 0.01 } }),
      sameRoom(room, { ...room, cover: { ...cover, capped: "survey" } }),
      sameRoom(room, { ...room }),
    ]).toEqual([false, false, true]);
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
      arrangement: "stacked",
      cover: null,
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
