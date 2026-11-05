/** The slice's Earth as `SYSTEM`'s `VIEW BODY` opens it: a body inspection at the epoch. */
import { galacticPositionFromLy } from "@hyperion/protocol";

import { toSystemBodiesModel } from "../lib/system/bodiesWire";
import type { BodyInspection } from "../view/scenes/inspectionScene";
import { FIXTURE_EARTH, FIXTURE_SYSTEM, FIXTURE_UNIVERSE, sliceBodies } from "./planetaryFixture";

/** The slice's designation, as `SYSTEM` would carry it from the chart. */
export const SLICE_DESIGNATION = "H7K 4C0RFZ D-7";

/** The slice's Earth's body inspection at the epoch, from a charted place. */
export function sliceEarthInspection(): BodyInspection {
  const result = toSystemBodiesModel(sliceBodies(), SLICE_DESIGNATION);
  if (result.kind !== "ok") {
    throw new Error(`the slice's answer is unusable: ${result.fault}`);
  }
  return {
    universe: FIXTURE_UNIVERSE,
    place: {
      kind: "charted",
      system: FIXTURE_SYSTEM,
      designation: SLICE_DESIGNATION,
      barycentre: galacticPositionFromLy([26_000, 0, 12]),
    },
    model: result.model,
    bodies: result.bodies,
    body: FIXTURE_EARTH,
    time: { seconds: 0, nanos: 0 },
  };
}
