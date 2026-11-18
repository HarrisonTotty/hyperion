import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { FakeStillDevice, FakeStillSource } from "../../test/fakeStillJob";
import { FakeStillsApi } from "../../test/fakeStills";
import { rgbaPng } from "../../test/pngFixtures";
import { TEST_STILL_REQUEST, testStillLabel } from "../../test/stillFixtures";
import { withStillLabel } from "../../view/stills/pngLabel";
import { type StillEncoder, StillSession } from "../../view/stills/stillJob";
import { StillStore } from "../../view/stills/stillStore";
import type { ViewStillsModel } from "./useViewStills";
import { ViewStills } from "./ViewStills";

/** A real PNG of the pixels as given, as `convertToBlob` would make, without a canvas. */
const PNG_ENCODER: StillEncoder = (rgba, size) =>
  Promise.resolve(rgbaPng(size.widthPx, size.heightPx, new Uint8Array(rgba.buffer)));

/** `VIEW`'s stills over the fakes: a still of 100 × 70 px, and a store holding `saved`. */
function model(stills: FakeStillsApi, device = new FakeStillDevice("absent")): ViewStillsModel {
  const store = new StillStore(stills);
  const source = new FakeStillSource(device, {
    ...TEST_STILL_REQUEST,
    size: { widthPx: 100, heightPx: 70 },
  });
  const session = new StillSession({
    source,
    encode: PNG_ENCODER,
    save: (png, label) => store.save(png, label),
    nowMs: () => 0,
    reportError: () => undefined,
  });
  return { store, session };
}

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

describe("VIEW's stills (R11.T10.c, R11.T10.b)", () => {
  it("lists the saved stills once it mounts", async () => {
    render(<ViewStills designator="PRIMARY" model={model(oneSaved())} />);
    const list = await screen.findByRole("list", { name: "Saved stills" });
    expect(within(list).getAllByRole("button")).toHaveLength(1);
  });

  it("stands in a panel of its own, titled with its view", () => {
    render(<ViewStills designator="PRIMARY" model={model(new FakeStillsApi())} />);
    expect(screen.getByRole("region", { name: "Stills PRIMARY" })).toBeInTheDocument();
  });

  it("takes a still from the keyboard, saves it and lists it", async () => {
    const user = userEvent.setup();
    const stills = new FakeStillsApi();
    render(<ViewStills designator="PRIMARY" model={model(stills)} />);
    await screen.findByText("NO STILLS SAVED");
    await user.tab();
    await user.keyboard("{Enter}");
    const saved = await screen.findByText("STILL SAVED");
    const list = await screen.findByRole("list", { name: "Saved stills" });
    expect([
      saved,
      within(list)
        .getAllByRole("button")
        .map((button) => button.textContent),
      stills.saved[0]?.label.size,
    ]).toEqual([
      expect.anything(),
      ["UT +13 yr 024/06:27:58 · 100 × 70 px"],
      { widthPx: 100, heightPx: 70 },
    ]);
  });

  it("says STILLS LIMITED after a loss in flight, and offers TAKE STILL again", async () => {
    const user = userEvent.setup();
    const device = new FakeStillDevice("absent");
    device.loseAtBatch = 0;
    render(<ViewStills designator="PRIMARY" model={model(new FakeStillsApi(), device)} />);
    await user.click(screen.getByRole("button", { name: "TAKE STILL" }));
    expect([
      await screen.findByText("STILLS LIMITED"),
      screen.getByRole("button", { name: "TAKE STILL" }).getAttribute("aria-disabled"),
    ]).toEqual([expect.anything(), null]);
  });
});
