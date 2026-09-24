import type { SystemSummaryDto, UniverseTime } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import {
  aSingleStarSummary,
  aSystemSummary,
  aTripleSummary,
  PIN_SYSTEM,
} from "../../test/systemFixtures";
import { add, dot, norm, scale, sub } from "../../spatial/vec3";
import { composePosition, positionAt } from "../orbit";
import { layoutHierarchy, orbitNormal } from "./hierarchy";
import type { SystemModel } from "./model";
import { toSystemModel } from "./wire";

const STAR_0 = `${PIN_SYSTEM}.0000`;
const STAR_1 = `${PIN_SYSTEM}.0001`;
const STAR_2 = `${PIN_SYSTEM}.0002`;
const TIME: UniverseTime = { seconds: 123_456_789, nanos: 0 };

function modelOf(summary: SystemSummaryDto): SystemModel {
  const result = toSystemModel(summary, "H7K 4C0RFZ D-7");
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  return result.model;
}

function layoutOf(summary: SystemSummaryDto) {
  const model = modelOf(summary);
  return { model, layout: layoutHierarchy(model.hierarchy, model.hosts) };
}

describe("layoutHierarchy", () => {
  it("puts a single star at the barycentre, with no orbit and no plane", () => {
    const { layout } = layoutOf(aSingleStarSummary());

    expect(composePosition(layout.placements, STAR_0, TIME)).toEqual({ x: 0, y: 0, z: 0 });
    expect(layout.members).toEqual([]);
    expect(layout.primaryPairNormal).toBeNull();
    expect(layout.reachM.get(STAR_0)).toBe(0);
  });

  it("places a binary's stars about their barycentre by their masses", () => {
    const { model, layout } = layoutOf(aSystemSummary());
    const pair = model.hierarchy[0];
    if (pair?.kind !== "pair") {
      throw new Error("the pinned binary's root is a pair");
    }
    const a = composePosition(layout.placements, STAR_0, TIME);
    const b = composePosition(layout.placements, STAR_1, TIME);

    // The white dwarf's 2.5 M☉ at birth and its companion's 1 M☉, as the hierarchy places them.
    const apartM = norm(positionAt(pair.orbit, TIME));
    expect(norm(add(scale(a, 2.5), scale(b, 1)))).toBeLessThan(1e-12 * apartM);
    expect(norm(sub(sub(b, a), positionAt(pair.orbit, TIME)))).toBeLessThan(1e-12 * apartM);
  });

  it("reaches each binary star's share of the orbit's apoapsis distance", () => {
    const { layout } = layoutOf(aSystemSummary());
    const apoapsisM = 3_515_625_000_000 * 1.5;

    expect(layout.reachM.get(STAR_0)).toBeCloseTo((1 / 3.5) * apoapsisM, 0);
    expect(layout.reachM.get(STAR_1)).toBeCloseTo((2.5 / 3.5) * apoapsisM, 0);
  });

  it("keys the pair's orbit to the star it brought in, and none to the primary", () => {
    const { model, layout } = layoutOf(aSystemSummary());
    const pair = model.hierarchy[0];

    expect(layout.keyedOrbits.get(STAR_1)).toBe(pair?.kind === "pair" ? pair.orbit : null);
    expect(layout.keyedOrbits.has(STAR_0)).toBe(false);
  });

  it("takes the plane of a binary from its orbit's angular momentum", () => {
    const { model, layout } = layoutOf(aSystemSummary());
    const pair = model.hierarchy[0];
    if (pair?.kind !== "pair" || layout.primaryPairNormal === null) {
      throw new Error("the pinned binary has a plane");
    }
    const position = positionAt(pair.orbit, TIME);
    const later = positionAt(pair.orbit, { seconds: TIME.seconds + 86_400, nanos: 0 });
    const momentum = {
      x: position.y * later.z - position.z * later.y,
      y: position.z * later.x - position.x * later.z,
      z: position.x * later.y - position.y * later.x,
    };

    expect(dot(momentum, layout.primaryPairNormal) / norm(momentum)).toBeCloseTo(1, 9);
    expect(norm(orbitNormal(pair.orbit))).toBeCloseTo(1, 12);
  });

  describe("for the pinned hierarchical triple", () => {
    it("places the close pair's barycentre on the wide orbit, and its stars about it", () => {
      const { layout } = layoutOf(aTripleSummary());
      const a = composePosition(layout.placements, STAR_0, TIME);
      const b = composePosition(layout.placements, STAR_1, TIME);
      const c = composePosition(layout.placements, STAR_2, TIME);
      const closePair = scale(add(scale(a, 1.2), scale(b, 0.5)), 1 / 1.7);

      const apartM = norm(sub(c, closePair));
      expect(norm(add(scale(closePair, 1.7), scale(c, 0.29)))).toBeLessThan(1e-12 * apartM);
      expect(norm(sub(closePair, composePosition(layout.placements, "pair:1", TIME)))).toBeLessThan(
        1e-12 * apartM,
      );
    });

    it("keys each orbit to the first star of its outer member", () => {
      const { model, layout } = layoutOf(aTripleSummary());
      const [wide, close] = model.hierarchy;

      expect(layout.keyedOrbits.get(STAR_2)).toBe(wide?.kind === "pair" ? wide.orbit : null);
      expect(layout.keyedOrbits.get(STAR_1)).toBe(close?.kind === "pair" ? close.orbit : null);
    });

    it("takes the plane from the innermost pair that holds the primary", () => {
      const { model, layout } = layoutOf(aTripleSummary());
      const close = model.hierarchy[1];

      expect(layout.primaryPairNormal).toEqual(
        close?.kind === "pair" ? orbitNormal(close.orbit) : null,
      );
    });

    it("lists a member for each side of each pair, the inner pair's barycentre among them", () => {
      const { layout } = layoutOf(aTripleSummary());

      expect(layout.members.map((member) => [member.key, member.hostId])).toEqual([
        ["pair:1", null],
        [STAR_2, STAR_2],
        [STAR_0, STAR_0],
        [STAR_1, STAR_1],
      ]);
    });
  });
});
