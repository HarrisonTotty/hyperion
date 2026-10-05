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

describe("the view's label block's line breaks (R07.T19.b)", () => {
  const LINES = [
    { label: "TIME", value: "UT +0 yr 000/00:00:01" },
    { label: "CAMERA", value: "FREE · RATE 1.00 km/s" },
    { label: "FOV", value: "60°" },
    { label: "EXPOSURE", value: "EV100 -1.0 INHIBITED · OPERATOR" },
    { label: "STARS", value: "V 9.5 mag CAM · CLUSTERS AND WHITE DWARFS: NOT YET MODELLED" },
  ];

  function block(): HTMLElement {
    const { container } = render(
      <ViewLabelBlock lines={LINES} statements={[]} countLine={null} fault={null} />,
    );
    return container;
  }

  it("keeps each reading's text as it reads", () => {
    block();
    expect(screen.getAllByRole("status").map((output) => output.textContent)).toEqual(
      LINES.map((line) => line.value),
    );
  });

  it("sets each part between middle dots on a line of its own where it fits", () => {
    const parts = [...block().querySelectorAll(".view-label__part")].map(
      (part) => part.textContent,
    );
    expect(parts).toEqual([
      "UT +0 yr 000/00:00:01",
      "FREE",
      "RATE 1.00 km/s",
      "60°",
      "EV100 -1.0 INHIBITED",
      "OPERATOR",
      "V 9.5 mag CAM",
      "CLUSTERS AND WHITE DWARFS: NOT YET MODELLED",
    ]);
  });

  it("never breaks inside a quantity, a star limit and its kind, after UT or in a clock", () => {
    const runs = [...block().querySelectorAll(".view-label__run")].map((run) => run.textContent);
    expect(runs).toEqual(["UT +0 yr", "000/00:00:01", "1.00 km/s", "EV100 -1.0", "V 9.5 mag CAM"]);
  });
});
