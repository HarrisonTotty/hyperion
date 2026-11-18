import { BUDGET_ROWS, PASS_ROWS, PERF_SCENE_IDS, VIEW_ROLES, type ViewRole } from "@hyperion/perf";
import { describe, expect, it } from "vitest";

import {
  ATMOSPHERE_BEHIND_PASS,
  ATMOSPHERE_COMPOSITE_PASS,
  ATMOSPHERE_VIEW_PASS,
} from "../atmosphere/hillaire";
import { ATMOSPHERE_TABLES_PASS } from "../atmosphere/tables";
import { DISC_CELLS_PASS, FIGURE_CLUSTER_CULL_LABEL } from "../bodies/draw";
import type { RenderStyle } from "../camera/state";
import { CLOUD_LAYER_PASS } from "../clouds/cloudLayer";
import { DEPTH_PYRAMID_LABEL } from "../clusters/hzb";
import {
  VISIBILITY_LATE_RASTER_LABEL,
  VISIBILITY_RASTER_LABEL,
  VISIBILITY_RESOLVE_LABEL,
} from "../clusters/visibility";
import {
  ATMOSPHERE_PASS_LABELS,
  PHOTOREAL_PASS_LABELS,
  photorealisticPasses,
  SKY_PASS_LABEL,
} from "../photoreal/passes";
import { BLOOM_PASS } from "../post/bloomChain";
import { HISTOGRAM_PASS } from "../post/histogram";
import { TONEMAP_PASS } from "../post/tonemap";
import { RINGS_PASS_LABEL } from "../rings/ringSlab";
import { BAKE_SPLAT } from "../sky/bake";
import { LIT_VIEW_DISPLAY_LABEL } from "../spike/litView";
import { SPIKE_MAIN_VIEW_ID } from "../spike/spikeRun";
import { DETAIL_PASS_LABEL } from "../surface/decoration/detailTextureBake";
import { TERRAIN_PASS_LABEL, TERRAIN_SHADOW_PASS_LABEL } from "../terrain/gpu/material";
import { TERRAIN_CLUSTER_COMPACT_LABEL, TERRAIN_CLUSTER_CULL_LABEL } from "../terrain/terrainPass";
import {
  HULL_CLUSTER_CULL_LABEL,
  MESH_OCCLUDER_CULL_LABEL,
  WIREFRAME_PASS_LABEL,
} from "../wireframe/submit";
import { timedPass } from "../../test/passTimes";
import {
  attributePasses,
  passRolesOf,
  passRowOf,
  passStyleOf,
  type PassStyle,
  UnmappedPass,
} from "./passRows";
import { PERF_SCENES } from "./scenes";

/** R06's bake's pass label, a module constant it does not export (`sky/bake.ts`). */
const SKY_BAKE_PASS = "sky bake";

/**
 * A label no plan's pass takes, so that it has no row: R11's rings, clouds and ocean, the unmapped
 * passes these tests once named, have theirs (R11.T7, R11.T8, R11.T9.c).
 */
const UNROWED_PASS = "no plan's pass";

/**
 * Every pass label R12.T0's record lists as timed, with R11's that have landed (`decoration`,
 * `clouds`, `rings`), each with the style that submits it, from their modules' constants.
 */
const TIMED_PASSES: ReadonlyArray<readonly [string, PassStyle]> = [
  [SKY_PASS_LABEL, "photorealistic"],
  [DISC_CELLS_PASS, "photorealistic"],
  [TERRAIN_SHADOW_PASS_LABEL, "photorealistic"],
  [PHOTOREAL_PASS_LABELS.bodies, "photorealistic"],
  [PHOTOREAL_PASS_LABELS.discs, "photorealistic"],
  [ATMOSPHERE_TABLES_PASS, "photorealistic"],
  [ATMOSPHERE_VIEW_PASS, "photorealistic"],
  [ATMOSPHERE_BEHIND_PASS, "photorealistic"],
  [ATMOSPHERE_COMPOSITE_PASS, "photorealistic"],
  [HISTOGRAM_PASS, "photorealistic"],
  [BLOOM_PASS, "photorealistic"],
  [TONEMAP_PASS, "photorealistic"],
  [PHOTOREAL_PASS_LABELS.symbology, "photorealistic"],
  [PHOTOREAL_PASS_LABELS.figureIds, "photorealistic"],
  [PHOTOREAL_PASS_LABELS.figureResolve, "photorealistic"],
  [VISIBILITY_RASTER_LABEL, "photorealistic"],
  [VISIBILITY_LATE_RASTER_LABEL, "photorealistic"],
  [VISIBILITY_RESOLVE_LABEL, "photorealistic"],
  [DEPTH_PYRAMID_LABEL, "photorealistic"],
  [PHOTOREAL_PASS_LABELS.bodiesLate, "photorealistic"],
  [TERRAIN_PASS_LABEL, "photorealistic"],
  [LIT_VIEW_DISPLAY_LABEL, "photorealistic"],
  [DETAIL_PASS_LABEL, "photorealistic"],
  [CLOUD_LAYER_PASS, "photorealistic"],
  [RINGS_PASS_LABEL, "photorealistic"],
  [WIREFRAME_PASS_LABEL, "wireframe"],
  // A photorealistic view's symbology culls its hulls and mesh occluders too (R12.T4.c).
  [HULL_CLUSTER_CULL_LABEL, "both"],
  [MESH_OCCLUDER_CULL_LABEL, "both"],
  [TERRAIN_CLUSTER_CULL_LABEL, "both"],
  [TERRAIN_CLUSTER_COMPACT_LABEL, "both"],
  [FIGURE_CLUSTER_CULL_LABEL, "both"],
  [SKY_BAKE_PASS, "both"],
  [BAKE_SPLAT.name, "both"],
  [`terrain clusters (${SPIKE_MAIN_VIEW_ID}) stats`, "both"],
  [`terrain clusters (${SPIKE_MAIN_VIEW_ID}) over counts`, "both"],
];

function submits(passStyle: PassStyle, viewStyle: RenderStyle): boolean {
  return passStyle === "both" || passStyle === viewStyle;
}

/** Whether `PASS_ROWS` gives the pass a row on `role`. */
function mapped(role: ViewRole, label: string): boolean {
  try {
    return BUDGET_ROWS.includes(passRowOf(role, label));
  } catch (error: unknown) {
    if (error instanceof UnmappedPass) {
      return false;
    }
    throw error;
  }
}

/** A pass of a resolve, timed at one quantum. */
function pass(label: string): ReturnType<typeof timedPass> {
  return timedPass(label, 65_536);
}

describe("PASS_ROWS", () => {
  it("has a row for every timed pass in each role a catalogue scene can time it in", () => {
    const timed = PERF_SCENE_IDS.flatMap((scene) =>
      Object.values(PERF_SCENES[scene].views).flatMap((views) =>
        views.flatMap((view) =>
          TIMED_PASSES.filter(([, style]) => submits(style, view.style)).map(
            ([label]) => `${scene}: ${view.role}:${label}`,
          ),
        ),
      ),
    );
    expect(timed.length).toBeGreaterThan(TIMED_PASSES.length);
    const unmapped = timed.filter((entry) => {
      const [, key = ""] = entry.split(": ");
      const colon = key.indexOf(":");
      const role = VIEW_ROLES.find((known) => known === key.slice(0, colon));
      return role === undefined || !mapped(role, key.slice(colon + 1));
    });
    expect(unmapped).toEqual([]);
  });

  it("covers every built pass of R07's photorealistic pass list on the primary", () => {
    const labels = (["high", "low"] as const).flatMap((setting) =>
      photorealisticPasses(setting)
        .passes.filter(({ built }) => built)
        .flatMap((entry) => entry.labels ?? [entry.label]),
    );
    expect(labels.filter((label) => !mapped("primary", label))).toEqual([]);
    expect(ATMOSPHERE_PASS_LABELS.map((label) => passRowOf("primary", label))).toEqual([
      "atmosphere-tables",
      "atmosphere-frame",
      "atmosphere-frame",
      "atmosphere-frame",
    ]);
  });

  it("puts the same wireframe passes in the instruments' row and in the station's", () => {
    expect(passRowOf("instrument", WIREFRAME_PASS_LABEL)).toBe("cockpit-instruments");
    expect(passRowOf("station", WIREFRAME_PASS_LABEL)).toBe("station-wireframe");
    expect(passRowOf("instrument", TERRAIN_CLUSTER_CULL_LABEL)).toBe("cockpit-instruments");
    expect(passRowOf("primary", TERRAIN_CLUSTER_CULL_LABEL)).toBe("terrain");
  });

  it("keys the terrain's cluster counters by their prefix, whatever the view's ID", () => {
    expect(passRowOf("primary", "terrain clusters (instrument-1) stats")).toBe("terrain");
    expect(passRowOf("station", "terrain clusters (spike) over counts")).toBe("station-wireframe");
  });

  it("names each label's style as its rows give it", () => {
    expect(TIMED_PASSES.map(([label]) => [label, passStyleOf(label)])).toEqual(
      TIMED_PASSES.map(([label, style]) => [label, style]),
    );
    expect(passStyleOf(UNROWED_PASS)).toBeNull();
  });

  it("holds keys of known roles only", () => {
    for (const key of Object.keys(PASS_ROWS)) {
      expect(key).toMatch(/^(primary|instrument|station):.+/);
    }
  });
});

describe("an unmapped pass", () => {
  it("throws, naming its role and label", () => {
    expect(() => passRowOf("primary", UNROWED_PASS)).toThrow(UnmappedPass);
    expect(() => passRowOf("station", PHOTOREAL_PASS_LABELS.bodies)).toThrow(
      "station:bodies has no row",
    );
  });

  it("throws from a resolve that holds it", () => {
    const roles = passRolesOf(PERF_SCENES["cockpit-descent"].views.low);
    expect(() => attributePasses({ passes: [timedPass(UNROWED_PASS, 1)] }, roles)).toThrow(
      `primary:${UNROWED_PASS}`,
    );
  });
});

describe("a resolve's role", () => {
  const cockpit = passRolesOf(PERF_SCENES["cockpit-descent"].views.high);
  const station = passRolesOf(PERF_SCENES["station-wireframe-descent"].views.high);
  it("is the scene's wireframe view's where it holds a wireframe pass", () => {
    const passes = [pass(WIREFRAME_PASS_LABEL), pass(TERRAIN_CLUSTER_CULL_LABEL)];
    expect(attributePasses({ passes }, cockpit).map(({ key }) => key)).toEqual([
      "instrument:view:wireframe",
      "instrument:terrain cluster cull",
    ]);
    expect(attributePasses({ passes }, station).map(({ row }) => row)).toEqual([
      "station-wireframe",
      "station-wireframe",
    ]);
  });

  it("is the primary's where it holds a photorealistic pass", () => {
    const passes = [pass(TERRAIN_CLUSTER_CULL_LABEL), pass(PHOTOREAL_PASS_LABELS.bodies)];
    expect(attributePasses({ passes }, cockpit).map(({ key }) => key)).toEqual([
      "primary:terrain cluster cull",
      "primary:bodies",
    ]);
  });

  it("is the scene's first view's where it holds shared passes alone", () => {
    const passes = [pass(SKY_BAKE_PASS)];
    expect(attributePasses({ passes }, cockpit)[0]?.key).toBe("primary:sky bake");
    expect(attributePasses({ passes }, station)[0]?.key).toBe("station:sky bake");
  });

  it("gives each pass its own style's role where it holds passes of both styles", () => {
    const passes = [pass(DETAIL_PASS_LABEL), pass(WIREFRAME_PASS_LABEL), pass(SKY_BAKE_PASS)];
    expect(attributePasses({ passes }, cockpit).map(({ key }) => key)).toEqual([
      "primary:decoration",
      "instrument:view:wireframe",
      "primary:sky bake",
    ]);
  });
});
