import {
  type AlertBandDto,
  type ClassCensusDto,
  type ClassCensusStatusDto,
  type ContactDto,
  type TransientKindDto,
  universeTimeToYears,
} from "@hyperion/protocol";
import {
  type KeyboardEvent,
  type MouseEvent,
  useCallback,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

import {
  formatBearing,
  formatListPosition,
  formatSignificant,
  formatUniverseTimeYr,
  TIME_SYSTEM_LABEL,
} from "../lib/format";
import { toCatalogueClass } from "../lib/galaxy/featureWire";
import { catalogueClassWords } from "../lib/galaxy/featureWords";
import type { Acknowledgement } from "../lib/useAlerts";
import { useScrollMetrics } from "../lib/useScrollMetrics";
import { windowRange } from "../lib/windowRange";
import { StaleSlot } from "./StaleSlot";
import { StatusText } from "./StatusText";

/** How an alert list is ordered: by the guide's three keys. */
export type AlertSort = "priority" | "time" | "system";

/** Each order's choice, in the order the control offers them. */
const SORTS: ReadonlyArray<{ readonly sort: AlertSort; readonly label: string }> = [
  { sort: "priority", label: "PRIORITY" },
  { sort: "time", label: "TIME" },
  { sort: "system", label: "SYSTEM" },
];

/**
 * The alert list's single key, which acknowledges the active alert while the list has the focus:
 * `A`, which the `GALAXY` display's pages leave free (`M`, `K`, `Q`, `D` and the chart's own).
 */
export const ACKNOWLEDGE_KEY = "A";

/**
 * The guide's four alert classes, most urgent first: the order `PRIORITY` sorts by. A sensor
 * detection is Advisory (plan 12, Design note 11), the only class raised so far.
 */
const CLASS_RANK = { emergency: 0, warning: 1, caution: 2, advisory: 3 } as const;

/** The ship system that raises a sensor detection, which names it (the guide's "Alerts"). */
export const SENSORS_SYSTEM = "SENSORS";

/** The guide's event kind for a detection, whose source may not yet be resolved. */
const TRANSIENT_WORD = "TRANSIENT";

/**
 * What the census says before the server has scanned, so that an empty list is not read as a sky
 * with no transients in it.
 */
export const NO_CENSUS = "NO CENSUS: the server has not scanned";

/** Height of a row, in `rem`: three 1 rem lines, the alert's text, what it is and when and how bright. */
const ROW_REM = 3;

/** Rows rendered beyond each end of the visible ones, so that a scroll shows no gap. */
const OVERSCAN_ROWS = 4;

/** What kind of event a contact is, in words; `UNCLASSIFIED` while its light does not say. */
export function transientKindWords(kind: TransientKindDto): string {
  let words: string;
  switch (kind) {
    case "unclassified":
      words = "UNCLASSIFIED";
      break;
    case "core_collapse":
      words = "CORE-COLLAPSE SUPERNOVA";
      break;
    case "type_ia":
      words = "TYPE Ia SUPERNOVA";
      break;
    case "luminous_red_nova":
      words = "LUMINOUS RED NOVA";
      break;
    case "kilonova":
      words = "KILONOVA";
      break;
    case "nova":
      words = "NOVA";
      break;
    case "dwarf_nova":
      words = "DWARF NOVA";
      break;
    case "xray_transient":
      // The guide's name for a transient low-mass X-ray binary's outburst.
      words = "X-RAY NOVA";
      break;
    case "xray_burst":
      words = "X-RAY BURST";
      break;
    case "be_x_outburst":
      words = "Be X-RAY OUTBURST";
      break;
    case "giant_eruption":
      words = "GIANT ERUPTION";
      break;
    case "tidal_disruption":
      words = "TIDAL DISRUPTION";
      break;
    case "central_flare":
      words = "CENTRE FLARE";
      break;
    case "flare":
      words = "FLARE";
      break;
    case "glitch":
      words = "GLITCH";
      break;
    case "magnetar_burst":
      words = "MAGNETAR BURST";
      break;
    case "fu_orionis_outburst":
      words = "FU ORIONIS OUTBURST";
      break;
  }
  return words;
}

/** A band a flux is in, as the astronomers name it: its letter, or `MID INFRARED`, `RADIO`, `X-RAY`. */
export function alertBandWords(band: AlertBandDto): string {
  let words: string;
  switch (band) {
    case "u":
    case "b":
    case "v":
    case "r":
    case "i":
    case "j":
    case "h":
    case "k":
      words = band.toUpperCase();
      break;
    case "mid_infrared":
      words = "MID INFRARED";
      break;
    case "radio":
      words = "RADIO";
      break;
    case "x_ray":
      words = "X-RAY";
      break;
  }
  return words;
}

/** How a class's census stands, in words. */
function censusStatusWords(status: ClassCensusStatusDto): string {
  let words: string;
  switch (status) {
    case "complete":
      words = "COMPLETE";
      break;
    case "over_limit":
      words = "OVER LIMIT";
      break;
    case "pending":
      words = "PENDING";
      break;
    // Its source is not built, or the server lends its walk no global list (plan 12, P12.T5.d).
    case "unwalked":
      words = "NOT WALKED";
      break;
  }
  return words;
}

/**
 * A class's census line in words, its class's name and how its census stands: `ACCRETING WD
 * PENDING`, `STELLAR MERGER OVER LIMIT`, `LUMINOUS BLUE VARIABLE NOT WALKED`; a class this build
 * does not know is Missing, the em dash.
 */
export function censusLineWords(census: ClassCensusDto): string {
  const name = catalogueClassWords(toCatalogueClass(census.class)) ?? "—";
  return `${name} ${censusStatusWords(census.status)}`;
}

/**
 * An alert's text, as the guide writes a sensor detection: the system, the kind of event, its
 * direction and whether its source is resolved, `SENSORS TRANSIENT 047° +12°: unresolved`; the
 * direction Missing for a sighting from the host's own place, which has none.
 */
export function alertText(contact: ContactDto): string {
  const direction =
    contact.bearing === null
      ? "—"
      : formatBearing(
          contact.bearing.azimuth_deg,
          contact.bearing.elevation_deg,
          contact.bearing.frame,
        );
  const resolution = contact.level === "resolved" ? "resolved" : "unresolved";
  return `${SENSORS_SYSTEM} ${TRANSIENT_WORD} ${direction}: ${resolution}`;
}

/** A contact's flux in its band: `FLUX V 3.20E-12 W/m²`, the unit written beside it. */
function fluxReading(contact: ContactDto): string {
  return `FLUX ${alertBandWords(contact.band)} ${formatSignificant(contact.flux_w_m2)} W/m²`;
}

/** When a contact was first seen, the time its alert was raised: `UT +0.52 yr`. */
function raisedReading(contact: ContactDto): string {
  return `${TIME_SYSTEM_LABEL} ${formatUniverseTimeYr(universeTimeToYears(contact.first_seen))} yr`;
}

/**
 * What a contact is, as far as its light says: its kind, and for a resolved one its host's
 * designation; a bearing names no host.
 */
function whatReading(contact: ContactDto): string {
  const kind = transientKindWords(contact.kind);
  return contact.level === "resolved" ? `${kind} · ${contact.designation}` : kind;
}

/** An acknowledgement's state in the row, in a word: confirmed, or the command's own state. */
function acknowledgementWords(
  contact: ContactDto,
  acknowledgement: Acknowledgement | undefined,
): string | null {
  if (contact.acknowledged) {
    return "ACKNOWLEDGED";
  }
  if (acknowledgement === undefined) {
    return null;
  }
  let words: string;
  switch (acknowledgement.kind) {
    case "pending":
      words = "PENDING";
      break;
    case "rejected":
      words = "REJECTED";
      break;
    case "timed_out":
      words = "TIMED OUT";
      break;
  }
  return words;
}

/** The order of two contacts by when they were raised, newest first, then by number, newest first. */
function byTimeNewestFirst(a: ContactDto, b: ContactDto): number {
  return (
    b.first_seen.seconds - a.first_seen.seconds ||
    b.first_seen.nanos - a.first_seen.nanos ||
    b.contact - a.contact
  );
}

/** A contact's alert class's rank, the most urgent 0: a detection's is Advisory's (Design note 11). */
function classRankOf(_contact: ContactDto): number {
  return CLASS_RANK.advisory;
}

/** The ship system that raised a contact's alert: the sensors, for every detection. */
function systemOf(_contact: ContactDto): string {
  return SENSORS_SYSTEM;
}

/**
 * The contacts in an alert list's order.
 *
 * @remarks
 * - `priority`: by class, the most urgent first, then the unacknowledged before the acknowledged,
 *   then the newest first.
 * - `time`: by when each was raised, its first sighting's time, the newest first.
 * - `system`: by the name of the ship system that raised it, then the newest first.
 *
 * Ties are broken by contact number, the newest first, so that the order is total.
 */
export function sortAlerts(
  contacts: ReadonlyArray<ContactDto>,
  sort: AlertSort,
): ReadonlyArray<ContactDto> {
  let order: (a: ContactDto, b: ContactDto) => number;
  switch (sort) {
    case "priority":
      order = (a, b) =>
        classRankOf(a) - classRankOf(b) ||
        Number(a.acknowledged) - Number(b.acknowledged) ||
        byTimeNewestFirst(a, b);
      break;
    case "time":
      order = byTimeNewestFirst;
      break;
    case "system":
      order = (a, b) => systemOf(a).localeCompare(systemOf(b)) || byTimeNewestFirst(a, b);
      break;
  }
  return contacts.toSorted(order);
}

/** The accessible name of a row, which stands for its content. */
function rowName(contact: ContactDto, ack: string | null, stale: boolean): string {
  return [
    alertText(contact),
    whatReading(contact),
    contact.phase === "onset" ? "ONSET" : "IN PROGRESS",
    raisedReading(contact),
    fluxReading(contact).replace("W/m²", "watts per square metre"),
    ack,
    stale ? "stale" : null,
  ]
    .filter((part) => part !== null)
    .join(", ");
}

/** Props of {@link AlertList}. */
export interface AlertListProps {
  /** The contacts on show, in any order. */
  readonly contacts: ReadonlyArray<ContactDto>;
  /** Each class's census; empty until the server has scanned. */
  readonly census: ReadonlyArray<ClassCensusDto>;
  /** The acknowledgements sent and not yet confirmed, by contact. */
  readonly acknowledgements: ReadonlyMap<number, Acknowledgement>;
  readonly sort: AlertSort;
  readonly onSort: (sort: AlertSort) => void;
  /** The active row's contact, which the keyboard acts on; `null` with none. */
  readonly activeContact: number | null;
  /** Makes a row active, by the keyboard, a click or a tap. */
  readonly onActivate: (contact: ContactDto) => void;
  /** Acknowledges a contact. */
  readonly onAcknowledge: (contact: number) => void;
  /** Why acknowledging is held back, such as `NO CARRIER`; `null` when it acts. */
  readonly heldBack: string | null;
  /** Whether what is shown is a snapshot the link no longer backs: each row trails one `S`. */
  readonly stale: boolean;
}

/**
 * A station's alert list (the guide's "Alerts"): the sensor detections, as Advisory alerts, sorted
 * by priority, time or system, from which each is acknowledged, and the census of each class under
 * it (plan 12, P12.T12).
 *
 * @remarks
 * Each row is an Advisory alert, presented as the guide's class is: coloured text, no sound, no
 * flash, nothing to reduce under `prefers-reduced-motion`. Its first line is the alert's text in
 * `--status-advisory`, `SENSORS TRANSIENT 047° +12°: unresolved`; its second what the light says
 * it is, `UNCLASSIFIED`, or a resolved contact's kind and host, and its acknowledgement; its third
 * when it was raised and its flux in its band. A contact at `bearing` level names no host anywhere.
 * Acknowledgement never hides a row.
 *
 * The list is a `listbox` whose rows are `option`s, windowed as the system list is, with its
 * position and total under it: `ArrowUp` and `ArrowDown` move the active row, `PageUp` and
 * `PageDown` a window, `Home` and `End` the ends, and a click or a tap makes a row active. `A`
 * acknowledges the active row while the list has the focus, as `ACKNOWLEDGE` does, which shows
 * the key; acknowledging is a command to the server, so it is a `.command`, closed-loop: the row
 * reads `PENDING` until the server answers, and the line beside the command the result in words.
 * It is held back, and says why, with no row active, the row acknowledged already or pending, or
 * the link down. The order is a display control, `SORT` and its three keys, the chosen one
 * pressed, one stop in the tab order, the arrows moving between them.
 *
 * Under the list stands a census line per class, its class's name and how its census stands, in
 * words (`ACCRETING WD PENDING`), and before the server has scanned, `NO CENSUS`, so that a list
 * with no alerts is not read as a sky with none. A stale list is muted, as its panel has it, each
 * row trailing one `S` at its end, its name ending `, stale`. The newest unacknowledged alert's
 * text is announced politely, an Advisory's being awareness only.
 */
export function AlertList({
  contacts,
  census,
  acknowledgements,
  sort,
  onSort,
  activeContact,
  onActivate,
  onAcknowledge,
  heldBack,
  stale,
}: AlertListProps) {
  const baseId = useId();
  const reasonId = useId();
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

  const sorted = sortAlerts(contacts, sort);
  const rowHeightPx = ROW_REM * metrics.remPx;
  const viewportPx = metrics.viewportPx;
  const total = sorted.length;
  const range = windowRange(scrollTopPx, rowHeightPx, viewportPx, total, OVERSCAN_ROWS);
  const rowsInWindow = Math.max(1, Math.floor(viewportPx / rowHeightPx));
  const activeIndex = sorted.findIndex((contact) => contact.contact === activeContact);
  const active = sorted[activeIndex] ?? null;

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

  // A row made active elsewhere, or moved by a new order, is scrolled into view by its index.
  useLayoutEffect(() => {
    if (activeIndex >= 0) {
      showRow(activeIndex, rowHeightPx, viewportPx);
    }
  }, [activeIndex, rowHeightPx, viewportPx, showRow]);

  const activate = (index: number): void => {
    const contact = sorted[index];
    if (contact === undefined) {
      return;
    }
    showRow(index, rowHeightPx, viewportPx);
    onActivate(contact);
  };

  const pendingOf = (contact: ContactDto): boolean =>
    acknowledgements.get(contact.contact)?.kind === "pending";
  let acknowledgeHeld: string | null = heldBack;
  if (acknowledgeHeld === null) {
    if (active === null) {
      acknowledgeHeld = "NO ALERT SELECTED";
    } else if (active.acknowledged) {
      acknowledgeHeld = "ACKNOWLEDGED";
    } else if (pendingOf(active)) {
      acknowledgeHeld = "ACKNOWLEDGE PENDING";
    }
  }
  const acknowledgeActive = (): void => {
    if (acknowledgeHeld === null && active !== null) {
      onAcknowledge(active.contact);
    }
  };

  const onClick = (event: MouseEvent<HTMLDivElement>): void => {
    const row = event.target instanceof Element ? event.target.closest("[data-index]") : null;
    const index = Number(row instanceof HTMLElement ? row.dataset["index"] : Number.NaN);
    if (Number.isInteger(index)) {
      activate(index);
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    if (total === 0 || event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    if (event.key.toUpperCase() === ACKNOWLEDGE_KEY && !event.shiftKey) {
      event.preventDefault();
      if (!event.repeat) {
        acknowledgeActive();
      }
      return;
    }
    // With no row active the first is, as the list opens on it.
    const from = activeIndex < 0 ? 0 : activeIndex;
    let next: number;
    switch (event.key) {
      case "ArrowDown":
        next = activeIndex < 0 ? 0 : from + 1;
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
    activate(Math.min(total - 1, Math.max(0, next)));
  };

  const rows: number[] = [];
  if (range !== null) {
    for (let index = range.first; index <= range.last; index += 1) {
      rows.push(index);
    }
  }
  const newest = sortAlerts(
    contacts.filter((contact) => !contact.acknowledged),
    "time",
  )[0];
  const activeAcknowledgement = active === null ? undefined : acknowledgements.get(active.contact);
  // A pending acknowledgement is said by the command's reason; its result once it settles here.
  let commandResult: string | null = null;
  switch (activeAcknowledgement?.kind) {
    case "rejected":
      commandResult = `ACKNOWLEDGE REJECTED: ${activeAcknowledgement.reason}`;
      break;
    case "timed_out":
      commandResult = "ACKNOWLEDGE TIMED OUT";
      break;
    case "pending":
    case undefined:
      break;
  }

  return (
    <div className={stale ? "alert-list alert-list--stale" : "alert-list"}>
      <div className="alert-list__head">
        <fieldset className="form-choice alert-list__sort">
          <legend className="alert-list__sort-legend">SORT</legend>
          {SORTS.map(({ sort: each, label }) => (
            <button
              key={each}
              type="button"
              className="control mode-control__button"
              aria-pressed={each === sort}
              // One stop in the tab order, the order chosen's, the arrows moving between them.
              tabIndex={each === sort ? 0 : -1}
              data-sort={each}
              onClick={() => {
                onSort(each);
              }}
              onKeyDown={(event) => {
                if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
                  return;
                }
                event.preventDefault();
                const index = SORTS.findIndex((option) => option.sort === each);
                const step = event.key === "ArrowRight" ? 1 : SORTS.length - 1;
                const other = SORTS[(index + step) % SORTS.length]?.sort ?? each;
                onSort(other);
                event.currentTarget.parentElement
                  ?.querySelector<HTMLButtonElement>(`[data-sort="${other}"]`)
                  ?.focus();
              }}
            >
              {label}
            </button>
          ))}
        </fieldset>
      </div>
      {total === 0 ? (
        <p className="alert-list__empty">NO ALERTS</p>
      ) : (
        <div
          className="alert-list__scroll"
          // A windowed list, which no native control can be (plan 05, design note D17).
          // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
          role="listbox"
          aria-label="Alerts"
          aria-keyshortcuts={ACKNOWLEDGE_KEY}
          tabIndex={0}
          aria-activedescendant={active === null ? undefined : `${baseId}-${active.contact}`}
          ref={setList}
          onClick={onClick}
          onKeyDown={onKeyDown}
          onScroll={(event) => {
            offsetRef.current = event.currentTarget.scrollTop;
            setScrollTopPx(event.currentTarget.scrollTop);
          }}
        >
          <div
            className="alert-list__rows"
            role="presentation"
            style={{ height: `${total * ROW_REM}rem` }}
          >
            {rows.map((index) => {
              const contact = sorted[index];
              if (contact === undefined) {
                return null;
              }
              const ack = acknowledgementWords(contact, acknowledgements.get(contact.contact));
              return (
                <div
                  key={contact.contact}
                  id={`${baseId}-${contact.contact}`}
                  // `option` belongs to a `select`, which cannot be windowed (D17).
                  // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
                  role="option"
                  aria-selected={contact.contact === activeContact}
                  aria-posinset={index + 1}
                  aria-setsize={total}
                  aria-label={rowName(contact, ack, stale)}
                  className="alert-list__row"
                  style={{ transform: `translateY(${index * ROW_REM}rem)` }}
                  data-index={index}
                >
                  <span className="alert-list__line">
                    <span className="alert-list__text">{alertText(contact)}</span>
                  </span>
                  <span className="alert-list__line">
                    <span className="alert-list__what">{whatReading(contact)}</span>
                    <span className="alert-list__ack">{ack ?? ""}</span>
                  </span>
                  <span className="alert-list__line">
                    <span className="alert-list__number">{raisedReading(contact)}</span>
                    <span className="alert-list__number alert-list__flux">
                      {fluxReading(contact)}
                      <StaleSlot stale={stale} />
                    </span>
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}
      {range === null ? null : (
        <p className="list-position">
          {formatListPosition(range.firstVisible, range.lastVisible, total)}
        </p>
      )}
      <div className="alert-list__actions">
        <button
          type="button"
          className="command"
          aria-keyshortcuts={ACKNOWLEDGE_KEY}
          // Held back rather than disabled, so that it keeps its focus and can say why.
          aria-disabled={acknowledgeHeld === null ? undefined : "true"}
          aria-describedby={acknowledgeHeld === null ? undefined : reasonId}
          onClick={acknowledgeActive}
        >
          <span className="control__key">{ACKNOWLEDGE_KEY}</span> ACKNOWLEDGE
        </button>
        {acknowledgeHeld === null ? null : (
          <span className="alert-list__reason" id={reasonId}>
            <StatusText text={acknowledgeHeld} />
          </span>
        )}
        {commandResult === null ? null : (
          <output className="alert-list__result">
            <StatusText text={commandResult} />
          </output>
        )}
      </div>
      <ul className="alert-list__census" aria-label="Census by class">
        {census.length === 0 ? (
          <li className="alert-list__census-none">
            <StatusText text={NO_CENSUS} />
          </li>
        ) : (
          census.map((line) => <li key={line.class}>{censusLineWords(line)}</li>)
        )}
      </ul>
      <output className="visually-hidden">{newest === undefined ? "" : alertText(newest)}</output>
    </div>
  );
}
