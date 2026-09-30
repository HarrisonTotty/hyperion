/**
 * The shared wire fixture of plan 14's bodies, `packages/protocol/fixtures/planetary.json`, as the
 * client decodes it, and answers built like it (P14.T37).
 *
 * @remarks
 * The Rust wire-form tests of `hyperion-protocol` require every message of the file to round-trip
 * exactly, so the client is tested against what the server is pinned to send; each message is
 * decoded through `decodeServerMessage`, as a frame is. Builders make the variations a test needs
 * from those answers, and say what they change.
 */
import {
  type BodyRecordDto,
  type BodySummaryDto,
  decodeServerMessage,
  type RequestKind,
  type ResponseBody,
  type ResponseFor,
} from "@hyperion/protocol";

import fixture from "../../../../../../packages/protocol/fixtures/planetary.json" with { type: "json" };

/** The name of a response of the fixture. */
type ResponseName =
  | "system_bodies_response"
  | "system_bodies_populated"
  | "body_detail_response"
  | "body_detail_mass_and_orbit"
  | "body_detail_contact";

/** The fixture's system, which every body ID extends. */
export const FIXTURE_SYSTEM = "0200080020000000";

/** The fixture's universe. */
export const FIXTURE_UNIVERSE = "000000000000002a";

/** The slice's rocky planet, an Earth at 1 AU, and its gas giant, a Jupiter. */
export const FIXTURE_EARTH = `${FIXTURE_SYSTEM}.0300`;
export const FIXTURE_JUPITER = `${FIXTURE_SYSTEM}.0500`;

function isResponseOf<K extends RequestKind>(body: ResponseBody, kind: K): body is ResponseFor<K> {
  return body.kind === kind;
}

/** The fixture's response `name`, decoded as the client decodes a frame, as a response of `kind`. */
function response<K extends RequestKind>(name: ResponseName, kind: K): ResponseFor<K> {
  const message = decodeServerMessage(JSON.stringify(fixture[name]));
  if (message.type !== "response" || !isResponseOf(message.body, kind)) {
    throw new Error(`the fixture's ${name} is not a ${kind} response`);
  }
  return message.body;
}

/** The slice's answer: one Sun-like star, its zone, an Earth and a Jupiter. */
export function sliceBodies(): ResponseFor<"system_bodies"> {
  return response("system_bodies_response", "system_bodies");
}

/** The answer that exercises every kind, state and section state, at the `bulk` level. */
export function populatedBodies(): ResponseFor<"system_bodies"> {
  return response("system_bodies_populated", "system_bodies");
}

/** The slice's Earth at the `full` level. */
export function earthDetail(): ResponseFor<"body_detail"> {
  return response("body_detail_response", "body_detail");
}

/** The slice's Earth at the `mass_and_orbit` level. */
export function earthMassAndOrbit(): ResponseFor<"body_detail"> {
  return response("body_detail_mass_and_orbit", "body_detail");
}

/** The slice's Earth at the `contact` level. */
export function earthContact(): ResponseFor<"body_detail"> {
  return response("body_detail_contact", "body_detail");
}

/**
 * The slice's Jupiter as a whole record at the `full` level: its list entry with a surface that
 * does not apply, as a gas giant's does not, and hooks not modelled, like the Earth's.
 */
export function jupiterDetail(): ResponseFor<"body_detail"> {
  const jupiter = sliceBodies().bodies.find((body) => body.id === FIXTURE_JUPITER);
  if (jupiter === undefined) {
    throw new Error("the slice's answer lists its Jupiter");
  }
  const record: BodyRecordDto = {
    ...jupiter,
    surface: { state: "not_applicable" },
    hooks: { state: "not_modelled" },
  };
  return { ...earthDetail(), record };
}

/** The slice's answer with its bodies changed by `change`, as a later generator might send it. */
export function sliceBodiesWith(
  change: (body: BodySummaryDto) => BodySummaryDto,
): ResponseFor<"system_bodies"> {
  const slice = sliceBodies();
  return { ...slice, bodies: slice.bodies.map(change) };
}

/** The populated answer with its bodies changed by `change`, as a later generator might send it. */
export function populatedBodiesWith(
  change: (body: BodySummaryDto) => BodySummaryDto,
): ResponseFor<"system_bodies"> {
  const populated = populatedBodies();
  return { ...populated, bodies: populated.bodies.map(change) };
}
