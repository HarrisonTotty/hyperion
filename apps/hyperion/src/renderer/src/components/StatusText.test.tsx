import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { stylesheetRule } from "../test/stylesheet";
import { StatusText } from "./StatusText";

/** The line rendered in an `output` of its own, as `StatusLine` sets it. */
function renderLine(text: string, renderPart?: (part: string) => string) {
  render(
    <output>
      <StatusText text={text} renderPart={renderPart} />
    </output>,
  );
  return screen.getByRole("status");
}

describe("StatusText", () => {
  it("sets a state and the sentence after its colon in two spans, tracked and untracked", () => {
    renderLine("NOT AVAILABLE: no room for DUST OVERLAY at this window size");

    expect(screen.getByText("NOT AVAILABLE:")).toHaveClass("status-text__state");
    expect(screen.getByText("no room for DUST OVERLAY at this window size")).toHaveClass(
      "status-text__sentence",
    );
  });

  it("puts the space after the colon in the sentence's span", () => {
    renderLine("EXTINCTION REJECTED: no such system");

    expect(screen.getByText("EXTINCTION REJECTED:").textContent).toBe("EXTINCTION REJECTED:");
    expect(screen.getByText("no such system").textContent).toBe(" no such system");
  });

  it.each([
    "SCENE STALE: NO CARRIER",
    "AUTO NOT AVAILABLE: NO IMAGE TO METER",
    "SCENE NOT AVAILABLE: NO CARRIER",
  ])("spaces a line upper case after its colon whole, as one state: %s", (text) => {
    const line = renderLine(text);

    expect(screen.getByText(text)).toHaveClass("status-text__state");
    expect(line.querySelector(".status-text__sentence")).toBeNull();
  });

  it.each(["PENDING", "NO CARRIER", "NOT YET FORMED"])(
    "spaces a line with no colon whole, as one state: %s",
    (text) => {
      const line = renderLine(text);

      expect(screen.getByText(text)).toHaveClass("status-text__state");
      expect(line.querySelector(".status-text__sentence")).toBeNull();
    },
  );

  it("splits a reason holding its own colon at the first", () => {
    renderLine("REJECTED: storage: the save could not be written");

    expect(screen.getByText("REJECTED:")).toHaveClass("status-text__state");
    expect(screen.getByText("storage: the save could not be written")).toHaveClass(
      "status-text__sentence",
    );
  });

  it("keeps the line's text as it was, so that a live region reads the same words", () => {
    const text = "DIRECTIONS UNDEFINED: grid aligned to -X";

    expect(renderLine(text).textContent).toBe(text);
  });

  it("sets each part's words through renderPart, the space included", () => {
    const line = renderLine("NOT AVAILABLE: the exposure is INHIBITED · OPERATOR", (part) =>
      part.toLowerCase(),
    );

    expect(screen.getByText("not available:")).toHaveClass("status-text__state");
    expect(screen.getByText("the exposure is inhibited · operator").textContent).toBe(
      " the exposure is inhibited · operator",
    );
    expect(line.childElementCount).toBe(2);
  });

  it("tracks the state at the status lines' 0.15em and the sentence not at all, and nothing more", () => {
    expect([
      stylesheetRule(".status-text__state"),
      stylesheetRule(".status-text__sentence"),
    ]).toEqual(["\n  letter-spacing: 0.15em;\n", "\n  letter-spacing: 0;\n"]);
  });
});
