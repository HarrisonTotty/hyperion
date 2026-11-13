import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { FakeResizeObserver } from "../../test/FakeResizeObserver";
import { testStillLabel } from "../../test/stillFixtures";
import { StillList } from "./StillList";

const NEWER = { id: "20261113T120000002Z-00000002", label: testStillLabel({}, false) };
const OLDER = {
  id: "20261113T120000001Z-00000001",
  label: testStillLabel({ setting: "low", decoration: false, survey: [], annunciations: [] }),
};

/** The terms and values of the label on show, by `screen`: a folded label is hidden. */
function shownLabel(): ReadonlyArray<readonly [string | null, string | null]> {
  const terms = screen.getAllByRole("term");
  const values = screen.getAllByRole("definition");
  return terms.map((term, i) => [term.textContent, values[i]?.textContent ?? null] as const);
}

describe("the saved stills (R11.T10.c)", () => {
  it("says STILLS: PENDING before the first listing, and NO STILLS SAVED for none", () => {
    const view = render(<StillList state={{ kind: "listing" }} />);
    const pending = screen.getByText("STILLS: PENDING");
    view.rerender(<StillList state={{ kind: "listed", stills: [] }} />);
    expect([pending, screen.getByText("NO STILLS SAVED")]).toEqual([
      expect.anything(),
      expect.anything(),
    ]);
  });

  it("says the folder could not be read where the listing failed, and how to retry", () => {
    render(<StillList state={{ kind: "failed", reason: "EACCES" }} />);
    expect(screen.getByText(/NOT LISTED/u).closest("p")?.textContent).toBe(
      "NOT LISTED: the stills folder could not be read, reopen VIEW to retry",
    );
  });

  it("lists each still by its time in UT and its size, newest first, its label folded", () => {
    render(<StillList state={{ kind: "listed", stills: [NEWER, OLDER] }} />);
    const list = screen.getByRole("list", { name: "Saved stills" });
    expect(
      within(list)
        .getAllByRole("button")
        .map((button) => [button.textContent, button.getAttribute("aria-expanded")]),
    ).toEqual([
      ["UT +13 yr 024/06:27:58 · 16 × 8 px", "false"],
      ["UT +13 yr 024/06:27:58 · 16 × 8 px", "false"],
    ]);
  });

  it("shows every field of a still's label from the keyboard, in the view's words, and folds it", async () => {
    const user = userEvent.setup();
    render(<StillList state={{ kind: "listed", stills: [NEWER, OLDER] }} />);
    await user.tab();
    await user.keyboard("{Enter}");
    const shown = shownLabel();
    const annunciations = within(screen.getByRole("list", { name: "" }))
      .getAllByRole("listitem")
      .map((item) => item.textContent);
    await user.keyboard("{Enter}");
    expect([shown, annunciations, screen.queryAllByRole("term")]).toEqual([
      [
        ["FRAME", "BODY TEST EARTH"],
        ["TIME", "UT +13 yr 024/06:27:58"],
        ["CAMERA", "ORBIT"],
        ["FOV", "60°"],
        ["EXPOSURE", "EV100 12.4 AUTO"],
        ["QUALITY", "HIGH"],
        ["DECORATION", "ON"],
        ["SURVEY COVERAGE", "TEST EARTH 34.0 % · FINEST 1.2 km"],
        ["ANNUNCIATIONS", "ATMOSPHERE: APPROXIMATETERRAIN: STREAMING"],
        ["SIZE", "16 × 8 px"],
      ],
      ["ATMOSPHERE: APPROXIMATE", "TERRAIN: STREAMING"],
      [],
    ]);
  });

  it("opens one still's label at a time", async () => {
    const user = userEvent.setup();
    render(<StillList state={{ kind: "listed", stills: [NEWER, OLDER] }} />);
    const [first, second] = screen.getAllByRole("button");
    if (first === undefined || second === undefined) {
      throw new Error("two stills are listed");
    }
    await user.click(first);
    await user.click(second);
    expect([first.getAttribute("aria-expanded"), second.getAttribute("aria-expanded")]).toEqual([
      "false",
      "true",
    ]);
  });

  it("says NONE for a still with no survey and no annunciation", async () => {
    const user = userEvent.setup();
    render(<StillList state={{ kind: "listed", stills: [OLDER] }} />);
    await user.click(screen.getByRole("button"));
    expect(shownLabel().slice(5)).toEqual([
      ["QUALITY", "LOW"],
      ["DECORATION", "OFF"],
      ["SURVEY COVERAGE", "NONE"],
      ["ANNUNCIATIONS", "NONE"],
      ["SIZE", "16 × 8 px"],
    ]);
  });

  it("gives no row of the still's own twice where the view's readings gave it", async () => {
    const user = userEvent.setup();
    const label = testStillLabel({
      readings: [{ label: "QUALITY", value: "LOW" }],
      setting: "high",
    });
    render(<StillList state={{ kind: "listed", stills: [{ id: NEWER.id, label }] }} />);
    await user.click(screen.getByRole("button"));
    expect(shownLabel().filter(([term]) => term === "QUALITY")).toEqual([["QUALITY", "LOW"]]);
  });

  it("says which stills are in view, and of how many, where the list scrolls", () => {
    // jsdom lays nothing out: a list 100 px tall whose entries are 60 px each, the first in view.
    vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(180);
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(100);
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
      this: HTMLElement,
    ) {
      const siblings = [...(this.parentElement?.children ?? [])];
      const index = this.tagName === "LI" ? siblings.indexOf(this) : -1;
      return index < 0 ? new DOMRect(0, 0, 100, 100) : new DOMRect(0, 60 * index, 100, 60);
    });
    const third = { id: "20261113T120000000Z-00000000", label: testStillLabel() };
    render(<StillList state={{ kind: "listed", stills: [NEWER, OLDER, third] }} />);
    act(() => {
      FakeResizeObserver.resizeAll();
    });
    expect(screen.getByText("1-2 of 3")).toBeInTheDocument();
  });

  it("says nothing of its position where every still is in view", () => {
    render(<StillList state={{ kind: "listed", stills: [NEWER, OLDER] }} />);
    act(() => {
      FakeResizeObserver.resizeAll();
    });
    expect(screen.queryByText(/ of 2$/u)).toBeNull();
  });
});
