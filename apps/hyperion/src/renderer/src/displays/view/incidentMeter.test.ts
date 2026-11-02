import { describe, expect, it } from "vitest";

import { PHASE_GIANT, PHASE_PLANETS, PHASE_STAR, phaseScene } from "../../view/scenes/phaseScene";
import { incidentBodyOf, incidentReadingLx } from "./incidentMeter";
import { startRun, type ViewRun } from "./viewRun";

/** The phase scene's run, its camera in the system frame, held to no body. */
const RUN = startRun(phaseScene());

/** `RUN` with its camera held to `body`'s frame, where it stands. */
function heldTo(body: string): ViewRun {
  return {
    ...RUN,
    camera: { ...RUN.camera, pose: { ...RUN.camera.pose, frame: { kind: "body", body } } },
  };
}

describe("where the INCIDENT meter reads the light (ux-calls-view, item 3b)", () => {
  it("reads at the FRAME body over the selected one", () => {
    expect(incidentBodyOf(heldTo(PHASE_PLANETS[0]), { kind: "body", body: PHASE_GIANT })).toBe(
      PHASE_PLANETS[0],
    );
  });

  it("reads at the selected body with no FRAME body, or where the FRAME body is not lit", () => {
    const selected = { kind: "body", body: PHASE_GIANT } as const;
    expect([incidentBodyOf(RUN, selected), incidentBodyOf(heldTo(PHASE_STAR), selected)]).toEqual([
      PHASE_GIANT,
      PHASE_GIANT,
    ]);
  });

  it("reads nothing yet in a scene lit by its sky's discs before its sky is held, and the light once it is", () => {
    const served: ViewRun = { ...RUN, scene: { ...RUN.scene, provenance: { kind: "server" } } };
    const hosts = RUN.scene.hostDiscs ?? [];
    expect([
      incidentReadingLx(served, null, null),
      (incidentReadingLx(heldTo(PHASE_PLANETS[0]), null, null) ?? 0) > 0,
      incidentReadingLx({ ...served, camera: heldTo(PHASE_PLANETS[0]).camera }, null, hosts) ===
        incidentReadingLx(heldTo(PHASE_PLANETS[0]), null, null),
    ]).toEqual([null, true, true]);
  });

  it("has nowhere to read with neither, which reads no light", () => {
    expect([
      incidentBodyOf(RUN, null),
      incidentBodyOf(RUN, { kind: "body", body: PHASE_STAR }),
      incidentReadingLx(RUN, null, null),
    ]).toEqual([null, null, 0]);
  });
});
