/**
 * The atmosphere on a live `VIEW` (plan R08, R08.T10.b): the cache's `body_detail` joined with the
 * terrain session's, and a refused record's line on the primary's plate; and `ATMOSPHERE:
 * APPROXIMATE`'s reasons on the label block's plate (R08.T10.c), set on the block itself, since no
 * scene the display test can draw yet carries the note: the kept scenes' media are thin, and the
 * display's optics workers answer nothing.
 */
import type { ResponseBody } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { FIXTURE_EARTH } from "../../test/planetaryFixture";
import { SCENE_TIDAL_RADIUS_M, sceneClock, sliceSceneSystem } from "../../test/sceneFixture";
import { statusLine } from "../../test/statusLine";
import {
  nominalStore,
  openUniverse,
  primaryLabelBlock,
  renderViewDisplay,
  settle,
  timedEngineSource,
  type ViewDisplayHarness,
} from "../../test/viewDisplayHarness";
import { atmosphereApproximateLines, atmosphereNotes } from "../../view/atmosphere/labels";
import { THICK_REASON } from "../../view/atmosphere/thick/regime";
import { ViewLabelBlock } from "./ViewLabelBlock";
import { atmosphereNoteStatement } from "./viewRun";

afterEach(() => {
  vi.useRealTimers();
});

/** WGS 84's equatorial radius, m: the slice's Earth's figure. */
const EARTH_RADIUS_M = 6_378_137;

/** `VIEW` drawing the server's scene photorealistically, the ship 2,000 km over the Earth. */
async function overEarthPhotoreal(): Promise<ViewDisplayHarness> {
  const fake = timedEngineSource();
  const view = renderViewDisplay({
    store: await nominalStore(),
    source: fake.source,
    engines: fake.engines,
  });
  await settle();
  view.advance(100);
  await openUniverse(view);
  const answer: ResponseBody = {
    kind: "subscribe",
    subscription: 5,
    state: {
      topic: "scene",
      sequence: 0,
      clock: sceneClock(3_000),
      ship: {
        position: { frame: "body", body: FIXTURE_EARTH, offset_m: [0, 0, EARTH_RADIUS_M + 2e6] },
        velocity_m_s: [0, 0, 0],
        time: { seconds: 3_000, nanos: 0 },
      },
      system: sliceSceneSystem(),
      tidal_radius_m: SCENE_TIDAL_RADIUS_M,
      craft: [],
    },
  };
  await act(async () => {
    view.socket.serverAnswers("subscribe", () => answer);
    await vi.advanceTimersByTimeAsync(0);
  });
  await settle();
  view.advance(300);
  await view.user.keyboard("4");
  view.advance(100);
  await settle();
  view.advance(300);
  await settle();
  return view;
}

describe("the atmosphere on a live VIEW (R08.T10.b)", () => {
  it("asks the Earth's record once for its atmosphere and its terrain together", async () => {
    const view = await overEarthPhotoreal();
    const asked = view.socket
      .requestsOfKind("body_detail")
      .filter((request) => request.body.body === FIXTURE_EARTH);
    expect(asked).toHaveLength(1);
  });

  it("states ATMOSPHERE REJECTED with the server's reason where it refuses the record", async () => {
    const view = await overEarthPhotoreal();
    const asked = view.socket
      .requestsOfKind("body_detail")
      .filter((request) => request.body.body === FIXTURE_EARTH);
    await act(async () => {
      for (const request of asked) {
        view.socket.serverRejects(request.id, {
          code: "unknown_body",
          message: "no such body",
          field: null,
        });
      }
      await vi.advanceTimersByTimeAsync(0);
    });
    view.advance(600);
    await settle();
    const rejected = within(primaryLabelBlock())
      .getAllByRole("status")
      .map((output) => output.textContent)
      .filter((text) => text.startsWith("ATMOSPHERE REJECTED"));
    expect(rejected).toEqual([
      expect.stringMatching(/^ATMOSPHERE REJECTED: for .+, no such body, reopen VIEW to retry$/u),
    ]);
  });
});

/** The label block with the notes and three bodies' reasons, as `photorealStatements` sets them. */
function renderPlate(): void {
  const lines = atmosphereApproximateLines([
    { designation: "KX 4417 /3", reasons: ["H2O: its absorption estimated, not measured"] },
    { designation: "KX 4417 /5", reasons: [THICK_REASON] },
    { designation: "KX 4417 /2", reasons: [THICK_REASON] },
  ]);
  const statements = atmosphereNotes(
    ["aerosolsNotYetModelled", "atmosphereApproximate"],
    [],
    lines,
  ).map(atmosphereNoteStatement);
  render(
    <ViewLabelBlock
      lines={[{ label: "STYLE", value: "PHOTOREALISTIC" }]}
      statements={statements}
      countLine={null}
      faults={[]}
    />,
  );
}

describe("ATMOSPHERE: APPROXIMATE's reasons on the plate (R08.T10.c, R08.T10.d)", () => {
  const notes = ["AEROSOLS AND ABSORBERS: NOT YET MODELLED", "ATMOSPHERE: APPROXIMATE"];
  const reasons = [
    "A thick atmosphere drawn by the thin-atmosphere method, for KX 4417 /2 and KX 4417 /5",
    "H2O: its absorption estimated, not measured, for KX 4417 /3",
  ];

  it("sets each reason on a line of its own straight after the note, a steady statement in the case given", () => {
    renderPlate();
    const shown = [
      ...notes.map((text) => screen.getByText(statusLine(text))),
      ...reasons.map((text) => screen.getByText(text)),
    ];
    const inOrder = shown.every((line, i) => {
      const before = shown[i - 1];
      return (
        before === undefined ||
        (before.compareDocumentPosition(line) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0
      );
    });
    // In the order said, each the plate's steady statement in `--text`, neither an alert nor a status.
    expect([
      inOrder,
      shown.every((line) => line.classList.contains("view-label__statement")),
      screen.queryByRole("alert"),
      screen.queryAllByRole("status").some((status) => shown.some((line) => status.contains(line))),
    ]).toEqual([true, true, null, false]);
  });

  it("sets each reason whole as a sentence, nothing in it tracked (R08.T10.d)", () => {
    renderPlate();
    expect(
      reasons.map((text) => {
        const line = screen.getByText(text);
        return [
          line.classList.contains("view-label__statement--sentence"),
          line.querySelector(".status-text__state"),
        ];
      }),
    ).toEqual(reasons.map(() => [true, null]));
  });

  it("tracks ATMOSPHERE: APPROXIMATE itself as a state", () => {
    renderPlate();
    expect(
      screen
        .getByText(statusLine("ATMOSPHERE: APPROXIMATE"))
        .classList.contains("view-label__statement--sentence"),
    ).toBe(false);
  });
});
