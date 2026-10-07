import { StaleMark } from "../../components/StaleMark";
import type { ElementSize } from "../../lib/useElementSize";
import {
  boxGapPx,
  type DestinationLabelPlace,
  destinationLabelBoxPx,
  destinationLabelPlace,
  LABEL_NEIGHBOUR_CLEARANCE_REM,
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
 * Where a mark's label stands about its mark: on its mark's line, to its right, or for the
 * destination's label one of its four places above or below its whole chevron set
 * (decision-r07-quality-and-destination, addendum B).
 */
export type MarkLabelPlace = "line" | DestinationLabelPlace;

/** A label's plate's size as laid out, CSS px. */
export interface PlateSizePx {
  readonly widthPx: number;
  readonly heightPx: number;
}

/**
 * A label's CSS transform at its place beside its mark, in CSS px at `devicePixelRatio` (R07.T16.g):
 * its plate's near edge `anchor.labelOffsetPx` right of the anchor, both in device px, centred on
 * the anchor's line by the plate's own `translate: 0 -50%`. At a destination's place
 * (`anchor.labelRisePx`; decision-r07-quality-and-destination, addendum B) its plate stands above or
 * below the whole chevron set: its bottom edge that far above the anchor, the transform's −50% and
 * the plate's own together lifting it by its whole height, or its top edge as far below, the
 * transform's +50% undoing the plate's −50%; and at a left-hand place its right edge stands as far
 * left of the anchor, the transform's −100% moving it back by its own width.
 *
 * @remarks
 * The draw list places it clear of the selection's bracket, whether or not the mark is selected, so
 * that selecting a mark never moves its label, and the destination's report moves it in the frame
 * in which its chevrons are first drawn, at once. A mark that is not the destination has no rise, and
 * its label stands on its line whatever place is asked.
 */
export function markLabelTransform(
  anchor: DrawAnchor,
  devicePixelRatio: number,
  place: MarkLabelPlace,
): string {
  const px = (devicePx: number): string => `${String(devicePx / devicePixelRatio)}px`;
  if (place === "line" || anchor.labelRisePx === null) {
    return `translate(${px(anchor.xPx + anchor.labelOffsetPx)}, ${px(anchor.yPx)})`;
  }
  const across = placeIsRight(place)
    ? px(anchor.xPx + anchor.labelOffsetPx)
    : `calc(${px(anchor.xPx - anchor.labelOffsetPx)} - 100%)`;
  const down = placeIsUpper(place)
    ? `calc(${px(anchor.yPx - anchor.labelRisePx)} - 50%)`
    : `calc(${px(anchor.yPx + anchor.labelRisePx)} + 50%)`;
  return `translate(${across}, ${down})`;
}

/**
 * Each mark's label's place (decision-r07-quality-and-destination, addendum B): every label on its
 * mark's line but the destination's, which takes the first of its four places whose plate lies
 * inside the stage and stands `LABEL_NEIGHBOUR_CLEARANCE_REM` clear of every other mark, at its
 * reach (`DrawAnchor.markReachPx`), of every other label's plate, at its place on its line, and of
 * the chrome over the stage (`destinationLabelPlace`); or the upper right where its plate is not yet
 * laid out.
 *
 * @remarks
 * Every other mark is taken at its bracket's place, whether or not it is selected, so that selecting
 * a mark never moves the destination's label. The chrome is the stage's as R07.T16.i names it: the
 * `PRIMARY` view's label block and each open instrument slot. T16.i keeps every other label clear of
 * it too.
 *
 * @param plates - The labels' plates' sizes as laid out, by their target's key.
 * @param stage - The stage's size, CSS px, its device-pixel ratio and its rem.
 * @param chrome - The chrome's boxes as laid out, CSS px from the stage's top left.
 * @returns The places by the labels' targets' keys.
 */
export function markLabelPlaces(
  anchors: ReadonlyArray<DrawAnchor>,
  plates: ReadonlyMap<string, PlateSizePx>,
  stage: Pick<ElementSize, "widthPx" | "heightPx" | "devicePixelRatio" | "remPx">,
  chrome: ReadonlyArray<ScreenBoxPx> = [],
): ReadonlyMap<string, MarkLabelPlace> {
  const ratio = stage.devicePixelRatio;
  const centreOf = (anchor: DrawAnchor): OffsetPx => ({
    xPx: anchor.xPx / ratio,
    yPx: anchor.yPx / ratio,
  });
  const places = new Map<string, MarkLabelPlace>();
  for (const anchor of anchors) {
    if (anchor.label === null) {
      continue;
    }
    const key = targetKey(anchor.target);
    const plate = plates.get(key);
    const risePx = anchor.labelRisePx;
    if (risePx === null || plate === undefined) {
      places.set(key, risePx === null ? "line" : "upper-right");
      continue;
    }
    const neighbours: ScreenBoxPx[] = [...chrome];
    for (const other of anchors) {
      const otherKey = targetKey(other.target);
      if (otherKey === key) {
        continue;
      }
      const centre = centreOf(other);
      neighbours.push(squareBoxPx(centre, other.markReachPx / ratio));
      const otherPlate = other.label === null ? undefined : plates.get(otherKey);
      if (otherPlate !== undefined) {
        neighbours.push({
          leftPx: centre.xPx + other.labelOffsetPx / ratio,
          topPx: centre.yPx - otherPlate.heightPx / 2,
          widthPx: otherPlate.widthPx,
          heightPx: otherPlate.heightPx,
        });
      }
    }
    const clearPx = LABEL_NEIGHBOUR_CLEARANCE_REM * stage.remPx;
    places.set(
      key,
      destinationLabelPlace(
        (place) =>
          destinationLabelBoxPx(
            place,
            centreOf(anchor),
            anchor.labelOffsetPx / ratio,
            risePx / ratio,
            plate.widthPx,
            plate.heightPx,
          ),
        (box) =>
          box.leftPx >= 0 &&
          box.topPx >= 0 &&
          box.leftPx + box.widthPx <= stage.widthPx &&
          box.topPx + box.heightPx <= stage.heightPx,
        (box) => neighbours.every((other) => boxGapPx(box, other) >= clearPx),
      ),
    );
  }
  return places;
}

/**
 * The marks' DOM labels over the canvas (plan R02, R02.T15.b; T12 and T13 as built): another
 * craft's target mark with its range, `FROM CAMERA` where there is no own ship, and its closure
 * rate, `—` where a velocity is not known; and a body drawn as its symbol with its designation
 * (Design note 13), each on a `--surface-0` plate. The text is refreshed with the readouts at 4 Hz;
 * the position is the published frame's until the drawing loop moves it through `labelRef`.
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
                node.style.transform = markLabelTransform(
                  anchor,
                  devicePixelRatio,
                  anchor.labelRisePx === null ? "line" : "upper-right",
                );
                // The drawing loop chooses a destination's place once the plates are laid out, and
                // shows it there, so that it never shows a frame at a place it then leaves.
                if (anchor.labelRisePx !== null) {
                  node.style.visibility = "hidden";
                }
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
