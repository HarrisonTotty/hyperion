/**
 * The `PRIMARY` view's plates as laid out, for `VIEW`'s room (plan R10, R10.T12.b): the live upper
 * plate, which always holds all its lines and only clips them while it scrolls, and the hidden
 * sizers' copies, the lower plate's three forms at its tallest and, while the slots are held for
 * the plates, both plates beside where the slots would stand (`ViewRoomSizers`).
 */
import { rootRemPx } from "../../lib/useElementSize";
import {
  type LowerPlateMeasures,
  type PlateMeasures,
  sameStage,
  type StageRoom,
  type UpperPlateMeasures,
} from "./viewRoom";

/** An element's laid-out height, CSS px, fractions kept. */
function heightOf(element: Element): number {
  return element.getBoundingClientRect().height;
}

/** A computed length, px, or 0 where it reads as none. */
function lengthPx(text: string): number {
  const px = Number.parseFloat(text);
  return Number.isFinite(px) ? px : 0;
}

/** How far a scrolling part's content runs past its box, px; none for a part that does not scroll. */
function hiddenPx(element: Element | null): number {
  return element instanceof HTMLElement && element.classList.contains("view-label__scroll")
    ? Math.max(0, element.scrollHeight - element.clientHeight)
    : 0;
}

/** A position line's height with its margins, CSS px. */
export function positionLinePx(element: Element): number {
  const style = getComputedStyle(element);
  return heightOf(element) + lengthPx(style.marginTop) + lengthPx(style.marginBottom);
}

/**
 * The upper plate's heights ({@link UpperPlateMeasures}) as `plate` is laid out: whole, its scrolled
 * part's content counted in full and its position line left out, and each of its lines before
 * `STARS`, which while they scroll are measured at the scroll's width.
 *
 * @param pinnedLines - How many of its first lines, `FRAME` to `CAMERA`, stay in view at the
 *   scroll's first position.
 * @param positionPx - A position line's height, from the sizers' sample.
 */
export function measureUpperPlate(
  plate: HTMLElement,
  pinnedLines: number,
  positionPx: number,
): UpperPlateMeasures {
  const body = plate.querySelector(":scope > .view-label__body");
  const middle = plate.querySelector(".view-label__middle");
  const shownPosition = plate.querySelector(".view-label__position");
  const shownPositionPx = shownPosition === null ? 0 : positionLinePx(shownPosition);
  const plateLaidPx = heightOf(plate);
  const wholePx = plateLaidPx + hiddenPx(middle) + hiddenPx(body) - shownPositionPx;
  // The position line of the whole plate's scroll stands under its body, that of the lines' scroll
  // inside it.
  const positionOutside = shownPosition !== null && shownPosition.parentElement === plate;
  const chromePx =
    plateLaidPx - (body === null ? 0 : heightOf(body)) - (positionOutside ? shownPositionPx : 0);
  const middleLinesPx =
    middle === null ? [] : [...middle.querySelectorAll(".view-label__line")].map(heightOf);
  return {
    wholePx,
    middleLinesPx,
    pinnedLines: Math.min(pinnedLines, middleLinesPx.length),
    bodyPx: wholePx - chromePx,
    positionPx,
  };
}

/**
 * The lower plate's heights ({@link LowerPlateMeasures}) from its forms in `root`, each a plate
 * under an element whose `data-form` names it: `whole`, `readout` (the readout given way) and
 * `survey` (the survey line's values and `DATUM` given way too); `null` without a whole form.
 */
export function measureLowerForms(root: Element): LowerPlateMeasures | null {
  const form = (name: string): number | null => {
    const plate = root.querySelector(`[data-form="${name}"] > .view-label`);
    return plate === null ? null : heightOf(plate);
  };
  const wholePx = form("whole");
  return wholePx === null
    ? null
    : { wholePx, readoutYieldedPx: form("readout"), surveyYieldedPx: form("survey") };
}

/** The plates' heights at the stage's full width and at the width beside the slots. */
export interface RoomMeasures {
  readonly full: PlateMeasures | null;
  readonly beside: PlateMeasures | null;
  /** The stage they were measured at: the overlay's size and the interface scale, or `null`. */
  readonly stage: StageRoom | null;
}

/** No plate measured yet. */
export const NOT_MEASURED: RoomMeasures = { full: null, beside: null, stage: null };

/** What {@link measureRoom} reads. */
export interface RoomMeasureInput {
  /** The live upper plate, or `null` while it is not mounted. */
  readonly upper: HTMLElement | null;
  /** The sizers' root (`ViewRoomSizers`), or `null`. */
  readonly sizers: HTMLElement | null;
  /** `FRAME` to `CAMERA`'s count among the upper plate's lines. */
  readonly pinnedLines: number;
  /** Whether the live plates stand beside the slots, or take the stage's full width. */
  readonly liveBeside: boolean;
}

/**
 * Both plates' heights at both widths, as laid out now: the live plates at their own width, and
 * the sizers' copies beside the slots while those are held for the plates. Beside the slots the
 * live plates stand for the full width too, a narrower and so a taller reckoning, until the
 * plates take the full width and are measured there.
 */
export function measureRoom(input: RoomMeasureInput): RoomMeasures {
  const { upper, sizers } = input;
  if (upper === null) {
    return NOT_MEASURED;
  }
  // The stage they are measured at: the overlay the live plate stands in.
  const overlay = upper.parentElement;
  const overlayBox = overlay?.getBoundingClientRect() ?? null;
  const stage: StageRoom | null =
    overlayBox === null
      ? null
      : { widthPx: overlayBox.width, heightPx: overlayBox.height, remPx: rootRemPx() };
  const sample = sizers?.querySelector("[data-sizer='position']") ?? null;
  const positionPx = sample === null ? 0 : positionLinePx(sample);
  const liveLower = sizers?.querySelector("[data-sizer='lower']") ?? null;
  const live: PlateMeasures = {
    upper: measureUpperPlate(upper, input.pinnedLines, positionPx),
    lower: liveLower === null ? null : measureLowerForms(liveLower),
  };
  if (input.liveBeside) {
    return { full: live, beside: live, stage };
  }
  const besideRoot = sizers?.querySelector("[data-sizer='beside']") ?? null;
  const besideUpper = besideRoot?.querySelector("[data-form='upper'] > .view-label") ?? null;
  const beside: PlateMeasures | null =
    besideRoot === null || !(besideUpper instanceof HTMLElement)
      ? null
      : {
          upper: measureUpperPlate(besideUpper, input.pinnedLines, positionPx),
          lower: measureLowerForms(besideRoot),
        };
  return { full: live, beside, stage };
}

/** Within a twentieth of a pixel, so that sub-pixel jitter re-renders nothing. */
function samePx(a: number | null, b: number | null): boolean {
  return a === b || (a !== null && b !== null && Math.abs(a - b) < 0.05);
}

function samePlates(a: PlateMeasures | null, b: PlateMeasures | null): boolean {
  if (a === null || b === null) {
    return a === b;
  }
  const lines = a.upper.middleLinesPx;
  const otherLines = b.upper.middleLinesPx;
  const sameLower =
    a.lower === null || b.lower === null
      ? a.lower === b.lower
      : samePx(a.lower.wholePx, b.lower.wholePx) &&
        samePx(a.lower.readoutYieldedPx, b.lower.readoutYieldedPx) &&
        samePx(a.lower.surveyYieldedPx, b.lower.surveyYieldedPx);
  return (
    sameLower &&
    samePx(a.upper.wholePx, b.upper.wholePx) &&
    samePx(a.upper.bodyPx, b.upper.bodyPx) &&
    samePx(a.upper.positionPx, b.upper.positionPx) &&
    a.upper.pinnedLines === b.upper.pinnedLines &&
    lines.length === otherLines.length &&
    lines.every((linePx, index) => samePx(linePx, otherLines[index] ?? null))
  );
}

/** Whether two measures are the same within a twentieth of a pixel. */
export function sameRoomMeasures(a: RoomMeasures, b: RoomMeasures): boolean {
  return (
    samePlates(a.full, b.full) &&
    samePlates(a.beside, b.beside) &&
    (a.stage === b.stage || sameStage(a.stage, b.stage))
  );
}
