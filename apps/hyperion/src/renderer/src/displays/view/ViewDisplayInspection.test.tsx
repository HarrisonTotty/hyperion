/**
 * `VIEW`'s body inspection (the owner's request, 2026-11-01): the body `SYSTEM`'s `VIEW BODY` opens,
 * drawn photorealistically as a kept scene of its served records, and closed back to the scene
 * shown before.
 */
import { galacticPositionFromLy, type HostDiscDto } from "@hyperion/protocol";
import { act, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { toSystemBodiesModel } from "../../lib/system/bodiesWire";
import { aUniverse } from "../../test/galaxyFixtures";
import { FIXTURE_EARTH, FIXTURE_SYSTEM, sliceBodies } from "../../test/planetaryFixture";
import { fakeViewEngineSource } from "../../test/fakeViewEngine";
import { answerServedField } from "../../test/servedHandoverFixtures";
import { binaryFrame } from "../../test/binaryFrames";
import { InThreadSkyWorker, skyPayload, skyResponse } from "../../test/skyFixtures";
import { SUN_ABSOLUTE_V, sunLikeHostDisc } from "../../view/lighting/hostDisc";
import { METER_CLASS, type MeterMode } from "../../view/post/meter";
import {
  nominalStore,
  openUniverse,
  primaryLabelBlock,
  renderViewDisplay,
  settle,
  submittedBy,
  timedEngineSource,
  type ViewDisplayHarness,
} from "../../test/viewDisplayHarness";
import { PHOTOREAL_PASS_LABELS } from "../../view/photoreal/passes";
import { TONEMAP_PASS } from "../../view/post/tonemap";
import type { ServedSky } from "../../view/scene/model";
import type { InspectionSurvey } from "../../view/scenes/inspectionSurvey";
import { type InspectionOpening, inspectionOption } from "./ViewSceneProvider";

afterEach(() => {
  vi.useRealTimers();
});

/** The fixture's system's designation of record. */
const DESIGNATION = "H7K 4C0RFZ D-7";

/** The survey of the slice's Earth as the server records it: its third pass. */
const RECORDED: InspectionSurvey = { kind: "recorded", body: FIXTURE_EARTH, level: 8, revision: 3 };

/** The survey of the slice's Earth as the server refuses it. */
const REFUSED: InspectionSurvey = {
  kind: "refused",
  body: FIXTURE_EARTH,
  reason: "no such body",
  standing: "refused",
};

/**
 * The slice's Earth's inspection, opened once, as `SYSTEM`'s `VIEW BODY` opens it, its survey
 * `survey` (recorded at once unless given).
 */
function earthOpening(
  survey: Promise<InspectionSurvey> = Promise.resolve(RECORDED),
  shared: { readonly sky: ServedSky; readonly sequence: number } = { sky: "system", sequence: 1 },
): InspectionOpening {
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
      sky: shared.sky,
    }),
    sequence: shared.sequence,
    survey,
  };
}

/** The Earth opened as the `PLANET VIEWER`'s `sequence`th generated planet, its sky the shared one. */
function viewerOpening(sequence: number): InspectionOpening {
  return earthOpening(Promise.resolve(RECORDED), { sky: "shared", sequence });
}

/** A survey the test settles, as the server answers it. */
function surveyed(): {
  readonly survey: Promise<InspectionSurvey>;
  readonly settle: (outcome: InspectionSurvey) => void;
} {
  let settleSurvey: ((outcome: InspectionSurvey) => void) | null = null;
  const survey = new Promise<InspectionSurvey>((resolve) => {
    settleSurvey = resolve;
  });
  return {
    survey,
    settle: (outcome) => {
      if (settleSurvey === null) {
        throw new Error("the survey's promise is made at once");
      }
      settleSurvey(outcome);
    },
  };
}

/** `VIEW` mounted on the Earth's inspection, its engine made and its first frames drawn. */
async function inspecting(
  opening: InspectionOpening = earthOpening(),
): Promise<ViewDisplayHarness> {
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: await nominalStore(),
    source: fake.source,
    engines: fake.engines,
    inspection: opening,
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

/** The revisions the Earth's field is asked at, `null` for the whole field. */
function earthFieldAsks(view: ViewDisplayHarness): ReadonlyArray<number | null> {
  return view.socket
    .requestsOfKind("surface_field")
    .filter((request) => request.body.body === FIXTURE_EARTH)
    .map((request) => request.body.have_revision);
}

/** The Earth's inspection with its field answered at revision 1 before its survey settles. */
async function fieldHeldBeforeSurvey(
  outcome: InspectionSurvey,
): Promise<ReadonlyArray<number | null>> {
  const { survey, settle: settleSurvey } = surveyed();
  const view = await inspecting(earthOpening(survey));
  await openUniverse(view);
  view.advance(600);
  await settle();
  answerServedField(view.socket, FIXTURE_EARTH, 64);
  await settle();
  settleSurvey(outcome);
  await settle();
  view.advance(300);
  await settle();
  return earthFieldAsks(view);
}

/**
 * Answers the view's latest sky request with one star and a solar twin's host disc for the
 * fixture's star (body index 0), as the server's sky brings it.
 */
async function skyArrivesWithSun(view: ViewDisplayHarness): Promise<void> {
  const sky = view.socket.requestsOfKind("sky").at(-1);
  if (sky === undefined) {
    throw new Error("the view asks no sky");
  }
  const payload = skyPayload([{ direction: [0, 0, -1], distanceLy: 100, vMag: 1 }], 2, null);
  await act(async () => {
    view.socket.serverSendsBinary(binaryFrame(sky.id, 0, 1, [...payload]));
    view.socket.serverResponds(sky.id, {
      kind: "sky",
      ...skyResponse(sky.body, payload, 1, 2),
      hosts: [sunLikeHostDisc({ star: 0 })],
    });
    await vi.advanceTimersByTimeAsync(0);
  });
  await settle();
}

/**
 * Answers the view's latest sky request, final, with one star and the hosts a sky that leaves no
 * system out carries: none.
 */
async function sharedSkyArrives(view: ViewDisplayHarness): Promise<void> {
  const sky = view.socket.requestsOfKind("sky").at(-1);
  if (sky === undefined) {
    throw new Error("the view asks no sky");
  }
  const payload = skyPayload([{ direction: [0, 0, -1], distanceLy: 100, vMag: 1 }], 2, null);
  await act(async () => {
    view.socket.serverSendsBinary(binaryFrame(sky.id, 0, 1, [...payload]));
    view.socket.serverResponds(sky.id, {
      kind: "sky",
      ...skyResponse(sky.body, payload, 1, 2),
      hosts: [],
    });
    await vi.advanceTimersByTimeAsync(0);
  });
  await settle();
}

/** The primary's `STARS` line reads `PENDING`: no sky of its arrival is held. */
function starsPendingShown(): boolean {
  return /STARS\s*PENDING/u.test(primaryLabelBlock().textContent);
}

/** Answers the view's latest `host_discs` request with `hosts`, as the server does at once. */
async function discsArrive(
  view: ViewDisplayHarness,
  hosts: ReadonlyArray<HostDiscDto>,
): Promise<void> {
  await act(async () => {
    view.socket.serverAnswers("host_discs", (body) => ({
      ...body,
      hosts: [...hosts],
    }));
    await vi.advanceTimersByTimeAsync(0);
  });
  await settle();
}

/** The `EV100` of the first `AUTO` reading among `readings`, or `NaN` with none. */
function firstAutoEv(readings: ReadonlyArray<string>): number {
  const first = readings.find((reading) => reading.endsWith(" AUTO"));
  return Number(/^EV100 (-?\d+\.\d) AUTO$/u.exec(first ?? "")?.[1]);
}

/** The primary's meter as its `EXPOSURE METER` panel names it. */
function meterReadout(): string {
  return within(screen.getByRole("region", { name: "Exposure meter PRIMARY" })).getByRole(
    "status",
    {
      name: "METER",
    },
  ).textContent;
}

/** `opening` opened on `meter` in its scene's own `INCIDENT`'s place. */
function openingOn(meter: MeterMode, opening: InspectionOpening): InspectionOpening {
  const { option } = opening;
  return {
    ...opening,
    option: { ...option, make: () => ({ ...option.make(), startMeter: meter }) },
  };
}

/** The primary's passes submitted to `fake` once `opening`'s first frames are drawn. */
async function primaryPassesOf(opening: InspectionOpening): Promise<string[]> {
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: await nominalStore(),
    source: fake.source,
    engines: fake.engines,
    inspection: opening,
  });
  await settle();
  view.advance(100);
  await settle();
  view.advance(300);
  return fake.submissions.filter((each) => submittedBy(each) === "view").map((each) => each.label);
}

describe("the inspection's first frames (fix-view-light)", () => {
  it("holds its first photorealistic frame back until a reading, so that none shows at MAN's -1", async () => {
    // Opened on AVG, whose reading is the image's histogram.
    const primary = await primaryPassesOf(openingOn("average", earthOpening()));
    const firstTonemap = primary.indexOf(TONEMAP_PASS);
    const histogramsBefore = primary
      .slice(0, firstTonemap)
      .filter((label) => label === PHOTOREAL_PASS_LABELS.histogram).length;
    expect(histogramsBefore).toBeGreaterThanOrEqual(2);
  });

  it("takes no histogram on INCIDENT, its opening meter (ux-calls-view, item 3b)", async () => {
    const primary = await primaryPassesOf(earthOpening());
    expect(primary.includes(PHOTOREAL_PASS_LABELS.histogram)).toBe(false);
  });
});

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

  it("asks its body's newly surveyed cells once its survey is recorded", async () => {
    expect(await fieldHeldBeforeSurvey(RECORDED)).toEqual([null, 1]);
  });

  it("asks its body's field marked for training, of the server's scratch log", async () => {
    const view = await inspecting();
    await openUniverse(view);
    view.advance(600);
    await settle();

    const asks = view.socket
      .requestsOfKind("surface_field")
      .filter((request) => request.body.body === FIXTURE_EARTH)
      .map((request) => request.body.scope);
    expect(asks.length).toBeGreaterThan(0);
    expect(new Set(asks)).toEqual(new Set(["training"]));
  });

  it("asks nothing more of its body's field where its survey is refused", async () => {
    expect(await fieldHeldBeforeSurvey(REFUSED)).toEqual([null]);
  });

  it("states its positions as seen from its body on its plate, in SHIP's place", async () => {
    await inspecting();

    const plate = primaryLabelBlock().textContent;
    expect(plate).toContain(`POSITIONS AS SEEN FROM ${DESIGNATION} /768`);
    expect(plate).not.toContain("POSITIONS AS SEEN FROM SHIP");
  });

  it("states its positions as seen from its body on an instrument's slot too", async () => {
    const view = await inspecting();
    await view.user.click(
      within(screen.getByRole("group", { name: "INSTRUMENT 1" })).getByRole("button", {
        name: "OPEN",
      }),
    );
    view.advance(300);

    const slot = screen.getByRole("region", { name: "INSTRUMENT 1" });
    expect([
      within(slot).getByText(`POSITIONS AS SEEN FROM ${DESIGNATION} /768`).textContent,
      within(slot).queryByText("POSITIONS AS SEEN FROM SHIP"),
    ]).toEqual([`POSITIONS AS SEEN FROM ${DESIGNATION} /768`, null]);
  });

  it("states SURVEY PASS: PENDING on its plate while its survey is out", async () => {
    await inspecting(earthOpening(surveyed().survey));

    expect(primaryLabelBlock().textContent).toContain("SURVEY PASS: PENDING");
  });

  it("states its survey's refusal on its plate", async () => {
    const view = await inspecting(earthOpening(Promise.resolve(REFUSED)));
    view.advance(300);
    await settle();

    expect(primaryLabelBlock().textContent).toContain("SURVEY PASS REJECTED: no such body");
  });

  it("states nothing of its survey once it is recorded", async () => {
    const view = await inspecting();
    view.advance(300);
    await settle();

    expect(primaryLabelBlock().textContent).not.toContain("SURVEY PASS");
  });

  it("opens exposed for its body on INCIDENT, AUTO at the light falling on it at once (ux-calls-view, item 3b)", async () => {
    vi.stubGlobal("Worker", InThreadSkyWorker);
    const fake = fakeViewEngineSource();
    // A sunlit disc in the histogram, which INCIDENT does not read.
    fake.image.pixels = [{ meterClass: METER_CLASS.litBody, bin: 150, count: 200 }];
    const view = renderViewDisplay({
      store: await nominalStore(),
      source: fake.source,
      engines: fake.engines,
      inspection: earthOpening(),
    });
    await openUniverse(view);
    view.advance(600);
    await settle();
    // MAN's EV100 -1.0 stands while its sky's discs are awaited, and the meter says nothing yet.
    const unlit = exposureReadout();
    const awaited = screen.queryByText(/NO LIGHT/u);
    await skyArrivesWithSun(view);
    const readings = await readingsOver(view, 4);

    // Then AUTO at the incident reading at once: a solar twin at the fixture Earth's 0.985 au,
    // 15.70 + 2 log₂(1 ÷ 0.985) = 15.75.
    const first = readings.find((reading) => reading.endsWith(" AUTO"));
    const appliedEv = Number(/^EV100 (-?\d+\.\d) AUTO$/u.exec(first ?? "")?.[1]);
    expect([unlit, awaited, meterReadout(), Math.abs(appliedEv - 15.75) <= 0.1]).toEqual([
      "EV100 -1.0 MAN",
      null,
      "INCIDENT",
      true,
    ]);
  });

  it("is lit by its system's discs on their own while its sky is pending (plans R06, R07)", async () => {
    vi.stubGlobal("Worker", InThreadSkyWorker);
    const fake = fakeViewEngineSource();
    const view = renderViewDisplay({
      store: await nominalStore(),
      source: fake.source,
      engines: fake.engines,
      inspection: earthOpening(),
    });
    await openUniverse(view);
    view.advance(600);
    await settle();
    const [asked] = view.socket.requestsOfKind("host_discs");
    const pending = primaryLabelBlock().textContent;
    await discsArrive(view, [sunLikeHostDisc({ star: 0 })]);
    const readings = await readingsOver(view, 4);

    // Asked once, of the inspection's system at its time; lit with no sky held, AUTO at the
    // incident reading at once, a solar twin at the fixture Earth's 0.985 au (EV100 15.75).
    expect([
      view.socket.requestsOfKind("host_discs").length,
      asked?.body.system,
      asked?.body.time,
      pending.includes("LIGHTING: PENDING"),
      primaryLabelBlock().textContent.includes("LIGHTING: PENDING"),
      /STARS\s*PENDING/u.test(primaryLabelBlock().textContent),
      Math.abs(firstAutoEv(readings) - 15.75) <= 0.1,
    ]).toEqual([1, FIXTURE_SYSTEM, { seconds: 3_600, nanos: 0 }, true, false, true, true]);
  });

  it("states its lighting not received once its system's discs are answered empty, its sky still pending", async () => {
    vi.stubGlobal("Worker", InThreadSkyWorker);
    const fake = fakeViewEngineSource();
    const view = renderViewDisplay({
      store: await nominalStore(),
      source: fake.source,
      engines: fake.engines,
      inspection: earthOpening(),
    });
    await openUniverse(view);
    view.advance(600);
    await settle();
    await discsArrive(view, []);
    view.advance(300);
    await settle();
    const block = primaryLabelBlock().textContent;
    expect([block.includes("LIGHTING: NOT RECEIVED"), starsPendingShown()]).toEqual([true, true]);
  });

  it("is lit by its sky's discs once its sky is held, as before the discs on their own", async () => {
    vi.stubGlobal("Worker", InThreadSkyWorker);
    const fake = fakeViewEngineSource();
    const view = renderViewDisplay({
      store: await nominalStore(),
      source: fake.source,
      engines: fake.engines,
      inspection: earthOpening(),
    });
    await openUniverse(view);
    view.advance(600);
    await settle();
    // The discs on their own a tenth as bright as the sky's own: 2.5 magnitudes fainter.
    await discsArrive(view, [sunLikeHostDisc({ star: 0, absoluteV: SUN_ABSOLUTE_V + 2.5 })]);
    const before = firstAutoEv(await readingsOver(view, 4));
    await skyArrivesWithSun(view);
    const after = (await readingsOver(view, 8)).at(-1) ?? "";

    // AUTO moves from the fainter discs' reading, 15.75 - log₂ 10 = 12.43, to the sky's, 15.75.
    const afterEv = Number(/^EV100 (-?\d+\.\d) AUTO$/u.exec(after)?.[1]);
    expect([Math.abs(before - 12.43) <= 0.15, Math.abs(afterEv - 15.75) <= 0.15]).toEqual([
      true,
      true,
    ]);
  });

  it("asks a generated planet's shared sky with no system left out (plan R15)", async () => {
    vi.stubGlobal("Worker", InThreadSkyWorker);
    const view = await inspecting(viewerOpening(1));
    await openUniverse(view);
    view.advance(600);
    await settle();
    expect(view.socket.requestsOfKind("sky").map((request) => request.body.exclude_system)).toEqual(
      [null],
    );
  });

  it("keeps the shared sky through the next generated planet, asking no sky in the hold (plan R15)", async () => {
    vi.stubGlobal("Worker", InThreadSkyWorker);
    const view = await inspecting(viewerOpening(1));
    await openUniverse(view);
    view.advance(600);
    await settle();
    await discsArrive(view, [sunLikeHostDisc({ star: 0 })]);
    await sharedSkyArrives(view);
    const heldFirst = !starsPendingShown();

    view.inspect(viewerOpening(2));
    await settle();
    view.advance(600);
    await settle();

    // The next planet's stage keeps the stars drawn, asks no sky, and is lit by its system's discs,
    // which it holds already: the same system here, kept by universe and system.
    expect([
      heldFirst,
      view.socket.requestsOfKind("sky").length,
      starsPendingShown(),
      primaryLabelBlock().textContent.includes("LIGHTING: PENDING"),
      view.socket.requestsOfKind("host_discs").length,
    ]).toEqual([true, 1, false, false, 1]);
  });

  it("keeps the shared sky's request in flight through the next generated planet, which it then draws (plan R15)", async () => {
    vi.stubGlobal("Worker", InThreadSkyWorker);
    const view = await inspecting(viewerOpening(1));
    await openUniverse(view);
    view.advance(600);
    await settle();
    const [first] = view.socket.requestsOfKind("sky");

    view.inspect(viewerOpening(2));
    await settle();
    view.advance(600);
    await settle();
    const pendingThrough = starsPendingShown();
    await sharedSkyArrives(view);

    expect([
      first !== undefined && view.socket.cancelledIds().includes(first.id),
      view.socket.requestsOfKind("sky").length,
      pendingThrough,
      starsPendingShown(),
    ]).toEqual([false, 1, true, false]);
  });

  it("asks its own sky as another system's inspection opens after a generated planet", async () => {
    vi.stubGlobal("Worker", InThreadSkyWorker);
    const view = await inspecting(viewerOpening(1));
    await openUniverse(view);
    view.advance(600);
    await settle();
    await sharedSkyArrives(view);

    view.inspect(earthOpening(Promise.resolve(RECORDED), { sky: "system", sequence: 2 }));
    await settle();
    view.advance(600);
    await settle();

    expect([
      view.socket.requestsOfKind("sky").map((request) => request.body.exclude_system),
      starsPendingShown(),
    ]).toEqual([[null, FIXTURE_SYSTEM], true]);
  });

  it("keeps a meter the operator chose in it, through another scene's own choice and back (ux-calls-view, item 3b)", async () => {
    const fake = fakeViewEngineSource();
    const view = renderViewDisplay({
      store: await nominalStore(),
      source: fake.source,
      engines: fake.engines,
      inspection: earthOpening(),
    });
    // Its image drawn, its pipelines made, beside which the meter's panel stands.
    await settle();
    view.advance(100);
    await readingsOver(view, 4);
    await view.user.click(
      within(screen.getByRole("region", { name: "Exposure meter PRIMARY" })).getByRole("button", {
        name: "LIT",
      }),
    );
    view.advance(300);
    await view.user.click(sceneButton("PHASE TEST"));
    view.advance(300);
    // PHASE TEST drawn photorealistic, beside which its meter's panel stands, and DARK chosen there.
    await view.user.click(screen.getByRole("button", { name: "PHOTOREALISTIC" }));
    await settle();
    view.advance(100);
    await readingsOver(view, 4);
    await view.user.click(
      within(screen.getByRole("region", { name: "Exposure meter PRIMARY" })).getByRole("button", {
        name: "DARK",
      }),
    );
    view.advance(300);
    const elsewhere = meterReadout();
    await view.user.click(sceneButton("BODY INSPECTION"));
    await settle();
    view.advance(100);
    await readingsOver(view, 4);
    expect([elsewhere, meterReadout()]).toEqual(["DARK", "LIT"]);
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
