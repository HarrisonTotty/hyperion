import { describe, expect, it } from "vitest";

import type { ClusterPassStats } from "../../../view/clusters/stats";
import { PRIMARY_VIEW_ID } from "../viewNames";
import { type ClusterSession, InspectionClusters } from "./inspectionClusters";

const BODY = "41ffecb200000002.0400";

/** A session whose body's pass counts `stats` once measuring, recording each switch and view. */
function sessionOf(stats: ReadonlyArray<ClusterPassStats>): {
  readonly session: ClusterSession;
  readonly calls: string[];
} {
  const calls: string[] = [];
  let measuring = false;
  return {
    calls,
    session: {
      pass: (body) =>
        body === BODY
          ? {
              measureClusters: (on) => {
                measuring = on;
                calls.push(`measure ${String(on)}`);
              },
              clusterStats: (view) => {
                calls.push(`stats ${view}`);
                return measuring ? stats : null;
              },
            }
          : null,
    },
  };
}

const STATS: ReadonlyArray<ClusterPassStats> = [
  { label: "lit", clusters: 120, triangles: 9_000, byStride: [120, 0, 0, 0] },
  { label: "lit 2", clusters: 30, triangles: 2_000, byStride: [30, 0, 0, 0] },
];

describe("the inspection run's cluster counters (plan R14, R14.T3.d)", () => {
  it("switches the inspected body's counters on and gives the primary's, by pass", () => {
    const watch = new InspectionClusters();
    const { session, calls } = sessionOf(STATS);
    watch.watchTerrain(session);
    expect([watch.counts(BODY), calls]).toEqual([
      {
        passes: [
          { label: "lit", clusters: 120, triangles: 9_000 },
          { label: "lit 2", clusters: 30, triangles: 2_000 },
        ],
      },
      ["measure true", `stats ${PRIMARY_VIEW_ID}`],
    ]);
  });

  it("gives none for another body, with no session, or once the session is let go", () => {
    const watch = new InspectionClusters();
    const before = watch.counts(BODY);
    const letGo = watch.watchTerrain(sessionOf(STATS).session);
    const other = watch.counts("41ffecb200000002.0500");
    const none = watch.counts(null);
    letGo();
    expect([before, other, none, watch.counts(BODY)]).toEqual([null, null, null, null]);
  });

  it("keeps a later session where an earlier one is let go", () => {
    const watch = new InspectionClusters();
    const letFirstGo = watch.watchTerrain(sessionOf([]).session);
    watch.watchTerrain(sessionOf(STATS).session);
    letFirstGo();
    expect(watch.counts(BODY)?.passes.length).toBe(2);
  });
});
