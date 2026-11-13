import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { FrameReading } from "./FrameReading";

describe("FrameReading (plan 12, P12.T11.d)", () => {
  it("reads FRAME and the frame's name as a term and its definition", () => {
    render(
      <dl>
        <FrameReading frameName="GALACTIC" />
      </dl>,
    );

    const term = screen.getByText("FRAME", { selector: "dt" });
    expect(term).toHaveClass("field__label");
    expect(term.nextElementSibling).toHaveTextContent(/^GALACTIC$/u);
    expect(term.parentElement).toHaveClass("field", "spatial-reading");
  });

  it("gives the frame's name, a word, no stale slot", () => {
    render(
      <dl>
        <FrameReading frameName="SYSTEM BARYCENTRIC" />
      </dl>,
    );

    const name = screen.getByText("FRAME", { selector: "dt" }).nextElementSibling;
    expect(name?.querySelector(".stale-slot")).toBeNull();
  });
});
