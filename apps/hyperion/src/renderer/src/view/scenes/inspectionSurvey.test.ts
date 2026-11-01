import {
  decodeServerMessage,
  encodeClientMessage,
  galacticPositionFromLy,
  RequestClient,
} from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { toSystemBodiesModel } from "../../lib/system/bodiesWire";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import {
  FIXTURE_EARTH,
  FIXTURE_SYSTEM,
  FIXTURE_UNIVERSE,
  sliceBodies,
  sliceBodiesWith,
} from "../../test/planetaryFixture";
import type { BodyInspection } from "./inspectionScene";
import { INSPECTION_SURVEY, surveyInspected } from "./inspectionSurvey";

/** The display's time the inspection opens at: an hour after the epoch. */
const OPENED = { seconds: 3_600, nanos: 0 };

/** A request client over a fake socket the test plays the server on, through the wire's codec. */
function linked(): { readonly client: RequestClient; readonly socket: FakeWebSocket } {
  const socket = new FakeWebSocket("ws://127.0.0.1:7878/ws");
  socket.binaryType = "arraybuffer";
  const client = new RequestClient((message) => {
    socket.send(encodeClientMessage(message));
    return true;
  });
  socket.addEventListener("message", (event: Event) => {
    if (event instanceof MessageEvent && typeof event.data === "string") {
      client.handleServerMessage(decodeServerMessage(event.data));
    }
  });
  socket.serverOpens();
  return { client, socket };
}

/** The slice's Earth's inspection, from `answer`, as `SYSTEM` makes one. */
function earthInspection(answer: ReturnType<typeof sliceBodies>): BodyInspection {
  const result = toSystemBodiesModel(answer, "H7K 4C0RFZ D-7");
  if (result.kind !== "ok") {
    throw new Error(`the slice's answer is unusable: ${result.fault}`);
  }
  return {
    universe: FIXTURE_UNIVERSE,
    place: {
      kind: "charted",
      system: FIXTURE_SYSTEM,
      designation: "H7K 4C0RFZ D-7",
      barycentre: galacticPositionFromLy([26_000, 0, 12]),
    },
    model: result.model,
    bodies: result.bodies,
    body: FIXTURE_EARTH,
    time: OPENED,
  };
}

describe("the body inspection's survey (the owner's request)", () => {
  it("surveys the whole body in one pass, orbital at 50 m, at the inspection's time", async () => {
    const { client, socket } = linked();
    const surveyed = surveyInspected(client, earthInspection(sliceBodies()));
    socket.serverAnswers("survey_pass", () => ({ kind: "survey_pass", revision: 3 }));

    // The Earth's ∛(a² c) is 6,371.0 km: level 8, 6 · 4⁸ cells.
    expect(await surveyed).toEqual({
      kind: "recorded",
      body: FIXTURE_EARTH,
      level: 8,
      revision: 3,
    });
    expect(socket.requestsOfKind("survey_pass").map(({ body }) => body)).toEqual([
      {
        kind: "survey_pass",
        universe: FIXTURE_UNIVERSE,
        body: FIXTURE_EARTH,
        from: OPENED,
        to: OPENED,
        source: INSPECTION_SURVEY.source,
        resolution_m: 50,
        cells: [{ start: 0, end: 393_216 }],
      },
    ]);
  });

  it("gives the server's refusal", async () => {
    const { client, socket } = linked();
    const surveyed = surveyInspected(client, earthInspection(sliceBodies()));
    const [asked] = socket.requestsOfKind("survey_pass");
    if (asked === undefined) {
      throw new Error("the survey was not asked");
    }
    socket.serverRejects(asked.id, {
      code: "unknown_body",
      message: "no such body",
      field: "body",
    });

    expect(await surveyed).toEqual({
      kind: "refused",
      body: FIXTURE_EARTH,
      reason: "no such body",
      standing: "refused",
    });
  });

  it("gives the server's own failure as a fault", async () => {
    const { client, socket } = linked();
    const surveyed = surveyInspected(client, earthInspection(sliceBodies()));
    const [asked] = socket.requestsOfKind("survey_pass");
    if (asked === undefined) {
      throw new Error("the survey was not asked");
    }
    socket.serverRejects(asked.id, { code: "storage_failed", message: "disk full", field: null });

    expect(await surveyed).toEqual({
      kind: "refused",
      body: FIXTURE_EARTH,
      reason: "disk full",
      standing: "fault",
    });
  });

  it("asks nothing of a body whose record holds no figure to give its level", async () => {
    const { client, socket } = linked();

    const figureless = sliceBodiesWith(({ figure: _figure, ...summary }) => summary);

    expect(await surveyInspected(client, earthInspection(figureless))).toEqual({
      kind: "refused",
      body: FIXTURE_EARTH,
      reason: "its record holds no figure to give its level",
      standing: "refused",
    });
    expect(socket.requestsOfKind("survey_pass")).toEqual([]);
  });
});
