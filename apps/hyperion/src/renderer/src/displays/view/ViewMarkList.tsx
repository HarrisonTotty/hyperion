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
import { type ClosureReading, type MarkRow, rangeText } from "./viewRun";

/** A row's height, rem. */
const ROW_REM = 2;

/** Rows drawn beyond the window on each side. */
const OVERSCAN_ROWS = 4;

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
 * each with its range, from the own ship or labelled `FROM CAMERA`, as a windowed `listbox` from
 * which the keyboard selects a mark, which moves the bracket reticle on the view.
 *
 * @remarks
 * Windowed as plan 05's lists are (its design note D17), though a kept scene's list is short. Stars
 * are not listed: they are not targets.
 */
export function ViewMarkList({ rows, selectedKey, onSelect, stale = false }: ViewMarkListProps) {
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
        <span>RANGE</span>
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
                  <span className={stale ? "stale" : undefined}>{rangeReading}</span>
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
    </div>
  );
}
