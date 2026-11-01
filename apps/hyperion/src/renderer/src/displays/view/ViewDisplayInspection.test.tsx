/**
 * `VIEW`'s body inspection (the owner's request, 2026-11-01): the body `SYSTEM`'s `VIEW BODY` opens,
 * drawn photorealistically as a kept scene of its served records, and closed back to the scene
 * shown before.
 */
import { galacticPositionFromLy, type SurfaceRevisionDto } from "@hyperion/protocol";
import { screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { toSystemBodiesModel } from "../../lib/system/bodiesWire";
import { aUniverse } from "../../test/galaxyFixtures";
import { FIXTURE_EARTH, FIXTURE_SYSTEM, sliceBodies } from "../../test/planetaryFixture";
import { fakeViewEngineSource } from "../../test/fakeViewEngine";
import {
  DEFAULT_MAN_EV100,
  ev100FromAverageLuminance,
  exposureScale,
} from "../../view/photometry/exposure";
import { binCentreLuminance } from "../../view/post/histogram";
import { METER_CLASS } from "../../view/post/meter";
import {
  nominalStore,
  openUniverse,
  primaryLabelBlock,
  renderViewDisplay,
  settle,
  timedEngineSource,
  type ViewDisplayHarness,
} from "../../test/viewDisplayHarness";
import type { InspectionSurvey } from "../../view/scenes/inspectionSurvey";
import type { TerrainSession } from "../../view/terrain/session";
import { type InspectionOpening, inspectionOption } from "./ViewSceneProvider";

afterEach(() => {
  vi.useRealTimers();
});

/** The fixture's system's designation of record. */
const DESIGNATION = "H7K 4C0RFZ D-7";

/** The survey of the slice's Earth as the server records it: its third pass. */
const RECORDED: InspectionSurvey = { kind: "recorded", body: FIXTURE_EARTH, level: 8, revision: 3 };

/** The slice's Earth's inspection, opened once, as `SYSTEM`'s `VIEW BODY` opens it. */
function earthOpening(survey: InspectionSurvey = RECORDED): InspectionOpening {
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
    survey: Promise.resolve(survey),
  };
}

/** `VIEW` mounted on the Earth's inspection, its engine made and its first frames drawn. */
async function inspecting(
  watchTerrain?: (session: TerrainSession) => () => void,
): Promise<ViewDisplayHarness> {
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: await nominalStore(),
    source: fake.source,
    engines: fake.engines,
    inspection: earthOpening(),
    ...(watchTerrain === undefined ? {} : { watchTerrain }),
  });
  await settle();
  // The first photorealistic frame starts the pipelines' compile; once made, the view draws it.
  view.advance(100);
  await settle();
  view.advance(300);
  return view;
}

/** The primary's exposure as its panel reads it: `EV100 7.3 AUTO`. */
function exposureReadout(): string {
  return (
    within(screen.getByRole("region", { name: "Exposure PRIMARY" })).getAllByRole("status")[0]
      ?.textContent ?? ""
  );
}

/** The primary's exposure readings, one each quarter of a second for `count` of them. */
async function readingsOver(view: ViewDisplayHarness, count: number): Promise<string[]> {
  if (count === 0) {
    return [];
  }
  view.advance(250);
  await settle();
  const reading = exposureReadout();
  return [reading, ...(await readingsOver(view, count - 1))];
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

  it("tells its terrain session the revision of its body's survey once the server records it", async () => {
    const noted: Array<ReadonlyArray<SurfaceRevisionDto>> = [];
    await inspecting((session) => {
      vi.spyOn(session, "noteSurfaceRevisions").mockImplementation((revisions) => {
        noted.push(revisions);
      });
      return () => undefined;
    });

    expect(noted).toEqual([[{ body: FIXTURE_EARTH, revision: 3 }]]);
  });

  it("opens exposed for its body, AUTO at its image's first metered value at once", async () => {
    const fake = fakeViewEngineSource();
    // A sunlit disc: brighter than the display's MAN default, EV100 -1.0, by some 14 stops.
    fake.image.pixels = [{ meterClass: METER_CLASS.litBody, bin: 150, count: 200 }];
    const view = renderViewDisplay({
      store: await nominalStore(),
      source: fake.source,
      engines: fake.engines,
      inspection: earthOpening(),
    });
    const readings = await readingsOver(view, 8);

    // MAN's EV100 -1.0 stands until the first metered frame, then AUTO at the meter's reading of
    // the frame drawn at it, at once: a walk at AUTO's 3 EV a second would be short of 0 here.
    const first = readings.find((reading) => reading.endsWith(" AUTO"));
    const appliedEv = Number(/^EV100 (-?\d+\.\d) AUTO$/u.exec(first ?? "")?.[1]);
    const meteredEv = ev100FromAverageLuminance(
      binCentreLuminance(150) / exposureScale(DEFAULT_MAN_EV100),
    );
    expect(readings[0]).toBe("EV100 -1.0 MAN");
    expect(Math.abs(appliedEv - meteredEv)).toBeLessThanOrEqual(0.5);
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
