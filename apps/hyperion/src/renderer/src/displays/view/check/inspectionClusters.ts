/**
 * The inspection run's watch of `VIEW`'s terrain session (plan R14; R14.T3.d's counts): the
 * inspected body's lit pass, whose cluster counters its frame profile reads as each window ends.
 *
 * @remarks
 * `ViewDisplay` hands the watch the session it holds (`watchTerrain`), as it does the terrain run's
 * probe. A read switches the pass's counters on (`TerrainPass.measureClusters`), so that a window's
 * first read has its frames' counts read back by its last; it gives the primary's latest
 * (`TerrainPass.clusterStats`), by pass, or `null` where the body has no lit pass or none has come
 * back.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import type { InspectionTimingClusters } from "../../../../../preload/api";
import type { ViewId } from "../../../view/camera/state";
import type { ClusterPassStats } from "../../../view/clusters/stats";
import { PRIMARY_VIEW_ID } from "../viewNames";

/** What the watch reads of a terrain session: its body's lit pass's counters (`TerrainSession`'s). */
export interface ClusterSession {
  pass(body: BodyIdHex): {
    measureClusters(on: boolean): void;
    clusterStats(view: ViewId): ReadonlyArray<ClusterPassStats> | null;
  } | null;
}

/** The inspection run's watch of `VIEW`'s terrain session (see the module's remarks). */
export class InspectionClusters {
  #session: ClusterSession | null = null;

  /** Holds `session` while `VIEW` holds it; returns the let-go. */
  readonly watchTerrain = (session: ClusterSession): (() => void) => {
    this.#session = session;
    return () => {
      if (this.#session === session) {
        this.#session = null;
      }
    };
  };

  /** `body`'s lit pass's counters in the primary, switched on by the read (see the module's remarks). */
  counts(body: BodyIdHex | null): InspectionTimingClusters | null {
    const pass = body === null ? null : (this.#session?.pass(body) ?? null);
    if (pass === null) {
      return null;
    }
    pass.measureClusters(true);
    const stats = pass.clusterStats(PRIMARY_VIEW_ID);
    return stats === null
      ? null
      : { passes: stats.map(({ label, clusters, triangles }) => ({ label, clusters, triangles })) };
  }
}
