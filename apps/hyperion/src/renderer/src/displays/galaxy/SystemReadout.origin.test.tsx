/**
 * The system readout's readings of what placed the selected system (plan 09, P09.T41.c): its
 * `ORIGIN`, a member's `MEMBER CLASS` and a supernova entry's state, and the plan's three readings
 * of a value this build does not know, each cast at the test's boundary as the decoder trusts it.
 */
import type {
  CatalogueClassWire,
  MemberClassWire,
  PlacementKind,
  SupernovaStateWire,
  SystemOriginWire,
  SystemRecord,
} from "@hyperion/protocol";
import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { toChartFeatures } from "../../lib/galaxy/featureWire";
import type { ChartFeature, ChartSystem } from "../../lib/galaxy/model";
import { toChartResult } from "../../lib/galaxy/wire";
import { localFrameAt } from "../../geometry/frame";
import { vec3 } from "../../geometry/vec3";
import { aFeaturesInRange, OPEN_CLUSTER } from "../../test/featureFixtures";
import { anUnknownLayer, anUnknownPopulation, aSystemsInRange } from "../../test/galaxyFixtures";
import { SystemReadout } from "./SystemReadout";
import { distanceUnitFor } from "./chartModel";

const CENTRE = [26_000, 0, 0] as const;
const FRAME = localFrameAt(vec3(...CENTRE));

/**
 * An origin this build does not know, typed as the decoder types it: the enum grows (plan 04's
 * reserved list), and pinned content's would come as this one.
 */
function anUnknownOrigin(): SystemOriginWire {
  const pinned: unknown = { type: "pinned" };
  // The decoder's own trust at the wire (`decodeServerMessage`), made where the test plays the
  // server.
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return pinned as SystemOriginWire;
}

/** A catalogue class this build does not know, typed as the decoder types it. */
function anUnknownClass(): CatalogueClassWire {
  // The decoder's own trust at the wire, made where the test plays the server.
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return "magnetar" as CatalogueClassWire;
}

/** One system 1 ly out, its row carrying what `row` adds, through the chart's adapter. */
function aSystem(
  row: Partial<
    Pick<SystemRecord, "origin" | "member_class" | "supernova" | "population" | "placement">
  >,
) {
  const response = aSystemsInRange({
    centreLy: CENTRE,
    systems: [{ relLy: [1, 0, 0], layer: "c" }],
  });
  const [record] = response.systems;
  if (record === undefined) {
    throw new Error("the fixture built no system");
  }
  const [system] = toChartResult({ ...response, systems: [{ ...record, ...row }] }).systems;
  if (system === undefined) {
    throw new Error("the adapter kept no system");
  }
  return system;
}

/** The golden's open cluster, as the chart holds it. */
const CLUSTER: ChartFeature | undefined = toChartFeatures(
  aFeaturesInRange({ features: [OPEN_CLUSTER] }),
).features[0];

function renderReadout(system: ChartSystem) {
  render(
    <SystemReadout
      system={system}
      frame={FRAME}
      driveRangeLy={50}
      timeYr={0}
      distanceUnit={distanceUnitFor(50)}
      stars={null}
      stale={false}
      starsStale={false}
      extinction={null}
      features={CLUSTER === undefined ? [] : [CLUSTER]}
    />,
  );
  return screen.getByRole("status", { name: "Selected system" });
}

/** The value beside a label in the readout, or `undefined` where the readout has no such row. */
function valueOf(readout: HTMLElement, label: string): string | null | undefined {
  const term = within(readout).queryAllByText(label, { selector: "dt" })[0];
  return term === undefined ? undefined : (term.nextElementSibling?.textContent ?? null);
}

/** Every reading of the readout, label and value, in order. */
function readings(readout: HTMLElement): Array<[string | null, string | null]> {
  return [...readout.querySelectorAll("dt")].map((term) => [
    term.textContent,
    term.nextElementSibling?.textContent ?? null,
  ]);
}

/** A supernova's state as P09.T40 would send it once its gate opens. */
const SUPERNOVA: SupernovaStateWire = {
  type: "supernova",
  age_yr: 2_400,
  shell: {
    radius_ly: 30,
    shock_speed_km_s: 2_600,
    phase: "sedov_taylor",
    emission: { x_ray_and_radio: true, optical: true, neutral_hydrogen: false },
  },
  remnant: {
    offset_ly: 1.25,
    bow_shock: false,
    nebula: { power_w: 4.2e31 },
    survivor_offset_ly: null,
  },
};

const LISTED_UNKNOWN: MemberClassWire = { type: "listed", class: anUnknownClass() };

describe("SystemReadout's origin (plan 09, P09.T41.c)", () => {
  it("says the origin is not yet modelled for a row from a server that does not send it", () => {
    const readout = renderReadout(aSystem({}));

    expect(valueOf(readout, "ORIGIN")).toBe("NOT YET MODELLED");
    expect(valueOf(readout, "MEMBER CLASS")).toBeUndefined();
    expect(valueOf(readout, "SUPERNOVA")).toBeUndefined();
  });

  it("reads a grid system's origin as its birth population", () => {
    expect(valueOf(renderReadout(aSystem({ origin: { type: "grid" } })), "ORIGIN")).toBe(
      "OLD THIN DISC",
    );
  });

  it("names a feature's member by its feature's kind and designation, with its class", () => {
    const readout = renderReadout(
      aSystem({
        origin: { type: "feature_member", feature: OPEN_CLUSTER.id },
        member_class: {
          type: "cluster",
          kind: "living",
          band: "a",
          generation: "first",
          multiplicity: "binary",
        },
      }),
    );

    expect(valueOf(readout, "ORIGIN")).toBe("OC GPF F207");
    expect(valueOf(readout, "MEMBER CLASS")).toBe("LIVING · A · FIRST · BINARY");
  });

  it("names a member of a feature the chart does not hold by its feature's ID", () => {
    const readout = renderReadout(
      aSystem({ origin: { type: "feature_member", feature: "e859e05c80000000" } }),
    );

    expect(valueOf(readout, "ORIGIN")).toBe("FEATURE E859E05C80000000");
    // A member whose row carries no class reads it Missing.
    expect(valueOf(readout, "MEMBER CLASS")).toBe("—");
  });

  it("reads the centre's member and its episode and tracer", () => {
    const readout = renderReadout(
      aSystem({
        origin: { type: "centre_member" },
        member_class: {
          type: "centre",
          kind: "white_dwarf",
          band: "c",
          episode: 2,
          tracer: "stars",
        },
      }),
    );

    expect(valueOf(readout, "ORIGIN")).toBe("CENTRE");
    expect(valueOf(readout, "MEMBER CLASS")).toBe("WHITE DWARF · C · EPISODE 2 · STARS");
  });

  it("reads a catalogue system's class in words", () => {
    const readout = renderReadout(
      aSystem({ origin: { type: "catalogue_system", class: "xray_binary" } }),
    );

    expect(valueOf(readout, "ORIGIN")).toBe("X-RAY BINARY");
    expect(valueOf(readout, "MEMBER CLASS")).toBeUndefined();
  });

  it("reads a supernova entry's shell and what it left", () => {
    const readout = renderReadout(
      aSystem({
        origin: { type: "catalogue_system", class: "core_collapse" },
        supernova: SUPERNOVA,
      }),
    );

    expect(valueOf(readout, "ORIGIN")).toBe("CORE-COLLAPSE SUPERNOVA");
    expect(valueOf(readout, "SUPERNOVA")).toBe("REMNANT SHELL");
    expect(valueOf(readout, "SN AGE")).toBe("2.40 kyr");
    expect(valueOf(readout, "SHELL RADIUS")).toBe("30.0 ly");
    expect(valueOf(readout, "SHELL PHASE")).toBe("SEDOV-TAYLOR");
    expect(valueOf(readout, "SHELL EMISSION")).toBe("X-RAY AND RADIO · OPTICAL");
    expect(valueOf(readout, "REMNANT OFFSET")).toBe("1.25 ly");
    expect(valueOf(readout, "BOW SHOCK")).toBe("NO");
    expect(valueOf(readout, "PWN POWER")).toBe("4.20E31 W");
    expect(valueOf(readout, "SURVIVOR OFFSET")).toBeUndefined();
  });

  it("reads a progenitor's time to its explosion and a Type Ia pair's period", () => {
    const readout = renderReadout(
      aSystem({
        origin: { type: "catalogue_system", class: "type_ia" },
        supernova: { type: "progenitor", until_explosion_yr: 812, pair_period_s: 95.5 },
      }),
    );

    expect(valueOf(readout, "SUPERNOVA")).toBe("PROGENITOR");
    expect(valueOf(readout, "EXPLODES IN")).toBe("0.812 kyr");
    expect(valueOf(readout, "PAIR PERIOD")).toBe("1.11E-3 d");
  });
});

/** Each placement, and its words in a grid system's `ORIGIN` (the guide's nomenclature). */
const PLACEMENTS: ReadonlyArray<readonly [PlacementKind, string]> = [
  ["field", "FIELD"],
  ["retained", "RETAINED"],
  ["displaced_remnant", "DISPLACED REMNANT"],
  ["runaway", "RUNAWAY"],
  ["walkaway", "WALKAWAY"],
  ["hypervelocity_survivor", "HYPERVELOCITY SURVIVOR"],
];

describe("SystemReadout's placement (plan 08, P08.T15.a)", () => {
  it.each(PLACEMENTS)(
    "reads a grid system placed %s as its population and placement",
    (placement, words) => {
      const readout = renderReadout(aSystem({ origin: { type: "grid" }, placement }));

      expect(valueOf(readout, "ORIGIN")).toBe(`OLD THIN DISC · ${words}`);
    },
  );

  it("reads a row with its placement but no origin as a grid system's, not as not modelled", () => {
    // P09.T40.a's gate holds the origins back, and only a grid row carries a placement.
    const readout = renderReadout(aSystem({ placement: "displaced_remnant" }));

    expect(valueOf(readout, "ORIGIN")).toBe("OLD THIN DISC · DISPLACED REMNANT");
    expect(valueOf(readout, "MEMBER CLASS")).toBeUndefined();
  });

  it("keeps the placement of a row whose population it does not know, the population Missing", () => {
    const readout = renderReadout(
      aSystem({ population: anUnknownPopulation(), placement: "runaway" }),
    );

    expect(valueOf(readout, "ORIGIN")).toBe("— · RUNAWAY");
  });

  it("reads a feature's member by its feature whatever its row says of placement", () => {
    const readout = renderReadout(
      aSystem({
        origin: { type: "feature_member", feature: OPEN_CLUSTER.id },
        placement: "field",
      }),
    );

    expect(valueOf(readout, "ORIGIN")).toBe("OC GPF F207");
  });
});

describe("SystemReadout's values this build does not know (plan 09, P09.T41.c)", () => {
  /** The readings of the same row with no origin, to which every other reading is held. */
  function baseline(): Array<[string | null, string | null]> {
    const readout = renderReadout(aSystem({ origin: { type: "grid" } }));
    const rows = readings(readout).filter(([label]) => label !== "ORIGIN");
    readout.remove();
    return rows;
  }

  it("reads an origin it does not know, { type: pinned }, Missing, keeping the row and the rest", () => {
    const others = baseline();
    const readout = renderReadout(aSystem({ origin: anUnknownOrigin() }));

    expect(valueOf(readout, "ORIGIN")).toBe("—");
    expect(valueOf(readout, "DESIG")).toBe("H7K 4C0RFZ C-1");
    expect(readings(readout).filter(([label]) => label !== "ORIGIN")).toEqual(others);
  });

  it("reads a catalogue system's class it does not know, magnetar, Missing, and the rest as before", () => {
    const others = baseline();
    const readout = renderReadout(
      aSystem({ origin: { type: "catalogue_system", class: anUnknownClass() } }),
    );

    expect(valueOf(readout, "ORIGIN")).toBe("—");
    expect(readings(readout).filter(([label]) => label !== "ORIGIN")).toEqual(others);
  });

  it("reads a listed member's class it does not know, magnetar, Missing, and the rest as before", () => {
    const known = renderReadout(
      aSystem({
        origin: { type: "centre_member" },
        member_class: { type: "listed", class: "tidal_disruption_victim" },
      }),
    );
    const others = readings(known).filter(([label]) => label !== "MEMBER CLASS");
    known.remove();

    const readout = renderReadout(
      aSystem({ origin: { type: "centre_member" }, member_class: LISTED_UNKNOWN }),
    );

    expect(valueOf(readout, "MEMBER CLASS")).toBe("—");
    expect(readings(readout).filter(([label]) => label !== "MEMBER CLASS")).toEqual(others);
  });

  it("reads a grid system's population it does not know, outer_halo, Missing", () => {
    const readout = renderReadout(
      aSystem({ origin: { type: "grid" }, population: anUnknownPopulation() }),
    );

    expect(valueOf(readout, "ORIGIN")).toBe("—");
  });

  it("reads a member's class with a band's layer it does not know, its letter Missing, and the rest as before", () => {
    const cluster = (memberClass: Extract<MemberClassWire, { type: "cluster" }>) =>
      aSystem({
        origin: { type: "feature_member", feature: OPEN_CLUSTER.id },
        member_class: memberClass,
      });
    const known = renderReadout(
      cluster({
        type: "cluster",
        kind: "living",
        band: "c",
        generation: "first",
        multiplicity: "single",
      }),
    );
    const others = readings(known).filter(([label]) => label !== "MEMBER CLASS");
    known.remove();

    const readout = renderReadout(
      cluster({
        type: "cluster",
        kind: "living",
        band: anUnknownLayer(),
        generation: "first",
        multiplicity: "single",
      }),
    );

    expect(valueOf(readout, "MEMBER CLASS")).toBe("LIVING · — · FIRST · SINGLE");
    expect(readings(readout).filter(([label]) => label !== "MEMBER CLASS")).toEqual(others);
  });
});
