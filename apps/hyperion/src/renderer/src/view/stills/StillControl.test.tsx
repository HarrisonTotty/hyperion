import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { describedText } from "../../test/statusLine";
import {
  STILL_CONTROL_IDLE,
  StillControl,
  type StillControlState,
  type StillStep,
} from "./StillControl";

/** A still running at `step`. */
function running(step: StillStep): StillControlState {
  return { ...STILL_CONTROL_IDLE, activity: { kind: "running", step } };
}

/** The control in `state`, and its `TAKE STILL`'s and `SIZE`'s calls. */
function renderControl(state: StillControlState) {
  const onTake = vi.fn<() => void>();
  const onSize = vi.fn<(scale: number) => void>();
  const user = userEvent.setup();
  const view = render(<StillControl state={state} onTake={onTake} onSize={onSize} />);
  return { onTake, onSize, user, view };
}

describe("the view's still control (R11.T10.c)", () => {
  it("takes a still from the keyboard: Tab to TAKE STILL, then Enter or Space", async () => {
    const { onTake, user } = renderControl(STILL_CONTROL_IDLE);
    await user.tab();
    expect(screen.getByRole("button", { name: "TAKE STILL" })).toHaveFocus();
    await user.keyboard("{Enter}");
    await user.keyboard(" ");
    expect(onTake).toHaveBeenCalledTimes(2);
  });

  it("holds TAKE STILL back while a still runs, focusable and saying why", async () => {
    const { onTake, user } = renderControl(
      running({ kind: "rendering", donePasses: 3, totalPasses: 12 }),
    );
    await user.tab();
    const take = screen.getByRole("button", { name: "TAKE STILL" });
    await user.keyboard("{Enter}");
    expect([take.getAttribute("aria-disabled"), onTake.mock.calls.length]).toEqual(["true", 0]);
    expect(take).toHaveFocus();
    expect(describedText(take)).toBe("NOT AVAILABLE: a still is in progress");
  });

  it("shows the still's progress as a progress element of its passes, with their count", () => {
    renderControl(running({ kind: "rendering", donePasses: 3, totalPasses: 12 }));
    const bar = screen.getByRole("progressbar", { name: "PROGRESS" });
    expect([
      bar.getAttribute("value"),
      bar.getAttribute("max"),
      screen.getByText(/^PASSES/u).textContent,
      screen.getByRole("status").textContent,
    ]).toEqual(["3", "12", "PASSES 3 OF 12", "RENDERING"]);
  });

  it("keeps the passes done in a field as wide as the total, so that the bar holds still", () => {
    renderControl(running({ kind: "rendering", donePasses: 7, totalPasses: 12_480 }));
    const done = screen.getByText("7");
    expect([done.style.minWidth, screen.getByText(/^PASSES/u).textContent]).toEqual([
      "6ch",
      "PASSES 7 OF 12,480",
    ]);
  });

  it("says TERRAIN: STREAMING while the still waits for its frustum's terrain", () => {
    renderControl(running({ kind: "streaming", donePasses: 0, totalPasses: 12 }));
    expect(screen.getByRole("status").textContent).toBe("TERRAIN: STREAMING");
  });

  it("has no progress element while no still runs", () => {
    renderControl({ ...STILL_CONTROL_IDLE, activity: { kind: "idle", last: { kind: "saved" } } });
    expect([screen.queryByRole("progressbar"), screen.getByRole("status").textContent]).toEqual([
      null,
      "STILL SAVED",
    ]);
  });

  it("says why the last still was not saved, and what to do once a loss abandoned one", () => {
    const { view } = renderControl({
      ...STILL_CONTROL_IDLE,
      activity: { kind: "idle", last: { kind: "not-saved", reason: "the disk is full" } },
    });
    const first = screen.getByRole("status").textContent;
    view.rerender(
      <StillControl
        state={{
          standing: "suspended",
          activity: { kind: "idle", last: { kind: "abandoned" } },
          heldBack: null,
          size: null,
        }}
        onTake={() => undefined}
        onSize={() => undefined}
      />,
    );
    const statuses = screen.getAllByRole("status").map((status) => status.textContent);
    expect([first, statuses]).toEqual([
      "STILL NOT SAVED: the disk is full",
      ["STILLS LIMITED", "STILL NOT SAVED: the graphics device was lost, TAKE STILL to retry"],
    ]);
  });

  it("says STILLS LIMITED and still offers TAKE STILL, the explicit request", async () => {
    const { onTake, user } = renderControl({ ...STILL_CONTROL_IDLE, standing: "suspended" });
    await user.click(screen.getByRole("button", { name: "TAKE STILL" }));
    expect([screen.getByText("STILLS LIMITED"), onTake.mock.calls.length]).toEqual([
      expect.anything(),
      1,
    ]);
  });

  it("says STILLS DISABLED and holds TAKE STILL back once a second loss disabled them", async () => {
    const { onTake, user } = renderControl({
      standing: "disabled",
      activity: { kind: "idle", last: { kind: "abandoned" } },
      heldBack: null,
      size: null,
    });
    const take = screen.getByRole("button", { name: "TAKE STILL" });
    await user.click(take);
    expect([
      screen.getAllByRole("status").map((status) => status.textContent),
      onTake.mock.calls.length,
      describedText(take),
    ]).toEqual([
      ["STILLS DISABLED", "STILL NOT SAVED: the graphics device was lost, relaunch to retry"],
      0,
      "NOT AVAILABLE: stills disabled by a second device loss, relaunch to retry",
    ]);
  });

  it("holds TAKE STILL back with the reason it is given", () => {
    renderControl({ ...STILL_CONTROL_IDLE, heldBack: "NOT AVAILABLE: no WebGPU" });
    expect(describedText(screen.getByRole("button", { name: "TAKE STILL" }))).toBe(
      "NOT AVAILABLE: no WebGPU",
    );
  });
});

describe("the still control's SIZE (R11.T10.e.1)", () => {
  const sized: StillControlState = {
    ...STILL_CONTROL_IDLE,
    size: {
      scale: 2,
      options: [
        { scale: 1, size: { widthPx: 1920, heightPx: 1080 } },
        { scale: 2, size: { widthPx: 3840, heightPx: 2160 } },
        { scale: 4, size: { widthPx: 7680, heightPx: 4320 } },
      ],
    },
  };

  it("offers each size in its pixels, the chosen one pressed, in a group named SIZE", () => {
    renderControl(sized);
    const group = screen.getByRole("group", { name: "SIZE" });
    const pressed = screen
      .getAllByRole("button", { pressed: true })
      .map((button) => button.textContent);
    expect([group, pressed]).toEqual([expect.anything(), ["3840 × 2160 px"]]);
  });

  it("chooses the next still's size from the keyboard", async () => {
    const { onSize, user } = renderControl(sized);
    screen.getByRole("button", { name: "7680 × 4320 px" }).focus();
    await user.keyboard("{Enter}");
    expect(onSize.mock.calls).toEqual([[4]]);
  });

  it("offers no size while the view's output size is not known", () => {
    renderControl(STILL_CONTROL_IDLE);
    expect(screen.queryByRole("group", { name: "SIZE" })).toBeNull();
  });
});
