/**
 * `VIEW`'s room (plan R10, R10.T12.b; decision-r10-view-block-room §3): the `PRIMARY` view's
 * label block as two plates, the order in which their parts give way where the stage cannot hold
 * both whole, and the instrument slots held where it cannot hold them, decided by one pure function
 * of the plates' laid-out heights, the stage's size and the slots.
 *
 * @remarks
 * The upper plate stands at the stage's top left with its lines, statements, star count and
 * faults; the lower plate, while the view states a body's terrain, at its bottom left with the
 * survey line, `DATUM` and the terrain readout, which R10.T12 fills ({@link LowerPlatePart}). Both
 * take the width left of the slots that stand, inset 0.5 rem from the stage and 0.5 rem apart.
 * Where they cannot both stand whole, these give way in turn, each at once and as a cut:
 *
 * 1. the readout, to `NOT AVAILABLE: no room for GROUND AT LINE OF SIGHT at this window size`;
 * 2. the survey line's values and `DATUM`, to one line naming what is held back;
 * 3. the upper plate's lines before `STARS`, which scroll with their position and total, keeping
 *    `FRAME` to `CAMERA` in view at their first position;
 * 4. the open instruments, which are held so that the plates take the stage's full width.
 *
 * Where even that does not hold them, the scroll shrinks to one line (5), and then the whole upper
 * plate below its `VIEW` line scrolls, held at its foot (6): wherever steps 1 to 4 fail, which the
 * design expects only below the floor, 80 × 45 rem, since the alternative would be a cut. The
 * statements, the star count, the faults, the `STARS` line and those after it, the survey line's
 * states and the yield lines never give way. Each part returns, as a cut, once it would stand whole
 * with one label row ({@link LINE_SPARE_REM}) to spare, so that a part that gave way at the edge
 * does not come back and go again as one statement clears and returns.
 *
 * The width test (R10.T12.c; decision-r10-t12a-scope §7 item 4): a part of the lower plate whose
 * widest unbroken run, at its tallest, passes its plate gives way in the same order whatever the
 * heights (the readout first, then the survey line's values and `DATUM`), so that no run stands
 * past a plate: beside a slot in a tall, narrow window, the readout's widest runs pass a plate of
 * about 13 rem ({@link leastStepOf}).
 *
 * A slot is held where the stage cannot hold it whole, for the window, the interface scale or its
 * own statements: `INSTRUMENT 2` first, which while `INSTRUMENT 1` stands stands as its stub, its
 * title row with `CLOSE` and `NOT AVAILABLE: no room at this window size`; else held whole. A held
 * slot draws nothing and returns as it was with a line to spare; none ever stands past the stage.
 */

/** The inset of each plate and slot from the stage's edge, and between two slots, rem. */
export const STAGE_INSET_REM = 0.5;

/** The least space between the two plates, rem. */
export const PLATE_GAP_REM = 0.5;

/**
 * The stage's height, rem, below which, while no slot stands, the primary's plates stand either side
 * of the picture's centre ({@link PlateArrangement}; plan R10, R10.T12.c): stacked, the tallest
 * states measured at the floor (`TERRAIN TEST`, the primary photorealistic in `CHASE`, its upper
 * plate 25.1 rem, and the readout at its tallest, 13.1 rem, with the insets and the gap) take 39.7
 * rem. The floor's stage is 33.5 rem tall; 1920 × 1080's at 125% about 42.4 and at 100% 56, and
 * 1600 × 900's at 100% about 44.8, where the plates stand stacked as before.
 */
export const COLUMNS_BELOW_HEIGHT_REM = 40;

/**
 * The least width of a stage on which the plates stand in columns, rem: each column then at least
 * 22.5 rem wide, holding the readout's widest unbroken run (`MATERIAL`'s, about 16.5 rem) and its
 * labels whole; the floor's stage is 51 rem wide. Below it, under the floor, they stand stacked as
 * before.
 */
export const COLUMNS_MIN_WIDTH_REM = 48;

/** The space each plate in columns keeps from the picture's centre, rem: the tripod's ticks reach 0.44. */
export const CENTRE_CLEAR_REM = 1;

/**
 * One label row, rem, the room a part that gave way needs to spare before it returns: the label
 * block's 0.875 rem type at B612's 1.25 line height, the ruling's "about 1.09 rem".
 */
export const LINE_SPARE_REM = 0.875 * 1.25;

/**
 * An instrument slot's height and width before it is laid out, rem (decision-r07-t19, item 2a): its
 * title and padding, and its 15 × 11.25 rem canvas beside a label block up to 18 rem wide, whose
 * dozen lines (the stars' reading over three) set the height, 15.9 rem as measured.
 */
export const INSTRUMENT_SLOT_REM = { heightRem: 16, widthRem: 35 } as const;

/**
 * The least width left to the primary's plates beside the slots, rem: the 230 px they had at
 * 1280 × 720 with both slots open, which decision-r07-t19 accepted.
 */
export const PRIMARY_LABEL_MIN_REM = 14;

/**
 * A held `INSTRUMENT 2`'s stub's height before it is laid out, rem: its 0.5 rem padding about a
 * title row as tall as its 2 rem `CLOSE` and its reason's line, as measured at 1280 × 720.
 */
export const STUB_HEIGHT_REM = 4;

/** The stub's reason, in its slot's corner (decision-r10-view-block-room §3). */
export const STUB_REASON = "NOT AVAILABLE: no room at this window size";

/** The parts of the lower plate, as R10.T12 fills them (decision-r10-view-block-room §3). */
export type LowerPartKind = "state" | "survey" | "datum" | "readout";

/**
 * A part of the lower plate, in the guide's order: the survey line or its state, `DATUM`, then
 * `GROUND AT LINE OF SIGHT` and its readings (R10.T12's `SurveyLine` and `TerrainReadout`).
 *
 * @remarks
 * A state line (`SURVEY COVERAGE: PENDING`, `TERRAIN: NOT YET MODELLED`,
 * `SURVEY COVERAGE REJECTED: …`, `SURVEY DATA INVALID: …`) never gives way. The others give way
 * in the ruled order, each reckoned at its `tallest`: the readout with every reading Estimated,
 * stale and `GRAZING`, and `DATUM` with `SEA LEVEL`, so that none comes and goes with its own
 * readings. The `tallest` form is laid out only in the hidden sizer and never shown.
 */
export type LowerPlatePart<Node> =
  | {
      readonly kind: "state";
      /** Its key among the plate's parts, its text. */
      readonly key: string;
      readonly node: Node;
    }
  | {
      readonly kind: "survey" | "datum" | "readout";
      /** The part as it stands now. */
      readonly node: Node;
      /** The part in its tallest form, by which the room is reckoned. */
      readonly tallest: Node;
    };

/** The name a yield line gives each part it holds back, as the guide names them. */
const HELD_NAMES: Readonly<Record<YieldingPart, string>> = {
  survey: "SURVEY COVERAGE",
  datum: "DATUM",
  readout: "GROUND AT LINE OF SIGHT",
};

/**
 * The yield line in the place of the lower plate's parts that give way: `NOT AVAILABLE: no room for
 * GROUND AT LINE OF SIGHT at this window size`, or `NOT AVAILABLE: no room for SURVEY COVERAGE,
 * DATUM or GROUND AT LINE OF SIGHT at this window size`, naming only what it holds back, in the
 * plate's order (decision-r10-view-block-room §3).
 *
 * @throws RangeError for no part, since a line holding nothing back is never stated.
 */
export function lowerYieldLine(kinds: ReadonlyArray<YieldingPart>): string {
  const names = (["survey", "datum", "readout"] as const)
    .filter((kind) => kinds.includes(kind))
    .map((kind) => HELD_NAMES[kind]);
  const last = names.at(-1);
  if (last === undefined) {
    throw new RangeError("a yield line holds something back");
  }
  const list = names.length === 1 ? last : `${names.slice(0, -1).join(", ")} or ${last}`;
  return `NOT AVAILABLE: no room for ${list} at this window size`;
}

/** The upper plate's heights at one width, CSS px, as its hidden sizer laid it out. */
export interface UpperPlateMeasures {
  /** The plate whole: its lines unscrolled, with no position line. */
  readonly wholePx: number;
  /** Each of its lines before `STARS` in order, `FRAME` to `METER`: the lines that scroll. */
  readonly middleLinesPx: ReadonlyArray<number>;
  /** How many of those lead and stay in view at the scroll's first position: `FRAME` to `CAMERA`. */
  readonly pinnedLines: number;
  /** Everything under its `VIEW` line: its lines, statements, star count and faults. */
  readonly bodyPx: number;
  /** A position line under a scroll (`1-6 of 11`), with its margin. */
  readonly positionPx: number;
}

/** A part of the lower plate that gives way. */
export type YieldingPart = Exclude<LowerPartKind, "state">;

/** The lower plate's heights at one width, CSS px, each of its groups at its tallest. */
export interface LowerPlateMeasures {
  /** The plate with every part standing. */
  readonly wholePx: number;
  /** With its readout given way to its yield line; `null` where it holds no readout. */
  readonly readoutYieldedPx: number | null;
  /** With its survey line's values and `DATUM` given way too; `null` where it holds neither. */
  readonly surveyYieldedPx: number | null;
  /**
   * The parts whose widest unbroken run, at its tallest, passes the plate at this width (R10.T12.c;
   * decision-r10-t12a-scope §7 item 4), each of which gives way whatever the heights; none where
   * every run stands within it.
   */
  readonly passing: ReadonlyArray<YieldingPart>;
}

/** Both plates' heights at one width. */
export interface PlateMeasures {
  readonly upper: UpperPlateMeasures;
  /** The lower plate's, or `null` while the view states no terrain lines. */
  readonly lower: LowerPlateMeasures | null;
}

/** What the decision reads of the stage, CSS px. */
export interface StageRoom {
  readonly widthPx: number;
  readonly heightPx: number;
  /** CSS px in a `rem` at the interface scale. */
  readonly remPx: number;
}

/** What the decision reads of a slot. */
export interface SlotRoomInput {
  readonly open: boolean;
  /**
   * Its height as it last stood whole, rem: its panel's as measured, so that a slot held on a scale
   * change is reckoned at the new scale; {@link INSTRUMENT_SLOT_REM}'s before it has stood.
   */
  readonly heightRem: number;
}

/** What {@link viewRoom} decides from. */
export interface RoomInput {
  /** The stage, or `null` before it is laid out. */
  readonly stage: StageRoom | null;
  readonly slots: readonly [SlotRoomInput, SlotRoomInput];
  /** The stub's height, rem, as last laid out, or {@link STUB_HEIGHT_REM}. */
  readonly stubHeightRem: number;
  /** The plates at the stage's full width, or `null` before they are measured. */
  readonly full: PlateMeasures | null;
  /** The plates at the width beside the slots, or `null` where it is not measured. */
  readonly beside: PlateMeasures | null;
  /**
   * The stage the plates were measured at, or `null` before they are: measures taken at another
   * stage than `stage`, in the render after a resize or a change of interface scale, are stale.
   */
  readonly measuredStage: StageRoom | null;
  /** Whether the stage fits the plates in columns while no slot stands: `columnsFit(stage)`. */
  readonly columns: boolean;
}

/**
 * Where an open slot stands: whole in its corner, as its stub (`INSTRUMENT 2` while `INSTRUMENT 1`
 * stands), or held whole; a closed slot is `closed`.
 */
export type SlotStanding = "closed" | "stands" | "stub" | "held";

/**
 * How the upper plate stands: `whole`; its lines before `STARS` scrolling in a box `heightPx`
 * tall (steps 3 and 5); or everything under its `VIEW` line scrolling, held at its foot (step 6).
 */
export type UpperRoom =
  | { readonly kind: "whole" }
  | { readonly kind: "middle"; readonly heightPx: number; readonly lines: number }
  | { readonly kind: "body"; readonly heightPx: number };

/**
 * How far the plates have given way: 0 nothing, 1 the readout, 2 the survey line and `DATUM`, 3 the
 * upper plate's lines before `STARS` scrolling down to `FRAME`–`CAMERA`, 5 that scroll below them, 6
 * the whole upper plate scrolling; the slots held for the plates (step 4) is
 * {@link ViewRoom.heldForPlates}.
 */
export type PlateStep = 0 | 1 | 2 | 3 | 5 | 6;

/**
 * How the primary's plates stand (plan R10, R10.T12.c): `stacked`, both at the stage's left, the
 * upper plate at its top and the lower plate at its foot, sharing its height; or `columns`, where no
 * slot stands on a stage under {@link COLUMNS_BELOW_HEIGHT_REM} tall and at least
 * {@link COLUMNS_MIN_WIDTH_REM} wide, either side of the picture's centre, the upper plate at the
 * top left and the lower plate at the bottom right, from {@link CENTRE_CLEAR_REM} past the centre,
 * neither within that of it and each with the stage's whole height, so that neither covers the
 * point the readout reads.
 */
export type PlateArrangement = "stacked" | "columns";

/** The room decided: what stands, what gives way and what is held. */
export interface ViewRoom {
  /** Each slot's standing, after step 4. */
  readonly slots: readonly [SlotStanding, SlotStanding];
  /** Each slot's standing by its own room alone, before step 4: the hysteresis's. */
  readonly ownSlots: readonly [SlotStanding, SlotStanding];
  /** Whether the open slots are held for the plates' room (step 4). */
  readonly heldForPlates: boolean;
  readonly step: PlateStep;
  /** Whether the readout stands; while it gives way so do its tripod and its list note. */
  readonly readout: "stands" | "yields";
  /** Whether the survey line's values and `DATUM` stand. */
  readonly survey: "stands" | "yields";
  readonly upper: UpperRoom;
  /** How the plates stand: stacked, beside the slots among them, or in columns (R10.T12.c). */
  readonly arrangement: PlateArrangement;
  /**
   * The stage it was decided at, from plates measured there, or `null` where it was not (before
   * the stage is laid out, or on stale measures): only a room decided at the same stage, so
   * measured, sets the next one's hysteresis, so that a resize or a change of interface scale is
   * decided afresh, as no statement coming and going is.
   */
  readonly basis: StageRoom | null;
}

/** Whether two stages are one: the same size to half a pixel, at the same interface scale. */
export function sameStage(a: StageRoom | null, b: StageRoom | null): boolean {
  return (
    a !== null &&
    b !== null &&
    Math.abs(a.widthPx - b.widthPx) < 0.5 &&
    Math.abs(a.heightPx - b.heightPx) < 0.5 &&
    a.remPx === b.remPx
  );
}

/** A slot's standing before anything is laid out. */
function standingBefore(slot: SlotRoomInput): SlotStanding {
  return slot.open ? "stands" : "closed";
}

/** An open slot held for the plates' room (step 4). */
function heldForRoom(each: SlotStanding): SlotStanding {
  return each === "closed" ? "closed" : "held";
}

/**
 * Whether the plates stand in columns on `stage` while no slot stands: under
 * {@link COLUMNS_BELOW_HEIGHT_REM} tall and at least {@link COLUMNS_MIN_WIDTH_REM} wide (R10.T12.c).
 */
export function columnsFit(stage: StageRoom): boolean {
  return (
    stage.heightPx < COLUMNS_BELOW_HEIGHT_REM * stage.remPx &&
    stage.widthPx >= COLUMNS_MIN_WIDTH_REM * stage.remPx
  );
}

/**
 * The room before anything is laid out: every open slot stands, and nothing gives way; the plates
 * in columns where none is open and the stage fits them (`columnsFit`), so that they are first
 * measured as they will stand.
 */
export function wholeRoom(slots: RoomInput["slots"], columnsFitting = false): ViewRoom {
  const standing: readonly [SlotStanding, SlotStanding] = [
    standingBefore(slots[0]),
    standingBefore(slots[1]),
  ];
  const columns = columnsFitting && standing.every((each) => each === "closed");
  return {
    slots: standing,
    ownSlots: standing,
    heldForPlates: false,
    step: 0,
    readout: "stands",
    survey: "stands",
    upper: { kind: "whole" },
    arrangement: columns ? "columns" : "stacked",
    basis: null,
  };
}

/** Whether the stage is wide enough for a slot beside the primary's plates at their least width. */
export function slotWidthFits(stage: StageRoom): boolean {
  return (
    stage.widthPx >=
    (INSTRUMENT_SLOT_REM.widthRem + 3 * STAGE_INSET_REM + PRIMARY_LABEL_MIN_REM) * stage.remPx
  );
}

/** The slots by their own room: the stage's width, then its height for each, `INSTRUMENT 2` first held. */
function slotsByOwnRoom(
  input: RoomInput,
  stage: StageRoom,
  previous: ViewRoom | null,
): [SlotStanding, SlotStanding] {
  const { remPx } = stage;
  const inset = STAGE_INSET_REM * remPx;
  const spare = LINE_SPARE_REM * remPx;
  const [one, two] = input.slots;
  const wide = slotWidthFits(stage);
  // A slot held returns only with a line to spare; one just opened stands where it fits.
  const fits = (usedPx: number, was: SlotStanding | undefined): boolean =>
    usedPx + (was === "held" || was === "stub" ? spare : 0) <= stage.heightPx;
  const oneStands =
    one.open && wide && fits(inset + one.heightRem * remPx + inset, previous?.ownSlots[0]);
  const first: SlotStanding = !one.open ? "closed" : oneStands ? "stands" : "held";
  let second: SlotStanding;
  if (!two.open) {
    second = "closed";
  } else if (!wide) {
    second = "held";
  } else if (oneStands) {
    const below = inset + one.heightRem * remPx + inset;
    if (fits(below + two.heightRem * remPx + inset, previous?.ownSlots[1])) {
      second = "stands";
    } else {
      second =
        // A slot held whole returns as its stub, too, only with a line to spare.
        below +
          input.stubHeightRem * remPx +
          inset +
          (previous?.ownSlots[1] === "held" ? spare : 0) <=
        stage.heightPx
          ? "stub"
          : "held";
    }
  } else {
    second = fits(inset + two.heightRem * remPx + inset, previous?.ownSlots[1]) ? "stands" : "held";
  }
  return [first, second];
}

/** The lower plate's height at a step, or `null` with no lower plate. */
function lowerAt(lower: LowerPlateMeasures | null, step: PlateStep): number | null {
  if (lower === null) {
    return null;
  }
  if (step >= 2 && lower.surveyYieldedPx !== null) {
    return lower.surveyYieldedPx;
  }
  if (step >= 1 && lower.readoutYieldedPx !== null) {
    return lower.readoutYieldedPx;
  }
  return lower.wholePx;
}

/**
 * The least step at which every part of the lower plate that stands has its widest run within the
 * plate (the width test; R10.T12.c): 2 where the survey line's or `DATUM`'s passes it, 1 where the
 * readout's does, else 0. A run's width does not come and go with the readings, the parts being
 * measured at their tallest; a part it gave way returns, as every part does, once it would stand
 * with a line to spare.
 */
export function leastStepOf(lower: LowerPlateMeasures | null): PlateStep {
  if (lower === null) {
    return 0;
  }
  const { passing } = lower;
  if (lower.surveyYieldedPx !== null && (passing.includes("survey") || passing.includes("datum"))) {
    return 2;
  }
  return lower.readoutYieldedPx !== null && passing.includes("readout") ? 1 : 0;
}

/** The plates' standing at one width, or `null` where none of the steps allowed holds them. */
interface PlatesStanding {
  readonly step: PlateStep;
  readonly upper: UpperRoom;
  /**
   * How far the lower plate's parts give way, where it is decided apart from the upper plate's (in
   * columns, R10.T12.c); else the step says it.
   */
  readonly lowerStep?: PlateStep;
}

/**
 * The least step at which both plates stand within `roomPx`, the stage less its insets, up to
 * `lastStep`; a step below `previousStep`, a return, only with a line to spare.
 */
function platesAt(
  measures: PlateMeasures,
  roomPx: number,
  gapPx: number,
  sparePx: number,
  previousStep: PlateStep,
  lastStep: PlateStep,
): PlatesStanding | null {
  const { upper, lower } = measures;
  const fits = (upperPx: number, step: PlateStep): boolean => {
    const lowerPx = lowerAt(lower, step);
    const usedPx = upperPx + (lowerPx === null ? 0 : gapPx + lowerPx);
    return usedPx + (step < previousStep ? sparePx : 0) <= roomPx;
  };
  const hasReadout = lower !== null && lower.readoutYieldedPx !== null;
  const hasSurvey = lower !== null && lower.surveyYieldedPx !== null;
  // A part whose widest run passes the plate gives way however tall the plates are.
  const least = leastStepOf(lower);
  const steps: ReadonlyArray<PlateStep> = [0, 1, 2];
  for (const step of steps) {
    const applies = step === 0 || (step === 1 && hasReadout) || (step === 2 && hasSurvey);
    if (applies && step >= least && step <= lastStep && fits(upper.wholePx, step)) {
      return { step, upper: { kind: "whole" } };
    }
  }
  const lines = upper.middleLinesPx;
  const middlePx = lines.reduce((sum, linePx) => sum + linePx, 0);
  const shellPx = upper.wholePx - middlePx + upper.positionPx;
  // The lines in view at the scroll's first position, most first: down to FRAME–CAMERA at step 3,
  // then below them to one line at step 5. A scroll of every line is no scroll.
  for (let count = lines.length - 1; count >= 1; count -= 1) {
    const step: PlateStep = count >= upper.pinnedLines ? 3 : 5;
    if (step > lastStep) {
      break;
    }
    const heightPx = lines.slice(0, count).reduce((sum, linePx) => sum + linePx, 0);
    if (fits(shellPx + heightPx, step)) {
      return { step, upper: { kind: "middle", heightPx, lines: count } };
    }
  }
  return null;
}

/**
 * The last resort, step 6: everything under the upper plate's `VIEW` line scrolls in what the
 * lower plate, given way to its yield lines, leaves, at least a line.
 */
function bodyScrollAt(
  measures: PlateMeasures,
  roomPx: number,
  gapPx: number,
  sparePx: number,
): PlatesStanding {
  const { upper, lower } = measures;
  const lowerPx = lowerAt(lower, 6);
  const bodyShellPx = upper.wholePx - upper.bodyPx + upper.positionPx;
  const heightPx = Math.max(
    sparePx,
    roomPx - bodyShellPx - (lowerPx === null ? 0 : gapPx + lowerPx),
  );
  return { step: 6, upper: { kind: "body", heightPx } };
}

/** The lower plate's step in columns: the least that holds it in `roomPx`, a return with a line spare. */
function lowerStepIn(
  lower: LowerPlateMeasures | null,
  roomPx: number,
  sparePx: number,
  previousStep: PlateStep,
): PlateStep {
  if (lower === null) {
    return 0;
  }
  const steps: ReadonlyArray<PlateStep> = [0, 1, 2];
  const applies = (step: PlateStep): boolean =>
    step === 0 ||
    (step === 1 && lower.readoutYieldedPx !== null) ||
    (step === 2 && lower.surveyYieldedPx !== null);
  const least = leastStepOf(lower);
  const held = steps.filter((step) => applies(step) && step >= least);
  return (
    held.find((step) => {
      const lowerPx = lowerAt(lower, step) ?? 0;
      return lowerPx + (step < previousStep ? sparePx : 0) <= roomPx;
    }) ??
    held.at(-1) ??
    0
  );
}

/** How far a room's upper plate gave way: 0 whole, its scroll's step, or 6 scrolling whole. */
function upperStepOf(room: ViewRoom): PlateStep {
  let step: PlateStep;
  switch (room.upper.kind) {
    case "whole":
      step = 0;
      break;
    case "middle":
      step = room.step;
      break;
    case "body":
      step = 6;
      break;
  }
  return step;
}

/**
 * The plates in columns (R10.T12.c): each in the stage's whole height, decided apart, the lower
 * plate's parts giving way only for its own height and widths and the upper plate's lines
 * scrolling only for its own; the step the further of the two.
 */
function columnsAt(
  measures: PlateMeasures,
  roomPx: number,
  sparePx: number,
  previous: ViewRoom | null,
): PlatesStanding {
  const { upper, lower } = measures;
  const previousLower: PlateStep =
    previous === null
      ? 0
      : previous.survey === "yields"
        ? 2
        : previous.readout === "yields"
          ? 1
          : 0;
  const lowerStep = lowerStepIn(lower, roomPx, sparePx, previousLower);
  const upperOnly: PlateMeasures = { upper, lower: null };
  const previousUpper: PlateStep = previous === null ? 0 : upperStepOf(previous);
  const upperStanding =
    platesAt(upperOnly, roomPx, 0, sparePx, previousUpper, 5) ??
    bodyScrollAt(upperOnly, roomPx, 0, sparePx);
  const upperStep = upperStanding.upper.kind === "whole" ? 0 : upperStanding.step;
  return {
    step: upperStep > lowerStep ? upperStep : lowerStep,
    upper: upperStanding.upper,
    lowerStep,
  };
}

/**
 * The room decided for the stage's present measures (decision-r10-view-block-room §3), from the
 * room decided before, whose standing sets each part's hysteresis.
 *
 * @remarks
 * First the slots by their own room ({@link SlotStanding}); then, beside those that stand, the
 * plates at the least step up to 3 that holds them; where none does, every open slot is held (step
 * 4) and the plates take the full width, at any step. With no slot standing, on a stage that fits
 * them, the plates stand in columns ({@link PlateArrangement}), each decided in the stage's whole
 * height (R10.T12.c). Slots held for the plates return once the
 * plates beside them would stand at step 3 or less with a line to spare. Before the stage or the
 * plates are measured, nothing gives way.
 *
 * @param decidedBefore - The room decided before, or `null` for the first; its hysteresis holds
 *   only where it was decided at this stage from plates measured there, and these plates were too.
 */
export function viewRoom(input: RoomInput, decidedBefore: ViewRoom | null): ViewRoom {
  const { stage, full } = input;
  if (stage === null || !(stage.remPx > 0) || full === null) {
    return wholeRoom(input.slots, input.columns);
  }
  const fresh = sameStage(input.measuredStage, stage);
  const previous =
    decidedBefore !== null && fresh && sameStage(decidedBefore.basis, stage) ? decidedBefore : null;
  const basis = fresh ? stage : null;
  const { remPx } = stage;
  const roomPx = stage.heightPx - 2 * STAGE_INSET_REM * remPx;
  const gapPx = PLATE_GAP_REM * remPx;
  const sparePx = LINE_SPARE_REM * remPx;
  const previousStep = previous?.step ?? 0;
  const ownSlots = slotsByOwnRoom(input, stage, previous);
  const anyStands = ownSlots.some((each) => each === "stands" || each === "stub");
  const decided = (
    slots: readonly [SlotStanding, SlotStanding],
    heldForPlates: boolean,
    plates: PlatesStanding,
    arrangement: PlateArrangement,
  ): ViewRoom => {
    const lower = full.lower;
    const hasReadout = lower !== null && lower.readoutYieldedPx !== null;
    const hasSurvey = lower !== null && lower.surveyYieldedPx !== null;
    const lowerStep = plates.lowerStep ?? plates.step;
    return {
      slots,
      ownSlots,
      heldForPlates,
      step: plates.step,
      readout: lowerStep >= 1 && hasReadout ? "yields" : "stands",
      survey: lowerStep >= 2 && hasSurvey ? "yields" : "stands",
      upper: plates.upper,
      arrangement,
      basis,
    };
  };
  if (anyStands) {
    const beside = input.beside;
    if (beside === null) {
      return decided(ownSlots, false, { step: 0, upper: { kind: "whole" } }, "stacked");
    }
    // Held for the plates, the slots return only with a line to spare beside them.
    const returning = previous?.heldForPlates === true;
    const besideStanding = platesAt(
      beside,
      returning ? roomPx - sparePx : roomPx,
      gapPx,
      sparePx,
      previousStep,
      3,
    );
    if (besideStanding !== null) {
      return decided(ownSlots, false, besideStanding, "stacked");
    }
  }
  const held: readonly [SlotStanding, SlotStanding] = [
    heldForRoom(ownSlots[0]),
    heldForRoom(ownSlots[1]),
  ];
  // With no slot standing, the plates stand in columns where the stage fits them (R10.T12.c).
  const arrangement: PlateArrangement = input.columns ? "columns" : "stacked";
  const plates =
    arrangement === "columns"
      ? columnsAt(full, roomPx, sparePx, previous)
      : (platesAt(full, roomPx, gapPx, sparePx, previousStep, 5) ??
        bodyScrollAt(full, roomPx, gapPx, sparePx));
  return anyStands
    ? decided(held, true, plates, arrangement)
    : decided(ownSlots, false, plates, arrangement);
}

/**
 * Whether the stage has room to open one more slot beside those that stand, as `room` has them:
 * its width, the height the slots that stand take (a stub as its stub, a held slot as nothing)
 * with the new slot taken to be as tall as those that stand, and the plates beside a slot standing
 * by step 3; `true` before the stage is laid out (decision-r10-view-block-room §3, `roomForSlot`).
 */
export function roomToOpen(input: RoomInput, room: ViewRoom): boolean {
  const { stage } = input;
  if (stage === null || !(stage.remPx > 0)) {
    return true;
  }
  if (!slotWidthFits(stage)) {
    return false;
  }
  const { remPx } = stage;
  const inset = STAGE_INSET_REM * remPx;
  const standing = room.slots.flatMap((each, index): number[] => {
    if (each === "stands") {
      return [(input.slots[index]?.heightRem ?? INSTRUMENT_SLOT_REM.heightRem) * remPx];
    }
    return each === "stub" ? [input.stubHeightRem * remPx] : [];
  });
  const usedPx = standing.reduce((sum, heightPx) => sum + heightPx + inset, 0);
  const slotPx =
    standing.length === 0
      ? INSTRUMENT_SLOT_REM.heightRem * remPx
      : standing.reduce((sum, heightPx) => sum + heightPx, 0) / standing.length;
  if (inset + usedPx + slotPx + inset > stage.heightPx) {
    return false;
  }
  const beside = input.beside;
  if (beside === null) {
    return true;
  }
  const roomPx = stage.heightPx - 2 * STAGE_INSET_REM * remPx;
  return (
    platesAt(beside, roomPx, PLATE_GAP_REM * remPx, LINE_SPARE_REM * remPx, room.step, 3) !== null
  );
}

/** Whether two rooms decide the same, so that a room decided again keeps its identity. */
export function sameRoom(a: ViewRoom, b: ViewRoom): boolean {
  const sameUpper =
    a.upper.kind === b.upper.kind &&
    (a.upper.kind === "whole" ||
      (b.upper.kind !== "whole" &&
        a.upper.heightPx === b.upper.heightPx &&
        (a.upper.kind !== "middle" ||
          (b.upper.kind === "middle" && a.upper.lines === b.upper.lines))));
  return (
    sameUpper &&
    a.slots[0] === b.slots[0] &&
    a.slots[1] === b.slots[1] &&
    a.ownSlots[0] === b.ownSlots[0] &&
    a.ownSlots[1] === b.ownSlots[1] &&
    a.heldForPlates === b.heldForPlates &&
    a.step === b.step &&
    a.readout === b.readout &&
    a.survey === b.survey &&
    a.arrangement === b.arrangement &&
    (a.basis === b.basis || sameStage(a.basis, b.basis))
  );
}
