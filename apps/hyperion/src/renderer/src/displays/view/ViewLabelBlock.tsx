import type { ReactNode } from "react";

import { StaleMark } from "../../components/StaleMark";
import { StatusLine } from "../../components/StatusLine";
import { type LabelField, type LabelLine, MISSING_READING } from "./viewRun";

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
 * value with its band (`EV100 -1.0`), a direction with its elevation and any `FROM +X`
 * (`047° +12° FROM +X`, or `— -90°` at the vertical; R07.T19.f), a clock reading
 * (`000/00:00:01`) and a number with its sign and unit (`1.00 km/s`, `12,480 km`, `100 s/s`); a
 * number with a unit it touches (`60°`) has no break in it.
 */
const UNBREAKABLE =
  /UT [+-]?\d[\d,.]* yr|V -?\d[\d.]* mag (?:EYE|CAM)|EV100 -?\d[\d.]*|(?:\d{3}°|—) [+-]\d{2,}°(?: FROM \+X)?|\d{3}\/\d{2}:\d{2}:\d{2}|[+-]?\d[\d,.]*(?:E[+-]?\d+)? (?:km\/s|m\/s|s\/s|kyr|Myr|Gyr|yr|mag|AU|Gm|Mm|km|m|ly|s)(?![\w/])/g;

/**
 * A text with each of its runs set on one line, so that it breaks only at the spaces between them
 * (decision-r07-t19-layout, item 2): a reading's quantities, or the exposure's status phrases in
 * its notes (decision-r07-owner-ux-signoff, item 1). Its text is unchanged.
 *
 * @param runs - The runs that never break, a global pattern.
 * @param field - A value of the text set in a field of its own: the run that reads its text.
 */
export function unbrokenRuns(text: string, runs: RegExp, field?: LabelField): ReactNode {
  const nodes: ReactNode[] = [];
  let from = 0;
  for (const match of text.matchAll(runs)) {
    if (match.index > from) {
      nodes.push(withMissing(text.slice(from, match.index), `t${String(from)}`));
    }
    nodes.push(
      field !== undefined && match[0] === field.text ? (
        <FieldRun field={field} key={match.index} />
      ) : (
        <span className="view-label__run" key={match.index}>
          {withMissing(match[0], `r${String(match.index)}`)}
        </span>
      ),
    );
    from = match.index + match[0].length;
  }
  if (nodes.length === 0) {
    return withMissing(text, "t0");
  }
  if (from < text.length) {
    nodes.push(withMissing(text.slice(from), `t${String(from)}`));
  }
  return nodes;
}

/** Props of {@link FieldRun}. */
interface FieldRunProps {
  /** The value and its field: its text, width and staleness. */
  readonly field: LabelField;
}

/**
 * A value set in its field: a run right-aligned in its fixed width, and while it is stale muted
 * with its trailing `S` inside that width, so that neither its growth nor the mark moves the line
 * (the guide's "Numbers" and "Data states"; R06.T11.f).
 */
function FieldRun({ field }: FieldRunProps) {
  return (
    <span
      className="view-label__run view-label__field"
      style={{ minWidth: `${String(field.widthCh)}ch` }}
    >
      {field.stale ? (
        <>
          <span className="stale">{field.text}</span>
          <StaleMark />
        </>
      ) : (
        field.text
      )}
    </span>
  );
}

/**
 * A text with each em dash in it set as the missing value, in `--text-muted` (the guide's "Data
 * states"), such as a direction's azimuth at the vertical (R07.T19.f); a text without one is
 * returned as it is.
 *
 * @param key - The text's key among its siblings, from which its parts' keys are made.
 */
function withMissing(text: string, key: string): ReactNode {
  if (!text.includes(MISSING_READING)) {
    return text;
  }
  const nodes: ReactNode[] = [];
  for (const [index, piece] of text.split(MISSING_READING).entries()) {
    if (index > 0) {
      nodes.push(
        <span className="readout__missing" key={`${key}-m${String(index)}`}>
          {MISSING_READING}
        </span>,
      );
    }
    if (piece.length > 0) {
      nodes.push(piece);
    }
  }
  return nodes;
}

/**
 * A reading's parts, each kept on one line where it fits, so that a narrow block breaks it at its
 * middle dots first (decision-r07-t19, item 2a) and then at a space between its unbreakable runs
 * (decision-r07-t19-layout, item 2): `EV100 -1.0 MAN` stays whole where it fits and breaks before
 * its level only where it cannot; a part longer than the block still wraps between its words.
 *
 * @remarks
 * The exposure panel's and the meter's readings are set through it too, so that the same reading
 * breaks by the same rules wherever it stands: `EV100 11.7 INHIBITED ·` | `NO IMAGE TO METER`,
 * never inside a status phrase (decision-r07-t19b-exposure-fit, item 1(c)). Its text is the
 * reading's, unchanged.
 *
 * @param field - A value of the reading set in a field of its own (`LabelLine.field`), or none.
 */
export function readingParts(value: string, field?: LabelField): ReactNode {
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
        {unbrokenRuns(part, UNBREAKABLE, field)}
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
 * reading of a stale server scene is muted with its trailing `S` (the guide's "Data states"); a
 * line's field, the sky's edge, is set in its fixed width and goes stale alone (R06.T11.f). A
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
                {readingParts(line.value, line.field)}
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
