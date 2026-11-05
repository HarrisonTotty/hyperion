import { describe, expect, it } from "vitest";

import { queryOverride } from "./geometryQuery";

describe("the smoke page's geometry override (plan R14, Design note 17)", () => {
  it("reads each field's values from the query, the far cascades' rate (R14.T6.b), the consumers' levers (R14.T4), the page pool's bytes (R14.T5.b) the edge patches' lever (R14.T1.g), the camera's strides (R14.T3.c), the occlusion (R14.T3.d), the shading (R14.T2.c), the lit selection's budget (R14.T3.e), the stand-ins' strides (R14.T3.c's stand-ins) the overlay's hidden marks (R10.T16.c), the edge bakes' stand-ins (R14.T3.c's edge stand-ins) and the disc cells (plan R07) among them", () => {
    expect(
      queryOverride(
        new URLSearchParams(
          "geometry-clusterPath=false&geometry-casterStrideCap=4&geometry-overlayDepth=redraw&geometry-litClusters=false&geometry-farCascadeRate=2&geometry-figureClusters=false&geometry-hullClusters=true&geometry-wireClusters=false&geometry-pageBytes=1048576&geometry-meshTolerancePx=2:0.5&geometry-finClusters=false&geometry-strides=measured&geometry-occlusion=hzb&geometry-shading=visibility&geometry-terrainBudget=triangles&geometry-standInStrides=own-grids&geometry-hiddenMarks=drawn&geometry-edgeStandIns=surveyed&geometry-discCells=per-cell",
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
      meshTolerancePx: { camera: 2, wireframeOccluder: 0.5 },
      finClusters: false,
      strides: "measured",
      occlusion: "hzb",
      shading: "visibility",
      terrainBudget: "triangles",
      standInStrides: "own-grids",
      hiddenMarks: "drawn",
      edgeStandIns: "surveyed",
      discCells: "per-cell",
    });
  });

  it("leaves out a value that is not its field's, and a field the query does not give", () => {
    expect([
      queryOverride(
        new URLSearchParams(
          "geometry-farCascadeRate=3&geometry-casterDepthCull=1&geometry-pageBytes=1MiB&geometry-meshTolerancePx=1&geometry-strides=on&geometry-occlusion=none&geometry-shading=deferred&geometry-terrainBudget=frames&geometry-standInStrides=masks&geometry-hiddenMarks=on&geometry-edgeStandIns=all&geometry-discCells=pooled",
        ),
      ),
      queryOverride(new URLSearchParams("")),
    ]).toEqual([{}, {}]);
  });
});
