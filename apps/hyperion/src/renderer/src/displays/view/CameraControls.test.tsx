import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { PrimaryModifier } from "../../lib/platform";
import { CameraControls } from "./CameraControls";
import type { CameraPlace } from "./cameraPlace";

const PLACE: CameraPlace = {
  position: "2.39 AU 216° +02°",
  pointing: "075° -73°",
  unit: "AU",
  heldToCraft: true,
  followsHull: true,
};

/** The camera panel at `rateStep` of the 0–12 steps, for `modifier`'s platform. */
function renderAt(rateStep: number, modifier: PrimaryModifier): void {
  render(
    <CameraControls
      preset="free"
      offered={["seat", "chase", "free"]}
      fovDeg={60}
      rateStep={rateStep}
      maxRateStep={12}
      place={PLACE}
      sceneStale={false}
      modifier={modifier}
      easedMoves={false}
      reducedMotion={false}
      onAction={() => undefined}
      onEasedMovesChange={() => undefined}
    />,
  );
}

/** The rate's limit reason's text, the drawn sign's name read in its place. */
function rateReason(): string | null {
  return screen.getByText(/RATE at its/u).textContent;
}

describe("the camera panel's rate limit (decision-r07-t19f-position, item 5)", () => {
  it.each([
    [12, "ctrl", "NOT AVAILABLE: CTRL+↑, RATE at its highest step"],
    [0, "ctrl", "NOT AVAILABLE: CTRL+↓, RATE at its lowest step"],
    [12, "meta", "NOT AVAILABLE: Command+↑, RATE at its highest step"],
    [0, "meta", "NOT AVAILABLE: Command+↓, RATE at its lowest step"],
  ] as const)("at step %i on %s reads %s", (rateStep, modifier, text) => {
    renderAt(rateStep, modifier);
    expect(rateReason()).toBe(text);
  });

  it("draws the Command sign on macOS, never typing it", () => {
    renderAt(0, "meta");
    const reason = screen.getByText(/RATE at its/u);
    expect([
      reason.querySelector("svg.glyph--command") !== null,
      reason.textContent.includes("⌘"),
    ]).toEqual([true, false]);
  });

  it("names no PAGE key", () => {
    renderAt(12, "ctrl");
    expect(rateReason()).not.toContain("PAGE");
  });
});

describe("the camera panel's readings (decision-r07-t19f-position, item 2)", () => {
  it("stands POSITION, then RATE with POINTING beside it, then the rate's limit reason", () => {
    renderAt(0, "ctrl");
    const order = [
      screen.getByRole("status", { name: "Camera position" }),
      screen.getByRole("status", { name: "Free camera rate" }),
      screen.getByRole("status", { name: "Camera pointing" }),
      screen.getByText(/RATE at its/u),
    ];
    const following = order
      .slice(1)
      .map((each, index) =>
        Boolean(
          (order[index]?.compareDocumentPosition(each) ?? 0) & Node.DOCUMENT_POSITION_FOLLOWING,
        ),
      );
    expect(following).toEqual([true, true, true]);
  });
});
