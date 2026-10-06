import { StaleMark } from "../../components/StaleMark";
import { markShiftDevicePx } from "../../lib/strokes";
import type { DrawAnchor } from "../../view/wireframe/drawList";
import { reticleGrowthPx } from "../../view/wireframe/symbology";
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

/** The space between a mark and its label, rem, beyond the selection's reticle as built. */
export const LABEL_OFFSET_REM = 0.75;

/**
 * How much further a label stands off its mark below a device-pixel ratio of 4/3, CSS px: the
 * selection's reticle's growth there ({@link reticleGrowthPx}), so that the reticle clears the
 * label's `--surface-0` plate by what it did as built (R07.T16.d; decision-thin-line-contrast). It is
 * 2.65 px at a ratio of 0.78125, 1.25 at 1, and 0 from 4/3 up.
 */
export function markLabelShiftPx(devicePixelRatio: number): number {
  const ratio = devicePixelRatio > 0 && Number.isFinite(devicePixelRatio) ? devicePixelRatio : 1;
  return reticleGrowthPx(markShiftDevicePx(ratio)) / ratio;
}

/** A label's CSS transform beside its mark: `anchor` in device px, at `devicePixelRatio`. */
export function markLabelTransform(anchor: DrawAnchor, devicePixelRatio: number): string {
  const shiftPx = markLabelShiftPx(devicePixelRatio);
  const shift = shiftPx > 0 ? ` + ${String(shiftPx)}px` : "";
  return `translate(calc(${String(anchor.xPx / devicePixelRatio)}px + ${String(LABEL_OFFSET_REM)}rem${shift}), ${String(anchor.yPx / devicePixelRatio)}px)`;
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
                node.style.transform = markLabelTransform(anchor, devicePixelRatio);
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
