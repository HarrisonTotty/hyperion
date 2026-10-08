import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { primaryModifierOf } from "../../lib/platform";
import { KeyLegend } from "./KeyLegend";

/** The legend for `platform`, with a canvas it describes, and a way to take both down. */
function renderFor(platform: string): {
  readonly canvas: HTMLElement;
  readonly unmount: () => void;
} {
  const { unmount } = render(
    <>
      {/* A stand-in for a view's canvas, which the legend describes. */}
      <button type="button" aria-describedby="keys">
        VIEW
      </button>
      <KeyLegend id="keys" modifier={primaryModifierOf(platform)} />
    </>,
  );
  return { canvas: screen.getByRole("button", { name: "VIEW" }), unmount };
}

describe("VIEW's key legend (R07.T19.f)", () => {
  it("names Ctrl with the arrows for the rate on Linux and Windows", () => {
    for (const platform of ["linux", "win32"]) {
      const { canvas, unmount } = renderFor(platform);
      expect(canvas).toHaveAccessibleDescription(
        "FOCUSED VIEW: DRAG/ARROWS TURN · FREE: W/S A/D R/F MOVE, Q/E ROLL, CTRL+↑/↓ RATE",
      );
      unmount();
    }
  });

  it("names Command with the arrows on macOS, its sign drawn and read as Command+", () => {
    const { canvas } = renderFor("darwin");
    expect(canvas).toHaveAccessibleDescription(
      "FOCUSED VIEW: DRAG/ARROWS TURN · FREE: W/S A/D R/F MOVE, Q/E ROLL, Command+↑/↓ RATE",
    );
  });
});
