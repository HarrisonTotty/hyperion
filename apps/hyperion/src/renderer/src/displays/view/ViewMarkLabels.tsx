import { StaleMark } from "../../components/StaleMark";
import type { ElementSize } from "../../lib/useElementSize";
import {
  boxGapPx,
  DESTINATION_LABEL_PLACES,
  type DestinationLabelPlace,
  destinationLabelBoxPx,
  LABEL_NEIGHBOUR_CLEARANCE_REM,
  LABEL_TEXT_CLEARANCE_REM,
  type ScreenBoxPx,
  type OffsetPx,
  placeIsRight,
  placeIsUpper,
  squareBoxPx,
} from "../../spatial/symbols";
import type { DrawAnchor } from "../../view/wireframe/drawList";
import {
  type ClosureReading,
  MISSING_READING,
  type MarkRow,
  rangeText,
  targetKey,
} from "./viewRun";

/** No closure rate, for a body's label. */
const NO_CLOSURE: ClosureReading = { kind: "none" };

/** Props of {@link ViewMarkLabels}. */
export interface ViewMarkLabelsProps {
  /** The marks as the latest published frame placed them, in device px: which carry a label. */
  readonly anchors: ReadonlyArray<DrawAnchor>;
  /** Device pixels in one CSS pixel, by which the anchors are placed in the DOM. */
  readonly devicePixelRatio: number;
  /** The list's rows, whose names, ranges and closure rates the labels repeat. */
  readonly rows: ReadonlyArray<MarkRow>;
  /**
   * Whether the ranges and closure rates are stale, a server scene held through a stale period
   * (R02.T17): a target's readings are muted with their trailing `S`.
   */
  readonly stale?: boolean | undefined;
  /**
   * Called with each label's element as it mounts, and with `null` as it goes, keyed by its
   * target's key, so that the drawing loop can move it with its mark every frame (RM1 validation
   * m10) while its text changes at the readouts' 4 Hz.
   */
  readonly labelRef?: ((key: string, node: HTMLElement | null) => void) | undefined;
}

/**
 * The places a label other than a destination's tries, in order (R07.T16.i;
 * decision-r07-quality-and-destination, Q6 (a)): on its mark's line to the right, the side a name
 * reads from its point, then mirrored to the left; then below and above, centred on the mark.
 */
export const MARK_LABEL_SIDES = ["right", "left", "below", "above"] as const;

/** One of the places of {@link MARK_LABEL_SIDES}. */
export type MarkLabelSide = (typeof MARK_LABEL_SIDES)[number];

/**
 * Where a mark's label stands after a frame's placing, or that it is hidden whole (`null`), which the
 * next frame's placing starts from (R07.T16.i): the destination's label at one of its four places
 * above or below its whole chevron set (decision-r07-quality-and-destination, addendum B), any other
 * at one of its sides ({@link MARK_LABEL_SIDES}). `changesMs` holds the frame times, ms, of its
 * latest changes of place or of whether it is shown, the last at most {@link LABEL_CHANGE_LIMIT}
 * within {@link LABEL_CHANGE_WINDOW_MS}.
 */
export type MarkLabelState =
  | {
      readonly kind: "destination";
      readonly place: DestinationLabelPlace | null;
      readonly changesMs: ReadonlyArray<number>;
    }
  | {
      readonly kind: "mark";
      readonly place: MarkLabelSide | null;
      readonly changesMs: ReadonlyArray<number>;
    };

/** A label's plate's size as laid out, CSS px. */
export interface PlateSizePx {
  readonly widthPx: number;
  readonly heightPx: number;
}

/**
 * The plate's padding either side of its text, rem: `.view-marks__label`'s `padding: 0 0.25rem`. A
 * label at its mark's right or left has its text this far inside its plate's near edge. The plate
 * has none above and below, so a label below or above its mark stands its plate this much further
 * out, its text as far from the mark as at the right.
 */
export const PLATE_SIDE_PADDING_REM = 0.25;

/**
 * How far every label's plate stands clear of the chrome over the stage, rem: one base unit
 * (decision-r07-quality-and-destination, addendum C, C3's rule 3), so that no name 1 px under the
 * label block reads as a line of it. A destination's label's first choice stands
 * `LABEL_NEIGHBOUR_CLEARANCE_REM` clear.
 */
export const CHROME_CLEARANCE_REM = 0.25;

/**
 * How far every label's plate stands inside each edge of the stage, rem: one base unit, C3's rule-3
 * clearance, as P05's furniture stands (the label block and the instrument slots stand 0.5 rem
 * in), so that no plate covers the canvas's inset 2 px focus ring at any scale
 * (decision-r07-quality-and-destination, addendum D, D5). A destination's label keeps it in both
 * its tiers.
 */
export const EDGE_CLEARANCE_REM = 0.25;

/**
 * How much clearer than it must be a place must stand for a label to move to it, or to come back
 * to it from hidden, rem (R07.T16.i): one base unit, on every count, the stage's edges included,
 * and its mark's centre that far inside the stage (addendum D, D3). So a mark that jitters, or
 * moves a fraction of this back and forth across an obstacle's edge or the stage's, leaves its
 * label where it is.
 */
export const LABEL_HYSTERESIS_REM = 0.25;

/**
 * The most changes of place, or of whether it is shown, that a label makes in any
 * {@link LABEL_CHANGE_WINDOW_MS} (decision-r07-quality-and-destination, Q6 (a): "never more than
 * three times a second", the guide's flash limit). A label makes one fewer by choice, moving or
 * coming back; one that has made those keeps its place while it fits, and where its place stops
 * fitting it is hidden, never shown in part, as the last. It comes back once the window allows.
 */
export const LABEL_CHANGE_LIMIT = 3;

/** The window over which {@link LABEL_CHANGE_LIMIT} counts a label's changes, ms: a second. */
export const LABEL_CHANGE_WINDOW_MS = 1000;

/**
 * A label's CSS transform at one of its sides, in CSS px at `devicePixelRatio` (R07.T16.g;
 * R07.T16.i), the plate's own `translate: 0 -50%` centring it on the transform's line: at the right,
 * its plate's near edge `anchor.labelOffsetPx` right of the anchor, both in device px, on the
 * anchor's line; at the left, its right edge as far left, the transform's −100% moving it back by
 * its own width; below and above, centred on the anchor by the transform's −50% across, its top edge
 * `labelOffsetPx` and {@link PLATE_SIDE_PADDING_REM} below the anchor, the transform's +50% undoing
 * the plate's −50%, or its bottom edge as far above, the two −50% lifting it by its whole height.
 *
 * @remarks
 * The draw list places it clear of the selection's bracket, whether or not the mark is selected, so
 * that selecting a mark never moves its label.
 */
export function sideLabelTransform(
  anchor: DrawAnchor,
  devicePixelRatio: number,
  side: MarkLabelSide,
): string {
  const px = (devicePx: number): string => `${String(devicePx / devicePixelRatio)}px`;
  const { xPx, yPx, labelOffsetPx: nearPx } = anchor;
  const padding = `${String(PLATE_SIDE_PADDING_REM)}rem`;
  let transform: string;
  switch (side) {
    case "right":
      transform = `translate(${px(xPx + nearPx)}, ${px(yPx)})`;
      break;
    case "left":
      transform = `translate(calc(${px(xPx - nearPx)} - 100%), ${px(yPx)})`;
      break;
    case "below":
      transform = `translate(calc(${px(xPx)} - 50%), calc(${px(yPx + nearPx)} + ${padding} + 50%))`;
      break;
    case "above":
      transform = `translate(calc(${px(xPx)} - 50%), calc(${px(yPx - nearPx)} - ${padding} - 50%))`;
      break;
  }
  return transform;
}

/**
 * A destination's label's CSS transform at one of its places, in CSS px at `devicePixelRatio`
 * (decision-r07-quality-and-destination, addendum B): above or below the whole chevron set, its
 * bottom edge `risePx` above the anchor, the transform's −50% and the plate's own together lifting
 * it by its whole height, or its top edge as far below, the transform's +50% undoing the plate's
 * −50%; at the right its near edge `anchor.labelOffsetPx` right of the anchor, at the left its right
 * edge as far left, the transform's −100% moving it back by its own width.
 *
 * @remarks
 * The destination's report moves it in the frame in which its chevrons are first drawn, at once.
 *
 * @param risePx - The anchor's `labelRisePx`, device px.
 */
export function destinationLabelTransform(
  anchor: DrawAnchor,
  devicePixelRatio: number,
  place: DestinationLabelPlace,
  risePx: number,
): string {
  const px = (devicePx: number): string => `${String(devicePx / devicePixelRatio)}px`;
  const { xPx, yPx, labelOffsetPx: nearPx } = anchor;
  const across = placeIsRight(place) ? px(xPx + nearPx) : `calc(${px(xPx - nearPx)} - 100%)`;
  const down = placeIsUpper(place)
    ? `calc(${px(yPx - risePx)} - 50%)`
    : `calc(${px(yPx + risePx)} + 50%)`;
  return `translate(${across}, ${down})`;
}

/**
 * A label's CSS transform for its state ({@link sideLabelTransform},
 * {@link destinationLabelTransform}), or `null` where it is hidden whole. A destination's state on
 * an anchor with no rise, a mark no longer the destination, is hidden too; `markLabelPlaces` never
 * gives one, since it reads the part from the same frame's anchor.
 */
export function markLabelTransform(
  anchor: DrawAnchor,
  devicePixelRatio: number,
  state: MarkLabelState,
): string | null {
  let transform: string | null = null;
  switch (state.kind) {
    case "mark":
      if (state.place !== null) {
        transform = sideLabelTransform(anchor, devicePixelRatio, state.place);
      }
      break;
    case "destination":
      if (state.place !== null && anchor.labelRisePx !== null) {
        transform = destinationLabelTransform(
          anchor,
          devicePixelRatio,
          state.place,
          anchor.labelRisePx,
        );
      }
      break;
  }
  return transform;
}

/** The stage as the labels are placed on it: its size, CSS px, its device-pixel ratio and its rem. */
export type MarkLabelStage = Pick<
  ElementSize,
  "widthPx" | "heightPx" | "devicePixelRatio" | "remPx"
>;

/** What a frame's labels are placed among, besides their marks (R07.T16.i). */
export interface MarkLabelSurroundings {
  /**
   * The chrome over the stage as laid out, CSS px from its top left: the `PRIMARY` view's label
   * block, each open instrument slot, and any other statement or plate over the canvas.
   */
  readonly chrome?: ReadonlyArray<ScreenBoxPx>;
  /** The selection's target's key, whose label is placed after the destination's and before the rest. */
  readonly selection?: string | null;
  /** Each label's state after the frame before, by its target's key; none for a label just mounted. */
  readonly previous?: ReadonlyMap<string, MarkLabelState>;
  /** The frame's time, ms, by which each label's changes are counted. */
  readonly nowMs: number;
}

/** One place a label may take, and whether its plate fits there. */
interface PlaceOption<Place> {
  readonly place: Place;
  /**
   * Whether the plate fits at the place with `marginPx` more clearance on every count, its mark's
   * centre that far inside the stage; with `standing`, also clear of where the labels not yet placed
   * stood after the frame before.
   */
  readonly fits: (marginPx: number, standing: boolean) => boolean;
}

/** Whether two boxes overlap, sharing more than an edge. */
function overlaps(a: ScreenBoxPx, b: ScreenBoxPx): boolean {
  return (
    a.leftPx < b.leftPx + b.widthPx &&
    b.leftPx < a.leftPx + a.widthPx &&
    a.topPx < b.topPx + b.heightPx &&
    b.topPx < a.topPx + a.heightPx
  );
}

/** Whether `box` stands at least `clearPx` from every one of `others`, overlapping none. */
function clearOfAll(box: ScreenBoxPx, others: Iterable<ScreenBoxPx>, clearPx: number): boolean {
  for (const other of others) {
    if (overlaps(box, other) || boxGapPx(box, other) < clearPx) {
      return false;
    }
  }
  return true;
}

/**
 * The place a label takes among `options`, in their order of preference, from where it stood
 * (R07.T16.i): `undefined` for a label just mounted, or just made the destination or no longer it;
 * `null` for a label hidden.
 *
 * - A label keeps its place while it fits. It moves to a place before it only where that fits by
 *   `hysteresisPx`, clear of where every label not yet placed stood, so that it takes no standing
 *   label's place for a better one.
 * - A label whose place no longer fits leaves it at once, for the first place that fits by the
 *   margin, the labels after it in the order giving way; or it is hidden whole.
 * - A label hidden comes back only to a place that fits by the margin, and a label just mounted
 *   takes the first place that fits; each clear of where every label not yet placed stood.
 * - A label that has made all but one of its {@link LABEL_CHANGE_LIMIT} changes in the window
 *   (`limited`) keeps its place while it fits, or is hidden, and changes nothing else.
 */
function choosePlace<Place>(
  options: ReadonlyArray<PlaceOption<Place>>,
  previous: Place | null | undefined,
  hysteresisPx: number,
  limited: boolean,
): Place | null {
  if (previous !== undefined && previous !== null) {
    const kept = options.findIndex((option) => option.place === previous && option.fits(0, false));
    if (kept >= 0) {
      const better = limited
        ? undefined
        : options
            .slice(0, kept)
            .find((option) => option.place !== previous && option.fits(hysteresisPx, true));
      return better?.place ?? previous;
    }
    if (limited) {
      return null;
    }
    return options.find((option) => option.fits(hysteresisPx, false))?.place ?? null;
  }
  if (limited) {
    return null;
  }
  const marginPx = previous === undefined ? 0 : hysteresisPx;
  return options.find((option) => option.fits(marginPx, true))?.place ?? null;
}

/** A label's place as shown, or `null` where it is hidden or has no state: what a change changes. */
function shownPlace(state: MarkLabelState | undefined): string | null {
  return state === undefined || state.place === null ? null : `${state.kind} ${state.place}`;
}

/**
 * Each mark's label's place in a frame, or that it is hidden whole (R07.T16.i;
 * decision-r07-quality-and-destination, Q6 (a) and addenda B to D). A label is never shown in part,
 * and names only a mark in the picture: each plate, its size as laid out, takes a place only while
 * its mark's centre lies inside the stage (addendum D, D3), and only where it stands
 * {@link EDGE_CLEARANCE_REM} inside each of the stage's edges (D5), {@link CHROME_CLEARANCE_REM}
 * clear of the chrome and `LABEL_NEIGHBOUR_CLEARANCE_REM` clear of the plates placed before it
 * (D4).
 *
 * - **The order:** the destination's label first, then the selection's, then the rest by range,
 *   nearest first.
 * - **A destination's label** tries its four places (`DESTINATION_LABEL_PLACES`), each above or
 *   below its whole chevron set with its near edge at the bracket place: first the first
 *   `LABEL_NEIGHBOUR_CLEARANCE_REM` clear of the chrome and of every other mark, at its reach
 *   (`DrawAnchor.markReachPx`), and every other label's plate at its place on its mark's line; else
 *   the first {@link CHROME_CLEARANCE_REM} clear of the chrome; else it is hidden whole. Each place
 *   stands {@link EDGE_CLEARANCE_REM} inside the stage's edges.
 * - **Every other label** tries its sides ({@link MARK_LABEL_SIDES}) and takes the first
 *   `LABEL_NEIGHBOUR_CLEARANCE_REM` clear of the destination's plate, `LABEL_TEXT_CLEARANCE_REM`
 *   clear of the destination's chevron set (the square of half-size `destinationSetReachPx`),
 *   {@link CHROME_CLEARANCE_REM} clear of the chrome, and `LABEL_NEIGHBOUR_CLEARANCE_REM` clear of
 *   the plates placed; else it is hidden whole.
 * - **Hysteresis:** a label leaves a place, or comes back from hidden, only for a place clear by
 *   {@link LABEL_HYSTERESIS_REM} more, its mark's centre as far inside the stage, and changes at
 *   most {@link LABEL_CHANGE_LIMIT} times in {@link LABEL_CHANGE_WINDOW_MS} (`choosePlace`).
 *
 * @remarks
 * The destination's place counts every other mark at its bracket's place, whether or not it is
 * selected, and every other label at its place on its line, so that the selection is no input to
 * it (addendum C, C1). A label moves into a place another label stood at after the frame before
 * only where its own place has stopped fitting, and then only where that label comes after it in
 * the order. So selecting a mark, which changes only the order, moves no label, and the order decides
 * where labels come to meet. A label at its limit of changes is hidden, not moved, when its mark
 * becomes or stops being the destination, until the window allows. A label whose plate is not laid
 * out is hidden. So is a label whose mark's centre has left the stage, the destination's and the
 * selection's included, whatever place would fit; its mark stays pickable within the draw list's
 * `ANCHOR_MARGIN_REM`, and in the list, and the destination's chevron set still holds other labels
 * off.
 *
 * @param plates - The labels' plates' sizes as laid out, by their target's key.
 * @param stage - The stage's size, CSS px, its device-pixel ratio and its rem.
 * @returns Each labelled anchor's label's state, by its target's key.
 */
export function markLabelPlaces(
  anchors: ReadonlyArray<DrawAnchor>,
  plates: ReadonlyMap<string, PlateSizePx>,
  stage: MarkLabelStage,
  { chrome = [], selection = null, previous = new Map(), nowMs }: MarkLabelSurroundings,
): ReadonlyMap<string, MarkLabelState> {
  const rank = (anchor: DrawAnchor): number => {
    if (anchor.labelRisePx !== null) {
      return 0;
    }
    return targetKey(anchor.target) === selection ? 1 : 2;
  };
  const labelled = anchors.filter((anchor) => anchor.label !== null);
  const frame: LabelFrame = {
    stage,
    chrome,
    // The destination's chevron sets, drawn whether or not its label is shown.
    sets: anchors.flatMap((anchor) =>
      anchor.labelRisePx === null
        ? []
        : [
            squareBoxPx(
              centreCssPx(anchor, stage),
              destinationSetReachCssPx(anchor.labelRisePx, stage),
            ),
          ],
    ),
    destinationPlates: [],
    placed: [],
    held: new Map(),
  };
  // Where each label stood after the last frame, at this frame's mark, until it is placed.
  for (const anchor of labelled) {
    const key = targetKey(anchor.target);
    const plate = plates.get(key);
    const was = previous.get(key);
    const box =
      plate === undefined || was === undefined ? null : heldBoxPx(anchor, plate, was, stage);
    if (box !== null) {
      frame.held.set(key, box);
    }
  }
  const hysteresisPx = LABEL_HYSTERESIS_REM * stage.remPx;
  const states = new Map<string, MarkLabelState>();
  for (const anchor of labelled.toSorted((a, b) =>
    rank(a) === rank(b) ? a.distanceM - b.distanceM : rank(a) - rank(b),
  )) {
    const key = targetKey(anchor.target);
    frame.held.delete(key);
    const plate = plates.get(key);
    const was = previous.get(key);
    const changesMs = (was?.changesMs ?? []).filter(
      (changedMs) => changedMs > nowMs - LABEL_CHANGE_WINDOW_MS,
    );
    // The last change in the window is kept for hiding, which is never refused.
    const limited = changesMs.length >= LABEL_CHANGE_LIMIT - 1;
    const risePx = anchor.labelRisePx;
    let state: MarkLabelState;
    if (risePx === null) {
      const place = choosePlace(
        plate === undefined ? [] : sideOptions(anchor, plate, frame),
        was?.kind === "mark" ? was.place : undefined,
        hysteresisPx,
        limited,
      );
      state = { kind: "mark", place, changesMs };
      if (place !== null && plate !== undefined) {
        frame.placed.push(sideBoxPx(anchor, plate, place, stage));
      }
    } else {
      const place = choosePlace(
        plate === undefined
          ? []
          : destinationOptions(anchor, plate, risePx, anchors, plates, frame),
        was?.kind === "destination" ? was.place : undefined,
        hysteresisPx,
        limited,
      );
      state = { kind: "destination", place, changesMs };
      if (place !== null && plate !== undefined) {
        frame.destinationPlates.push(destinationBoxPx(anchor, plate, place, risePx, stage));
      }
    }
    states.set(
      key,
      shownPlace(state) === shownPlace(was)
        ? state
        : { ...state, changesMs: [...changesMs, nowMs].slice(-LABEL_CHANGE_LIMIT) },
    );
  }
  return states;
}

/** What one frame's labels are placed against, as they are placed in turn. */
interface LabelFrame {
  readonly stage: MarkLabelStage;
  readonly chrome: ReadonlyArray<ScreenBoxPx>;
  /** The destination's chevron sets, each the square that holds it. */
  readonly sets: ReadonlyArray<ScreenBoxPx>;
  /** The destination's label's plate, once placed. */
  readonly destinationPlates: ScreenBoxPx[];
  /** Every other label's plate placed so far. */
  readonly placed: ScreenBoxPx[];
  /** Where each label not yet placed stood after the frame before, by its target's key. */
  readonly held: Map<string, ScreenBoxPx>;
}

/**
 * Where a label stood after the frame before, its plate at this frame's mark, CSS px; `null` where
 * it was hidden, or where its mark has since become the destination or ceased to be.
 */
function heldBoxPx(
  anchor: DrawAnchor,
  plate: PlateSizePx,
  was: MarkLabelState,
  stage: MarkLabelStage,
): ScreenBoxPx | null {
  let box: ScreenBoxPx | null = null;
  switch (was.kind) {
    case "mark":
      if (was.place !== null && anchor.labelRisePx === null) {
        box = sideBoxPx(anchor, plate, was.place, stage);
      }
      break;
    case "destination":
      if (was.place !== null && anchor.labelRisePx !== null) {
        box = destinationBoxPx(anchor, plate, was.place, anchor.labelRisePx, stage);
      }
      break;
  }
  return box;
}

/**
 * Whether `box` lies inside the stage, at least {@link EDGE_CLEARANCE_REM} and `marginPx` in from
 * each of its edges (decision-r07-quality-and-destination, addendum D, D5).
 */
function insideBy(box: ScreenBoxPx, stage: MarkLabelStage, marginPx: number): boolean {
  const inPx = EDGE_CLEARANCE_REM * stage.remPx + marginPx;
  return (
    box.leftPx >= inPx &&
    box.topPx >= inPx &&
    box.leftPx + box.widthPx <= stage.widthPx - inPx &&
    box.topPx + box.heightPx <= stage.heightPx - inPx
  );
}

/**
 * Whether an anchor's centre lies inside the stage, at least `marginPx` in from each of its edges,
 * on them at no margin (decision-r07-quality-and-destination, addendum D, D3): a label names a mark
 * in the picture, and a mark whose centre has left it has none.
 */
function centreInsideBy(anchor: DrawAnchor, stage: MarkLabelStage, marginPx: number): boolean {
  const { xPx, yPx } = centreCssPx(anchor, stage);
  return (
    xPx >= marginPx &&
    yPx >= marginPx &&
    xPx <= stage.widthPx - marginPx &&
    yPx <= stage.heightPx - marginPx
  );
}

/**
 * A destination's label's places, in their order: each of its four places
 * `LABEL_NEIGHBOUR_CLEARANCE_REM` clear of the chrome, of every other mark at its reach and of every
 * other label's plate at its place on its line; then each {@link CHROME_CLEARANCE_REM} clear of the
 * chrome alone (decision-r07-quality-and-destination, addendum C, C3). Each stands
 * {@link EDGE_CLEARANCE_REM} inside the stage's edges, and fits only while the destination's centre
 * lies inside the stage (addendum D, D3 and D5).
 *
 * @param risePx - The anchor's `labelRisePx`, device px.
 */
function destinationOptions(
  anchor: DrawAnchor,
  plate: PlateSizePx,
  risePx: number,
  anchors: ReadonlyArray<DrawAnchor>,
  plates: ReadonlyMap<string, PlateSizePx>,
  frame: LabelFrame,
): ReadonlyArray<PlaceOption<DestinationLabelPlace>> {
  const { stage, chrome } = frame;
  const neighbourPx = LABEL_NEIGHBOUR_CLEARANCE_REM * stage.remPx;
  const neighbours: ScreenBoxPx[] = [];
  for (const other of anchors) {
    const key = targetKey(other.target);
    const otherPlate = other.label === null ? undefined : plates.get(key);
    if (key !== targetKey(anchor.target)) {
      neighbours.push(
        squareBoxPx(centreCssPx(other, stage), other.markReachPx / stage.devicePixelRatio),
      );
      if (otherPlate !== undefined) {
        neighbours.push(sideBoxPx(other, otherPlate, "right", stage));
      }
    }
  }
  const atEach = (clear: (box: ScreenBoxPx, marginPx: number) => boolean) =>
    DESTINATION_LABEL_PLACES.map((place): PlaceOption<DestinationLabelPlace> => ({
      place,
      fits: (marginPx) => {
        const box = destinationBoxPx(anchor, plate, place, risePx, stage);
        return (
          centreInsideBy(anchor, stage, marginPx) &&
          insideBy(box, stage, marginPx) &&
          clear(box, marginPx)
        );
      },
    }));
  return [
    ...atEach(
      (box, marginPx) =>
        clearOfAll(box, chrome, neighbourPx + marginPx) &&
        clearOfAll(box, neighbours, neighbourPx + marginPx),
    ),
    ...atEach((box, marginPx) =>
      clearOfAll(box, chrome, CHROME_CLEARANCE_REM * stage.remPx + marginPx),
    ),
  ];
}

/**
 * Any other label's places, its sides in their order, each {@link EDGE_CLEARANCE_REM} inside the
 * stage's edges, `LABEL_NEIGHBOUR_CLEARANCE_REM` clear of the destination's plate,
 * `LABEL_TEXT_CLEARANCE_REM` clear of its chevron set, {@link CHROME_CLEARANCE_REM} clear of the
 * chrome, and `LABEL_NEIGHBOUR_CLEARANCE_REM` clear of the plates placed; and with `standing`, as
 * clear of where the labels not yet placed stood, so that a label moving by choice never forces a
 * later one out (decision-r07-quality-and-destination, addendum D, D4). Each fits only while the
 * mark's centre lies inside the stage (D3).
 */
function sideOptions(
  anchor: DrawAnchor,
  plate: PlateSizePx,
  frame: LabelFrame,
): ReadonlyArray<PlaceOption<MarkLabelSide>> {
  const { stage } = frame;
  const neighbourPx = LABEL_NEIGHBOUR_CLEARANCE_REM * stage.remPx;
  return MARK_LABEL_SIDES.map((place): PlaceOption<MarkLabelSide> => ({
    place,
    fits: (marginPx, standing) => {
      const box = sideBoxPx(anchor, plate, place, stage);
      return (
        centreInsideBy(anchor, stage, marginPx) &&
        insideBy(box, stage, marginPx) &&
        clearOfAll(box, frame.chrome, CHROME_CLEARANCE_REM * stage.remPx + marginPx) &&
        clearOfAll(box, frame.destinationPlates, neighbourPx + marginPx) &&
        clearOfAll(box, frame.sets, LABEL_TEXT_CLEARANCE_REM * stage.remPx + marginPx) &&
        clearOfAll(box, frame.placed, neighbourPx + marginPx) &&
        (!standing || clearOfAll(box, frame.held.values(), neighbourPx + marginPx))
      );
    },
  }));
}

/** An anchor's place on the stage, CSS px from its top left. */
function centreCssPx(anchor: DrawAnchor, stage: MarkLabelStage): OffsetPx {
  return { xPx: anchor.xPx / stage.devicePixelRatio, yPx: anchor.yPx / stage.devicePixelRatio };
}

/**
 * A label's plate's box at one of its sides, CSS px from the stage's top left, as
 * {@link sideLabelTransform} stands it there.
 */
function sideBoxPx(
  anchor: DrawAnchor,
  plate: PlateSizePx,
  side: MarkLabelSide,
  stage: MarkLabelStage,
): ScreenBoxPx {
  const centre = centreCssPx(anchor, stage);
  const nearPx = anchor.labelOffsetPx / stage.devicePixelRatio;
  const outPx = nearPx + PLATE_SIDE_PADDING_REM * stage.remPx;
  const { widthPx, heightPx } = plate;
  let leftPx: number;
  let topPx: number;
  switch (side) {
    case "right":
      leftPx = centre.xPx + nearPx;
      topPx = centre.yPx - heightPx / 2;
      break;
    case "left":
      leftPx = centre.xPx - nearPx - widthPx;
      topPx = centre.yPx - heightPx / 2;
      break;
    case "below":
      leftPx = centre.xPx - widthPx / 2;
      topPx = centre.yPx + outPx;
      break;
    case "above":
      leftPx = centre.xPx - widthPx / 2;
      topPx = centre.yPx - outPx - heightPx;
      break;
  }
  return { leftPx, topPx, widthPx, heightPx };
}

/**
 * A destination's label's plate's box at one of its places, CSS px from the stage's top left, as
 * {@link destinationLabelTransform} stands it there (`destinationLabelBoxPx`).
 *
 * @param risePx - The anchor's `labelRisePx`, device px.
 */
function destinationBoxPx(
  anchor: DrawAnchor,
  plate: PlateSizePx,
  place: DestinationLabelPlace,
  risePx: number,
  stage: MarkLabelStage,
): ScreenBoxPx {
  const ratio = stage.devicePixelRatio;
  return destinationLabelBoxPx(
    place,
    centreCssPx(anchor, stage),
    anchor.labelOffsetPx / ratio,
    risePx / ratio,
    plate.widthPx,
    plate.heightPx,
  );
}

/**
 * The half-size of the square that holds a destination's whole chevron set, CSS px
 * (`destinationSetReachPx`): its label's rise less the 0.25 rem by which its plate clears the set,
 * since `destinationLabelRisePx` is the reach and `LABEL_TEXT_CLEARANCE_REM`.
 *
 * @param risePx - The destination's anchor's `labelRisePx`, device px.
 */
export function destinationSetReachCssPx(risePx: number, stage: MarkLabelStage): number {
  return risePx / stage.devicePixelRatio - LABEL_TEXT_CLEARANCE_REM * stage.remPx;
}

/**
 * The marks' DOM labels over the canvas (plan R02, R02.T15.b; T12 and T13 as built): another
 * craft's target mark with its range, `FROM CAMERA` where there is no own ship, and its closure
 * rate, `—` where a velocity is not known; and a body drawn as its symbol with its designation
 * (Design note 13), each on a `--surface-0` plate. The text is refreshed with the readouts at 4 Hz.
 * Each mounts hidden, at the published frame's place, until the drawing loop places it through
 * `labelRef` ({@link markLabelPlaces}) and shows it.
 *
 * @remarks
 * Hidden from assistive technology: the list beside the view carries the same names, ranges and
 * closure rates.
 */
export function ViewMarkLabels({
  anchors,
  devicePixelRatio,
  rows,
  stale = false,
  labelRef,
}: ViewMarkLabelsProps) {
  return (
    <div className="view-marks" aria-hidden="true">
      {anchors.map((anchor) => {
        const { label } = anchor;
        const key = targetKey(anchor.target);
        const row = rows.find((each) => each.key === key);
        if (label === null || row === undefined) {
          return null;
        }
        const target = label.kind === "target";
        const closure = target ? row.closure : NO_CLOSURE;
        const staleReadings = stale && target;
        return (
          <span
            key={key}
            ref={(node: HTMLSpanElement | null) => {
              if (node === null) {
                return undefined;
              }
              // Placed here once, as it mounts, not through `style`: a 4 Hz render would put it
              // back where the published frame drew its mark, behind the loop's latest.
              if (node.style.transform === "") {
                node.style.transform =
                  anchor.labelRisePx === null
                    ? sideLabelTransform(anchor, devicePixelRatio, "right")
                    : destinationLabelTransform(
                        anchor,
                        devicePixelRatio,
                        "upper-right",
                        anchor.labelRisePx,
                      );
                // The drawing loop chooses its place once its plate is laid out, and shows it there
                // (R07.T16.i), so that it is never shown in part, nor a frame at a place it then
                // leaves.
                node.style.visibility = "hidden";
              }
              labelRef?.(key, node);
              return () => {
                labelRef?.(key, null);
              };
            }}
            className="view-marks__label"
          >
            {row.name}
            {target ? (
              <>
                {" "}
                <span className={staleReadings ? "stale" : undefined}>
                  {rangeText(row)}
                  {closure.kind === "known" ? ` ${closure.text}` : null}
                </span>
                {closure.kind === "unknown" ? (
                  <>
                    {" "}
                    <span className="readout__missing">{MISSING_READING}</span>
                  </>
                ) : null}
              </>
            ) : null}
            {staleReadings ? <StaleMark /> : null}
          </span>
        );
      })}
    </div>
  );
}
