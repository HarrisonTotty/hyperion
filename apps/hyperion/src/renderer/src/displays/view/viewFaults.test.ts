import { describe, expect, it } from "vitest";

import { type ViewId, viewId } from "../../view/camera/state";
import {
  graphicsAnnunciation,
  GraphicsStatusStore,
  initialGraphicsStatus,
} from "../../view/engine/status";
import {
  TERRAIN_COMPUTATION_FAILED,
  TERRAIN_NOT_AVAILABLE,
  type TerrainFault,
} from "../../view/terrain/annunciation";
import { faultsFor, terrainStatementOf } from "./viewFaults";

const PRIMARY = viewId("view");
const INSTRUMENT = viewId("instrument 1");

/** The primary showing the terrain's fault, its text `text`. */
function primaryFault(
  text: TerrainFault = TERRAIN_NOT_AVAILABLE,
): ReadonlyMap<ViewId, TerrainFault> {
  return new Map([[PRIMARY, text]]);
}

/** The graphics status after a device loss, or with nothing wrong. */
function graphics(lost: boolean): GraphicsStatusStore {
  const store = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
  if (lost) {
    store.dispatch({ kind: "device-lost", reason: "unknown", message: "lost" });
  }
  return store;
}

describe("a view's faults (R10.T16.b and R10.T16.e; decision-r10-t16-contracts §7)", () => {
  it("sets the atmosphere's lines after the graphics fault and the terrain's (R08.T10.b)", () => {
    const status = graphics(true).getSnapshot();
    const air = [
      "ATMOSPHERE NOT AVAILABLE: computation failed for A, reopen VIEW to retry",
      "ATMOSPHERE DATA INVALID: answer unreadable for B, reopen VIEW to retry",
    ];
    expect(
      faultsFor(status, graphicsAnnunciation(status), "view", PRIMARY, primaryFault(), air),
    ).toEqual(["GRAPHICS DEVICE LOST: re-creating", TERRAIN_NOT_AVAILABLE, ...air]);
  });

  it("sets the graphics fault first, then the terrain's, on a view that hands a body over", () => {
    const status = graphics(true).getSnapshot();
    expect(
      faultsFor(status, graphicsAnnunciation(status), "view", PRIMARY, primaryFault()),
    ).toEqual(["GRAPHICS DEVICE LOST: re-creating", TERRAIN_NOT_AVAILABLE]);
  });

  it("sets the terrain's alone with nothing wrong with the graphics", () => {
    const status = graphics(false).getSnapshot();
    expect(
      faultsFor(status, graphicsAnnunciation(status), "view", PRIMARY, primaryFault()),
    ).toEqual([TERRAIN_NOT_AVAILABLE]);
  });

  it("sets the terrain's by its cause: a computation that kept failing", () => {
    const status = graphics(true).getSnapshot();
    expect(
      faultsFor(
        status,
        graphicsAnnunciation(status),
        "view",
        PRIMARY,
        primaryFault(TERRAIN_COMPUTATION_FAILED),
      ),
    ).toEqual(["GRAPHICS DEVICE LOST: re-creating", TERRAIN_COMPUTATION_FAILED]);
  });

  it("sets neither on a view that hands nothing over", () => {
    const status = graphics(false).getSnapshot();
    expect(
      faultsFor(status, graphicsAnnunciation(status), "instrument 1", INSTRUMENT, primaryFault()),
    ).toEqual([]);
  });
});

describe("the terrain's statement beside its fault (the batch review's C2)", () => {
  it.each([
    ["with no fault", [], "TERRAIN: STREAMING"],
    ["beside a graphics fault alone", ["GRAPHICS DEVICE LOST"], "TERRAIN: STREAMING"],
    ["while the terrain's fault stands", ["GRAPHICS DEVICE LOST", TERRAIN_NOT_AVAILABLE], null],
    ["while the computation's fault stands", [TERRAIN_COMPUTATION_FAILED], null],
  ] as const)("gives the annunciation %s", (_name, faults, statement) => {
    expect(terrainStatementOf("TERRAIN: STREAMING", faults)).toBe(statement);
  });
});
