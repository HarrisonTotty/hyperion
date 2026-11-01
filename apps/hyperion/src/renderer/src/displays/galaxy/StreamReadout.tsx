import { SolarMassUnit } from "../../components/SolarMassUnit";
import { featureMarkLabel } from "../../lib/galaxy/featureWords";
import type { ChartFeature, GlobalListModel } from "../../lib/galaxy/model";
import { streamFigures } from "../../lib/galaxy/streamTracks";
import { formatHex64 } from "../../lib/seed";
import { useServerLink } from "../../lib/serverLink";
import { numberText } from "./parameterReadings";
import { SystemReading as Reading } from "./SystemReading";
import type { StreamReadoutQuery } from "./useStreamReadout";

/** Props of {@link StreamReadout}. */
export interface StreamReadoutProps {
  /** The stream's number on the open universe's global list. */
  readonly stream: number;
  /** What the readout reads of the stream, which the panel asks for (`useStreamReadout`). */
  readonly query: StreamReadoutQuery;
  /** The features of the chart on show, whose globular a living globular's stream names. */
  readonly features: ReadonlyArray<ChartFeature>;
}

/**
 * A stream's `STREAM PROGENITOR`: the living globular it is still being stripped from, by its
 * designation where the chart holds it (`GC NGC 104`) or its ID; a dissolved globular for an
 * orphan's stream; and the accreted galaxy the list's progenitors name as its source; `null` for a
 * stream the list lacks.
 */
function progenitorText(
  list: GlobalListModel,
  stream: number,
  features: ReadonlyArray<ChartFeature>,
): string | null {
  const entry = list.streams.find((each) => each.number === stream);
  if (entry === undefined) {
    return null;
  }
  if (entry.progenitor !== null) {
    const { progenitor } = entry;
    const feature = features.find((each) => each.id === progenitor);
    return feature === undefined
      ? `FEATURE ${formatHex64(progenitor)}`
      : featureMarkLabel(feature.kind, feature.designation);
  }
  const accreted = list.progenitors.find((each) => each.stream === stream);
  if (accreted === undefined) {
    return "DISSOLVED GLOBULAR";
  }
  let text: string;
  switch (accreted.kind) {
    case "dominant_merger":
      text = "DOMINANT MERGER";
      break;
    case "recent":
      text = "RECENT DWARF";
      break;
  }
  return text;
}

/**
 * The readout of a stream member's stream (plan 10, P10.T11), in the system readout's own list
 * after the member's `ORIGIN`: the stream's `STREAM MASS` and `STREAM PROGENITOR` once the global
 * list is answered, and its centre line's `STREAM LENGTH`, its median rms `STREAM WIDTH` and
 * one-axis `STREAM DISPERSION` once its track is ({@link streamFigures}).
 *
 * @remarks
 * Until an answer the rows it fills are absent, never zero, as the readout's other rows are, and
 * where the requests stand is said beside the readout, with `RETRY`, by the panel that asks
 * (rulings 13 and 14). A value the server does not send is Missing. The values are the stream's at
 * the epoch, the time of the global list, so they never go stale with the chart's time; they go
 * stale, muted with the guide's `S`, while the link is down.
 */
export function StreamReadout({ stream, query, features }: StreamReadoutProps) {
  // Every live value goes stale with the link; these come with no answer of the chart's.
  const stale = useServerLink().status !== "connected";
  const { list, track } = query;
  const entry = list?.streams.find((each) => each.number === stream);
  const figures = track === null ? null : streamFigures(track);
  return (
    <>
      {list === null ? null : (
        <>
          <Reading
            label="STREAM MASS"
            value={entry === undefined ? null : numberText(entry.massMsun, "msun")}
            unit={<SolarMassUnit />}
            stale={stale}
          />
          <Reading
            label="STREAM PROGENITOR"
            value={progenitorText(list, stream, features)}
            wide
            stale={stale}
          />
        </>
      )}
      {figures === null ? null : (
        <>
          <Reading
            label="STREAM LENGTH"
            value={figures.lengthLy === null ? null : numberText(figures.lengthLy, "ly")}
            unit="ly"
            stale={stale}
          />
          <Reading
            label="STREAM WIDTH"
            value={figures.widthLy === null ? null : numberText(figures.widthLy, "ly")}
            unit="ly"
            stale={stale}
          />
          <Reading
            label="STREAM DISPERSION"
            value={
              figures.dispersionKmS === null ? null : numberText(figures.dispersionKmS, "km_per_s")
            }
            unit="km/s"
            stale={stale}
          />
        </>
      )}
    </>
  );
}
