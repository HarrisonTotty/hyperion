import type { MassLayer } from "@hyperion/protocol";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { DisplayMode } from "../../components/ModeControl";
import { aCensus } from "../../test/galaxyFixtures";
import { describedText } from "../../test/statusLine";
import { type ChartModeControls, ChartControls, MOVE_OBSERVER_LABEL } from "./ChartControls";
import { layerBands, type StarFilter } from "./chartModel";
import { NO_CHART_TO_OBSERVE } from "./useLocalChart";

const BANDS = layerBands(aCensus().layers);

interface RenderOptions {
  readonly heldBack?: string | null;
  readonly hidden?: boolean;
}

function renderControls(
  modes: Partial<ChartModeControls> = {},
  { heldBack = null, hidden = false }: RenderOptions = {},
) {
  const onMode = vi.fn<(mode: DisplayMode) => void>();
  const onMoveObserver = vi.fn<() => void>();
  render(
    <ChartControls
      radiusChoiceLy={null}
      queryRadiusLy={50}
      onQueryRadius={vi.fn<(radiusLy: number) => void>()}
      minLayer="a"
      onMinLayer={vi.fn<(layer: MassLayer) => void>()}
      driveRangeLy={50}
      onDriveRange={vi.fn<(rangeLy: number) => void>()}
      timeYr={0}
      onTime={vi.fn<(timeYr: number) => void>()}
      bands={BANDS}
      starFilter="all"
      onStarFilter={vi.fn<(filter: StarFilter) => void>()}
      heldBack={heldBack}
      hidden={hidden}
      modes={{
        mode: "now",
        onMode,
        observedHeldBack: null,
        moveHeldBack: null,
        onMoveObserver,
        ...modes,
      }}
    />,
  );
  return { onMode, onMoveObserver };
}

describe("ChartControls' mode (plan 12, P12.T11)", () => {
  it("stands the mode control among the chart's controls", () => {
    renderControls();

    expect(screen.getByRole("group", { name: "M MODE" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "NOW", pressed: true })).toBeInTheDocument();
  });

  it("offers the observer's move in NOW too, where it sets the next observer", async () => {
    const user = userEvent.setup();
    const { onMoveObserver } = renderControls();

    await user.click(screen.getByRole("button", { name: MOVE_OBSERVER_LABEL }));

    expect(onMoveObserver).toHaveBeenCalledOnce();
  });

  it("moves the observer to the chart's centre on OBSERVER TO CENTRE", async () => {
    const user = userEvent.setup();
    const { onMoveObserver } = renderControls({ mode: "observed" });

    await user.click(screen.getByRole("button", { name: "OBSERVER TO CENTRE" }));

    expect(onMoveObserver).toHaveBeenCalledOnce();
  });

  it("holds the mode and the move back with the link's reason, said once", async () => {
    const user = userEvent.setup();
    const { onMode, onMoveObserver } = renderControls(
      { mode: "observed", observedHeldBack: "NO CARRIER", moveHeldBack: "NO CARRIER" },
      { heldBack: "NO CARRIER" },
    );

    for (const name of ["NOW", "OBSERVED", MOVE_OBSERVER_LABEL]) {
      const control = screen.getByRole("button", { name });
      expect(control).toHaveAttribute("aria-disabled", "true");
      expect(describedText(control)).toBe("NO CARRIER");
    }
    await user.click(screen.getByRole("button", { name: "NOW" }));
    await user.click(screen.getByRole("button", { name: MOVE_OBSERVER_LABEL }));

    expect(onMode).not.toHaveBeenCalled();
    expect(onMoveObserver).not.toHaveBeenCalled();
    expect(screen.getAllByText("NO CARRIER")).toHaveLength(1);
  });

  it("holds OBSERVED and the move back with no chart on show, saying why beside each", () => {
    renderControls({
      mode: "observed",
      observedHeldBack: NO_CHART_TO_OBSERVE,
      moveHeldBack: NO_CHART_TO_OBSERVE,
    });

    for (const name of ["OBSERVED", MOVE_OBSERVER_LABEL]) {
      const control = screen.getByRole("button", { name });
      expect(control).toHaveAttribute("aria-disabled", "true");
      expect(describedText(control)).toBe(NO_CHART_TO_OBSERVE);
    }
    expect(screen.getByRole("button", { name: "NOW" })).not.toHaveAttribute("aria-disabled");
  });

  it("keeps M live while the controls are hidden, as K is", async () => {
    const user = userEvent.setup();
    const { onMode } = renderControls({}, { hidden: true });

    await user.keyboard("m");

    expect(onMode).toHaveBeenCalledWith("observed");
  });
});
