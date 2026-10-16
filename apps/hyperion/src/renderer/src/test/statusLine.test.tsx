import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { StatusLine } from "../components/StatusLine";
import { describedText, statusLine } from "./statusLine";

/** A RETRY described by a refusal that `StatusText` splits into a state and a sentence. */
function renderRetry(): HTMLElement {
  render(
    <StatusLine
      text="REJECTED: busy"
      standing="refused"
      action={{ label: "RETRY", onAction: () => undefined }}
    />,
  );
  return screen.getByRole("button", { name: "RETRY" });
}

describe("statusLine", () => {
  it("finds a split line by its whole text, at the element that holds its parts", () => {
    renderRetry();

    expect(screen.getByText(statusLine("REJECTED: busy"))).toBe(screen.getByRole("status"));
  });
});

describe("describedText", () => {
  it("reads a split line's description with the space after its colon, as Chromium does", () => {
    expect(describedText(renderRetry())).toBe("REJECTED: busy");
  });

  // The helper's premise. Once jsdom's accessible description keeps the space, this fails, and
  // `toHaveAccessibleDescription` serves for split lines again.
  it("is needed while jsdom's accessible description runs a split line's parts together", () => {
    expect(renderRetry()).toHaveAccessibleDescription("REJECTED:busy");
  });

  it("refuses a describing element whose parts are named or hidden apart from their text", () => {
    render(
      <>
        <p id="reason">
          NOT AVAILABLE: <span aria-hidden="true">S</span>
        </p>
        <button type="button" aria-describedby="reason">
          OPEN
        </button>
      </>,
    );

    expect(() => describedText(screen.getByRole("button", { name: "OPEN" }))).toThrow(/#reason/u);
  });
});
