import type { UniverseTime } from "@hyperion/protocol";
import {
  type KeyboardEvent,
  type MouseEvent,
  useCallback,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

import { StaleSlot } from "../../components/StaleSlot";
import { formatListPosition } from "../../lib/format";
import type { SystemEvent } from "../../lib/system/eventsWire";
import { useScrollMetrics } from "../../lib/useScrollMetrics";
import { windowRange } from "../../lib/windowRange";
import { eventRow, nextEventIndex } from "./eventRows";

/** Height of a row, in `rem`: also the guide's smallest pointer target. */
const ROW_REM = 2;

/** Rows rendered beyond each end of the visible ones, so that a scroll shows no gap. */
const OVERSCAN_ROWS = 8;

/** Props of {@link EventList}. */
export interface EventListProps {
  /** The events of the answer on show, by time; its identity changes with each answer. */
  readonly events: ReadonlyArray<SystemEvent>;
  /** Each body's designation by its ID, from the bodies on show. */
  readonly designations: ReadonlyMap<string, string>;
  /** The display time, which every countdown runs from. */
  readonly now: UniverseTime;
  /** The body the display has selected, which a chosen event's row is selected for; `null` for none. */
  readonly selectedBody: string | null;
  /** Selects a body of the display, as the body list and the map do. */
  readonly onSelectBody: (id: string) => void;
  /** Whether the answer is a snapshot the link no longer backs: each row trails one `S`. */
  readonly stale: boolean;
}

/**
 * A system's events in time order, each its time in the display's time system, its countdown from
 * the display time, its kind and its body, from which an event is chosen with the keyboard and its
 * body selected in the display (plan 14, P14.T43.c).
 *
 * @remarks
 * A `listbox` whose rows are `option`s, always windowed, as the chart's system list is: a busy
 * system lists thousands of events a century. `ArrowUp` and `ArrowDown` move the active row one row,
 * `PageUp` and `PageDown` one window, `Home` and `End` to the ends, as the body list's keys move
 * its active row, outlined while the list has the focus; `Enter` or `Space`, or a click, chooses the
 * event and selects its body in the display, so that the map and the readout show where it falls.
 * The chosen event's row carries the selected row's rule while its body stays the one selected, and
 * loses it once the display selects another. An event whose body the bodies on show do not hold
 * reads the em dash for it, is disabled and selects nothing; its row's name says the body is not
 * listed, as the list's foot does while it is the active row. Each answer opens on the next event at
 * or after the display time. The time reads its system's label,
 * `UT`, in the display time's form, and the countdown the guide's `T-` before the event and `T+`
 * after it, both from whole seconds; each row's accessible name reads them in the same words. The
 * active row is always rendered, however far the list is scrolled from it, so that the list's
 * active descendant is always in the DOM.
 */
export function EventList({
  events,
  designations,
  now,
  selectedBody,
  onSelectBody,
  stale,
}: EventListProps) {
  const baseId = useId();
  const { ref: measureRef, metrics } = useScrollMetrics();
  const listRef = useRef<HTMLDivElement | null>(null);
  const offsetRef = useRef(0);
  const [scrollTopPx, setScrollTopPx] = useState(0);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [chosenId, setChosenId] = useState<string | null>(null);
  const [opened, setOpened] = useState<ReadonlyArray<SystemEvent> | null>(null);

  const setList = useCallback(
    (element: HTMLDivElement | null): void => {
      listRef.current = element;
      measureRef(element);
    },
    [measureRef],
  );

  // Each answer opens on the next event to come; an active event the new answer still lists stays.
  let active = activeId;
  if (opened !== events) {
    setOpened(events);
    if (active === null || !events.some((event) => event.id === active)) {
      active = events[nextEventIndex(events, now)]?.id ?? null;
      setActiveId(active);
    }
  }
  const activeIndex = Math.max(
    0,
    events.findIndex((event) => event.id === active),
  );

  const rowHeightPx = ROW_REM * metrics.remPx;
  const viewportPx = metrics.viewportPx;
  const total = events.length;
  const range = windowRange(scrollTopPx, rowHeightPx, viewportPx, total, OVERSCAN_ROWS);
  const rowsInWindow = Math.max(1, Math.floor(viewportPx / rowHeightPx));

  // The active row is kept in view by the list's own offset, since a row outside the window is
  // not in the DOM to be scrolled to. The offset is read from a ref, so that a scroll by hand never
  // snaps the list back.
  useLayoutEffect(() => {
    const list = listRef.current;
    if (list === null || viewportPx <= 0) {
      return;
    }
    const top = activeIndex * rowHeightPx;
    const offset = Math.max(top + rowHeightPx - viewportPx, Math.min(top, offsetRef.current));
    if (offset !== offsetRef.current) {
      offsetRef.current = offset;
      setScrollTopPx(offset);
      list.scrollTop = offset;
    }
  }, [activeIndex, rowHeightPx, viewportPx]);

  const choose = (index: number, select: boolean): void => {
    const event = events[index];
    if (event === undefined) {
      return;
    }
    setActiveId(event.id);
    // An event whose body is not on show has no body to select: choosing it would select the
    // display's fallback, the primary, in its place.
    if (select && designations.has(event.body)) {
      setChosenId(event.id);
      onSelectBody(event.body);
    }
  };

  // One handler for the whole list, so that a row carries no listener of its own: a row is an
  // option, which is operated through the list, not an element that acts by itself.
  const onClick = (event: MouseEvent<HTMLDivElement>): void => {
    const row = event.target instanceof Element ? event.target.closest("[data-index]") : null;
    const index = Number(row instanceof HTMLElement ? row.dataset["index"] : Number.NaN);
    if (Number.isInteger(index)) {
      choose(index, true);
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    if (total === 0 || event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    let next: number;
    switch (event.key) {
      case "ArrowDown":
        next = activeIndex + 1;
        break;
      case "ArrowUp":
        next = activeIndex - 1;
        break;
      case "PageDown":
        next = activeIndex + rowsInWindow;
        break;
      case "PageUp":
        next = activeIndex - rowsInWindow;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = total - 1;
        break;
      case "Enter":
      case " ":
        event.preventDefault();
        choose(activeIndex, true);
        return;
      default:
        return;
    }
    event.preventDefault();
    choose(Math.min(total - 1, Math.max(0, next)), false);
  };

  const indices: number[] = [];
  if (range !== null) {
    if (activeIndex < range.first) {
      indices.push(activeIndex);
    }
    for (let index = range.first; index <= range.last; index += 1) {
      indices.push(index);
    }
    if (activeIndex > range.last) {
      indices.push(activeIndex);
    }
  }
  const activeEvent = events[activeIndex];
  const activeUnlisted = activeEvent !== undefined && !designations.has(activeEvent.body);

  return (
    <div className="event-list">
      <div className="event-list__head">
        <span>TIME</span>
        <span>COUNTDOWN</span>
        <span>EVENT</span>
        <span>DESIG</span>
      </div>
      <div
        className="event-list__scroll"
        // A windowed list of thousands of rows, which no native control can be: `select` and
        // `datalist` render every option (plan 05, design note D17).
        // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
        role="listbox"
        aria-label="Events"
        // A windowed listbox takes the focus itself; its rows are options, not focusable elements.
        tabIndex={0}
        aria-activedescendant={
          activeEvent === undefined ? undefined : `${baseId}-${activeEvent.id}`
        }
        ref={setList}
        onClick={onClick}
        onKeyDown={onKeyDown}
        onScroll={(event) => {
          offsetRef.current = event.currentTarget.scrollTop;
          setScrollTopPx(event.currentTarget.scrollTop);
        }}
      >
        <div
          className="event-list__rows"
          // Carries the whole list's height and nothing else, so the options stay the listbox's own
          // children in the accessibility tree.
          role="presentation"
          style={{ height: `${total * ROW_REM}rem` }}
        >
          {indices.map((index) => {
            const event = events[index];
            if (event === undefined) {
              return null;
            }
            const row = eventRow(event, designations, now);
            return (
              <div
                key={row.id}
                id={`${baseId}-${row.id}`}
                // `option` belongs to a `select`, which cannot be windowed (D17).
                // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
                role="option"
                aria-selected={row.id === chosenId && event.body === selectedBody}
                aria-disabled={row.body === null ? true : undefined}
                aria-posinset={index + 1}
                aria-setsize={total}
                aria-label={[
                  row.time,
                  row.countdown,
                  row.kind,
                  row.body ?? "body not listed",
                  // The name hides the row's content, its `S` and that mark's word with it.
                  stale ? "stale" : null,
                ]
                  .filter((part) => part !== null)
                  .join(", ")}
                className={
                  index === activeIndex
                    ? "event-list__row event-list__row--active"
                    : "event-list__row"
                }
                style={{ transform: `translateY(${index * ROW_REM}rem)` }}
                data-index={index}
              >
                <span className="event-list__number">{row.time}</span>
                <span className="event-list__number">{row.countdown}</span>
                <span className="event-list__kind">{row.kind}</span>
                <span className="event-list__body">
                  <span className="event-list__designation">
                    {row.body === null ? <span className="readout__missing">—</span> : row.body}
                  </span>
                  <StaleSlot stale={stale} />
                </span>
              </div>
            );
          })}
        </div>
      </div>
      <div className="event-list__foot">
        {activeUnlisted ? <p className="event-list__note">BODY NOT LISTED</p> : null}
        {range === null ? null : (
          <p className="list-position">
            {formatListPosition(range.firstVisible, range.lastVisible, total)}
          </p>
        )}
      </div>
    </div>
  );
}
