/** The slice's Earth as `SYSTEM`'s `VIEW BODY` opens it: a body inspection at the epoch. */
import { galacticPositionFromLy } from "@hyperion/protocol";

import { toSystemBodiesModel } from "../lib/system/bodiesWire";
import type { BodyInspection } from "../view/scenes/inspectionScene";
import {
  FIXTURE_EARTH,
  FIXTURE_SYSTEM,
  FIXTURE_UNIVERSE,
  populatedBodies,
  sliceBodies,
} from "./planetaryFixture";

/** The slice's designation, as `SYSTEM` would carry it from the chart. */
export const SLICE_DESIGNATION = "H7K 4C0RFZ D-7";

/** The populated answer's moon `A b I`, `/257`, of its planet `A b`, `/256`. */
export const FIXTURE_MOON = `${FIXTURE_SYSTEM}.0101`;

/** The populated answer's planet `A b`, `/256`, the moon's. */
export const FIXTURE_MOONS_PLANET = `${FIXTURE_SYSTEM}.0100`;

/** The slice's Earth's body inspection at the epoch, from a charted place. */
export function sliceEarthInspection(): BodyInspection {
  return inspectionOf(sliceBodies(), FIXTURE_EARTH);
}

/** The populated answer's moon's body inspection at the epoch, from the same charted place. */
export function populatedMoonInspection(): BodyInspection {
  return inspectionOf(populatedBodies(), FIXTURE_MOON);
}

/** `body`'s inspection in `answer` at the epoch, from the slice's charted place. */
function inspectionOf(
  answer: ReturnType<typeof sliceBodies>,
  body: BodyInspection["body"],
): BodyInspection {
  const result = toSystemBodiesModel(answer, SLICE_DESIGNATION);
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
    body,
    time: { seconds: 0, nanos: 0 },
  };
}
