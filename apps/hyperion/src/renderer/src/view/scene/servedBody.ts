/**
 * A generated body as the server's scene draws it (plan R10, R10.T15.b): what the descent spike's
 * served world and the hand-over check's served world read of a body the server serves, through
 * R02's `fromServer.ts`.
 *
 * @remarks
 * - **The body** is the scene's at its latest push (`serverSceneAtPush`), read from its record at
 *   its own grant: its designation, kind, Hill radius at pericentre, appearance (its figure, with
 *   its pole, and its photometry) and rotation. A body drawn as a contact, one without a figure, a
 *   giant's envelope (`surfaceless`), or one whose rotation is not given, is no served world.
 * - **The star** is the body's host: the first star up its parents, or the system's first star
 *   for a body that orbits a pair or the barycentre, which a world flown alone in a kept scene
 *   cannot show beside its other star (a lean of T15.b.1's record).
 * - **Its rotation** is its rotation section's law at any time (`bodyRotationAt`), so that a
 *   served world turns it at times of its own from the push's (`ServedBody.epoch`).
 * - **The star's place** is its drawn centre less the body's at the push, and the body's galactic
 *   position its own drawn centre from the barycentre the scene states, at which a `sky` request
 *   answers the host discs that light it ({@link servedHostDiscs}).
 */
import type {
  BodyIdHex,
  BulkManifestDto,
  GalacticPosition,
  HostDiscDto,
  PendingBulkRequest,
  RequestOf,
  ResponseFor,
  SystemIdHex,
  UniverseIdHex,
  UniverseTime,
} from "@hyperion/protocol";

import { norm, sub, type Vec3 } from "../../geometry/vec3";
import type { SceneModel } from "../../lib/scene/model";
import type { WireAppearance } from "../appearance/fromWire";
import { galacticTranslated } from "../coords/position";
import type { Rotation3 } from "../coords/rotation";
import type { BodyFigure } from "../terrain/planet";
import { bodyRotationAt, serverSceneAtPush } from "./fromServer";
import type { ViewBody, ViewScene } from "./model";

/** The kinds of body a served world may be: a solid world's. */
export type ServedBodyKind = "planet" | "dwarf_planet" | "moon";

/** A generated body the server serves, as its scene drew it at one push. */
export interface ServedBody {
  readonly universe: UniverseIdHex;
  readonly system: SystemIdHex;
  /** The scene's time at the push it was read at. */
  readonly epoch: UniverseTime;
  /** The scene at that push, through `fromServer.ts`. */
  readonly scene: ViewScene;
  /** The body as the scene draws it at the push. */
  readonly body: ViewBody;
  readonly kind: ServedBodyKind;
  /** Its appearance, the scene's from its sections (`heldAppearanceOf`), its figure given. */
  readonly appearance: WireAppearance;
  /** Its reference spheroid with its pole, the appearance's: its heights' datum. */
  readonly figure: BodyFigure;
  /** Its Hill radius at pericentre, m. */
  readonly hillRadiusM: number;
  /** Its host star as the scene draws it. */
  readonly star: ViewBody;
  /** Its rotation from body-fixed to body axes at `time`: its rotation section's law. */
  readonly rotationAt: (time: UniverseTime) => Rotation3;
  /** The star's centre from the body's at the push, m along the galactic axes. */
  readonly towardStarM: Vec3;
  /** The body's centre at the push, galactic. */
  readonly position: GalacticPosition;
}

/** Why a body is no served world. */
export type ServedBodyGap =
  /** The scene holds no system to draw, or none that names this body. */
  | "not in the scene"
  /** It is a star, a contact or a body of unresolved kind. */
  | "not a world"
  /** A giant's envelope, which has no solid surface (P14.T35.e). */
  | "no solid surface"
  /** Its radius is not granted, so it has no figure to fly over. */
  | "no figure"
  /** Its Hill radius at pericentre is not known (its mass or orbit withheld). */
  | "no hill radius"
  /** Its rotation section is not given, which a turning world needs. */
  | "no rotation"
  /** The scene draws no star it could be lit by. */
  | "no star"
  /** The scene states no barycentre, from which its galactic position follows. */
  | "no place";

/** A served body, or why the scene gives none. */
export type ServedBodyRead =
  | { readonly kind: "served"; readonly served: ServedBody }
  | { readonly kind: "gap"; readonly gap: ServedBodyGap; readonly body: BodyIdHex };

/** The kind of a body that may be a served world, or `null` for any other. */
function servedKind(body: ViewBody): ServedBodyKind | null {
  let kind: ServedBodyKind | null;
  switch (body.kind) {
    case "planet":
    case "dwarf_planet":
    case "moon":
      kind = body.kind;
      break;
    case "star":
    case "unresolved":
      kind = null;
      break;
  }
  return kind;
}

/** `body`'s host star in `scene`: the first star up its parents, else the scene's first star. */
function hostStar(scene: ViewScene, body: ViewBody): ViewBody | null {
  const byId = new Map(scene.bodies.map((each) => [each.id, each]));
  let parent = body.parent;
  // The walk is bounded by the bodies, since a parent chain is one of them at most each.
  for (let steps = 0; parent !== null && steps <= scene.bodies.length; steps += 1) {
    const next = byId.get(parent);
    if (next === undefined) {
      break;
    }
    if (next.kind === "star") {
      return next;
    }
    parent = next.parent;
  }
  return scene.bodies.find((each) => each.kind === "star") ?? null;
}

/**
 * `body` as the served world `model`'s latest push draws it in `universe`
 * ({@link ServedBody}), or why it is none ({@link ServedBodyGap}).
 */
export function servedBodyOf(
  model: SceneModel,
  universe: UniverseIdHex,
  body: BodyIdHex,
): ServedBodyRead {
  const gap = (reason: ServedBodyGap): ServedBodyRead => ({ kind: "gap", gap: reason, body });
  const system = model.system;
  if (system === null) {
    return gap("not in the scene");
  }
  const systemId = system.model.system;
  // The place the scene states (R03.T16), else the system's ID with no position, as `VIEW` takes it
  // with no system opened on `SYSTEM` (`serverScene.ts`' `systemPlace`).
  const place = system.place ?? { kind: "unknown", system: systemId, designation: systemId };
  const scene = serverSceneAtPush(model, place);
  const drawn = scene?.bodies.find((each) => each.id === body);
  if (scene === null || drawn === undefined) {
    return gap("not in the scene");
  }
  const kind = servedKind(drawn);
  const record = system.bodies.bodies.find((each) => each.id === body);
  if (kind === null || drawn.retarded === null || record === undefined) {
    return gap("not a world");
  }
  if (drawn.surfaceless === true) {
    return gap("no solid surface");
  }
  const appearance = drawn.appearance;
  const figure = appearance?.figure ?? null;
  if (appearance === null || figure === null) {
    return gap("no figure");
  }
  if (drawn.hillRadiusM === null) {
    return gap("no hill radius");
  }
  const section = record.rotation;
  if (section.state !== "ok") {
    return gap("no rotation");
  }
  const star = hostStar(scene, drawn);
  if (star === null) {
    return gap("no star");
  }
  if (scene.barycentre === null) {
    return gap("no place");
  }
  return {
    kind: "served",
    served: {
      universe,
      system: systemId,
      epoch: scene.time,
      scene,
      body: drawn,
      kind,
      appearance,
      figure,
      hillRadiusM: drawn.hillRadiusM,
      star,
      rotationAt: (time) => {
        const rotation = bodyRotationAt(section, time);
        if (rotation === null) {
          throw new Error(`the rotation section of ${body} is ok and gave no rotation`);
        }
        return rotation;
      },
      towardStarM: sub(star.centreM, drawn.centreM),
      position: galacticTranslated(scene.barycentre, drawn.centreM),
    },
  };
}

/** The body's distance from its star at the push, m. */
export function starDistanceM(served: ServedBody): number {
  return norm(served.towardStarM);
}

/**
 * A camera limit no star outside the host system reaches, V, for a `sky` request that asks the
 * host discs alone: 3.3 magnitudes brighter than the Sun seen from 1 au (V −26.74), which a Sun-like
 * star would reach only within about 0.2 au and the most luminous stars known only within about
 * 200 au, so the census lists none, while every reply carries the observer's system's discs, which
 * read no census (R06's Design note 16, R06.T11.c).
 */
export const HOST_DISCS_LIMIT_V = -30;

/** What {@link servedHostDiscs} asks through: a request client's bulk request, of the sky. */
export interface HostDiscRequests {
  requestBulk(
    body: RequestOf<"sky">,
    manifestOf: (response: ResponseFor<"sky">) => BulkManifestDto | null,
  ): PendingBulkRequest<"sky">;
}

/** A served body's host discs, or why the server's sky gave none: its failure's code and words. */
export type HostDiscsRead =
  | { readonly ok: true; readonly discs: ReadonlyArray<HostDiscDto> }
  | { readonly ok: false; readonly message: string };

/**
 * The discs of `served`'s system's stars, as the server's `sky` answers them at the body at its
 * epoch (`exclude_system` its system, a camera's limit no star reaches): the discs a server scene
 * is lit by (`sceneHostDiscs`), for a served world flown in a kept scene. Its request is cancelled
 * once `signal` aborts, which the read then gives as a failure.
 */
export async function servedHostDiscs(
  requests: HostDiscRequests,
  served: ServedBody,
  signal: AbortSignal,
): Promise<HostDiscsRead> {
  const pending = requests.requestBulk(
    {
      kind: "sky",
      universe: served.universe,
      observer: served.position,
      time: served.epoch,
      eye: null,
      camera_limit_v: HOST_DISCS_LIMIT_V,
      n_max: 1,
      cone: null,
      exclude_system: served.system,
    },
    (response) => response.bulk,
  );
  const outcome = await cancelledOn(signal, pending);
  return outcome.ok
    ? { ok: true, discs: outcome.response.hosts }
    : {
        ok: false,
        message: `the sky's host discs at ${served.body.id} failed: ${outcome.error.code}: ${outcome.error.message}`,
      };
}

/**
 * `pending`'s outcome, its request cancelled when `signal` aborts first (it then settles
 * `aborted`), and the abort's listener removed once it settles.
 */
export async function cancelledOn<T>(
  signal: AbortSignal,
  pending: { readonly outcome: Promise<T>; cancel(): void },
): Promise<T> {
  const cancel = (): void => {
    pending.cancel();
  };
  if (signal.aborted) {
    cancel();
  }
  signal.addEventListener("abort", cancel, { once: true });
  try {
    return await pending.outcome;
  } finally {
    signal.removeEventListener("abort", cancel);
  }
}
