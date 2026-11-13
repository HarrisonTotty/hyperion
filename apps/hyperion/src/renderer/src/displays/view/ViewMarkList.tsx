import {
  type KeyboardEvent,
  type MouseEvent,
  useCallback,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

import { StaleMark } from "../../components/StaleMark";
import { formatListPosition } from "../../lib/format";
import { useScrollMetrics } from "../../lib/useScrollMetrics";
import { windowRange } from "../../lib/windowRange";
import { SURVEY_EDGE_NOTE, surveyEdgeNoteName } from "../../view/terrain/surveyEdge";
import { type ClosureReading, type MarkRow, rangeText, type SurveyEdgeListNote } from "./viewRun";

/** A row's height, rem. */
const ROW_REM = 2;

/** Rows drawn beyond the window on each side. */
const OVERSCAN_ROWS = 4;

/** A list with no notes. */
const NO_NOTES: ReadonlyArray<SurveyEdgeListNote> = [];

/**
 * The note of the view's line-of-sight mark (the guide's tripod, "In the list"; plan R10, R10.T12):
 * its text, as the readout's heading reads.
 */
export const GROUND_NOTE = "GROUND AT LINE OF SIGHT";

/** The note's accessible name, which gives the mark's place (the guide's tripod, "In the list"). */
export const GROUND_NOTE_NAME = "Ground at line of sight, at the view's centre";

/** A row's closure rate as its accessible name reads it, after a comma, or nothing. */
function closureName(closure: ClosureReading): string {
  let text: string;
  switch (closure.kind) {
    case "none":
      text = "";
      break;
    case "unknown":
      text = ", closure not known";
      break;
    case "known":
      text = `, closure ${closure.text}`;
      break;
  }
  return text;
}

/** Props of {@link ViewMarkList}. */
export interface ViewMarkListProps {
  readonly rows: ReadonlyArray<MarkRow>;
  /**
   * Its notes, the bodies' survey edges in the view's picture, nearest first (`surveyEdgeNotes`,
   * plan R10, R10.T16.c): set after the options, never one of them; none by default.
   */
  readonly notes?: ReadonlyArray<SurveyEdgeListNote> | undefined;
  /**
   * Whether the list notes the view's line-of-sight mark, `GROUND AT LINE OF SIGHT`, first of its
   * notes (plan R10, R10.T12): while the view's terrain readout stands with a point, the tripod
   * drawn or left out for another mark; not by default.
   */
  readonly groundNote?: boolean | undefined;
  /**
   * Whether the ranges are from the camera, the scene having no own ship (`rangesFromCamera`):
   * the range column's head then reads `RANGE FROM CAMERA` and its rows the bare range
   * (R07.T19.b).
   */
  readonly fromCamera: boolean;
  /** The selected row's key, or `null`. */
  readonly selectedKey: string | null;
  /** Called with the row the operator selects. */
  readonly onSelect: (row: MarkRow) => void;
  /**
   * Whether the ranges are stale, a server scene held through a stale period (R02.T17): each is
   * muted with its trailing `S`, and its row named stale.
   */
  readonly stale?: boolean | undefined;
}

/**
 * The view's list, the canvas's partner (plan R02, R02.T15.b; Design note 17): its bodies and craft,
 * each with its range, from the own ship or from the camera, as a windowed `listbox` from which the
 * keyboard selects a mark, which moves the bracket reticle on the view.
 *
 * @remarks
 * With no own ship the range column's head reads `RANGE FROM CAMERA`, its second line under
 * `RANGE`, and each row the bare range, so that the designations keep their width; each option's
 * accessible name keeps `FROM CAMERA` (R07.T19.b, decision-r07-t19-layout).
 * Windowed as plan 05's lists are (its design note D17), though a kept scene's list is short. Stars
 * are not listed: they are not targets.
 *
 * Its notes stand after the options and their position, outside the `listbox` and never windowed:
 * each a `note`, not focusable, which no click or key acts on and the list's keys pass over, its
 * text wrapping at its spaces, never truncated, its range muted with its `S` while the ranges are
 * stale, and named in the options' form (the guide's Views; decision-r10-t16-review §1). The
 * line-of-sight mark's note, `GROUND AT LINE OF SIGHT`, stands first of them while the view's
 * terrain readout has a point (R10.T12), named by the mark's place and carrying no range, since the
 * readout's `SLANT RANGE` is the point's; it is never stale, since it marks the picture's centre.
 */
export function ViewMarkList({
  rows,
  notes = NO_NOTES,
  groundNote = false,
  fromCamera,
  selectedKey,
  onSelect,
  stale = false,
}: ViewMarkListProps) {
  const baseId = useId();
  const { ref: measureRef, metrics } = useScrollMetrics();
  const listRef = useRef<HTMLDivElement | null>(null);
  const offsetRef = useRef(0);
  const [scrollTopPx, setScrollTopPx] = useState(0);
  const setList = useCallback(
    (element: HTMLDivElement | null): void => {
      listRef.current = element;
      measureRef(element);
    },
    [measureRef],
  );

  const rowHeightPx = ROW_REM * metrics.remPx;
  const viewportPx = metrics.viewportPx;
  const total = rows.length;
  const range = windowRange(scrollTopPx, rowHeightPx, viewportPx, total, OVERSCAN_ROWS);
  const rowsInWindow = Math.max(1, Math.floor(viewportPx / rowHeightPx));
  const selectedIndex = rows.findIndex((row) => row.key === selectedKey);

  const showRow = useCallback((index: number, rowPx: number, heightPx: number): void => {
    const top = index * rowPx;
    const offset = Math.max(top + rowPx - heightPx, Math.min(top, offsetRef.current));
    if (offset === offsetRef.current) {
      return;
    }
    offsetRef.current = offset;
    setScrollTopPx(offset);
    if (listRef.current !== null) {
      listRef.current.scrollTop = offset;
    }
  }, []);

  // The canvas selects marks too, and a row outside the window has no DOM to scroll to.
  useLayoutEffect(() => {
    if (selectedIndex >= 0) {
      showRow(selectedIndex, rowHeightPx, viewportPx);
    }
  }, [selectedIndex, rowHeightPx, viewportPx, showRow]);

  const select = (index: number): void => {
    const row = rows[index];
    if (row === undefined) {
      return;
    }
    showRow(index, rowHeightPx, viewportPx);
    if (row.key !== selectedKey) {
      onSelect(row);
    }
  };

  const onClick = (event: MouseEvent<HTMLDivElement>): void => {
    const element = event.target instanceof Element ? event.target.closest("[data-index]") : null;
    const index = Number(element instanceof HTMLElement ? element.dataset["index"] : Number.NaN);
    if (Number.isInteger(index)) {
      select(index);
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    if (total === 0 || event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    const from = selectedIndex < 0 ? -1 : selectedIndex;
    let next: number;
    switch (event.key) {
      case "ArrowDown":
        next = from + 1;
        break;
      case "ArrowUp":
        next = from < 0 ? 0 : from - 1;
        break;
      case "PageDown":
        next = from + rowsInWindow;
        break;
      case "PageUp":
        next = from - rowsInWindow;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = total - 1;
        break;
      default:
        return;
    }
    event.preventDefault();
    // The view's own keys must not also act on a key the list took.
    event.stopPropagation();
    select(Math.min(total - 1, Math.max(0, next)));
  };

  const shown: number[] = [];
  if (range !== null) {
    for (let index = range.first; index <= range.last; index += 1) {
      shown.push(index);
    }
  }

  return (
    <div className="view-list">
      <div className="view-list__head">
        <span>DESIG</span>
        <span>KIND</span>
        <span className="view-list__range-head">RANGE</span>
        {fromCamera ? (
          <span className="view-list__range-head view-list__range-head--from">FROM CAMERA</span>
        ) : null}
      </div>
      <div
        className="view-list__scroll"
        // A windowed list, which no native control can be: `select` renders every option (plan 05,
        // design note D17).
        // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
        role="listbox"
        aria-label="Marks in view"
        // A windowed listbox takes the focus itself; its rows are options, not focusable elements.
        tabIndex={0}
        aria-activedescendant={selectedIndex < 0 ? undefined : `${baseId}-${String(selectedIndex)}`}
        ref={setList}
        onClick={onClick}
        onKeyDown={onKeyDown}
        onScroll={(event) => {
          offsetRef.current = event.currentTarget.scrollTop;
          setScrollTopPx(event.currentTarget.scrollTop);
        }}
      >
        <div
          className="view-list__rows"
          role="presentation"
          style={{ height: `${String(total * ROW_REM)}rem` }}
        >
          {shown.map((index) => {
            const row = rows[index];
            if (row === undefined) {
              return null;
            }
            const rangeReading = rangeText(row);
            const closure = closureName(row.closure);
            return (
              <div
                key={row.key}
                id={`${baseId}-${String(index)}`}
                // `option` belongs to a `select`, which cannot be windowed (D17).
                // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
                role="option"
                aria-selected={row.key === selectedKey}
                aria-posinset={index + 1}
                aria-setsize={total}
                aria-label={`${row.name}, ${row.kind}, range ${rangeReading}${closure}${stale ? ", stale" : ""}`}
                className="view-list__row"
                style={{ transform: `translateY(${String(index * ROW_REM)}rem)` }}
                data-index={index}
              >
                <span className="view-list__name">{row.name}</span>
                <span className="view-list__kind">{row.kind}</span>
                <span className="view-list__number">
                  <span className={stale ? "stale" : undefined}>{row.range}</span>
                  {stale ? <StaleMark /> : null}
                </span>
              </div>
            );
          })}
        </div>
      </div>
      {range === null ? null : (
        <p className="list-position">
          {formatListPosition(range.firstVisible, range.lastVisible, total)}
        </p>
      )}
      {notes.length === 0 && !groundNote ? null : (
        <div className="view-list__notes">
          {groundNote ? (
            <p role="note" aria-label={GROUND_NOTE_NAME} className="view-list__note">
              {GROUND_NOTE}
            </p>
          ) : null}
          {notes.map((note) => (
            <p
              key={note.key}
              role="note"
              aria-label={surveyEdgeNoteName(note.designation, note, stale)}
              className="view-list__note"
            >
              {SURVEY_EDGE_NOTE} {note.designation}{" "}
              <span className={stale ? "stale" : undefined}>
                <span className="view-list__note-range">{note.range}</span>
                {note.fromCamera ? " FROM CAMERA" : null}
              </span>
              {stale ? <StaleMark /> : null}
            </p>
          ))}
        </div>
      )}
    </div>
  );
}
