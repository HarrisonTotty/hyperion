import { annunciation } from "../../components/RequestStatus";
import { formatNumber } from "../../lib/format";
import type { MapFeatureStatus } from "./mapFeatureStatus";
import type { GlobalListQuery } from "./useGlobalList";

/** Which of the stream overlay's kinds the page draws. */
export interface StreamOverlayShown {
  readonly streams: boolean;
  readonly cores: boolean;
}

/**
 * What the galaxy map's page says of its stream overlay (plan 10, P10.T11), or `null` while there
 * is nothing to say or neither of its kinds is shown.
 *
 * @remarks
 * The guide's rule that a layer still being computed says so, so that tracks and cores not yet
 * drawn never look absent. While the global list has not answered, or a newer request of it has
 * failed: its request's own state, after the subject of what waits on it, `TRACKS` while the
 * tracks are shown and `DWARF CORES` while only the cores are (`TRACKS PENDING`,
 * `TRACKS TIMED OUT`, `TRACKS REJECTED:` with `RETRY`). Then an answer that cannot be drawn, with
 * `RETRY`, which asks again: `TRACK DATA INVALID:` while the tracks are shown, and
 * `DWARF CORE DATA INVALID:` while the cores are. And once the list has answered, while the tracks
 * are shown, the count of its streams whose tracks are not drawn yet, those the server is still
 * building and those on their way, `TRACKS PENDING 12 OF 261` (lean, 2026-11-01: a draft row of
 * the guide's for the owner), its figures as {@link pendingCount} sets them. The link's state is
 * the link's, which the page states first, so a request held by a link that is down says nothing
 * here.
 */
export function streamTrackStatus(
  query: GlobalListQuery,
  shown: StreamOverlayShown,
): MapFeatureStatus | null {
  if (!shown.streams && !shown.cores) {
    return null;
  }
  const { state, list, tracks, trackFault, coreFault } = query;
  const said = state.kind === "link_down" ? null : annunciation(state);
  // A failure stands whether or not a list is kept; a refresh in flight behind a kept list does not.
  if (said !== null && (list === null || said.standing !== "waiting")) {
    return {
      text: `${shown.streams ? "TRACKS" : "DWARF CORES"} ${said.text}`,
      standing: said.standing,
      retry: said.standing !== "waiting",
    };
  }
  if (shown.streams && trackFault !== null) {
    return { text: `TRACK DATA INVALID: ${trackFault}`, standing: "fault", retry: true };
  }
  if (shown.cores && coreFault !== null) {
    return { text: `DWARF CORE DATA INVALID: ${coreFault}`, standing: "fault", retry: true };
  }
  if (list === null || !shown.streams) {
    return null;
  }
  const total = list.streams.length;
  const pending = list.streams.filter((stream) => !tracks.has(stream.number)).length;
  return pending === 0
    ? null
    : {
        text: `TRACKS PENDING ${pendingCount(pending, total)} OF ${formatNumber(total, 0)}`,
        standing: "waiting",
        retry: false,
      };
}

/**
 * The count of tracks pending, right-aligned with no-break spaces in a field as wide as the total's
 * digits, so that `OF <m>` keeps its place as the count falls; set in B612 Mono with the line.
 */
export function pendingCount(pending: number, total: number): string {
  const width = formatNumber(total, 0).length;
  return formatNumber(pending, 0).padStart(width, "\u00a0");
}
