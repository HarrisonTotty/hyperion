/**
 * What a `VIEW` view tells the atmosphere cache of its frame, and what its label block says of the
 * answer (plan R08, R08.T10.b).
 */
import type { BodyIdHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { detailOf } from "../../test/atmosphereRecords";
import { aHostDisc } from "../../test/litFixtures";
import { earthDetail } from "../../test/planetaryFixture";
import type { ClassMapDiscSurface } from "../../view/appearance/bodyAppearance";
import type { PhotometricLaw } from "../../view/appearance/law";
import { heldAppearanceOf } from "../../view/appearance/fromWire";
import { lawFor } from "../../view/appearance/phase";
import { AtmosphereCache } from "../../view/atmosphere/AtmosphereCache";
import { classMapGroundAlbedo } from "../../view/atmosphere/classMapGround";
import type { AtmosphereBuilder } from "../../view/atmosphere/opticsPool";
import { THICK_REASON } from "../../view/atmosphere/thick/regime";
import type { BodyLightSources } from "../../view/bodies/draw";
import { packClassMap } from "../../view/bodies/discSurface";
import type { LitRegime } from "../../view/bodies/regime";
import type { Rgb } from "../../view/photometry/toneCurve";
import type { ViewBody, ViewScene } from "../../view/scene/model";
import { phaseScene } from "../../view/scenes/phaseScene";
import {
  atmosphereFrameOf,
  atmosphereObserverOf,
  atmosphereShownOf,
  cloudFreeGrounds,
  ViewAtmosphereHandle,
} from "./viewAtmosphere";

/** A builder that never lands: a kept scene's bodies are never built, nor baked. */
const NEVER: AtmosphereBuilder = {
  build: () => ({ outcome: new Promise(() => undefined), cancel: () => undefined }),
  bakeThick: () => ({ outcome: new Promise(() => undefined), cancel: () => undefined }),
  bakePolarisation: () => ({ outcome: new Promise(() => undefined), cancel: () => undefined }),
  bakeDisc: () => ({ outcome: new Promise(() => undefined), cancel: () => undefined }),
  bakeDeck: () => ({ outcome: new Promise(() => undefined), cancel: () => undefined }),
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
      `Multiple scattering: drawn by the thin-atmosphere method beyond its checked range, for TEST GIANT and ${planet.designation}`,
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

describe("where a view's camera stands at a body (R08.T14.d.4)", () => {
  const R = 6_371_000;
  const sphere = { equatorialRadiusM: R, polarRadiusM: R, pole: vec3(0, 0, 1) };

  const cameraM = vec3(R + 10, 0, 0);

  it("gives its height above a sphere", () => {
    expect(atmosphereObserverOf(cameraM, sphere, vec3(1, 0, 0)).heightM).toBeCloseTo(10, 6);
  });

  it.each([
    ["overhead", vec3(1e11, 0, 0), 1],
    ["underfoot", vec3(-1e11, 0, 0), -1],
    ["on the horizontal", vec3(0, 1e11, 0), 0],
    ["just below the horizontal", vec3(-1e10, 1e11, 0), -0.0995037],
  ] as const)(
    "gives a light %s its elevation sine over the local horizontal",
    (_, towardM, sine) => {
      expect(atmosphereObserverOf(cameraM, sphere, towardM).lightElevationSine).toBeCloseTo(
        sine,
        6,
      );
    },
  );

  it.each([
    ["above the pole", vec3(0, 54_364e3 + 1e5, 0), vec3(0, 1, 0)],
    ["above the equator", vec3(0, 0, 60_268e3 + 1e5), vec3(0, 0, 1)],
  ] as const)(
    "reads a flattened body's height and normal on its datum %s",
    (_, cameraAtM, upward) => {
      const giant = { equatorialRadiusM: 60_268e3, polarRadiusM: 54_364e3, pole: vec3(0, 1, 0) };
      const observer = atmosphereObserverOf(cameraAtM, giant, upward);
      expect(observer.heightM).toBeCloseTo(1e5, 3);
      expect(observer.lightElevationSine).toBeCloseTo(1, 12);
    },
  );

  it("notes each body's observer where the view gives one", () => {
    const { scene, body } = phaseBody("TEST GIANT");
    const regimes = new Map<BodyIdHex, LitRegime>([[body.id, "disc"]]);
    const observer = { heightM: 5, lightElevationSine: -0.25 };
    const frame = atmosphereFrameOf(
      scene,
      regimes,
      new Map(),
      { scene: null, nowMs: 0 },
      true,
      (each) => (each.id === body.id ? observer : undefined),
    );
    expect(frame.bodies.find((each) => each.id === body.id)?.observer).toEqual(observer);
  });
});

describe("a view's cloud-free grounds (fix-r08-decked-air-opacity)", () => {
  /** A named class's law, lambertian. */
  const NAMED = lawFor([0.2, 0.25, 0.3], [1.5, 1.5, 1.5], "lambert");

  /** A map of one named class over every texel, shading elsewhere with `elsewhere`. */
  function oneClass(elsewhere: PhotometricLaw): ClassMapDiscSurface {
    return {
      kind: "class-map",
      weights: { kind: "texture", name: "cloud-free ground test" },
      texels: packClassMap(4, 1, () => [1]),
      laws: [NAMED],
      elsewhere,
      normals: null,
    };
  }

  /** A ground the view gives one body. */
  const GROUND: Rgb = [0.25, 0.25, 0.25];

  /**
   * PHASE TEST's giant drawn with the slice's Earth's photometry, its bodies having none, and its
   * map of one named class under that law.
   */
  function drawnGiant(): { readonly body: ViewBody; readonly surface: ClassMapDiscSurface } {
    const { body: giant } = phaseBody("TEST GIANT");
    const appearance = heldAppearanceOf(detailOf(earthDetail()).record);
    return { body: { ...giant, appearance }, surface: oneClass(appearance.photometry.law) };
  }

  /** The bodies of `scene`'s frame that carry a ground, each with it, the view giving `body` one. */
  function groundsNoted(scene: ViewScene, body: ViewBody): ReadonlyArray<unknown> {
    const regimes = new Map<BodyIdHex, LitRegime>([[body.id, "disc"]]);
    const groundOf = (each: ViewBody): Rgb | null => (each.id === body.id ? GROUND : null);
    return atmosphereFrameOf(
      scene,
      regimes,
      new Map(),
      { scene: null, nowMs: 0 },
      true,
      undefined,
      groundOf,
    )
      .bodies.filter((each) => "cloudFreeGround" in each)
      .map((each) => [each.id, each.cloudFreeGround]);
  }

  it("gives none without the view's class maps", () => {
    expect(cloudFreeGrounds(undefined)).toBeUndefined();
  });

  it("gives a body its drawn map's mean", () => {
    const { body, surface } = drawnGiant();
    expect(cloudFreeGrounds({ classMapOf: () => surface })?.(body)).toEqual(
      classMapGroundAlbedo(surface),
    );
  });

  it("asks the body's map under the body's own law", () => {
    const { body, surface } = drawnGiant();
    const asked: PhotometricLaw[] = [];
    cloudFreeGrounds({
      classMapOf: (_, elsewhere) => {
        asked.push(elsewhere);
        return surface;
      },
    })?.(body);
    expect(asked).toEqual([body.appearance?.photometry.law]);
  });

  it("gives null for a body with no map drawn", () => {
    const { body } = drawnGiant();
    expect(cloudFreeGrounds({ classMapOf: () => null })?.(body)).toBeNull();
  });

  it("notes a served scene's body's ground where the view gives one", () => {
    const { scene, body } = phaseBody("TEST GIANT");
    expect(groundsNoted({ ...scene, provenance: { kind: "server" } }, body)).toEqual([
      [body.id, GROUND],
    ]);
  });

  it("notes no ground for a kept scene's body, whose setting brings its own", () => {
    const { scene, body } = phaseBody("TEST GIANT");
    expect(groundsNoted(scene, body)).toEqual([]);
  });
});
