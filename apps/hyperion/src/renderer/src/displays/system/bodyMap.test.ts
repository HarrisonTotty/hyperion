import type { ResponseFor, UniverseTime } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { composePosition, positionAt } from "../../lib/orbit";
import { toSystemBodiesModel } from "../../lib/system/bodiesWire";
import {
  BODY_MIN_SIZE_CLASS,
  CONTACT_SIZE_CLASS,
  MOON_SIZE_CLASS,
} from "../../lib/system/bodySymbols";
import { layoutHierarchy, orbitNormal } from "../../lib/system/hierarchy";
import type { SizeClass, SymbolShape } from "../../spatial/marks";
import { SIZE_CLASS_REM, SYMBOL_STROKE_PX, symbolOutline } from "../../spatial/symbols";
import { dot, norm, scale, sub, vec3 } from "../../spatial/vec3";
import {
  FIXTURE_EARTH,
  FIXTURE_JUPITER,
  FIXTURE_SYSTEM,
  populatedBodies,
  sliceBodies,
  sliceBodiesWith,
} from "../../test/planetaryFixture";
import { BANDS } from "../../test/systemFixtures";
import {
  ALL_ZONE_LAYERS,
  bodyMarks,
  bodyPaths,
  bodyReachesAu,
  habitableOuterAu,
  layoutBodies,
  primaryZone,
  zoneAnnuli,
} from "./bodyMap";
import { fitRadiiAu, orbitPlane, orbitScene } from "./orbitMap";
import { METRES_PER_AU } from "./orbitScale";

const TIME: UniverseTime = { seconds: 3_155_760_000, nanos: 0 };
const POSITION_LY = vec3(26_000, 0, 12);

function built(response: ResponseFor<"system_bodies"> = sliceBodies()) {
  const result = toSystemBodiesModel(response, "H7K 4C0RFZ D-7");
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  const { model, bodies } = result;
  const layout = layoutHierarchy(model.hierarchy, model.hosts);
  return { model, bodies, layout, bodiesLayout: layoutBodies(model.system, bodies.bodies, layout) };
}

/** How far inside a circle of the same size a symbol's flattest side lies at a size class, px. */
function flatDepthPx(shape: SymbolShape, sizeClass: SizeClass): number {
  const outline = symbolOutline(shape);
  if (outline.kind !== "polygon") {
    throw new Error(`${shape} is not a polygon`);
  }
  const radiusPx = (SIZE_CLASS_REM[sizeClass] * 16) / 2;
  const inner = Math.min(
    ...outline.points.slice(1).map((point, index) => {
      const previous = outline.points[index] ?? point;
      return Math.hypot((point.x + previous.x) / 2, (point.y + previous.y) / 2);
    }),
  );
  return radiusPx * (1 - inner);
}

describe("bodyMarks", () => {
  it("draws each planet where its orbit about its star puts it at the display time", () => {
    const { bodies, bodiesLayout, layout } = built();
    const marks = bodyMarks(bodies.bodies, bodiesLayout, TIME);

    expect(marks.map((mark) => mark.id)).toEqual([FIXTURE_EARTH, FIXTURE_JUPITER]);
    for (const [index, mark] of marks.entries()) {
      const orbit = bodies.bodies[index]?.orbit;
      if (orbit?.state !== "ok") {
        throw new Error("the slice's planets have orbits");
      }
      const starM = composePosition(layout.placements, `${FIXTURE_SYSTEM}.0000`, TIME);
      const expected = scale(positionAt(orbit.value.orbit, TIME), 1 / METRES_PER_AU);
      expect(norm(starM)).toBe(0);
      expect(norm(sub(mark.position, expected))).toBeLessThan(1e-12 * norm(expected));
    }
  });

  it("draws a planet as the inverted triangle, a giant at size class 3 and a rocky one at 1", () => {
    const { bodies, bodiesLayout } = built();

    expect(
      bodyMarks(bodies.bodies, bodiesLayout, TIME).map((mark) => [mark.shape, mark.sizeClass]),
    ).toEqual([
      ["triangle-down", 1],
      ["triangle-down", 3],
    ]);
  });

  it("leaves destroyed, unbound and unformed bodies and populations undrawn", () => {
    const { bodies, bodiesLayout } = built(populatedBodies());

    expect(
      bodyMarks(bodies.bodies, bodiesLayout, TIME).map((mark) => [
        mark.id.slice(17),
        mark.shape,
        mark.sizeClass,
      ]),
    ).toEqual([
      ["0100", "triangle-down", 1],
      ["0101", "pentagon", MOON_SIZE_CLASS],
      ["e001", "triangle-down", BODY_MIN_SIZE_CLASS],
    ]);
  });

  it("puts a belt's member on its orbit about the belt's star", () => {
    const { bodies, bodiesLayout } = built(populatedBodies());
    const member = bodies.bodies.find((body) => body.kind.kind === "dwarf_planet");
    const mark = bodyMarks(bodies.bodies, bodiesLayout, TIME).find((m) => m.id === member?.id);
    if (member?.orbit.state !== "ok" || mark === undefined) {
      throw new Error("the populated answer's dwarf planet is drawn on its orbit");
    }

    expect(bodiesLayout.parentKeys.get(member.id)).toBe(`${FIXTURE_SYSTEM}.0000`);
    const expected = scale(positionAt(member.orbit.value.orbit, TIME), 1 / METRES_PER_AU);
    expect(norm(sub(mark.position, expected))).toBeLessThan(1e-12 * norm(expected));
  });

  it("stands a contact, whose orbit is withheld, where the server put it", () => {
    const response = sliceBodiesWith((body) => ({
      ...body,
      kind: { type: "unresolved" },
      orbit: { state: "not_resolved" },
      bulk: { state: "not_resolved" },
    }));
    const { bodies, bodiesLayout } = built(response);
    const [first] = bodyMarks(bodies.bodies, bodiesLayout, TIME);
    const positionM = bodies.bodies[0]?.positionM;
    if (first === undefined || positionM === null || positionM === undefined) {
      throw new Error("the contact is drawn at its position");
    }

    expect(first.shape).toBe("hexagon");
    expect(first.sizeClass).toBe(CONTACT_SIZE_CLASS);
    expect(first.position).toEqual(scale(positionM, 1 / METRES_PER_AU));
  });
});

describe("the body symbols' sizes", () => {
  it("draw the moon's pentagon and the contact's hexagon where they read apart from a circle", () => {
    // Apart: the flattest side lies at least half the outline's width inside the circle.
    const apart = SYMBOL_STROKE_PX / 2;

    expect(flatDepthPx("pentagon", MOON_SIZE_CLASS)).toBeGreaterThanOrEqual(apart);
    expect(flatDepthPx("hexagon", CONTACT_SIZE_CLASS)).toBeGreaterThanOrEqual(apart);
    expect(flatDepthPx("hexagon", BODY_MIN_SIZE_CLASS)).toBeLessThan(apart);
    expect(MOON_SIZE_CLASS).toBeGreaterThanOrEqual(BODY_MIN_SIZE_CLASS);
  });
});

describe("bodyPaths", () => {
  it("draws each planet's orbit about its star, the selected one's alone as selected", () => {
    const { bodies, bodiesLayout } = built();
    const paths = bodyPaths(bodies.bodies, bodiesLayout, TIME, FIXTURE_JUPITER);

    expect(paths.map((path) => [path.id, path.role])).toEqual([
      [FIXTURE_EARTH, "reference"],
      [FIXTURE_JUPITER, "selected"],
    ]);
    const orbit = bodies.bodies[0]?.orbit;
    if (orbit?.state !== "ok") {
      throw new Error("the Earth has an orbit");
    }
    // Every point of the Earth's path lies in its orbit's plane about the star.
    const normal = orbitNormal(orbit.value.orbit);
    for (const point of paths[0]?.points ?? []) {
      expect(Math.abs(dot(point, normal))).toBeLessThan(1e-9);
    }
  });

  it("draws no orbit for a body not present, and none for a moon at the system's scale", () => {
    const { bodies, bodiesLayout } = built(populatedBodies());

    expect(
      bodyPaths(bodies.bodies, bodiesLayout, TIME, null).map((path) => path.id.slice(17)),
    ).toEqual(["0100", "e001"]);
  });
});

describe("zoneAnnuli", () => {
  it("draws the snow line and the conservative habitable zone about the star, labelled", () => {
    const { bodies, layout } = built();

    expect(
      zoneAnnuli(bodies.zones, FIXTURE_SYSTEM, layout, TIME, ALL_ZONE_LAYERS).map((annulus) => [
        annulus.label,
        annulus.innerRadius * METRES_PER_AU,
        annulus.outerRadius * METRES_PER_AU,
        annulus.ticks,
      ]),
    ).toEqual([
      ["SNOW LINE", 337_938_907_867.118_65, 337_938_907_867.118_65, false],
      ["HABITABLE ZONE", 148_000_000_000, 253_000_000_000, false],
    ]);
  });

  it("draws a companion-bounded stable zone, and leaves out each layer switched off", () => {
    const slice = sliceBodies();
    const [zone] = slice.zones;
    if (zone === undefined) {
      throw new Error("the slice has a zone");
    }
    const { bodies, layout } = built({ ...slice, zones: [{ ...zone, outer_m: 4e12 }] });

    expect(
      zoneAnnuli(bodies.zones, FIXTURE_SYSTEM, layout, TIME, ALL_ZONE_LAYERS).map((a) => a.label),
    ).toEqual(["STABLE ZONE", "SNOW LINE", "HABITABLE ZONE"]);
    expect(
      zoneAnnuli(bodies.zones, FIXTURE_SYSTEM, layout, TIME, {
        ...ALL_ZONE_LAYERS,
        habitable: false,
      }).map((a) => a.label),
    ).toEqual(["STABLE ZONE", "SNOW LINE"]);
  });

  it("draws no habitable zone about a host with no light", () => {
    const slice = sliceBodies();
    const [zone] = slice.zones;
    if (zone?.habitable_zone === undefined || zone.habitable_zone === null) {
      throw new Error("the slice's zone has a habitable zone");
    }
    const dark = {
      recent_venus_m: 0,
      runaway_greenhouse_m: 0,
      moist_greenhouse_m: 0,
      maximum_greenhouse_m: 0,
      early_mars_m: 0,
      extrapolated: false,
    };
    const { bodies, layout } = built({ ...slice, zones: [{ ...zone, habitable_zone: dark }] });

    expect(
      zoneAnnuli(bodies.zones, FIXTURE_SYSTEM, layout, TIME, ALL_ZONE_LAYERS).map((a) => a.label),
    ).toEqual(["SNOW LINE"]);
  });
});

describe("the zoom presets with bodies", () => {
  it("fit the habitable zone's outer limit at INNER and the outermost planet at ALL", () => {
    const { model, bodies, layout, bodiesLayout } = built();
    const zone = primaryZone(bodies.zones, FIXTURE_SYSTEM, layout);
    const fit = fitRadiiAu(layout, model.hosts, {
      bodyReachesAu: bodyReachesAu(bodies.bodies, bodiesLayout),
      habitableOuterAu: habitableOuterAu(zone, FIXTURE_SYSTEM, layout),
    });

    expect(fit.inner).toBeCloseTo(253_000_000_000 / METRES_PER_AU, 12);
    expect(fit.all).toBeCloseTo((777_908_927_640 * 1.0489) / METRES_PER_AU, 12);
  });

  it("fit the fifth body at INNER when it lies inside the habitable zone's outer limit", () => {
    const { model, layout } = built();
    const reaches = [0.02, 0.03, 0.05, 0.07, 0.09, 0.12];

    expect(
      fitRadiiAu(layout, model.hosts, { bodyReachesAu: reaches, habitableOuterAu: 0.2 }),
    ).toEqual({ inner: 0.09, all: 0.12 });
  });

  it("fit the habitable zone at both when there are no planets", () => {
    const { model, layout } = built();

    expect(fitRadiiAu(layout, model.hosts, { bodyReachesAu: [], habitableOuterAu: 1.7 })).toEqual({
      inner: 1.7,
      all: 1.7,
    });
  });
});

describe("orbitScene with bodies", () => {
  it("draws the map on the server's system plane, with the planets, orbits and zones", () => {
    const { model, bodies, layout, bodiesLayout } = built();
    const plane = orbitPlane(layout, POSITION_LY, bodies.systemPlane);
    const scene = orbitScene({
      hosts: model.hosts,
      layout,
      plane,
      time: TIME,
      selectedId: FIXTURE_EARTH,
      bands: BANDS,
      fitRadiusAu: 6,
      bodies: { system: FIXTURE_SYSTEM, bodies, layout: bodiesLayout, zoneLayers: ALL_ZONE_LAYERS },
    });

    expect(plane.name).toBe("SYSTEM PLANE");
    expect(
      norm(
        sub(
          plane.frame.north,
          orbitNormal(bodies.systemPlane ?? { inclinationRad: 0, ascendingNodeRad: 0 }),
        ),
      ),
    ).toBeLessThan(1e-12);
    expect(scene.points.map((mark) => mark.shape)).toEqual([
      "circle",
      "triangle-down",
      "triangle-down",
    ]);
    expect(scene.selectedId).toBe(FIXTURE_EARTH);
    expect(scene.paths?.filter((path) => path.role === "selected").map((path) => path.id)).toEqual([
      FIXTURE_EARTH,
    ]);
    expect(scene.annuli?.map((annulus) => annulus.label)).toEqual(["SNOW LINE", "HABITABLE ZONE"]);
  });
});
