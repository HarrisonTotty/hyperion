import {
  type KeyboardEvent,
  type MouseEvent,
  useCallback,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

import { formatListPosition } from "../../lib/format";
import { useScrollMetrics } from "../../lib/useScrollMetrics";
import { windowRange } from "../../lib/windowRange";
import type { BodyRow } from "./bodyRows";

/** Height of a row, in `rem`: also the guide's smallest pointer target. */
const ROW_REM = 2;

/** Props of {@link BodyList}. */
export interface BodyListProps {
  readonly rows: ReadonlyArray<BodyRow>;
  readonly selectedId: string | null;
  readonly onSelect: (id: string) => void;
}

/** The accessible name of a row: everything it reads, in words. */
function rowName(row: BodyRow): string {
  const sma =
    row.semiMajorAxis === null ? "" : `, SMA ${row.semiMajorAxis.value} ${row.semiMajorAxis.unit}`;
  return `${row.designation}, ${row.kind}${sma}`;
}

/**
 * The system's bodies as a tree, from which one is selected with the keyboard, paired with the
 * orbit map (plan 14, P14.T43.a).
 *
 * @remarks
 * The canvas is opaque to the keyboard and to assistive technology, so the guide's 3D conventions
 * pair it with this list. It is a `tree` whose rows are `treeitem`s at their depth, hosts at the top
 * and, with `system_bodies`, planets under their hosts and moons and rings under their planets.
 * `ArrowUp` and `ArrowDown` move the active row, `Home` and `End` go to the ends, and `Enter` or
 * `Space` selects it; a click selects a row at once. Selection is shared with the map, and a
 * selection made there makes its row the active one. Each row reads the designation, the kind in
 * words and the semi-major axis, and the list says which rows are in view and how many there are.
 */
export function BodyList({ rows, selectedId, onSelect }: BodyListProps) {
  const baseId = useId();
  const { ref: measureRef, metrics } = useScrollMetrics();
  const listRef = useRef<HTMLDivElement | null>(null);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [followedId, setFollowedId] = useState(selectedId);
  const setList = useCallback(
    (element: HTMLDivElement | null): void => {
      listRef.current = element;
      measureRef(element);
    },
    [measureRef],
  );

  // A selection made on the map, or anywhere else, makes its row the active one.
  let active = activeId;
  if (selectedId !== followedId) {
    setFollowedId(selectedId);
    setActiveId(null);
    active = null;
  }
  const activeIndex = Math.max(
    0,
    rows.findIndex((row) => row.id === (active ?? selectedId)),
  );
  const activeRow = rows[activeIndex];

  const rowHeightPx = ROW_REM * metrics.remPx;
  const range = windowRange(metrics.scrollTopPx, rowHeightPx, metrics.viewportPx, rows.length, 0);

  // The active row is kept in view: the list scrolls by its own offset, a row's height at a time.
  useLayoutEffect(() => {
    const list = listRef.current;
    if (list === null || metrics.viewportPx <= 0) {
      return;
    }
    const top = activeIndex * rowHeightPx;
    const offset = Math.max(top + rowHeightPx - metrics.viewportPx, Math.min(top, list.scrollTop));
    if (offset !== list.scrollTop) {
      list.scrollTop = offset;
    }
  }, [activeIndex, rowHeightPx, metrics.viewportPx]);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    if (rows.length === 0 || event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    let next: number | null = null;
    switch (event.key) {
      case "ArrowDown":
        next = activeIndex + 1;
        break;
      case "ArrowUp":
        next = activeIndex - 1;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = rows.length - 1;
        break;
      case "Enter":
      case " ":
        event.preventDefault();
        if (activeRow !== undefined && activeRow.id !== selectedId) {
          onSelect(activeRow.id);
        }
        return;
      default:
        return;
    }
    event.preventDefault();
    const row = rows[Math.min(rows.length - 1, Math.max(0, next))];
    if (row !== undefined) {
      setActiveId(row.id);
    }
  };

  // One handler for the whole tree, so that a row carries no listener of its own: a row is an
  // item operated through the tree, not an element that acts by itself.
  const onClick = (event: MouseEvent<HTMLDivElement>): void => {
    const element = event.target instanceof Element ? event.target.closest("[data-id]") : null;
    const id = element instanceof HTMLElement ? element.dataset["id"] : undefined;
    const row = rows.find((candidate) => candidate.id === id);
    if (row === undefined) {
      return;
    }
    setActiveId(row.id);
    if (row.id !== selectedId) {
      onSelect(row.id);
    }
  };

  return (
    <div className="body-list">
      <div className="body-list__head">
        <span>DESIG</span>
        <span>KIND</span>
        <span className="body-list__number">SMA</span>
      </div>
      <div
        className="body-list__scroll"
        // A tree of the system's bodies, which no native element is: planets sit under their hosts
        // and moons under their planets (plan 14, P14.T43.a).
        // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
        role="tree"
        aria-label="Bodies"
        // The tree takes the focus itself; its rows are items, not focusable elements.
        tabIndex={0}
        aria-activedescendant={activeRow === undefined ? undefined : `${baseId}-${activeRow.id}`}
        ref={setList}
        onClick={onClick}
        onKeyDown={onKeyDown}
      >
        {rows.map((row, index) => {
          const siblings = rows.filter((candidate) => candidate.parentId === row.parentId);
          return (
            <div
              key={row.id}
              id={`${baseId}-${row.id}`}
              // `treeitem` belongs to the tree above, which no native element is.
              // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
              role="treeitem"
              aria-level={row.level}
              aria-posinset={siblings.indexOf(row) + 1}
              aria-setsize={siblings.length}
              aria-selected={row.id === selectedId}
              aria-label={rowName(row)}
              className={
                index === activeIndex ? "body-list__row body-list__row--active" : "body-list__row"
              }
              data-id={row.id}
            >
              <span className="body-list__designation">{row.designation}</span>
              <span className="body-list__kind">{row.kind}</span>
              <span className="body-list__number">
                {row.semiMajorAxis === null ? null : (
                  <>
                    {row.semiMajorAxis.value}{" "}
                    <span className="body-list__unit">{row.semiMajorAxis.unit}</span>
                  </>
                )}
              </span>
            </div>
          );
        })}
      </div>
      {range === null ? null : (
        <p className="list-position">
          {formatListPosition(range.firstVisible, range.lastVisible, rows.length)}
        </p>
      )}
    </div>
  );
}
