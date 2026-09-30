import {
  type SystemIdHex,
  type UniverseIdHex,
  type UniverseTime,
  universeTimeFromYears,
} from "@hyperion/protocol";

import type { ChartSystem } from "../../lib/galaxy/model";
import type { Vec3 } from "../../spatial/vec3";
import type { LayerBand } from "../galaxy/chartModel";

/**
 * What the `SYSTEM` display is opened on: the system selected on the `GALAXY` chart, at the chart's
 * time, with what the chart already knows of it (plan 14, P14.T41.a).
 *
 * @remarks
 * `system_summary` answers with the stars and their orbits, not with where the system is or what it
 * is called, which the chart's range query carried; so they cross with the ID and are not asked
 * again.
 */
export interface SystemTarget {
  readonly universe: UniverseIdHex;
  readonly system: SystemIdHex;
  /** The system's designation of record, which every body's designation extends. */
  readonly designation: string;
  /**
   * The system's position in the `GALACTIC` frame, which is where its barycentre is: the orbit
   * map's centre and the source of its core arrow's distance (the orchestrator's ruling 34.5).
   */
  readonly positionLy: Vec3;
  /** The chart's time, at which the display opens and is held. */
  readonly time: UniverseTime;
  /**
   * The mass bands of the chart's census, lightest first, which give each host its size class by
   * its initial mass, as the chart sizes a system (the orchestrator's ruling 36).
   */
  readonly bands: ReadonlyArray<LayerBand>;
}

/**
 * One opening of the `SYSTEM` display: its target, and how many openings came before, so that
 * opening the same system again starts it afresh, held at the chart's time.
 */
export interface SystemOpening {
  readonly target: SystemTarget;
  /** Counts the openings from 1; each opening is a new display state. */
  readonly sequence: number;
}

/**
 * The target `OPEN SYSTEM` opens for a system on the chart.
 *
 * @param timeYr - The chart's time, in years from the epoch, as the chart holds it.
 * @throws RangeError when the time is not finite, which a chart never holds.
 */
export function systemTargetFor(
  universe: UniverseIdHex,
  system: ChartSystem,
  timeYr: number,
  bands: ReadonlyArray<LayerBand>,
): SystemTarget {
  return {
    universe,
    system: system.id,
    designation: system.designation,
    positionLy: system.positionLy,
    time: universeTimeFromYears(timeYr),
    bands,
  };
}
