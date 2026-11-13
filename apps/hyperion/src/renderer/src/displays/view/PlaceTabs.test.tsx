import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { stylesheetRule } from "../../test/stylesheet";
import { groundHeldBack, type PlaceIds, type PlacePage, PlaceTabs } from "./PlaceTabs";

const IDS: PlaceIds = { tab: (page) => `tab-${page}`, panel: "page", reason: "reason" };

/** The tabs of `INSTRUMENT 1`'s place on `page`, `GROUND` held back with `reason` where given. */
function renderTabs(
  page: PlacePage,
  reason: string | null = null,
  onPage: (page: PlacePage) => void = () => undefined,
): void {
  render(
    <PlaceTabs
      page={page}
      groundReason={reason}
      onPage={onPage}
      designator="INSTRUMENT 1"
      ids={IDS}
    />,
  );
}

/** The place's tab list. */
function tabList(): HTMLElement {
  return screen.getByRole("tablist", { name: "Place INSTRUMENT 1" });
}

/** The place's tab named `name`. */
function tab(name: "CAMERA" | "GROUND"): HTMLElement {
  return within(tabList()).getByRole("tab", { name });
}

describe("PlaceTabs (R10.T12.d; guide draft D2; ux-reviewer's tab list)", () => {
  it("offers CAMERA and GROUND in that order", () => {
    renderTabs("camera");
    expect(
      within(tabList())
        .getAllByRole("tab")
        .map((each) => each.textContent),
    ).toEqual(["CAMERA", "GROUND"]);
  });

  it("selects the page standing", () => {
    renderTabs("ground");
    expect(
      [tab("CAMERA"), tab("GROUND")].map((each) => each.getAttribute("aria-selected")),
    ).toEqual(["false", "true"]);
  });

  it("states the instrument's designator after the tabs", () => {
    renderTabs("camera");
    expect(tabList().nextElementSibling?.textContent).toBe("INSTRUMENT 1");
  });

  it("makes the chosen page's tab the list's one stop in the tab order", () => {
    renderTabs("ground");
    expect([tab("CAMERA").tabIndex, tab("GROUND").tabIndex]).toEqual([-1, 0]);
  });

  it("asks for GROUND when it is clicked", async () => {
    const user = userEvent.setup();
    const onPage = vi.fn<(page: PlacePage) => void>();
    renderTabs("camera", null, onPage);
    await user.click(tab("GROUND"));
    expect(onPage).toHaveBeenCalledWith("ground");
  });

  it("asks for GROUND with the right arrow on CAMERA", async () => {
    const user = userEvent.setup();
    const onPage = vi.fn<(page: PlacePage) => void>();
    renderTabs("camera", null, onPage);
    tab("CAMERA").focus();
    await user.keyboard("{ArrowRight}");
    expect(onPage).toHaveBeenCalledWith("ground");
  });

  it("moves the focus with the arrow to the tab it chose", async () => {
    const user = userEvent.setup();
    renderTabs("camera");
    tab("CAMERA").focus();
    await user.keyboard("{ArrowRight}");
    expect(tab("GROUND")).toHaveFocus();
  });

  it("asks for CAMERA with Home on GROUND", async () => {
    const user = userEvent.setup();
    const onPage = vi.fn<(page: PlacePage) => void>();
    renderTabs("ground", null, onPage);
    tab("GROUND").focus();
    await user.keyboard("{Home}");
    expect(onPage).toHaveBeenCalledWith("camera");
  });

  it("holds GROUND back while the instrument's FRAME is no body with a surface, not disabling it", () => {
    renderTabs("camera", groundHeldBack("INSTRUMENT 1"));
    expect([
      tab("GROUND").getAttribute("aria-disabled"),
      tab("GROUND").hasAttribute("disabled"),
    ]).toEqual(["true", false]);
  });

  it("describes the held-back GROUND by its reason", () => {
    renderTabs("camera", groundHeldBack("INSTRUMENT 1"));
    // The reason's two parts (`StatusText`'s state and sentence) are read with each part trimmed
    // here, so the space after the colon is optional; Chromium keeps it (the outbox's capture), and
    // the wording, space and all, is pinned below.
    expect(tab("GROUND")).toHaveAccessibleDescription(
      /^NOT AVAILABLE: ?INSTRUMENT 1's FRAME is no body with a surface$/u,
    );
  });

  it("asks for nothing when the held-back GROUND is clicked", async () => {
    const user = userEvent.setup();
    const onPage = vi.fn<(page: PlacePage) => void>();
    renderTabs("camera", groundHeldBack("INSTRUMENT 1"), onPage);
    await user.click(tab("GROUND"));
    expect(onPage).not.toHaveBeenCalled();
  });

  it("asks for nothing when an arrow would choose the held-back GROUND", async () => {
    const user = userEvent.setup();
    const onPage = vi.fn<(page: PlacePage) => void>();
    renderTabs("camera", groundHeldBack("INSTRUMENT 1"), onPage);
    tab("CAMERA").focus();
    await user.keyboard("{ArrowRight}");
    expect(onPage).not.toHaveBeenCalled();
  });

  it("moves the focus onto the held-back GROUND with the arrow, so that the focus shows its reason", async () => {
    const user = userEvent.setup();
    renderTabs("camera", groundHeldBack("INSTRUMENT 1"));
    tab("CAMERA").focus();
    await user.keyboard("{ArrowRight}");
    expect(tab("GROUND")).toHaveFocus();
  });

  it("focuses the held-back GROUND when it is clicked, so that a press shows its reason", async () => {
    const user = userEvent.setup();
    renderTabs("camera", groundHeldBack("INSTRUMENT 1"));
    await user.click(tab("GROUND"));
    expect(tab("GROUND")).toHaveFocus();
  });

  it("words the held-back reason in the guide's NOT AVAILABLE form", () => {
    expect(groundHeldBack("INSTRUMENT 2")).toBe(
      "NOT AVAILABLE: INSTRUMENT 2's FRAME is no body with a surface",
    );
  });
});

describe("the held-back GROUND in the stylesheet (fix-r10-t12d-held-tab)", () => {
  it("strikes the held-back tab's name through in every state, at rest included", () => {
    expect(stylesheetRule('.view-place__tab[aria-disabled="true"]')).toContain(
      "text-decoration: line-through;",
    );
  });

  it("shows the reason while the held-back tab has the focus, a press's as well as the keyboard's", () => {
    expect(
      stylesheetRule(
        '.view-place__head:has(.view-place__tab[aria-disabled="true"]:is(:hover, :focus))\n  > .view-place__reason',
      ),
    ).toContain("visibility: visible;");
  });

  it("lets a press through the reason to the camera control under it", () => {
    expect(stylesheetRule(".view-place__reason")).toContain("pointer-events: none;");
  });
});
