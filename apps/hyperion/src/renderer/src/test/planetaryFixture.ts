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
  type BodyRotationDto,
  type BodySummaryDto,
  decodeServerMessage,
  type RequestKind,
  type ResponseBody,
  type ResponseFor,
} from "@hyperion/protocol";

import fixture from "../../../../../../packages/protocol/fixtures/planetary.json" with { type: "json" };
import { add, cross, scale, vec3, type Vec3 } from "../geometry/vec3";
import { orbitNormal } from "../lib/system/hierarchy";

/** The name of a response of the fixture. */
type ResponseName =
  | "system_bodies_response"
  | "system_bodies_populated"
  | "body_detail_response"
  | "body_detail_jupiter"
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

/**
 * The slice's Earth at the `full` level: its surface `ok` (P14.T35.e, illustrative values) and its
 * envelope not applicable.
 */
export function earthDetail(): ResponseFor<"body_detail"> {
  return response("body_detail_response", "body_detail");
}

/**
 * The slice's Earth at the `full` level as a server before P14.T35.e sent it: its surface not
 * modelled, and no `envelope` key.
 */
export function earthDetailOfAnOlderServer(): ResponseFor<"body_detail"> {
  const detail = earthDetail();
  const { envelope: _envelope, ...older } = detail.record;
  return { ...detail, record: { ...older, surface: { state: "not_modelled" } } };
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
 * does not apply, as a gas giant's does not, its envelope `ok` (P14.T35.e, illustrative values),
 * and hooks not modelled, like the Earth's.
 */
export function jupiterDetail(): ResponseFor<"body_detail"> {
  return response("body_detail_jupiter", "body_detail");
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

/**
 * A change, for {@link populatedBodiesWith}, that gives the populated answer's ring the plane's
 * normal `normal` in place of the shared fixture's own, which every server from the 22 → 23
 * batch sends (decision-p14-t56a §1).
 */
export function withRingNormal(
  normal: readonly [number, number, number],
): (body: BodySummaryDto) => BodySummaryDto {
  return (body) =>
    body.population.state === "ok" && body.population.value.type === "ring"
      ? {
          ...body,
          population: { state: "ok", value: { ...body.population.value, normal: [...normal] } },
        }
      : body;
}

/**
 * A change, for {@link populatedBodiesWith}, that leaves the populated answer's ring without its
 * plane's normal, as a server before the 22 → 23 batch sends it, whose rings lie in their planet's
 * orbital plane; the field stays optional on the wire (decision-p14-t56a §1).
 */
export function withoutRingNormal(body: BodySummaryDto): BodySummaryDto {
  if (body.population.state !== "ok" || body.population.value.type !== "ring") {
    return body;
  }
  const { normal: _sent, ...ring } = body.population.value;
  return { ...body, population: { state: "ok", value: ring } };
}

/** A change, for an answer's builder, that gives the body `id` the rotation law `rotation`. */
export function withBodyRotation(
  id: string,
  rotation: BodyRotationDto,
): (body: BodySummaryDto) => BodySummaryDto {
  return (body) =>
    body.id === id ? { ...body, rotation: { state: "ok", value: rotation } } : body;
}

/** A vector's components as the wire gives them. */
function onWire(v: Vec3): [number, number, number] {
  return [v.x, v.y, v.z];
}

/** A pole tilted from a body's orbit, with the rotation law that states it. */
export interface TiltedPole {
  /** The slice Earth's law, its axes and obliquity replaced. */
  readonly rotation: BodyRotationDto;
  /** The body's orbit normal, a unit vector along the galactic axes. */
  readonly orbitNormal: Vec3;
  /** The pole, a unit vector along the galactic axes. */
  readonly pole: Vec3;
  /**
   * The unit vector at `fromNormalRad` from the orbit normal, turned towards the pole in the plane
   * of the two, as P14.T56.a's local Laplace pole lies: the pole at the obliquity.
   */
  readonly turned: (fromNormalRad: number) => Vec3;
}

/**
 * A rotation law for the body `id` of `answer` whose pole stands `obliquityRad` from its orbit's
 * normal, turned about the orbit's ascending node N̂ by the right-hand rule, so that the equator's
 * ascending node on the orbital plane, ĥ × p̂ normalised as the simulation takes it
 * (`planetary/frames.rs`), is N̂; the rest of the law is the slice Earth's.
 *
 * @throws Error when the body is not in the answer or has no orbit.
 */
export function tiltedPole(
  answer: ResponseFor<"system_bodies">,
  id: string,
  obliquityRad: number,
): TiltedPole {
  const template = sliceBodies().bodies.find((body) => body.id === FIXTURE_EARTH)?.rotation;
  const body = answer.bodies.find((each) => each.id === id);
  if (template?.state !== "ok" || body?.orbit.state !== "ok") {
    throw new Error(`the answer's ${id} has an orbit, and the slice's Earth a rotation law`);
  }
  const { inclination_rad: inclination, ascending_node_rad: node } = body.orbit.value.orbit;
  const normal = orbitNormal({ inclinationRad: inclination, ascendingNodeRad: node });
  const ascending = vec3(Math.cos(node), Math.sin(node), 0);
  // ĥ turned by an angle about N̂: cos ĥ + sin (N̂ × ĥ).
  const towards = cross(ascending, normal);
  const turned = (fromNormalRad: number): Vec3 =>
    add(scale(normal, Math.cos(fromNormalRad)), scale(towards, Math.sin(fromNormalRad)));
  const pole = turned(obliquityRad);
  return {
    rotation: {
      ...template.value,
      pole: onWire(pole),
      equator_node: onWire(ascending),
      equator_quarter: onWire(cross(pole, ascending)),
      obliquity_rad: obliquityRad,
    },
    orbitNormal: normal,
    pole,
    turned,
  };
}
