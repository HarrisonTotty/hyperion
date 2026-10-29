import type { RequestOf, SceneStateDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { norm, sub } from "../../geometry/vec3";
import { FIXTURE_EARTH, FIXTURE_JUPITER, FIXTURE_SYSTEM } from "../../test/planetaryFixture";
import { sceneModelOf, stateNearEarth } from "../../test/sceneFixture";
import { skyResponse } from "../../test/skyFixtures";
import { TERRAIN_HOST_DISCS } from "../scenes/terrainScene";
import { galacticTranslated } from "../coords/position";
import { serverSceneAtPush } from "./fromServer";
import {
  HOST_DISCS_LIMIT_V,
  type ServedBody,
  servedBodyOf,
  servedHostDiscs,
  starDistanceM,
} from "./servedBody";

const UNIVERSE = "000000000000002a";

/** A promise and the function that settles it, for an answer the test gives when it chooses. */
function deferred<T>(): { readonly promise: Promise<T>; readonly resolve: (value: T) => void } {
  const held: { settle: ((value: T) => void) | null } = { settle: null };
  const promise = new Promise<T>((settle) => {
    held.settle = settle;
  });
  return {
    promise,
    resolve: (value) => {
      held.settle?.(value);
    },
  };
}

/** The slice's Earth, served, from the ship's scene in its frame. */
function servedEarth(state: SceneStateDto = stateNearEarth()): ServedBody {
  const read = servedBodyOf(sceneModelOf(state), UNIVERSE, FIXTURE_EARTH);
  if (read.kind !== "served") {
    throw new Error(`the slice's Earth is a served world, not ${read.gap}`);
  }
  return read.served;
}

describe("a served body, from the server's scene (R10.T15.b)", () => {
  it("is the scene's body as fromServer draws it at the push", () => {
    const model = sceneModelOf(stateNearEarth());
    const served = servedEarth();
    const place = model.system?.place;
    if (place === null || place === undefined) {
      throw new Error("the fixture's scene states its place");
    }
    const drawn = serverSceneAtPush(model, place)?.bodies.find((b) => b.id === FIXTURE_EARTH);
    expect(served.body).toEqual(drawn);
    expect([served.universe, served.system, served.kind]).toEqual([
      UNIVERSE,
      FIXTURE_SYSTEM,
      "planet",
    ]);
    expect(served.epoch).toEqual(model.clock.time);
    expect(served.figure).toBe(served.appearance.figure);
    expect(served.hillRadiusM).toBe(drawn?.hillRadiusM);
  });

  it("is lit by its host star, at its distance and in its direction", () => {
    const served = servedEarth();
    expect(served.star.kind).toBe("star");
    expect(served.body.parent).toBe(served.star.id);
    expect(served.towardStarM).toEqual(sub(served.star.centreM, served.body.centreM));
    // An Earth at 1 au.
    expect(starDistanceM(served) / 1.496e11).toBeCloseTo(1, 1);
    expect(norm(served.towardStarM)).toBe(starDistanceM(served));
  });

  it("turns by its rotation section's law at any time, the push's its drawn rotation", () => {
    const served = servedEarth();
    expect(served.rotationAt(served.epoch)).toEqual(served.body.rotation);
    const later = served.rotationAt({ seconds: served.epoch.seconds + 3_600, nanos: 0 });
    expect(later).not.toEqual(served.body.rotation);
  });

  it("stands where the scene draws it, from the barycentre it states", () => {
    const served = servedEarth();
    expect(served.position).toEqual(
      galacticTranslated(served.scene.barycentre ?? served.position, served.body.centreM),
    );
  });

  it.each([
    [`${FIXTURE_SYSTEM}.0900`, "not in the scene"],
    [`${FIXTURE_SYSTEM}.0000`, "not a world"],
    [FIXTURE_JUPITER, "no solid surface"],
  ] as const)("is none for %s: %s", (body, gap) => {
    expect(servedBodyOf(sceneModelOf(stateNearEarth()), UNIVERSE, body)).toEqual({
      kind: "gap",
      gap,
      body,
    });
  });

  it("is none where the scene holds no system", () => {
    const { system: _system, ...state } = stateNearEarth();
    const read = servedBodyOf(
      sceneModelOf({
        ...state,
        system: null,
        ship: {
          position: {
            frame: "galactic",
            position: { cell_ly: [8_000, 10, 20], offset_m: [1e15, 2e15, 3e15] },
          },
          velocity_m_s: [1e4, 0, 0],
          time: state.clock.time,
        },
      }),
      UNIVERSE,
      FIXTURE_EARTH,
    );
    expect(read).toEqual({ kind: "gap", gap: "not in the scene", body: FIXTURE_EARTH });
  });
});

describe("a served body's host discs", () => {
  it("asks the server's sky at the body, of its system, with a limit no star reaches", async () => {
    const served = servedEarth();
    const asked: Array<RequestOf<"sky">> = [];
    const hosts = [...TERRAIN_HOST_DISCS];
    const read = await servedHostDiscs(
      {
        requestBulk: (body) => {
          asked.push(body);
          const response = skyResponse(body, new Uint8Array(0), 0, 0, 0);
          return {
            outcome: Promise.resolve({
              ok: true,
              response: { kind: "sky", ...response, hosts },
              chunks: [],
            }),
            cancel: () => undefined,
          };
        },
      },
      served,
      new AbortController().signal,
    );
    expect(read).toEqual({ ok: true, discs: hosts });
    expect(asked).toEqual([
      {
        kind: "sky",
        universe: UNIVERSE,
        observer: served.position,
        time: served.epoch,
        eye: null,
        camera_limit_v: HOST_DISCS_LIMIT_V,
        n_max: 1,
        cone: null,
        exclude_system: FIXTURE_SYSTEM,
      },
    ]);
  });

  it("gives the server's refusal as its reason, naming its code", async () => {
    const read = await servedHostDiscs(
      {
        requestBulk: () => ({
          outcome: Promise.resolve({
            ok: false,
            error: { code: "queue_full", message: "the pool is full", field: null },
          }),
          cancel: () => undefined,
        }),
      },
      servedEarth(),
      new AbortController().signal,
    );
    expect(read).toEqual({
      ok: false,
      message: `the sky's host discs at ${FIXTURE_EARTH} failed: queue_full: the pool is full`,
    });
  });

  it("cancels its request once its signal aborts", async () => {
    const abort = new AbortController();
    let cancelled = 0;
    const answer = deferred<{ ok: false; error: { code: "aborted"; message: string } }>();
    const reading = servedHostDiscs(
      {
        requestBulk: () => ({
          outcome: answer.promise,
          cancel: () => {
            cancelled += 1;
            answer.resolve({ ok: false, error: { code: "aborted", message: "cancelled" } });
          },
        }),
      },
      servedEarth(),
      abort.signal,
    );
    abort.abort();
    expect(await reading).toMatchObject({ ok: false });
    expect(cancelled).toBe(1);
  });
});
