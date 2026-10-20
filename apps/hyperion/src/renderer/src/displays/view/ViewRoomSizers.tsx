import type { ReactNode, Ref } from "react";

import { ViewLabelBlock, type ViewLabelBlockProps } from "./ViewLabelBlock";
import { ViewLowerPlate } from "./ViewLowerPlate";
import type { LowerPlatePart, UpperRoom } from "./viewRoom";

/** The class of a plate beside the standing slots, which takes the width left of them. */
export const BESIDE_CLASS = "view-label--beside";

/** Props of {@link ViewRoomSizers}. */
export interface ViewRoomSizersProps {
  /** The upper plate's content, for its copy beside the slots. */
  readonly upper: Pick<ViewLabelBlockProps, "lines" | "statements" | "countLine" | "faults">;
  /** The lower plate's parts, or none while the view states no terrain lines. */
  readonly lower: ReadonlyArray<LowerPlatePart<ReactNode>>;
  /** Whether the live plates stand beside the slots, at whose width the lower plate's forms go. */
  readonly liveBeside: boolean;
  /** Whether to lay out both plates beside the slots too: while the slots are held for them. */
  readonly beside: boolean;
  /** The label of the upper plate's first line that never scrolls (`STARS`). */
  readonly scrollsFrom: string;
  /** Receives the sizers' root, in which `measureRoom` finds the copies. */
  readonly ref?: Ref<HTMLDivElement> | undefined;
}

/** Props of {@link LowerForms}. */
interface LowerFormsProps {
  readonly parts: ReadonlyArray<LowerPlatePart<ReactNode>>;
  /** The plates' class beside the slots, or none at the full width. */
  readonly className: string | undefined;
}

/** A block laid out whole. */
const WHOLE: UpperRoom = { kind: "whole" };

/** The lower plate's forms at its tallest: whole, its readout given way, and its survey too. */
function LowerForms({ parts, className }: LowerFormsProps) {
  const hasReadout = parts.some((part) => part.kind === "readout");
  const hasSurvey = parts.some((part) => part.kind === "survey" || part.kind === "datum");
  return (
    <>
      <div data-form="whole">
        <ViewLowerPlate
          parts={parts}
          readout="stands"
          survey="stands"
          tallest
          className={className}
        />
      </div>
      {hasReadout ? (
        <div data-form="readout">
          <ViewLowerPlate
            parts={parts}
            readout="yields"
            survey="stands"
            tallest
            className={className}
          />
        </div>
      ) : null}
      {hasSurvey ? (
        <div data-form="survey">
          <ViewLowerPlate
            parts={parts}
            readout="yields"
            survey="yields"
            tallest
            className={className}
          />
        </div>
      ) : null}
    </>
  );
}

/**
 * The hidden copies by which `VIEW`'s room is reckoned (plan R10, R10.T12.b;
 * decision-r10-view-block-room §3): a position line's sample; the lower plate's forms, each part
 * at its tallest, at the live plates' width; and, while the slots are held for the plates, the
 * upper plate whole and the lower plate's forms beside where the slots would stand, by which they
 * return.
 *
 * @remarks
 * Laid out over the stage as the live plates are, so that their widths resolve alike, but never
 * seen, never read by assistive technology and never taking the pointer. A lower part's markup
 * carries no `id` of its own, since a copy of it stands here. The upper plate's copy sets its lines
 * before `STARS` at a scroll's width, a scroll bar standing beside them, so that it reckons them as
 * tall as they would stand scrolled.
 */
export function ViewRoomSizers({
  upper,
  lower,
  liveBeside,
  beside,
  scrollsFrom,
  ref,
}: ViewRoomSizersProps) {
  return (
    <div className="view-room-sizers" aria-hidden="true" ref={ref}>
      <div className="view-label view-room-sizers__sample">
        <p className="list-position view-label__position" data-sizer="position">
          1-1 of 1
        </p>
      </div>
      {lower.length === 0 ? null : (
        <div data-sizer="lower">
          <LowerForms parts={lower} className={liveBeside ? BESIDE_CLASS : undefined} />
        </div>
      )}
      {beside ? (
        <div data-sizer="beside">
          <div data-form="upper">
            <ViewLabelBlock
              {...upper}
              scroll={{ from: scrollsFrom, room: WHOLE, view: "" }}
              className={BESIDE_CLASS}
            />
          </div>
          {lower.length === 0 ? null : <LowerForms parts={lower} className={BESIDE_CLASS} />}
        </div>
      ) : null}
    </div>
  );
}
