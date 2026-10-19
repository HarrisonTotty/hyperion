import { describe, expect, it } from "vitest";

import { viewId } from "../../view/camera/state";
import {
  graphicsAnnunciation,
  GraphicsStatusStore,
  initialGraphicsStatus,
} from "../../view/engine/status";
import { TERRAIN_NOT_AVAILABLE } from "../../view/terrain/annunciation";
import { faultsFor } from "./viewFaults";

const PRIMARY = viewId("view");
const INSTRUMENT = viewId("instrument 1");

/** The graphics status after a device loss, or with nothing wrong. */
function graphics(lost: boolean): GraphicsStatusStore {
  const store = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
  if (lost) {
    store.dispatch({ kind: "device-lost", reason: "unknown", message: "lost" });
  }
  return store;
}

describe("a view's faults (R10.T16.b; decision-r10-t16-contracts §7)", () => {
  it("sets the graphics fault first, then the terrain's, on a view that hands a body over", () => {
    const status = graphics(true).getSnapshot();
    expect(
      faultsFor(status, graphicsAnnunciation(status), "view", PRIMARY, new Set([PRIMARY])),
    ).toEqual(["GRAPHICS DEVICE LOST: re-creating", TERRAIN_NOT_AVAILABLE]);
  });

  it("sets the terrain's alone with nothing wrong with the graphics", () => {
    const status = graphics(false).getSnapshot();
    expect(
      faultsFor(status, graphicsAnnunciation(status), "view", PRIMARY, new Set([PRIMARY])),
    ).toEqual([TERRAIN_NOT_AVAILABLE]);
  });

  it("sets neither on a view that hands nothing over", () => {
    const status = graphics(false).getSnapshot();
    expect(
      faultsFor(
        status,
        graphicsAnnunciation(status),
        "instrument 1",
        INSTRUMENT,
        new Set([PRIMARY]),
      ),
    ).toEqual([]);
  });
});
