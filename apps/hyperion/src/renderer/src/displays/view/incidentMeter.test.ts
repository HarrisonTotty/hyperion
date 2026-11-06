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
    const sky = { request: { exclude_system: RUN.scene.system }, response: { hosts } };
    expect([
      incidentReadingLx(served, null, null, null),
      (incidentReadingLx(heldTo(PHASE_PLANETS[0]), null, null, null) ?? 0) > 0,
      incidentReadingLx({ ...served, camera: heldTo(PHASE_PLANETS[0]).camera }, null, sky, null) ===
        incidentReadingLx(heldTo(PHASE_PLANETS[0]), null, null, null),
    ]).toEqual([null, true, true]);
  });

  it("reads the light of the system's discs on their own before its sky is held (plans R06, R07)", () => {
    const served: ViewRun = {
      ...RUN,
      scene: { ...RUN.scene, provenance: { kind: "server" } },
      camera: heldTo(PHASE_PLANETS[0]).camera,
    };
    const asked = {
      universe: "000000000000002a",
      system: RUN.scene.system,
      discs: RUN.scene.hostDiscs ?? [],
    };
    const otherSystems = { ...asked, system: "0000000000000001" };
    expect([
      incidentReadingLx(served, null, null, asked),
      incidentReadingLx(served, null, null, otherSystems),
    ]).toEqual([incidentReadingLx(heldTo(PHASE_PLANETS[0]), null, null, null), null]);
  });

  it("has nowhere to read with neither, which reads no light", () => {
    expect([
      incidentBodyOf(RUN, null),
      incidentBodyOf(RUN, { kind: "body", body: PHASE_STAR }),
      incidentReadingLx(RUN, null, null, null),
    ]).toEqual([null, null, 0]);
  });
});
