/**
 * What a `VIEW` view tells the atmosphere cache of its frame, and what its label block says of the
 * answer (plan R08, R08.T10.b).
 */
import type { BodyIdHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { aHostDisc } from "../../test/litFixtures";
import { AtmosphereCache } from "../../view/atmosphere/AtmosphereCache";
import type { AtmosphereBuilder } from "../../view/atmosphere/opticsPool";
import { THICK_REASON } from "../../view/atmosphere/thick/regime";
import type { BodyLightSources } from "../../view/bodies/draw";
import type { LitRegime } from "../../view/bodies/regime";
import type { ViewBody, ViewScene } from "../../view/scene/model";
import { phaseScene } from "../../view/scenes/phaseScene";
import { atmosphereFrameOf, atmosphereShownOf, ViewAtmosphereHandle } from "./viewAtmosphere";

/** A builder that never lands: a kept scene's bodies are never built. */
const NEVER: AtmosphereBuilder = {
  build: () => ({ outcome: new Promise(() => undefined), cancel: () => undefined }),
  terminate: () => undefined,
};

/** A cache over no server: a kept scene's bodies bring their own settings. */
function keptCache(): AtmosphereCache {
  return new AtmosphereCache({
    requests: {
      request: () => {
        throw new Error("a kept scene asks nothing");
      },
    },
    builder: NEVER,
    tables: () => {
      throw new Error("a kept scene's unset body makes no tables");
    },
  });
}

/** `PHASE TEST` at its start, with the body named `designation`. */
function phaseBody(designation: string): { readonly scene: ViewScene; readonly body: ViewBody } {
  const scene = phaseScene().sceneAt(0);
  const body = scene.bodies.find((each) => each.designation === designation);
  if (body === undefined) {
    throw new Error(`PHASE TEST holds no ${designation}`);
  }
  return { scene, body };
}

/** One star's light at a body, from R06's disc. */
function sourcesOf(): BodyLightSources {
  const disc = aHostDisc();
  return {
    stars: [
      {
        host: { disc, centreM: vec3(1e11, 0, 0) },
        toStarM: vec3(1e11, 0, 0),
        illuminance: [1e5, 1e5, 1e5],
      },
    ],
    planetshine: [],
  };
}

describe("a view's atmosphere frame (R08.T10.b)", () => {
  it("notes each lit body at its last regime, a kept scene's with no setting unset", () => {
    const { scene, body } = phaseBody("TEST GIANT");
    const regimes = new Map<BodyIdHex, LitRegime>([[body.id, "disc"]]);
    const frame = atmosphereFrameOf(scene, regimes, new Map(), { scene: null, nowMs: 5 }, true);
    expect(frame.scene).toBeNull();
    expect(frame.nowMs).toBe(5);
    expect(frame.bodies.find((each) => each.id === body.id)).toEqual({
      id: body.id,
      regime: "disc",
      lights: [],
      kept: { kind: "unset" },
    });
    // A body with no regime on the last plan is a point.
    expect(frame.bodies.filter((each) => each.id !== body.id).map((each) => each.regime)).toEqual(
      frame.bodies.filter((each) => each.id !== body.id).map(() => "point"),
    );
  });

  it("names each light by its star's scene body, with R06's bake spectrum", () => {
    const { scene, body } = phaseBody("TEST GIANT");
    const sources = new Map([[body.id, sourcesOf()]]);
    const frame = atmosphereFrameOf(scene, new Map(), sources, { scene: null, nowMs: 0 }, true);
    const light = frame.bodies.find((each) => each.id === body.id)?.lights[0];
    expect(light?.star).toBe(`${scene.system}.${(0).toString(16).padStart(4, "0")}`);
    expect(light?.spectrum).toEqual(aHostDisc().bake_spectrum);
  });

  it("notes no body wanted in a wireframe frame, which draws no atmosphere", () => {
    const { scene, body } = phaseBody("TEST GIANT");
    const regimes = new Map<BodyIdHex, LitRegime>([[body.id, "disc"]]);
    expect(
      atmosphereFrameOf(scene, regimes, new Map(), { scene: null, nowMs: 0 }, false).bodies,
    ).toEqual([]);
  });

  it("gives R07's TEST GIANT, a kept body with no atmosphere set, ATMOSPHERE: NOT YET MODELLED", () => {
    const { scene, body } = phaseBody("TEST GIANT");
    const cache = keptCache();
    const view = cache.openView();
    const regimes = new Map<BodyIdHex, LitRegime>([[body.id, "disc"]]);
    const report = view.noteFrame(
      atmosphereFrameOf(scene, regimes, new Map(), { scene: null, nowMs: 0 }, true),
    );
    expect(atmosphereShownOf(report, scene).labels).toEqual(["atmosphereNotYetModelled"]);
    cache.dispose();
  });
});

describe("a view's atmosphere shown (R08.T10.b)", () => {
  it("names each body's problem by its designation, one line a cause", () => {
    const { scene, body } = phaseBody("TEST GIANT");
    const shown = atmosphereShownOf(
      {
        labels: [],
        problems: new Map([
          [body.id, { kind: "rejected", code: "internal", message: "it broke", refusal: false }],
        ]),
        reasons: new Map(),
        pending: false,
      },
      scene,
    );
    expect(shown.plate).toEqual([
      {
        statement: "ATMOSPHERE REJECTED: for TEST GIANT, it broke, reopen VIEW to retry",
        refusal: false,
      },
    ]);
    expect(shown.fault).toEqual([]);
  });

  it("says nothing of a body the scene no longer holds", () => {
    const { scene } = phaseBody("TEST GIANT");
    const shown = atmosphereShownOf(
      {
        labels: [],
        problems: new Map([["0300080020000000.0300", { kind: "failed" }]]),
        reasons: new Map(),
        pending: false,
      },
      scene,
    );
    expect([shown.plate, shown.fault]).toEqual([[], []]);
  });
});

describe("ATMOSPHERE: APPROXIMATE's reasons shown (R08.T10.c)", () => {
  const WATER = "H2O: its absorption estimated, not measured";

  it("names the bodies of each reason by their designations, in designation order", () => {
    const { scene, body: giant } = phaseBody("TEST GIANT");
    const planet = scene.bodies.find((each) => each.designation.startsWith("TEST PLANET"));
    if (planet === undefined) {
      throw new Error("PHASE TEST holds its planets");
    }
    const shown = atmosphereShownOf(
      {
        labels: ["atmosphereApproximate"],
        problems: new Map(),
        reasons: new Map([
          [planet.id, [THICK_REASON]],
          [giant.id, [THICK_REASON, WATER]],
        ]),
        pending: false,
      },
      scene,
    );
    expect(shown.approximate).toEqual([
      `A thick atmosphere drawn by the thin-atmosphere method, for TEST GIANT and ${planet.designation}`,
      "H2O: its absorption estimated, not measured, for TEST GIANT",
    ]);
  });

  it("names no body the scene no longer holds", () => {
    const { scene } = phaseBody("TEST GIANT");
    const shown = atmosphereShownOf(
      {
        labels: ["atmosphereApproximate"],
        problems: new Map(),
        reasons: new Map([["0300080020000000.0300", [WATER]]]),
        pending: false,
      },
      scene,
    );
    expect(shown.approximate).toEqual([]);
  });
});

describe("a view's handle on the atmosphere cache (R08.T10.b)", () => {
  it("opens one handle a cache, and lets its bodies go when the cache changes or it closes", () => {
    const { scene, body } = phaseBody("TEST GIANT");
    const first = keptCache();
    const second = keptCache();
    const handle = new ViewAtmosphereHandle();
    const regimes = new Map<BodyIdHex, LitRegime>([[body.id, "disc"]]);
    const input = handle.inputFor(first, null, 0);
    input?.view.noteFrame(
      atmosphereFrameOf(scene, regimes, new Map(), { scene: null, nowMs: 0 }, true),
    );
    expect(first.size).toBe(1);
    expect(handle.inputFor(first, null, 1)?.view).toBe(input?.view);
    const next = handle.inputFor(second, null, 2);
    expect(next?.view).not.toBe(input?.view);
    // The first cache's view is closed: its frames say nothing.
    expect(input?.view.noteFrame({ nowMs: 3, scene: null, bodies: [] }).labels).toEqual([]);
    handle.close();
    expect(handle.inputFor(null, null, 4)).toBeUndefined();
    first.dispose();
    second.dispose();
  });
});
