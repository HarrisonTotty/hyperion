import type { ReactNode } from "react";

import type { ReadoutLine } from "./readouts";
import { TerrainReading, type TerrainReadingLine } from "./TerrainReading";
import { TerrainStatus } from "./TerrainStatus";

/** Props of {@link TerrainReadout}. */
export interface TerrainReadoutProps {
  /**
   * The readout's lines (`groundReadoutLines`): its heading, then its readings, or `NOT SURVEYED`
   * or `NO GROUND` once in their place.
   */
  readonly lines: ReadonlyArray<ReadoutLine>;
}

/** A run of readings, set as one list, keyed by its first reading's label. */
function readingList(readings: ReadonlyArray<TerrainReadingLine>): ReactNode {
  return (
    <dl className="view-label__lines" key={`list ${readings[0]?.key ?? ""}`}>
      {readings.map((line) => (
        <TerrainReading key={line.key} line={line} />
      ))}
    </dl>
  );
}

/**
 * A view's terrain readout on its lower plate (plan R10, R10.T12; the guide's terrain readout):
 * the heading `GROUND AT LINE OF SIGHT`, then `ELEVATION`, `SLOPE`, `SLANT RANGE`, `MATERIAL`,
 * `ULTIMATE BEARING CAPACITY`, `PAD FRICTION` and `RESOLUTION` in the label block's line markup,
 * or `NOT SURVEYED` or `NO GROUND` once in their place, steady in `--text`.
 *
 * @remarks
 * Each run of readings is one `dl.view-label__lines`; the heading, a heading element as a
 * readout's heading is, and a status stand apart from them. Carries no `id`, since a copy of it
 * stands in the view's hidden sizer, in its tallest form.
 */
export function TerrainReadout({ lines }: TerrainReadoutProps) {
  const nodes: ReactNode[] = [];
  let run: TerrainReadingLine[] = [];
  const flush = (): void => {
    if (run.length > 0) {
      nodes.push(readingList(run));
      run = [];
    }
  };
  for (const line of lines) {
    switch (line.kind) {
      case "reading":
        run.push(line);
        break;
      case "status":
        flush();
        nodes.push(<TerrainStatus key={line.key} text={line.text} failure={line.failure} />);
        break;
      case "heading":
        flush();
        nodes.push(
          <h3 className="view-label__heading" key={line.key}>
            {line.text}
          </h3>,
        );
        break;
    }
  }
  flush();
  return <>{nodes}</>;
}
