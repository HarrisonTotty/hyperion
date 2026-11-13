import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { FakeStillsApi } from "../../test/fakeStills";
import { rgbaPng } from "../../test/pngFixtures";
import { describedText } from "../../test/statusLine";
import { testStillLabel } from "../../test/stillFixtures";
import { withStillLabel } from "../../view/stills/pngLabel";
import { STILL_RENDERING_NOT_BUILT, ViewStills } from "./ViewStills";

/** A store holding one saved still. */
function oneSaved(): FakeStillsApi {
  const stills = new FakeStillsApi();
  const label = testStillLabel();
  stills.saved.push({
    id: "20261113T000000000Z-00000001",
    png: withStillLabel(rgbaPng(16, 8), label),
    label,
  });
  return stills;
}

describe("VIEW's stills (R11.T10.c)", () => {
  it("lists the saved stills once it mounts", async () => {
    render(<ViewStills designator="PRIMARY" stills={oneSaved()} />);
    const list = await screen.findByRole("list", { name: "Saved stills" });
    expect(within(list).getAllByRole("button")).toHaveLength(1);
  });

  it("stands in a panel of its own, titled with its view", () => {
    render(<ViewStills designator="PRIMARY" stills={new FakeStillsApi()} />);
    expect(screen.getByRole("region", { name: "Stills PRIMARY" })).toBeInTheDocument();
  });

  it("holds TAKE STILL back until the renderer draws a still's tiles, saying why", async () => {
    const user = userEvent.setup();
    const stills = new FakeStillsApi();
    render(<ViewStills designator="PRIMARY" stills={stills} />);
    await screen.findByText("NO STILLS SAVED");
    await user.tab();
    const take = screen.getByRole("button", { name: "TAKE STILL" });
    await user.keyboard("{Enter}");
    expect([take, describedText(take), stills.calls]).toEqual([
      document.activeElement,
      STILL_RENDERING_NOT_BUILT,
      ["listStills"],
    ]);
  });

  it("reads window.hyperion's stills where none are given", async () => {
    render(<ViewStills designator="PRIMARY" />);
    expect(await screen.findByText("NO STILLS SAVED")).toBeInTheDocument();
  });
});
