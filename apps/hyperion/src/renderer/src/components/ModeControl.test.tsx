import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { describedText } from "../test/statusLine";
import { type DisplayMode, MODE_KEY, ModeControl, type ModeControlProps } from "./ModeControl";

/** The reason a chart gives for holding `OBSERVED` back with nothing on show to observe from. */
const NO_CHART = "NO CHART: centre one to observe from it";

function renderControl(overrides: Partial<ModeControlProps> = {}) {
  const onMode = vi.fn<(mode: DisplayMode) => void>();
  const props: ModeControlProps = {
    mode: "now",
    onMode,
    heldBackId: null,
    observedHeldBack: null,
    ...overrides,
  };
  render(
    <>
      <p id="link-reason">NO CARRIER</p>
      <ModeControl {...props} />
      <label>
        NAME <input type="text" />
      </label>
    </>,
  );
  return { onMode };
}

describe("ModeControl (plan 12, P12.T11)", () => {
  it("offers NOW and OBSERVED under its key, shown on the control and named for assistive technology", () => {
    renderControl();

    const group = screen.getByRole("group", { name: "M MODE" });
    expect(group).toHaveAttribute("aria-keyshortcuts", "M");
    expect(MODE_KEY).toBe("M");
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual([
      "NOW",
      "OBSERVED",
    ]);
  });

  it("says which mode is chosen by its pressed state and a rule under it, not by colour alone", () => {
    renderControl({ mode: "observed" });

    expect(screen.getByRole("button", { name: "OBSERVED", pressed: true })).toHaveClass(
      "mode-control__button",
    );
    expect(screen.getByRole("button", { name: "NOW", pressed: false })).toBeInTheDocument();
  });

  it("steps to the other mode when M is pressed", async () => {
    const user = userEvent.setup();
    const { onMode } = renderControl();

    await user.keyboard("m");

    expect(onMode).toHaveBeenCalledWith("observed");
  });

  it("steps back to NOW on M from OBSERVED", async () => {
    const user = userEvent.setup();
    const { onMode } = renderControl({ mode: "observed" });

    await user.keyboard("M");

    expect(onMode).toHaveBeenCalledWith("now");
  });

  it("leaves M to a text field that has the focus, and to a modified or held key", async () => {
    const user = userEvent.setup();
    const { onMode } = renderControl();

    await user.click(screen.getByRole("textbox", { name: "NAME" }));
    await user.keyboard("m");
    await user.keyboard("{Shift>}m{/Shift}");
    await user.keyboard("{Control>}m{/Control}");

    expect(onMode).not.toHaveBeenCalled();
  });

  it("is reached with Tab and chosen with Enter or Space", async () => {
    const user = userEvent.setup();
    const { onMode } = renderControl();

    await user.tab();
    expect(screen.getByRole("button", { name: "NOW" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "OBSERVED" })).toHaveFocus();
    await user.keyboard("{Enter}");
    await user.keyboard(" ");

    expect(onMode.mock.calls).toEqual([["observed"], ["observed"]]);
  });

  it("holds both back with the link's reason, keeping their focus, and M does nothing", async () => {
    const user = userEvent.setup();
    const { onMode } = renderControl({ heldBackId: "link-reason", observedHeldBack: "NO CARRIER" });

    for (const name of ["NOW", "OBSERVED"]) {
      const button = screen.getByRole("button", { name });
      expect(button).toHaveAttribute("aria-disabled", "true");
      expect(describedText(button)).toBe("NO CARRIER");
    }
    await user.click(screen.getByRole("button", { name: "OBSERVED" }));
    await user.keyboard("m");

    expect(onMode).not.toHaveBeenCalled();
    // The link's reason is said once, by the controls the mode stands among.
    expect(screen.getAllByText("NO CARRIER")).toHaveLength(1);
  });

  it("holds OBSERVED alone back with its own reason beside it", async () => {
    const user = userEvent.setup();
    const { onMode } = renderControl({ observedHeldBack: NO_CHART });

    const observed = screen.getByRole("button", { name: "OBSERVED" });
    expect(observed).toHaveAttribute("aria-disabled", "true");
    expect(describedText(observed)).toBe(NO_CHART);
    expect(screen.getByRole("button", { name: "NOW" })).not.toHaveAttribute("aria-disabled");
    await user.click(observed);
    await user.keyboard("m");

    expect(onMode).not.toHaveBeenCalled();
  });

  it("goes on choosing NOW while only OBSERVED is held back", async () => {
    const user = userEvent.setup();
    const { onMode } = renderControl({ mode: "observed", observedHeldBack: NO_CHART });

    await user.keyboard("m");

    expect(onMode).toHaveBeenCalledWith("now");
  });
});
