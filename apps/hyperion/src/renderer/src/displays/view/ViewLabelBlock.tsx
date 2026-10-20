import { type ReactNode, type Ref, useCallback, useLayoutEffect, useRef } from "react";

import { StaleMark } from "../../components/StaleMark";
import { StatusLine } from "../../components/StatusLine";
import { StatusText } from "../../components/StatusText";
import { formatListPosition } from "../../lib/format";
import { type ItemsInView, useItemsInView } from "../../lib/itemsInView";
import type { UpperRoom } from "./viewRoom";
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
   * The faults standing while the view draws, each set as its own fault line, in order: a graphics
   * fault first (`GRAPHICS DEVICE LOST: re-creating`, or this view's `GRAPHICS VIEW REFUSED: …`),
   * then the terrain's (`TERRAIN NOT AVAILABLE: …`, plan R10, R10.T16.b; decision-r10-t16-contracts
   * §7), so that a device loss never hides the standing one; none while nothing is wrong.
   */
  readonly faults: ReadonlyArray<string>;
  /** The block's ID, by which its view's canvas is described (R07.T19), or none. */
  readonly id?: string | undefined;
  /**
   * How the block scrolls in its room, the `PRIMARY` view's upper plate's (plan R10, R10.T12.b;
   * decision-r10-view-block-room §3); none for a block that never scrolls, an instrument's.
   */
  readonly scroll?: LabelScroll | undefined;
  /** More class names for the plate: `view-label--beside` beside the standing slots. */
  readonly className?: string | undefined;
  /** Receives the plate, which the display measures for its room (R10.T12.b). */
  readonly ref?: Ref<HTMLDivElement> | undefined;
}

/** How a label block scrolls in its room ({@link ViewLabelBlockProps.scroll}). */
export interface LabelScroll {
  /**
   * The label of its first line that never scrolls, `STARS`: the lines before it, `FRAME` to
   * `METER`, stand in a part of their own that scrolls while `room` has them scroll.
   */
  readonly from: string;
  /** How the block stands in its room ({@link UpperRoom}). */
  readonly room: UpperRoom;
  /**
   * The view's name, `PRIMARY`, by which a scrolling part is named while it scrolls: `PRIMARY lines`
   * for its lines, `PRIMARY label block` for everything under its `VIEW` line.
   */
  readonly view: string;
}

/** What a reading of the block counts as one in the whole plate's scroll (step 6). */
const BODY_ITEMS = ".view-label__line, .view-label__statement, .view-label__count, .request-status";

/** A scroll's position line under it, `1-6 of 11` (the guide's "Layout"), or none before it is measured. */
function positionLine(range: ItemsInView | null): ReactNode {
  return range === null ? null : (
    <p className="list-position view-label__position">
      {formatListPosition(range.firstVisible, range.lastVisible, range.total)}
    </p>
  );
}

/** Whether a scrolling element stands at its foot, to the pixel. */
function atFoot(element: HTMLElement): boolean {
  return element.scrollTop + element.clientHeight >= element.scrollHeight - 1;
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
 * A reading's fields as its runs take them: in the reading's order, each once (R06.T13.j). `next`
 * is the index of the field the next run may take.
 */
interface FieldQueue {
  readonly fields: ReadonlyArray<LabelField>;
  next: number;
}

/**
 * A text with each of its runs set on one line, so that it breaks only at the spaces between them
 * (decision-r07-t19-layout, item 2): a reading's quantities, or the exposure's status phrases in
 * its notes (decision-r07-owner-ux-signoff, item 1). Its text is unchanged.
 *
 * @param runs - The runs that never break, a global pattern.
 * @param fields - Values of the text set in fields of their own, in the text's order, each taken
 *   once: each the run that reads its text, the sky's edge, or the start of one before a space, the
 *   sky's limit before its kind, whose rest stays on the run's line after the field (R06.T13.j).
 */
export function unbrokenRuns(
  text: string,
  runs: RegExp,
  fields: ReadonlyArray<LabelField> = [],
): ReactNode {
  return setRuns(text, runs, { fields, next: 0 });
}

/**
 * {@link unbrokenRuns} with the reading's fields as a queue, which the runs of every part of one
 * reading take from in turn.
 */
function setRuns(text: string, runs: RegExp, queue: FieldQueue): ReactNode {
  const nodes: ReactNode[] = [];
  let from = 0;
  for (const match of text.matchAll(runs)) {
    if (match.index > from) {
      nodes.push(withMissing(text.slice(from, match.index), `t${String(from)}`));
    }
    const run = match[0];
    const field = queue.fields[queue.next];
    const whole = field !== undefined && field.text === run;
    const leading = field !== undefined && run.startsWith(`${field.text} `);
    let node: ReactNode;
    if (field !== undefined && whole) {
      queue.next += 1;
      node = <FieldRun field={field} isRun key={match.index} />;
    } else if (field !== undefined && leading) {
      queue.next += 1;
      node = (
        <span className="view-label__run" key={match.index}>
          <FieldRun field={field} isRun={false} />
          {withMissing(run.slice(field.text.length), `r${String(match.index)}`)}
        </span>
      );
    } else {
      node = (
        <span className="view-label__run" key={match.index}>
          {withMissing(run, `r${String(match.index)}`)}
        </span>
      );
    }
    nodes.push(node);
    from = match.index + run.length;
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
  /**
   * Whether the field is a run of its own: `true` for a whole run that never breaks, the sky's
   * edge; `false` for the start of one, the sky's limit, set inside its run before its kind.
   */
  readonly isRun: boolean;
}

/**
 * A value set in its field: right-aligned in its fixed width, and while it is stale muted with its
 * trailing `S` inside that width, so that neither its growth nor the mark moves the line (the
 * guide's "Numbers" and "Data states"; R06.T11.f, and for the limit, whose `S` so stands after its
 * unit and before its kind, R06.T13.j).
 */
function FieldRun({ field, isRun }: FieldRunProps) {
  return (
    <span
      className={isRun ? "view-label__run view-label__field" : "view-label__field"}
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

/** How a reading's parts are set. */
export interface ReadingOptions {
  /**
   * Whether each middle dot leads the part after it (R06.T13.j): set at the head of that part's
   * box, on one line with its first word or run, the break at the space before it, so that no dot
   * stands alone at a line's end. Off unless asked: the sky's `STARS` readings and the wireframe's
   * `CONTOUR INTERVAL` ask it (`LabelLine.dotsLead`), and every other reading keeps its dots
   * between its parts.
   */
  readonly dotsLead?: boolean;
}

/** A run of {@link UNBREAKABLE} at the head of a text, matched once (neither global nor sticky). */
const UNBREAKABLE_AT_HEAD = new RegExp(`^(?:${UNBREAKABLE.source})`);

/**
 * A part after a middle dot that leads it ({@link ReadingOptions.dotsLead}): the dot, its space
 * and the part's first run or word on one line, then the rest of the part's runs.
 */
function ledRuns(part: string, queue: FieldQueue): ReactNode {
  const head = UNBREAKABLE_AT_HEAD.exec(part)?.[0] ?? /^\S*/u.exec(part)?.[0] ?? "";
  return (
    <>
      <span className="view-label__run">
        {"· "}
        {setRuns(head, UNBREAKABLE, queue)}
      </span>
      {setRuns(part.slice(head.length), UNBREAKABLE, queue)}
    </>
  );
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
 * reading's, unchanged, whether its dots stand between its parts or lead them.
 *
 * @param fields - The values of the reading set in fields of their own (`LabelLine.fields`), or
 *   none.
 */
export function readingParts(
  value: string,
  fields: ReadonlyArray<LabelField> = [],
  options: ReadingOptions = {},
): ReactNode {
  const queue: FieldQueue = { fields, next: 0 };
  const lead = options.dotsLead === true;
  const seen = new Map<string, number>();
  const nodes: ReactNode[] = [];
  for (const part of value.split(PART_SEPARATOR)) {
    const after = nodes.length > 0;
    if (after) {
      // A leading dot goes inside the next part, and the space before it is the break.
      nodes.push(lead ? " " : PART_SEPARATOR);
    }
    const count = seen.get(part) ?? 0;
    seen.set(part, count + 1);
    nodes.push(
      <span className="view-label__part" key={`${part}:${String(count)}`}>
        {after && lead ? ledRuns(part, queue) : setRuns(part, UNBREAKABLE, queue)}
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
 * line's fields, the sky's limit and edge, are each set in a fixed width and go stale alone, and
 * the sky's `STARS` reading sets each middle dot at the head of the part after it (R06.T11.f,
 * R06.T13.j). Each fault, the graphics' and then the terrain's, is set as a `StatusLine` fault line
 * of its own, in `--status-caution`, apart from the steady statements (the guide's "Alerts": a
 * console's report on its own graphics is never an alert). The statements are set through `StatusText`, so one that gives a sentence
 * after its state (the descent spike's `TEST PLANET: provisional, …`) tracks the state alone; the
 * rest are upper case whole and tracked whole, as before (P05.T13). The stars' count line is a
 * reading of numbers, so an `output` in B612 Mono (the guide's "Typography"), not a statement.
 *
 * On the `PRIMARY` view's upper plate (`scrollsFrom`, plan R10, R10.T12.b;
 * decision-r10-view-block-room §3) the lines before `STARS` stand in a part of their own, which
 * scrolls where its room says so, as a readout does, with their position and total beneath them
 * (`1-6 of 11`, the guide's "Layout"); below the floor everything under the `VIEW` line scrolls
 * instead, held at its foot until the operator scrolls it away, so that its faults and last
 * statements stay in view. A scrolling part is a named region that takes the focus and the
 * pointer only while it scrolls, so that the keyboard and the wheel reach it and a drag that starts
 * on it turns no camera; the `STARS` line and those after it, the statements, the count line and
 * the faults never scroll but there.
 */
export function ViewLabelBlock({
  lines,
  statements,
  countLine,
  faults,
  id,
  scroll,
  className,
  ref,
}: ViewLabelBlockProps) {
  const room = scroll?.room ?? WHOLE;
  const from = scroll?.from;
  const split = from === undefined ? -1 : lines.findIndex((line) => line.label === from);
  const middle = from === undefined ? [] : split < 0 ? lines : lines.slice(0, split);
  const rest = from === undefined ? lines : split < 0 ? [] : lines.slice(split);
  const middleScrolls = room.kind === "middle";
  const bodyScrolls = room.kind === "body";
  const scrollStyle =
    room.kind === "whole" ? undefined : { maxHeight: `${String(room.heightPx)}px` };
  const viewName = scroll?.view ?? "";
  // Measured afresh whenever what the block holds may have changed.
  const contentKey = [
    ...lines.map((line) => `${line.label} ${line.value}`),
    ...statements,
    countLine ?? "",
    ...faults,
  ].join("\n");
  const middleView = useItemsInView(".view-label__line", contentKey, "middle");
  const bodyView = useItemsInView(BODY_ITEMS, contentKey, "body");
  // The whole plate's scroll is held at its foot, so that its faults and last statements stay in
  // view, until the operator scrolls it away from there (step 6).
  const bodyRef = useRef<HTMLElement | null>(null);
  const heldAtFoot = useRef(true);
  const bodyViewRef = bodyView.ref;
  const setBody = useCallback(
    (element: HTMLElement | null): void => {
      bodyRef.current = element;
      heldAtFoot.current = true;
      bodyViewRef(element);
    },
    [bodyViewRef],
  );
  useLayoutEffect(() => {
    const body = bodyRef.current;
    if (bodyScrolls && body !== null && heldAtFoot.current) {
      body.scrollTop = body.scrollHeight;
    }
  });
  const lineOf = (line: LabelLine) => (
    <div className="field view-label__line" key={line.label}>
      <dt className="field__label">{line.label}</dt>
      <dd>
        <output aria-live="off" className={line.stale === true ? "stale" : undefined}>
          {readingParts(line.value, line.fields, { dotsLead: line.dotsLead === true })}
        </output>
        {line.stale === true ? <StaleMark /> : null}
      </dd>
    </div>
  );
  return (
    <div
      className={className === undefined ? "view-label" : `view-label ${className}`}
      id={id}
      ref={ref}
    >
      <p className="view-label__class">VIEW</p>
      {/* A section is a region only while it is named: while it scrolls. */}
      <section
        className={bodyScrolls ? "view-label__body view-label__scroll" : "view-label__body"}
        ref={bodyScrolls ? setBody : undefined}
        style={bodyScrolls ? scrollStyle : undefined}
        onScroll={
          bodyScrolls
            ? (event) => {
                heldAtFoot.current = atFoot(event.currentTarget);
              }
            : undefined
        }
        // A scrolling region with nothing focusable in it takes the focus itself, so that the
        // keyboard scrolls it (WCAG 2.1.1), and only while it scrolls; the rule allows tabIndex on
        // interactive roles only.
        // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={bodyScrolls ? 0 : undefined}
        aria-label={bodyScrolls ? `${viewName} label block` : undefined}
      >
        {from === undefined ? (
          <dl className="view-label__lines">{rest.map(lineOf)}</dl>
        ) : (
          <>
            <section
              className={
                middleScrolls ? "view-label__middle view-label__scroll" : "view-label__middle"
              }
              ref={middleScrolls ? middleView.ref : undefined}
              style={middleScrolls ? scrollStyle : undefined}
              // As the whole plate's scroll, above.
              // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex
              tabIndex={middleScrolls ? 0 : undefined}
              aria-label={middleScrolls ? `${viewName} lines` : undefined}
            >
              <dl className="view-label__lines">{middle.map(lineOf)}</dl>
            </section>
            {middleScrolls ? positionLine(middleView.range) : null}
            {rest.length === 0 ? null : <dl className="view-label__lines">{rest.map(lineOf)}</dl>}
          </>
        )}
        {statements.map((statement) => (
          <p className="view-label__statement" key={statement}>
            <StatusText text={statement} />
          </p>
        ))}
        {countLine === null ? null : (
          <p className="view-label__count">
            <output aria-live="off">{countLine}</output>
          </p>
        )}
        {faults.map((fault) => (
          <StatusLine key={fault} text={fault} standing="fault" />
        ))}
      </section>
      {bodyScrolls ? positionLine(bodyView.range) : null}
    </div>
  );
}

/** A block that stands whole. */
const WHOLE: UpperRoom = { kind: "whole" };
