import { useId } from "react";

import { annunciation, RequestStatus } from "../../components/RequestStatus";
import { StaleMark } from "../../components/StaleMark";
import { StaleSlot } from "../../components/StaleSlot";
import { StatusLine } from "../../components/StatusLine";
import { StatusText } from "../../components/StatusText";
import { EventList } from "./EventList";
import { formatTimeSpan } from "./eventRows";
import { compareTimes } from "./requestTime";
import type { SystemViewState } from "./useSystemView";

/** Props of {@link EventsPanel}. */
export interface EventsPanelProps {
  readonly view: SystemViewState;
}

/**
 * The `EVENTS` panel: the events on the system's bodies in the century around the display time,
 * from `body_events`, each with its time in the display's time system and its countdown from the
 * display time (plan 14, P14.T43.c).
 *
 * @remarks
 * It stands under the display time in the map's column, since every countdown runs from that time.
 * `LISTED` reads the span the list holds every event of, the century asked about; where a busy
 * system holds more events than one answer lists, the server lists those of a shorter span about
 * its middle, the display time, and `ASKED` then reads the century beside it, so that a short list
 * is never read as a quiet century. The display asks again about the display time once that time
 * leaves the middle half of the listed span. Until there is an answer, or while the system is not
 * yet formed, the panel says so without being a live region of its own, as the bodies' panel does;
 * a server that does not serve the events, one built before them or of a generator version before
 * 25 (P14.T31.c's hold), is `NOT YET MODELLED`, and a century with none reads `NO EVENTS`. An answer the link no longer backs, or whose newer request
 * failed, is muted and named stale, with the guide's trailing `S`.
 */
export function EventsPanel({ view }: EventsPanelProps) {
  const titleId = useId();
  const { events, eventsStale: stale, model, fault } = view;
  const designations = new Map(view.rows.map((row) => [row.id, row.designation]));
  const shown = events.shown;
  const listed = shown?.kind === "ok" ? shown.events : null;

  let empty: string | null = null;
  if (fault !== null) {
    empty = "SYSTEM DATA INVALID";
  } else if (model === null) {
    empty = annunciation(view.status)?.text ?? null;
  } else if (!model.formed) {
    empty = "NOT YET FORMED";
  } else if (events.unserved) {
    empty = "NOT YET MODELLED";
  } else if (listed !== null && listed.events.length === 0) {
    empty = "NO EVENTS";
  }
  const cut =
    listed !== null &&
    (compareTimes(listed.listedFrom, listed.from) !== 0 ||
      compareTimes(listed.listedTo, listed.to) !== 0);

  return (
    <section
      className={
        stale
          ? "panel system__events events-panel events-panel--stale"
          : "panel system__events events-panel"
      }
      aria-labelledby={titleId}
    >
      <h2 className="panel__title" id={titleId}>
        Events{stale ? <StaleMark /> : null}
      </h2>
      {listed === null ? null : (
        <dl className="readout events-panel__span">
          <dt>LISTED</dt>
          <dd className="events-panel__value">
            {formatTimeSpan(listed.listedFrom, listed.listedTo)}
            <StaleSlot stale={stale} />
          </dd>
          {cut ? (
            <>
              <dt>ASKED</dt>
              <dd className="events-panel__value">
                {formatTimeSpan(listed.from, listed.to)}
                <StaleSlot stale={stale} />
              </dd>
            </>
          ) : null}
        </dl>
      )}
      {shown?.kind === "fault" ? (
        <StatusLine
          text={`EVENTS DATA INVALID: ${shown.fault}`}
          standing="fault"
          action={{ label: "RETRY", onAction: view.retry }}
        />
      ) : empty === null && listed !== null ? (
        <EventList
          events={listed.events}
          designations={designations}
          now={view.displayTime.time}
          selectedBody={view.selected?.kind === "body" ? view.selected.body.id : null}
          onSelectBody={view.select}
          stale={stale}
        />
      ) : empty === null ? null : (
        <p className="panel__empty">
          <StatusText text={empty} />
        </p>
      )}
      {/*
       * The request's state, under the list it would replace, or in its place before the first
       * answer: `PENDING`, or why it failed, with `RETRY`.
       */}
      {empty !== null || events.unserved ? null : (
        <RequestStatus state={events.state} onRetry={view.retry} />
      )}
    </section>
  );
}
