import type { FeatureIdHex } from "@hyperion/protocol";
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
import { StaleSlot } from "../../components/StaleSlot";
import { formatListPosition, formatSignificant } from "../../lib/format";
import { featureKindAbbreviation, featureKindWords } from "../../lib/galaxy/featureWords";
import type { ChartFeature } from "../../lib/galaxy/model";
import { useScrollMetrics } from "../../lib/useScrollMetrics";
import { windowRange } from "../../lib/windowRange";
import { type ChartDistanceUnit, formatFeatureDistance } from "./chartModel";

/** Height of a row, in `rem`: also the guide's smallest pointer target, as the system list's. */
const ROW_REM = 2;

/** Rows rendered beyond each end of the visible ones, so that a scroll shows no gap. */
const OVERSCAN_ROWS = 8;

/** Props of {@link FeatureList}. */
export interface FeatureListProps {
  readonly features: ReadonlyArray<ChartFeature>;
  readonly selectedId: FeatureIdHex | null;
  readonly onSelect: (id: FeatureIdHex) => void;
  /**
   * The unit and decimals every distance on this chart is written with (plan 05, design note D21;
   * plan 09, P09.T42).
   */
  readonly distanceUnit: ChartDistanceUnit;
  /** Whether the chart is a snapshot the link no longer backs: each row then trails one `S`. */
  readonly stale: boolean;
}

/**
 * Every large feature a chart holds, nearest first, from which one is selected with the keyboard
 * (plan 09, P09.T41.c): the chart's second list, beside its systems'.
 *
 * @remarks
 * The system list's twin ({@link SystemList}), with the same keyboard selection: a windowed
 * `listbox` whose rows are `option`s, each carrying its place in the whole list; `ArrowUp` and
 * `ArrowDown` move one row, `PageUp` and `PageDown` one window, `Home` and `End` the ends; the
 * selection follows the active row, and a selection made on the chart scrolls the list to it. A
 * row names the feature, its kind by the abbreviation beside its mark (`OC`, `CENTRE`), its
 * distance to its centre at the chart's precision, or to three figures where that would pass the
 * column ({@link formatFeatureDistance}), its radius at the chart's time and its present
 * mass, each to the guide's three figures; the row's accessible name gives the kind in words, so
 * that the abbreviation is never the only signal. A stale chart's rows each trail one `S`, at the
 * end of the mass, and are named stale.
 */
export function FeatureList({
  features,
  selectedId,
  onSelect,
  distanceUnit,
  stale,
}: FeatureListProps) {
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
  const total = features.length;
  const range = windowRange(scrollTopPx, rowHeightPx, viewportPx, total, OVERSCAN_ROWS);
  const rowsInWindow = Math.max(1, Math.floor(viewportPx / rowHeightPx));
  const selectedIndex = features.findIndex((feature) => feature.id === selectedId);

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

  // The chart selects features too, and a row outside the window is not in the DOM to be scrolled
  // to. The offset is read from a ref, so that a scroll by hand never snaps the selection back.
  useLayoutEffect(() => {
    if (selectedIndex >= 0) {
      showRow(selectedIndex, rowHeightPx, viewportPx);
    }
  }, [selectedIndex, rowHeightPx, viewportPx, showRow]);

  const select = (index: number): void => {
    const feature = features[index];
    if (feature === undefined) {
      return;
    }
    showRow(index, rowHeightPx, viewportPx);
    if (feature.id !== selectedId) {
      onSelect(feature.id);
    }
  };

  // One handler for the whole list, as the system list has it: a row is an option.
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
    // With nothing selected the nearest feature is the active row, as the list opens on it.
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
    <div className="system-list feature-list">
      <div className="system-list__head">
        <span>DESIG</span>
        <span>KIND</span>
        <span>{`DIST ${distanceUnit.symbol}`}</span>
        <span>EXTENT ly</span>
        <span>MASS</span>
      </div>
      <div
        className="system-list__scroll"
        // A windowed list, which no native control can be (plan 05, design note D17).
        // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
        role="listbox"
        aria-label="Features by distance"
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
          // Carries the whole list's height and nothing else, so the options stay the listbox's own.
          role="presentation"
          style={{ height: `${String(total * ROW_REM)}rem` }}
        >
          {rows.map((index) => {
            const feature = features[index];
            if (feature === undefined) {
              return null;
            }
            const distance = formatFeatureDistance(feature.distanceLy, distanceUnit);
            const extent = formatSignificant(feature.extentLy);
            const mass = formatSignificant(feature.massMsun);
            return (
              <div
                key={feature.id}
                id={`${baseId}-${feature.id}`}
                // `option` belongs to a `select`, which cannot be windowed (D17).
                // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
                role="option"
                aria-selected={feature.id === selectedId}
                aria-posinset={index + 1}
                aria-setsize={total}
                aria-label={[
                  feature.designation,
                  featureKindWords(feature.kind),
                  `${distance} ${distanceUnit.symbol}`,
                  `radius ${extent} ly`,
                  `${mass} solar masses`,
                  stale ? "stale" : null,
                ]
                  .filter((part) => part !== null)
                  .join(", ")}
                className="system-list__row"
                style={{ transform: `translateY(${String(index * ROW_REM)}rem)` }}
                data-index={index}
              >
                <span className="system-list__designation">{feature.designation}</span>
                <span className="system-list__class">{featureKindAbbreviation(feature.kind)}</span>
                <span className="system-list__number">{distance}</span>
                <span className="system-list__number">{extent}</span>
                <span className="system-list__number feature-list__mass">
                  {mass} <SolarMassUnit />
                  <StaleSlot stale={stale} />
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
