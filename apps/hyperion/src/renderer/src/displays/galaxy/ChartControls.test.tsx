import type { MassLayer } from "@hyperion/protocol";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { aCensus } from "../../test/galaxyFixtures";
import { ChartControls } from "./ChartControls";
import { layerBands, type StarFilter } from "./chartModel";

const BANDS = layerBands(aCensus().layers);

function renderControls(overrides: Partial<Parameters<typeof ChartControls>[0]> = {}) {
  const onQueryRadius = vi.fn<(radiusLy: number) => void>();
  const onMinLayer = vi.fn<(layer: MassLayer) => void>();
  const onDriveRange = vi.fn<(rangeLy: number) => void>();
  const onTime = vi.fn<(timeYr: number) => void>();
  const onStarFilter = vi.fn<(filter: StarFilter) => void>();
  render(
    <ChartControls
      radiusChoiceLy={null}
      queryRadiusLy={50}
      onQueryRadius={onQueryRadius}
      minLayer="a"
      onMinLayer={onMinLayer}
      driveRangeLy={50}
      onDriveRange={onDriveRange}
      timeYr={0}
      onTime={onTime}
      bands={BANDS}
      starFilter="all"
      onStarFilter={onStarFilter}
      heldBack={null}
      {...overrides}
    />,
  );
  return { onQueryRadius, onMinLayer, onDriveRange, onTime, onStarFilter };
}

function radius(): HTMLSelectElement {
  const select = screen.getByLabelText("QUERY RADIUS");
  if (!(select instanceof HTMLSelectElement)) {
    throw new Error("QUERY RADIUS is not a select");
  }
  return select;
}

describe("ChartControls", () => {
  it("offers every query radius from 0.01 ly to 500 ly", () => {
    renderControls();

    const options = within(radius())
      .getAllByRole("option")
      .map((option) => option.textContent);

    expect(options).toHaveLength(15);
    expect(options[0]).toBe("0.01 ly");
    expect(options.at(-1)).toBe("500 ly");
  });

  it("reports the radius the operator chooses", async () => {
    const user = userEvent.setup();
    const { onQueryRadius } = renderControls();

    await user.selectOptions(radius(), "20");

    expect(onQueryRadius).toHaveBeenCalledWith(20);
  });

  it("follows the drive range while the operator has not chosen a radius", () => {
    renderControls({ radiusChoiceLy: null, driveRangeLy: 80, queryRadiusLy: 100 });

    expect(radius()).toHaveValue("100");
  });

  it("keeps a radius the operator chose when the drive range changes", () => {
    renderControls({ radiusChoiceLy: 20, driveRangeLy: 80, queryRadiusLy: 20 });

    expect(radius()).toHaveValue("20");
  });

  it("starts with the radius the default drive range asks for", () => {
    renderControls();

    expect(radius()).toHaveValue("50");
  });

  it("names the mass floors by the census's bands, the lightest also ALL", () => {
    renderControls();

    const group = screen.getByRole("group", { name: "MIN MASS solar masses" });
    expect(
      within(group)
        .getAllByRole("radio")
        .map((option) => option.closest("label")?.textContent),
    ).toEqual(["ALL 0.08", "0.5", "0.75", "2.5", "8"]);
  });

  it("names the mass floors by their layers until the first census arrives", () => {
    renderControls({ bands: null });

    expect(screen.getByRole("radio", { name: "ALL A" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "E" })).toBeInTheDocument();
  });

  it("reports layer C when the floor 0.75 is chosen", async () => {
    const user = userEvent.setup();
    const { onMinLayer } = renderControls();

    await user.click(screen.getByRole("radio", { name: "0.75" }));

    expect(onMinLayer).toHaveBeenCalledWith("c");
  });

  it("enters a drive range when the field is left", async () => {
    const user = userEvent.setup();
    const { onDriveRange } = renderControls();

    await user.clear(screen.getByLabelText("DRIVE RANGE"));
    await user.type(screen.getByLabelText("DRIVE RANGE"), "80");
    await user.tab();

    expect(onDriveRange).toHaveBeenCalledWith(80);
  });

  it("refuses a drive range outside the steps it offers and reports nothing", async () => {
    const user = userEvent.setup();
    const { onDriveRange } = renderControls();

    await user.clear(screen.getByLabelText("DRIVE RANGE"));
    await user.type(screen.getByLabelText("DRIVE RANGE"), "900{Enter}");

    expect(screen.getByText("DRIVE RANGE INVALID: enter 0.01 to 500 ly")).toBeInTheDocument();
    expect(onDriveRange).not.toHaveBeenCalled();
  });

  it("enters a chart time with Enter", async () => {
    const user = userEvent.setup();
    const { onTime } = renderControls();

    await user.clear(screen.getByLabelText("CHART TIME"));
    await user.type(screen.getByLabelText("CHART TIME"), "12.5{Enter}");

    expect(onTime).toHaveBeenCalledWith(12.5);
  });

  it("refuses a chart time outside the clock window and reports nothing", async () => {
    const user = userEvent.setup();
    const { onTime } = renderControls();

    await user.clear(screen.getByLabelText("CHART TIME"));
    await user.type(screen.getByLabelText("CHART TIME"), "1001{Enter}");

    expect(screen.getByText("CHART TIME INVALID: enter -1000 to 1000 yr")).toBeInTheDocument();
    expect(screen.getByLabelText("CHART TIME")).toHaveAttribute("aria-invalid", "true");
    expect(onTime).not.toHaveBeenCalled();
  });

  it("shows the chart time in universe time", () => {
    renderControls({ timeYr: 12.5 });

    expect(screen.getByLabelText("CHART TIME")).toHaveValue("+12.50");
    expect(screen.getByText("UT")).toBeInTheDocument();
  });

  it("holds every control back with the link's reason while the link is down", async () => {
    const user = userEvent.setup();
    const { onMinLayer, onQueryRadius, onTime } = renderControls({ heldBack: "NO CARRIER" });

    await user.click(screen.getByRole("radio", { name: "0.75" }));
    await user.clear(screen.getByLabelText("CHART TIME"));

    expect(screen.getByLabelText("QUERY RADIUS")).toHaveAccessibleDescription("NO CARRIER");
    expect(screen.getByRole("radio", { name: "0.75" })).toHaveAttribute("aria-disabled", "true");
    expect(onMinLayer).not.toHaveBeenCalled();
    expect(onQueryRadius).not.toHaveBeenCalled();
    expect(onTime).not.toHaveBeenCalled();
  });

  it("offers the STARS filters with the key that steps through them", () => {
    renderControls({ starFilter: "living" });

    const group = screen.getByRole("group", { name: "K STARS" });
    expect(
      within(group)
        .getAllByRole("radio")
        .map((option) => option.closest("label")?.textContent),
    ).toEqual(["ALL", "LIVING", "REMNANTS"]);
    expect(within(group).getByRole("radio", { name: "LIVING" })).toBeChecked();
    expect(group).toHaveAttribute("aria-keyshortcuts", "K");
  });

  it("reports the STARS filter the operator chooses", async () => {
    const user = userEvent.setup();
    const { onStarFilter } = renderControls();

    await user.click(screen.getByRole("radio", { name: "REMNANTS" }));

    expect(onStarFilter).toHaveBeenCalledWith("remnants");
  });

  it("steps to the next STARS filter when K is pressed", async () => {
    const user = userEvent.setup();
    const { onStarFilter } = renderControls({ starFilter: "remnants" });

    await user.keyboard("k");

    expect(onStarFilter).toHaveBeenCalledWith("all");
  });

  it("leaves K to a text field that has the focus", async () => {
    const user = userEvent.setup();
    const { onStarFilter } = renderControls();

    await user.click(screen.getByLabelText("DRIVE RANGE"));
    await user.keyboard("k");

    expect(onStarFilter).not.toHaveBeenCalled();
  });

  it("keeps the STARS filter acting while the link is down, since it asks nothing", async () => {
    const user = userEvent.setup();
    const { onStarFilter } = renderControls({ heldBack: "NO CARRIER" });

    await user.click(screen.getByRole("radio", { name: "LIVING" }));

    expect(onStarFilter).toHaveBeenCalledWith("living");
    expect(screen.getByRole("radio", { name: "LIVING" })).not.toHaveAttribute("aria-disabled");
  });
});
