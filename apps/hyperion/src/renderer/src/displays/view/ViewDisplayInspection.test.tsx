/**
 * `VIEW`'s body inspection (the owner's request, 2026-11-01): the body `SYSTEM`'s `VIEW BODY` opens,
 * drawn photorealistically as a kept scene of its served records, and closed back to the scene
 * shown before.
 */
import { galacticPositionFromLy } from "@hyperion/protocol";
import { screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { toSystemBodiesModel } from "../../lib/system/bodiesWire";
import { aUniverse } from "../../test/galaxyFixtures";
import { FIXTURE_EARTH, FIXTURE_SYSTEM, sliceBodies } from "../../test/planetaryFixture";
import {
  nominalStore,
  openUniverse,
  primaryLabelBlock,
  renderViewDisplay,
  settle,
  timedEngineSource,
  type ViewDisplayHarness,
} from "../../test/viewDisplayHarness";
import { type InspectionOpening, inspectionOption } from "./ViewSceneProvider";

afterEach(() => {
  vi.useRealTimers();
});

/** The fixture's system's designation of record. */
const DESIGNATION = "H7K 4C0RFZ D-7";

/** The slice's Earth's inspection, opened once, as `SYSTEM`'s `VIEW BODY` opens it. */
function earthOpening(): InspectionOpening {
  const result = toSystemBodiesModel(sliceBodies(), DESIGNATION);
  if (result.kind !== "ok") {
    throw new Error(`the slice's answer is unusable: ${result.fault}`);
  }
  return {
    option: inspectionOption({
      universe: aUniverse().id,
      place: {
        kind: "charted",
        system: FIXTURE_SYSTEM,
        designation: DESIGNATION,
        barycentre: galacticPositionFromLy([26_000, 0, 12]),
      },
      model: result.model,
      bodies: result.bodies,
      body: FIXTURE_EARTH,
      time: { seconds: 3_600, nanos: 0 },
    }),
    sequence: 1,
  };
}

/** `VIEW` mounted on the Earth's inspection, its engine made and its first frames drawn. */
async function inspecting(): Promise<ViewDisplayHarness> {
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: await nominalStore(),
    source: fake.source,
    engines: fake.engines,
    inspection: earthOpening(),
  });
  await settle();
  // The first photorealistic frame starts the pipelines' compile; once made, the view draws it.
  view.advance(100);
  await settle();
  view.advance(300);
  return view;
}

function sceneButton(name: string): HTMLElement {
  return within(screen.getByRole("group", { name: "Scene" })).getByRole("button", { name });
}

describe("VIEW's body inspection (the owner's request)", () => {
  it("opens on BODY INSPECTION, photorealistic, framed on the body from a free camera", async () => {
    await inspecting();

    expect(sceneButton("BODY INSPECTION")).toHaveAttribute("aria-pressed", "true");
    expect(sceneButton("SERVER")).toHaveAttribute("aria-pressed", "false");
    expect(
      screen.getByRole("application", { name: "VIEW, PHOTOREALISTIC, PRIMARY, FREE" }).tagName,
    ).toBe("CANVAS");
    expect(primaryLabelBlock().textContent).toMatch(
      new RegExp(`FRAME.*BODY ${DESIGNATION} /768.*STYLE.*PHOTOREALISTIC.*SCENE.*BODY INSPECTION`),
    );
  });

  it("asks its body's record and its sky of the server, as the server's scene does", async () => {
    const view = await inspecting();
    await openUniverse(view);
    await settle();
    view.advance(600);
    await settle();

    expect(
      view.socket
        .requestsOfKind("body_detail")
        .some((request) => request.body.body === FIXTURE_EARTH),
    ).toBe(true);
    expect(
      view.socket
        .requestsOfKind("sky")
        .some((request) => request.body.exclude_system === FIXTURE_SYSTEM),
    ).toBe(true);
  });

  it("gives the focus to the scene shown when its CLOSE goes with it", async () => {
    const view = await inspecting();
    screen.getByRole("button", { name: "CLOSE BODY INSPECTION" }).focus();

    await view.user.keyboard("{Enter}");
    view.advance(100);

    expect(sceneButton("SERVER")).toHaveFocus();
  });

  it("closes back to the scene shown before it, and is no longer offered", async () => {
    const view = await inspecting();

    await view.user.click(screen.getByRole("button", { name: "CLOSE BODY INSPECTION" }));
    view.advance(100);

    expect(sceneButton("SERVER")).toHaveAttribute("aria-pressed", "true");
    expect(
      within(screen.getByRole("group", { name: "Scene" })).queryByRole("button", {
        name: "BODY INSPECTION",
      }),
    ).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "CLOSE BODY INSPECTION" })).not.toBeInTheDocument();
  });
});
