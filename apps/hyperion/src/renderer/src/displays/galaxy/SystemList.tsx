import type { SystemIdHex } from "@hyperion/protocol";
import {
  type KeyboardEvent,
  type MouseEvent,
  useCallback,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

import { SolarMassUnit } from "../../components/SolarMassUnit";
import { formatLengthLy, formatListPosition, formatMassMsun } from "../../lib/format";
import type { ChartSystem } from "../../lib/galaxy/model";
import { objectKindLabel } from "../../lib/system/words";
import { useScrollMetrics } from "../../lib/useScrollMetrics";
import { windowRange } from "../../lib/windowRange";

/** Height of a row, in `rem`: also the guide's smallest pointer target. */
const ROW_REM = 2;

/** Rows rendered beyond each end of the visible ones, so that a scroll shows no gap. */
const OVERSCAN_ROWS = 8;

/**
 * Whether a system lies within the drive range, in words as the column shows them: `IN RANGE` as
 * the readout has it, or `OUT`, since the list's 5 rem column cannot hold the readout's
 * `OUT OF RANGE` (the orchestrator's ruling 15).
 */
function rangeWords(inRange: boolean): string {
  return inRange ? "IN RANGE" : "OUT";
}

/**
 * The same in the words the rest of the display uses, for the row's accessible name: the column's
 * `OUT` is shortened for its width, which a screen reader does not have (the orchestrator's
 * ruling 28).
 */
function rangeName(inRange: boolean): string {
  return inRange ? "IN RANGE" : "OUT OF RANGE";
}

/**
 * What a row's primary is, for its accessible name: the kind in words and the class, or that the
 * system is not yet formed at the chart's time.
 */
function starName(system: ChartSystem): string {
  if (system.star === null) {
    return "NOT YET FORMED";
  }
  return `${objectKindLabel(system.star.kind)} ${system.star.spectralClass}`;
}

interface SystemListProps {
  readonly systems: ReadonlyArray<ChartSystem>;
  readonly selectedId: SystemIdHex | null;
  readonly onSelect: (id: SystemIdHex) => void;
  /** The range a system counts as in (plan 05, design note D9). */
  readonly driveRangeLy: number;
  /** Decimals every distance on this chart is written with (plan 05, design note D21). */
  readonly distanceDecimals: number;
}

/**
 * Every system a chart holds, nearest first, from which one is selected with the keyboard.
 *
 * @remarks
 * The canvas is opaque to the keyboard and to assistive technology, so the guide's 3D conventions
 * pair it with this list. It is a `listbox` whose rows are `option`s, always windowed: one code path
 * whether the chart holds forty systems or four thousand, which is why each row carries its place in
 * the whole list. `ArrowUp` and `ArrowDown` move one row, `PageUp` and `PageDown` one window, `Home`
 * and `End` the ends; the selection follows the active row and is reported, and the list scrolls it
 * into view by its index, since a row outside the window is not in the DOM to scroll to. A selection
 * made on the chart scrolls the list to it as well. Whether a system is within the drive range is
 * repeated in words, so that the chart's `--accent` is never the only signal. The `CLASS` column
 * holds the primary's spectral class as an astronomer writes it (`G2V`, `DA4.2`, `NS`), and the
 * row's accessible name its kind in words as well, so that a symbol's shape is never the only signal
 * of what a star is (plan 06, design note 17); a system not yet formed at the chart's time has no
 * class, an em dash, and is named `NOT YET FORMED`.
 */
export function SystemList({
  systems,
  selectedId,
  onSelect,
  driveRangeLy,
  distanceDecimals,
}: SystemListProps) {
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
  const total = systems.length;
  const range = windowRange(scrollTopPx, rowHeightPx, viewportPx, total, OVERSCAN_ROWS);
  const rowsInWindow = Math.max(1, Math.floor(viewportPx / rowHeightPx));
  const selectedIndex = systems.findIndex((system) => system.id === selectedId);

  /** Brings the row at `index` into view, by the list's own offset: an unwindowed row has no DOM. */
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

  // The chart selects systems too, and a row outside the window is not in the DOM to be scrolled
  // to. The offset is read from a ref, so that a scroll by hand never snaps the selection back.
  useLayoutEffect(() => {
    if (selectedIndex >= 0) {
      showRow(selectedIndex, rowHeightPx, viewportPx);
    }
  }, [selectedIndex, rowHeightPx, viewportPx, showRow]);

  const select = (index: number): void => {
    const system = systems[index];
    if (system === undefined) {
      return;
    }
    showRow(index, rowHeightPx, viewportPx);
    if (system.id !== selectedId) {
      onSelect(system.id);
    }
  };

  // One handler for the whole list, so that a row carries no listener of its own: a row is an
  // option, which is operated through the list, not an element that acts by itself.
  const onClick = (event: MouseEvent<HTMLDivElement>): void => {
    const row = event.target instanceof Element ? event.target.closest("[data-index]") : null;
    const index = Number(row instanceof HTMLElement ? row.dataset["index"] : Number.NaN);
    if (Number.isInteger(index)) {
      select(index);
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    if (total === 0 || event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    // With nothing selected the nearest system is the active row, as the list opens on it.
    const from = selectedIndex < 0 ? 0 : selectedIndex;
    let next: number;
    switch (event.key) {
      case "ArrowDown":
        next = from + 1;
        break;
      case "ArrowUp":
        next = from - 1;
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
    select(Math.min(total - 1, Math.max(0, next)));
  };

  const rows: number[] = [];
  if (range !== null) {
    for (let index = range.first; index <= range.last; index += 1) {
      rows.push(index);
    }
  }

  return (
    <div className="system-list">
      <div className="system-list__head">
        <span>DESIG</span>
        <span>CLASS</span>
        <span>DIST ly</span>
        <span>
          INIT MASS <SolarMassUnit />
        </span>
        {/*
         * The setting's one name, as the readout's reading of the same words has it: `RANGE` alone
         * is kept for the chart's curve labels, where the chart is the context (the orchestrator's
         * ruling 15). No `SET`, since the column holds within-or-beyond words, not the value.
         */}
        <span>DRIVE RANGE</span>
      </div>
      <div
        className="system-list__scroll"
        // A windowed list of a few thousand rows, which no native control can be: `select` and
        // `datalist` render every option (plan 05, design note D17).
        // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
        role="listbox"
        aria-label="Systems by distance"
        // A windowed listbox takes the focus itself; its rows are options, not focusable elements.
        tabIndex={0}
        aria-activedescendant={selectedId === null ? undefined : `${baseId}-${selectedId}`}
        ref={setList}
        onClick={onClick}
        onKeyDown={onKeyDown}
        onScroll={(event) => {
          offsetRef.current = event.currentTarget.scrollTop;
          setScrollTopPx(event.currentTarget.scrollTop);
        }}
      >
        <div
          className="system-list__rows"
          // Carries the whole list's height and nothing else, so the options stay the listbox's own
          // children in the accessibility tree.
          role="presentation"
          style={{ height: `${total * ROW_REM}rem` }}
        >
          {rows.map((index) => {
            const system = systems[index];
            if (system === undefined) {
              return null;
            }
            const inRange = system.distanceLy <= driveRangeLy;
            const distance = formatLengthLy(system.distanceLy, distanceDecimals);
            const mass = formatMassMsun(system.initialMassMsun);
            return (
              <div
                key={system.id}
                id={`${baseId}-${system.id}`}
                // `option` belongs to a `select`, which cannot be windowed (D17).
                // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
                role="option"
                aria-selected={system.id === selectedId}
                aria-posinset={index + 1}
                aria-setsize={total}
                aria-label={`${system.designation}, ${starName(system)}, ${distance} ly, ${mass} solar masses, ${rangeName(inRange)}`}
                className={
                  inRange ? "system-list__row system-list__row--in-range" : "system-list__row"
                }
                style={{ transform: `translateY(${index * ROW_REM}rem)` }}
                data-index={index}
              >
                <span className="system-list__designation">{system.designation}</span>
                {system.star === null ? (
                  <span className="system-list__class readout__missing">—</span>
                ) : (
                  <span className="system-list__class">{system.star.spectralClass}</span>
                )}
                <span className="system-list__number">{distance}</span>
                <span className="system-list__number">{mass}</span>
                <span className="system-list__range">{rangeWords(inRange)}</span>
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
