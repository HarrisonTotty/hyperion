import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ViewLabelBlock } from "./ViewLabelBlock";

describe("the view's label block", () => {
  it("shows a terrain annunciation as a steady statement, not an alert or a fault", () => {
    render(
      <ViewLabelBlock
        lines={[{ label: "STYLE", value: "PHOTOREALISTIC" }]}
        statements={["TERRAIN: DETAIL LIMITED"]}
        countLine={null}
        fault={null}
      />,
    );
    const line = screen.getByText("TERRAIN: DETAIL LIMITED");
    // The statement class carries no status colour: the plate's `--text`.
    expect(line.className).toBe("view-label__statement");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryAllByRole("status").some((s) => s.contains(line))).toBe(false);
  });
});
