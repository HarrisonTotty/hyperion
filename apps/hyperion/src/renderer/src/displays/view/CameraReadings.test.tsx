import { render, renderHook, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { IDENTITY_QUATERNION } from "../../view/camera/quaternion";
import { precisionScene } from "../../view/scenes/precision";
import { type CameraPlace, useCameraPlace } from "./cameraPlace";
import { CameraReadings } from "./CameraReadings";
import { startRun, type ViewRun } from "./viewRun";

const HELD: CameraPlace = {
  position: "2.39 AU 216° +02°",
  pointing: "— -90°",
  unit: "AU",
  heldToCraft: true,
};

describe("the camera panel's readings (R07.T19.f)", () => {
  it("states the rate, where the camera looks and where it is", () => {
    render(<CameraReadings rateStep={6} place={HELD} sceneStale={false} />);
    expect([
      screen.getByRole("status", { name: "Free camera rate" }).textContent,
      screen.getByRole("status", { name: "Camera pointing" }).textContent,
      screen.getByRole("status", { name: "Camera position" }).textContent,
    ]).toEqual(["RATE 1.00 km/s", "— -90°", "2.39 AU 216° +02°"]);
  });

  it("mutes a place held to the own ship with its S while the server's scene is stale", () => {
    render(<CameraReadings rateStep={6} place={HELD} sceneStale />);
    expect([
      screen.getByRole("status", { name: "Camera position" }).classList.contains("stale"),
      screen.getByRole("status", { name: "Camera pointing" }).classList.contains("stale"),
      screen.getAllByText("stale").length,
    ]).toEqual([true, true, 2]);
  });

  it("keeps a free camera's place live while the server's scene is stale", () => {
    render(<CameraReadings rateStep={6} place={{ ...HELD, heldToCraft: false }} sceneStale />);
    expect(screen.getByRole("status", { name: "Camera position" })).not.toHaveClass("stale");
  });

  it("sets a missing azimuth's em dash in the muted missing state", () => {
    render(<CameraReadings rateStep={6} place={HELD} sceneStale={false} />);
    expect(screen.getByText("—")).toHaveClass("readout__missing");
  });
});

/** The precision scene with its camera free `rangeM` coreward of the barycentre. */
function freeAtRange(rangeM: number): ViewRun {
  const run = startRun(precisionScene());
  return {
    ...run,
    camera: {
      ...run.camera,
      preset: "free",
      pose: {
        frame: { kind: "system", system: run.scene.system },
        positionM: vec3(0, -rangeM, 0),
        orientation: IDENTITY_QUATERNION,
      },
    },
  };
}

describe("a camera's place as its readouts show it (R07.T19.f)", () => {
  it("keeps the range's unit from one published run to the next, within its hysteresis", () => {
    const { result, rerender } = renderHook((run: ViewRun) => useCameraPlace(run), {
      initialProps: freeAtRange(0.98e9),
    });
    const first = result.current.position;
    rerender(freeAtRange(1.02e9));
    expect([first, result.current.position]).toEqual(["980 Mm 000° +00°", "1020 Mm 000° +00°"]);
  });
});
