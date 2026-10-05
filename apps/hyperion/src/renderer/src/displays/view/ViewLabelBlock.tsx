import type { ReactNode } from "react";

import { StaleMark } from "../../components/StaleMark";
import { StatusLine } from "../../components/StatusLine";
import type { LabelLine } from "./viewRun";

/** Props of {@link ViewLabelBlock}. */
export interface ViewLabelBlockProps {
  /** The readings, refreshed at 4 Hz (`useThrottledValue` upstream). */
  readonly lines: ReadonlyArray<LabelLine>;
  /** Steady statements while their conditions hold: `POSITIONS AS SEEN FROM SHIP` and the rest. */
  readonly statements: ReadonlyArray<string>;
  /** The interim stars' count line (`STARS 1,204 DRAWN · …`), or `null` before an answer. */
  readonly countLine: string | null;
  /**
   * A graphics fault standing while the view draws (`GRAPHICS DEVICE LOST: re-creating`, or this
   * view's `GRAPHICS VIEW REFUSED: …`), or `null`.
   */
  readonly fault: string | null;
  /** The block's ID, by which its view's canvas is described (R07.T19), or none. */
  readonly id?: string | undefined;
}

/** The separator of a reading made of parts (the guide's "Voice and nomenclature"). */
const PART_SEPARATOR = " · ";

/**
 * The runs of a reading that never break (decision-r07-t19-layout, item 2), longest first: `UT`
 * with its first group (`UT +0 yr`), a star limit with its kind (`V 9.5 mag CAM`), an exposure
 * value with its band (`EV100 -1.0`), a clock reading (`000/00:00:01`) and a number with its sign
 * and unit (`1.00 km/s`, `12,480 km`); a number with a unit it touches (`60°`) has no break in it.
 */
const UNBREAKABLE =
  /UT [+-]?\d[\d,.]* yr|V -?\d[\d.]* mag (?:EYE|CAM)|EV100 -?\d[\d.]*|\d{3}\/\d{2}:\d{2}:\d{2}|[+-]?\d[\d,.]*(?:E[+-]?\d+)? (?:km\/s|m\/s|kyr|Myr|Gyr|yr|mag|AU|Gm|Mm|km|m|ly|s)(?![\w/])/g;

/**
 * A part of a reading with its unbreakable runs each set on one line, so that it breaks only at
 * the spaces between them.
 */
function partRuns(part: string): ReactNode {
  const nodes: ReactNode[] = [];
  let from = 0;
  for (const match of part.matchAll(UNBREAKABLE)) {
    if (match.index > from) {
      nodes.push(part.slice(from, match.index));
    }
    nodes.push(
      <span className="view-label__run" key={match.index}>
        {match[0]}
      </span>,
    );
    from = match.index + match[0].length;
  }
  if (nodes.length === 0) {
    return part;
  }
  if (from < part.length) {
    nodes.push(part.slice(from));
  }
  return nodes;
}

/**
 * A reading's parts, each kept on one line where it fits, so that a narrow block breaks it at its
 * middle dots first (decision-r07-t19, item 2a) and then at a space between its unbreakable runs
 * (decision-r07-t19-layout, item 2): `EV100 -1.0 MAN` stays whole where it fits and breaks before
 * its level only where it cannot; a part longer than the block still wraps between its words.
 */
function readingParts(value: string): ReactNode {
  const seen = new Map<string, number>();
  const nodes: ReactNode[] = [];
  for (const part of value.split(PART_SEPARATOR)) {
    if (nodes.length > 0) {
      nodes.push(PART_SEPARATOR);
    }
    const count = seen.get(part) ?? 0;
    seen.set(part, count + 1);
    nodes.push(
      <span className="view-label__part" key={`${part}:${String(count)}`}>
        {partRuns(part)}
      </span>,
    );
  }
  return nodes;
}

/**
 * The view's label block (plan R02, R02.T15.c; Design note 16): the display class `VIEW` and what
 * the picture is, its frame, time, style, camera, field of view, exposure and star source, as DOM
 * text on a `--surface-0` plate over the canvas, never drawn into it.
 *
 * @remarks
 * A reading breaks at its middle dots first, then at a space, never inside a quantity, between a
 * star limit and its kind, after `UT` or inside a clock reading, and its continuation lines hang
 * under the value (decision-r07-t19-layout, item 2).
 * Each reading is an `output`; none is announced as it changes, since they change continuously. A
 * reading of a stale server scene is muted with its trailing `S` (the guide's "Data states"). A
 * graphics fault is set as `StatusLine`'s fault, in `--status-caution`, apart from the steady
 * statements (the guide's "Alerts": a console's report on its own graphics is never an alert). The
 * stars' count line is a reading of numbers, so an `output` in B612 Mono (the guide's
 * "Typography"), not a statement.
 */
export function ViewLabelBlock({ lines, statements, countLine, fault, id }: ViewLabelBlockProps) {
  return (
    <div className="view-label" id={id}>
      <p className="view-label__class">VIEW</p>
      <dl className="view-label__lines">
        {lines.map((line) => (
          <div className="field view-label__line" key={line.label}>
            <dt className="field__label">{line.label}</dt>
            <dd>
              <output aria-live="off" className={line.stale === true ? "stale" : undefined}>
                {readingParts(line.value)}
              </output>
              {line.stale === true ? <StaleMark /> : null}
            </dd>
          </div>
        ))}
      </dl>
      {statements.map((statement) => (
        <p className="view-label__statement" key={statement}>
          {statement}
        </p>
      ))}
      {countLine === null ? null : (
        <p className="view-label__count">
          <output aria-live="off">{countLine}</output>
        </p>
      )}
      {fault === null ? null : <StatusLine text={fault} standing="fault" />}
    </div>
  );
}
