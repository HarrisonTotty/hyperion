import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { statusLine } from "../../test/statusLine";
import { stylesheetRule } from "../../test/stylesheet";
import { LabelStatementLine } from "./LabelStatementLine";

/** `ATMOSPHERE: APPROXIMATE`'s two reasons of the ruling's examples (plan R08, R08.T10.d). */
const REASONS = [
  "A thick atmosphere drawn by the thin-atmosphere method, for KX 4417 /2",
  "e− scattering: left out, the console can draw no more layers, for TEST SATURN",
];

describe("a label block's statement line", () => {
  it.each(REASONS)("sets a reason whole as a sentence, no part tracked: %s", (text) => {
    render(<LabelStatementLine statement={{ text, standing: "sentence" }} />);
    const line = screen.getByText(text);
    expect([
      line.tagName,
      line.className,
      line.textContent,
      line.querySelector(".status-text__state"),
    ]).toEqual(["P", "view-label__statement view-label__statement--sentence", text, null]);
  });

  it("sets a sentence with no letter spacing", () => {
    expect(stylesheetRule(".view-label__statement--sentence")).toContain("letter-spacing: 0;");
  });

  it("gives a statement that is not a sentence an annunciation's letter spacing", () => {
    expect(stylesheetRule(".view-label__statement")).toContain("letter-spacing: 0.15em;");
  });

  it("tracks ATMOSPHERE: APPROXIMATE whole as a state", () => {
    render(<LabelStatementLine statement="ATMOSPHERE: APPROXIMATE" />);
    const line = screen.getByText(statusLine("ATMOSPHERE: APPROXIMATE"));
    expect([line.className, [...line.children].map((part) => part.className)]).toEqual([
      "view-label__statement",
      ["status-text__state"],
    ]);
  });
});
