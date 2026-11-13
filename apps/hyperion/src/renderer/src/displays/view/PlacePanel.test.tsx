/**
 * The camera panel's place while `CONTROLS` names an instrument (plan R10, R10.T12.d): one panel
 * named for its page, its tabs mounted across a change of page, and a held-back `GROUND`'s reason.
 */
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";

import { describedText } from "../../test/statusLine";
import { PlacePanel } from "./PlacePanel";
import { groundHeldBack, type PlacePage } from "./PlaceTabs";

/** Props of {@link Place}. */
interface PlaceProps {
  readonly initial: PlacePage;
  readonly reason?: string | null;
  readonly hidden?: boolean;
}

/** `INSTRUMENT 1`'s place, its page kept by the test as the display keeps it. */
function Place({ initial, reason = null, hidden = false }: PlaceProps) {
  const [page, setPage] = useState<PlacePage>(initial);
  return (
    <PlacePanel
      page={page}
      groundReason={reason}
      onPage={setPage}
      designator="INSTRUMENT 1"
      id="place"
      hidden={hidden}
    >
      <p>{page === "ground" ? "THE GROUND PAGE" : "THE CAMERA PAGE"}</p>
    </PlacePanel>
  );
}

/** The place's tab named `name`. */
function tab(name: "CAMERA" | "GROUND"): HTMLElement {
  return within(screen.getByRole("tablist", { name: "Place INSTRUMENT 1" })).getByRole("tab", {
    name,
  });
}

describe("PlacePanel (R10.T12.d; guide draft D2)", () => {
  it("is a region named for the camera page while CAMERA stands", () => {
    render(<Place initial="camera" />);
    expect(screen.getByRole("region", { name: "Camera INSTRUMENT 1" })).toBeInTheDocument();
  });

  it("is a region named for the ground page while GROUND stands", () => {
    render(<Place initial="ground" />);
    expect(screen.getByRole("region", { name: "Ground INSTRUMENT 1" })).toBeInTheDocument();
  });

  it("shows the chosen page in the tab panel its tab controls", () => {
    render(<Place initial="ground" />);
    expect(screen.getByRole("tabpanel", { name: "GROUND" })).toHaveTextContent("THE GROUND PAGE");
  });

  it("keeps the focus on the tab pressed from the keyboard as the page changes", async () => {
    const user = userEvent.setup();
    render(<Place initial="camera" />);
    tab("CAMERA").focus();
    await user.keyboard("{ArrowRight}");
    expect(tab("GROUND")).toHaveFocus();
  });

  it("keeps the focus on a tab chosen with Enter", async () => {
    const user = userEvent.setup();
    render(<Place initial="ground" />);
    tab("GROUND").focus();
    await user.keyboard("{ArrowLeft}");
    await user.keyboard("{Enter}");
    expect(tab("CAMERA")).toHaveFocus();
  });

  it("describes the held-back GROUND by that reason", () => {
    render(<Place initial="camera" reason={groundHeldBack("INSTRUMENT 1")} />);
    expect(describedText(tab("GROUND"))).toBe(
      "NOT AVAILABLE: INSTRUMENT 1's FRAME is no body with a surface",
    );
  });

  it("folds with the compact layout's CAMERA button, by its ID", () => {
    render(<Place initial="camera" hidden />);
    expect(screen.queryByRole("region", { name: "Camera INSTRUMENT 1" })).toBeNull();
  });
});
