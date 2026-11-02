import { describe, expect, it } from "vitest";

import { queryOverride } from "./geometryQuery";

describe("the smoke page's geometry override (plan R14, Design note 17)", () => {
  it("reads each field's values from the query, the far cascades' rate (R14.T6.b), the consumers' levers (R14.T4) and the page pool's bytes (R14.T5.b) among them", () => {
    expect(
      queryOverride(
        new URLSearchParams(
          "geometry-clusterPath=false&geometry-casterStrideCap=4&geometry-overlayDepth=redraw&geometry-litClusters=false&geometry-farCascadeRate=2&geometry-figureClusters=false&geometry-hullClusters=true&geometry-wireClusters=false&geometry-pageBytes=1048576",
        ),
      ),
    ).toEqual({
      clusterPath: false,
      casterStrideCap: 4,
      overlayDepth: "redraw",
      litClusters: false,
      farCascadeRate: 2,
      figureClusters: false,
      hullClusters: true,
      wireClusters: false,
      pageBytes: 1_048_576,
    });
  });

  it("leaves out a value that is not its field's, and a field the query does not give", () => {
    expect([
      queryOverride(
        new URLSearchParams(
          "geometry-farCascadeRate=3&geometry-casterDepthCull=1&geometry-pageBytes=1MiB",
        ),
      ),
      queryOverride(new URLSearchParams("")),
    ]).toEqual([{}, {}]);
  });
});
