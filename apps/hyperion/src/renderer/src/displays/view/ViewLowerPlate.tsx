import type { ReactNode, Ref } from "react";

import { StatusText } from "../../components/StatusText";
import { type LowerPartKind, type LowerPlatePart, lowerYieldLine } from "./viewRoom";

/** Props of {@link ViewLowerPlate}. */
export interface ViewLowerPlateProps {
  /** Its parts in the guide's order, as R10.T12 fills them ({@link LowerPlatePart}). */
  readonly parts: ReadonlyArray<LowerPlatePart<ReactNode>>;
  /** Whether the readout stands or gives way to its yield line (`ViewRoom.readout`). */
  readonly readout: "stands" | "yields";
  /** Whether the survey line's values and `DATUM` stand or give way (`ViewRoom.survey`). */
  readonly survey: "stands" | "yields";
  /** Each part in its tallest form, as the hidden sizer lays it out; as it stands now by default. */
  readonly tallest?: boolean | undefined;
  /** The plate's ID, by which its view's canvas is described, or none. */
  readonly id?: string | undefined;
  /** More class names for the plate: `view-label--beside` beside the standing slots. */
  readonly className?: string | undefined;
  /** Receives the plate, which the display measures. */
  readonly ref?: Ref<HTMLDivElement> | undefined;
}

/** A part that gives way: the survey line's values, `DATUM` or the readout. */
type YieldingKind = Exclude<LowerPartKind, "state">;

/** One entry of the plate as it is set: a part, or the yield line in the place of those held back. */
type Entry =
  | { readonly kind: "part"; readonly key: string; readonly node: ReactNode }
  | { readonly kind: "yield"; readonly key: string; readonly text: string };

/**
 * The plate's entries in order: each part that stands, and where parts give way, one yield line in
 * the place of the first of them, naming every part it holds back (decision-r10-view-block-room
 * §3); the state lines always stand.
 */
function entriesOf(
  parts: ViewLowerPlateProps["parts"],
  yielded: ReadonlySet<YieldingKind>,
  tallest: boolean,
): Entry[] {
  const heldBack = parts.flatMap((part): YieldingKind[] =>
    part.kind !== "state" && yielded.has(part.kind) ? [part.kind] : [],
  );
  const entries: Entry[] = [];
  let yieldSet = false;
  for (const part of parts) {
    if (part.kind === "state") {
      entries.push({ kind: "part", key: `state ${part.key}`, node: part.node });
    } else if (!yielded.has(part.kind)) {
      entries.push({ kind: "part", key: part.kind, node: tallest ? part.tallest : part.node });
    } else if (!yieldSet) {
      yieldSet = true;
      entries.push({ kind: "yield", key: "yield", text: lowerYieldLine(heldBack) });
    }
  }
  return entries;
}

/**
 * The `PRIMARY` view's lower plate (plan R10, R10.T12.b; decision-r10-view-block-room §3): a
 * `--surface-0` plate at the stage's bottom left, inset 0.5 rem and standing on its foot, so that
 * nothing on it moves as the upper plate's statements come and go, holding R10.T12's survey line
 * or its state, `DATUM`, then `GROUND AT LINE OF SIGHT` and its readings.
 *
 * @remarks
 * Where the stage cannot hold both plates whole the readout gives way first, to
 * `NOT AVAILABLE: no room for GROUND AT LINE OF SIGHT at this window size`, and then the survey
 * line's values and `DATUM` with it, to one line naming only what it holds back
 * (`NOT AVAILABLE: no room for SURVEY COVERAGE, DATUM or GROUND AT LINE OF SIGHT at this window size`,
 * or `NOT AVAILABLE: no room for SURVEY COVERAGE at this window size` where the survey line stands
 * alone), set as a statement in the place of the first part held back. A state line never gives
 * way. The parts are R10.T12's own markup; the plate sets them in order.
 */
export function ViewLowerPlate({
  parts,
  readout,
  survey,
  tallest = false,
  id,
  className,
  ref,
}: ViewLowerPlateProps) {
  const yielded = new Set<YieldingKind>([
    ...(readout === "yields" || survey === "yields" ? (["readout"] as const) : []),
    ...(survey === "yields" ? (["survey", "datum"] as const) : []),
  ]);
  return (
    <div
      className={
        className === undefined
          ? "view-label view-label--lower"
          : `view-label view-label--lower ${className}`
      }
      id={id}
      ref={ref}
    >
      {entriesOf(parts, yielded, tallest).map((entry) =>
        entry.kind === "part" ? (
          <div className="view-label__group" key={entry.key}>
            {entry.node}
          </div>
        ) : (
          <p className="view-label__statement view-label__yield" key={entry.key}>
            <StatusText text={entry.text} />
          </p>
        ),
      )}
    </div>
  );
}
