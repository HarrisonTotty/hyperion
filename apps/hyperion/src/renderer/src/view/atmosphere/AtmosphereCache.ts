/**
 * The atmospheres of one render engine's device, shared by its photorealistic views (plan R08,
 * R08.T10.a; Design notes 7, 12 and 14; `decision-r08-t10a-leans.md`): each lit body's
 * `body_detail` asked, its medium built in the optics workers (`opticsPool.ts`), its per-planet
 * tables made and held, its labels and its problems.
 *
 * @remarks
 * **What is wanted.** Each view opens a handle ({@link AtmosphereCache.openView}) and notes each
 * frame's bodies through it. A body in the `disc`, `mesh` or `terrain` regime is wanted (item 8),
 * as is the camera's local body where terrain draws it, which R08.T10.b passes as `terrain`; a
 * point never is. A kept scene's body brings its own setting and is never asked for.
 *
 * **Requests** (`body_detail`, `lib/system/bodiesWire.ts`'s `toBodyDetailRequest`), at most one a
 * body a trigger: the body's first want; a change of the scene's arrival; a change of its granted
 * detail; an event on it ({@link AtmosphereCache.noteBodyEvent}, for P14.T31's `body_events`, which
 * no server sends today); and, after `link_lost` or while the link is down, the link's return. A
 * changed light set rebuilds from the reply held, with no request. A reply whose key
 * (`atmosphereKey`) equals the drawn entry's keeps the entry, with no rebuild.
 *
 * **What an answer becomes** (`decision-r08-t10a-leans.md`, follow-up to item 6), by one function:
 *
 * - a server too busy (`too_many_requests`, `queue_full`, its `cancelled`) refuses nothing: the
 *   body is asked again by itself after `FIELD_RETRY_DELAY_MS`, doubling to
 *   `FIELD_RETRY_MAX_DELAY_MS` (`terrain/coarseFieldStore.ts`'s), with no line and no log, the
 *   count reset by any answer taken; the wait stops while no view wants the body;
 * - any other refusal is held as `rejected`, with the server's code and reason, and logs nothing;
 * - an answer `toBodyDetail` cannot take, or `protocol_violation`, is held as `invalid`, with one
 *   `console.error`;
 * - each of the two is asked afresh at the body's next trigger and once failures are forgotten,
 *   and cleared by an answer taken.
 *
 * **A failed build** (item 6), a throw from the assembly or from the tables' factory other than
 * `EngineUnavailable`, is held as `failed`, with one `console.error`; it is retried at the next
 * trigger and once failures are forgotten, and cleared by a build that lands. A throw from a
 * view's drawing of a body's atmosphere (`HillaireAtmosphere.over`, `skySources`, `drawFrame`),
 * other than a lost device's, is item 6's failure (`decision-r08-t10b-leans.md` §2). The renderer
 * reports it to the cache ({@link AtmosphereView.reportDrawFailure}), which holds the body
 * `failed`, logs once, and raises `ATMOSPHERE NOT AVAILABLE: computation failed for
 * <designations>, reopen VIEW to retry` on every view that draws it larger than a point. It is
 * retried at the next trigger and when `VIEW` is shown again. A kept scene whose media's load, once
 * a page, failed sets its bodies `failed`, held as `loadFailed`, whose line reads `relaunch to
 * retry` and stands when `VIEW` is shown again (`decision-r08-t10b-ux.md`, item 4). `EngineUnavailable`
 * raises nothing, and the tables are made again at a later frame, after the restore. A body with a
 * problem draws no atmosphere and carries no atmosphere label.
 *
 * **`ATMOSPHERE: PENDING`** stands for a server body asked, or building, with nothing of it drawn;
 * none while a re-request's previous state is drawn. Each view holds it with its own
 * {@link PendingLineHold} (item 9).
 *
 * **`ATMOSPHERE: APPROXIMATE`'s reasons** (R08.T10.c; `decision-r08-approximate-plate.md`, item 1b)
 * are reported by body beside the scene's labels ({@link AtmosphereViewReport.reasons}), for each
 * wanted body drawn with the label, so that the label block names each reason and its bodies.
 *
 * **The tables** are made through the injected factory ({@link atmosphereTablesOn} in the app) and
 * owned here (item 3): R08.T10.b hands them to each view's `HillaireAtmosphere`, which does not
 * dispose them. Their bytes, each entry's `rowBytes` as built, are held to
 * {@link ATMOSPHERE_CACHE_BYTES} by evicting the least recently wanted (item 10); a body wanted
 * this frame by any view is never evicted. Eviction disposes the tables and keeps the assembled
 * medium and the reply, so that a body wanted again has its tables made at once, with no request.
 * A new arrival lets go every body no view wants, its reply belonging to the scene left.
 * `AtmosphereTables.dispose` releases the tables' buffer and textures (R08.T10.b), so an eviction
 * frees the body's per-planet memory; each view takes a body's tables afresh every frame
 * (`PhotorealFrame.atmospheres`), so none draws with tables disposed.
 *
 * **The thick bake** (R08.T14.c; Design note 9's routing). A drawn body whose regime the router
 * routes to its bake ({@link AtmosphereCacheOptions.routes}, `thick/routing.ts`'s
 * `thickBakeRouted`, which routes none until R08.T14.d's gates pass) has its bake asked once its
 * tables are made, every band in one request ({@link AtmosphereBuilder.bakeThick}), and the bake
 * given to its tables when it lands (`AtmosphereTables.setThickTables`). Its labels follow the
 * bake, at most one of `ATMOSPHERE: COMPUTING` and `ATMOSPHERE: APPROXIMATE` (`labels.ts`'s
 * precedence; the guide's rows):
 *
 * - while it bakes, `atmosphereComputing` stands in `atmosphereApproximate`'s place, and no reason
 *   is reported;
 * - baked, `THICK_REASON` is dropped, and `atmosphereApproximate` stands only where another reason
 *   still holds it, such as an NH₄SH stand-in's;
 * - failed, `atmosphereApproximate` stands with every reason, `THICK_REASON` among them, until the
 *   body's atmosphere is next computed: a new entry drawn for it (its new key's build landed, or a
 *   kept scene's new setting), or a new cache. A failed bake is never item 6's fault: it raises no
 *   `ATMOSPHERE NOT AVAILABLE` and no problem (decision-r08-t10a-leans, UX review item 1, "For
 *   T14.c").
 *
 * The baked tables are kept with the entry, so that its tables made again after an eviction, or
 * refused them while the device was lost, are given them with no new bake. A new entry drawn for the
 * body, the body dropped, or the cache disposed cancels its bake. A body the router does not route
 * asks for none and is drawn as before.
 */

import type {
  BodyIdHex,
  DetailLevelDto,
  PendingRequest,
  RequestOf,
  RequestOutcome,
  SystemIdHex,
  UniverseIdHex,
  UniverseTime,
} from "@hyperion/protocol";
import { isBulkAssemblyFailure } from "@hyperion/protocol";

import { isRefusal } from "../../lib/refusal";
import { toBodyDetail, toBodyDetailRequest } from "../../lib/system/bodiesWire";
import type { BodyDetail } from "../../lib/system/model";
import { heldAppearanceOf } from "../appearance/fromWire";
import type { LitRegime } from "../bodies/regime";
import { EngineUnavailable } from "../engine/resilientEngine";
import type { RenderEngine } from "../engine/types";
import { SUN_BAKE_SPECTRUM } from "../lighting/hostDisc";
import type { Rgb } from "../photometry/toneCurve";
import { ANNUNCIATION_CLEAR_MS, ANNUNCIATION_ONSET_MS } from "../terrain/annunciation";
import { FIELD_RETRY_DELAY_MS, FIELD_RETRY_MAX_DELAY_MS } from "../terrain/coarseFieldStore";
import type { AssembledMedium } from "./assemble";
import {
  ATMOSPHERE_OPTICS_VERSION,
  atmosphereKey,
  type BodyAtmosphereInput,
  type BodyLight,
  clampedGroundAlbedo,
  type KeptAtmosphere,
} from "./atmosphereInput";
import {
  ATMOSPHERE_ANSWER_UNREADABLE,
  ATMOSPHERE_LABELS,
  type AtmosphereLabel,
  type AtmosphereProblem,
} from "./labels";
import type { AtmosphereMedium } from "./medium";
import { bodyGravity } from "./oblate";
import { keptBakeLightsOf } from "./opticsMessages";
import type { AtmosphereBuilder, OpticsBuild, ThickBake, ThickBakeAsk } from "./opticsPool";
import type { BakeSpectrum } from "./spectralColour";
import { type AbsorberCurveSlots, type AtmosphereBody, AtmosphereTables } from "./tables";
import type { MsSourceTable, ThickBakeBody } from "./thick/bake";
import { spectralColumnOf } from "./thick/columnLayout";
import {
  approximateRegime,
  type AtmosphereRegime,
  type BodyCover,
  CLEAR_COVER,
  classifyRegime,
  THICK_REASON,
} from "./thick/regime";
import { thickBakeRouted } from "./thick/routing";
import type { SpectralSource } from "./thick/spectralColumn";

/**
 * The cache's size, B: 32,000,000 of per-planet rows, decimal as `PER_PLANET_ROW_BYTES` is,
 * counted by each entry's tables' `rowBytes` as built (`decision-r08-t10a-leans.md` item 10):
 * about 73 measured Earths with one star (439,200 B since R08.T11.e; 56 at 570,264 B before), or 16
 * bodies at the 2 MB row. Only the bodies
 * wanted this frame can take it past this.
 */
export const ATMOSPHERE_CACHE_BYTES = 32_000_000;

/**
 * Where `body_detail` is asked: shaped as the terrain session's `TerrainRequests`
 * (`decision-r08-t10a-leans.md` item 4), the one kind the cache asks, which the link's request
 * client takes as it is; R08.T10.b joins the fetch with R10.T16.a's (`BodyDetailShare`).
 */
export interface AtmosphereRequests {
  /** Asks a body's record; the link's request client, or the `body_detail` share's (R08.T10.b). */
  request(body: RequestOf<"body_detail">): PendingRequest<"body_detail">;
}

/**
 * What the cache needs of a body's per-planet tables (item 3): `AtmosphereTables`' own, with what
 * the thick bake's route reads of them, their plan's bands and the body and ground they are built
 * for, and gives them, its bake (R08.T14.c).
 */
export type CachedTables = Pick<
  AtmosphereTables,
  "rowBytes" | "dispose" | "setThickTables" | "plan" | "body" | "bottomRadiusM"
>;

/** What a body's per-planet tables are made from. */
export interface BuiltAtmosphere {
  readonly medium: AtmosphereMedium;
  /** R_ref, m: the tables' ground. */
  readonly bottomRadiusM: number;
  /** Each light slot's curves, in `lightsAt`'s order: none for a medium with no absorber. */
  readonly curves: AbsorberCurveSlots;
  /** The body the tables are sliced for; `null` for one slice and one band. */
  readonly body: AtmosphereBody | null;
  /**
   * The source of each of the medium's terms, in their order, for its thick bake's spectral column
   * (`thick/columnLayout.ts`'s `spectralColumnOf`; R08.T14.c): a server body's assembly's
   * (`DrawnMedium.sources`), a kept case's; absent for a kept medium that cannot bake.
   */
  readonly sources?: ReadonlyArray<SpectralSource>;
}

/**
 * Makes a body's per-planet tables.
 *
 * @throws `EngineUnavailable` while the engine has no device, which the cache passes in silence
 *   and asks again at a later frame; anything else is the body's `failed` problem.
 */
export type AtmosphereTablesFactory<T extends CachedTables> = (built: BuiltAtmosphere) => T;

/**
 * The app's factory: `new AtmosphereTables(engine, …)` on the engine's device, which rebuilds them
 * itself after a device loss (`AtmosphereTables`' remarks).
 */
export function atmosphereTablesOn(
  engine: RenderEngine,
): AtmosphereTablesFactory<AtmosphereTables> {
  return (built) =>
    new AtmosphereTables(engine, built.medium, built.bottomRadiusM, built.curves, built.body);
}

/**
 * A kept scene's setting of a body's atmosphere: none set, airless, or a medium the scene built
 * with its curves and labels (`reference/cases.ts`'s fixtures), taken as it is, its tables made
 * here. The router (`thick/regime.ts`'s `classifyRegime`) routes a scene's medium under its cover,
 * and a regime past `thin` adds `atmosphereApproximate` with its reason, `THICK_REASON`
 * (R08.T13.d; R08.T10.c gave the provisional thick line's the same way). A scene's own labels
 * carry no reasons: one that holds `atmosphereApproximate` would stand on the plate with none,
 * which no kept scene passes.
 */
export type KeptBodyAtmosphere =
  | KeptAtmosphere
  | {
      readonly kind: "medium";
      /** Compared by identity: another object makes the body's tables again. */
      readonly built: BuiltAtmosphere;
      readonly labels: ReadonlyArray<AtmosphereLabel>;
      /**
       * The body's cover as the router reads it; `CLEAR_COVER` if absent, a clear sky over a
       * ground, as the reference cases were traced (R08.T13.d). Compared by identity as `built`
       * is: another object routes the body again and, as another `built` does, makes its tables
       * again, so a scene holds one cover object for as long as the body's cover stands.
       */
      readonly cover?: BodyCover;
    }
  /**
   * A medium the scene is still building, its fixtures' optics loading lazily (R08.T10.b): nothing
   * drawn, and `ATMOSPHERE: PENDING` under the view's hold, as a server body's asked record.
   */
  | { readonly kind: "loading" }
  /**
   * A medium the scene could not build, its media's load, once a page, having failed (R08.T10.b;
   * `decision-r08-t10b-ux.md`, item 4): item 6's fault, held as `loadFailed` with the scene's own
   * one log, `relaunch to retry`, and standing when `VIEW` is shown again; never `unset`.
   */
  | { readonly kind: "failed" };

/** The scene a view's frame shows, as its requests take it. */
export interface AtmosphereScene {
  readonly universe: UniverseIdHex;
  readonly system: SystemIdHex;
  /** The system's designation of record, which `toBodyDetail` names the record by. */
  readonly designation: string;
  /** The scene's time, at which a body's record is asked. */
  readonly time: UniverseTime;
  /**
   * The scene's arrival, compared by identity: another object asks again for every wanted body,
   * and lets go every body no view wants. The views of one scene pass the same object.
   */
  readonly arrival: object;
  /** Each body's granted detail level; a change asks again for the body. */
  readonly grants: ReadonlyMap<BodyIdHex, DetailLevelDto>;
  /** Whether the link to the server is up; no request is sent while it is down. */
  readonly linkUp: boolean;
}

/** One of a frame's lit bodies. */
export interface AtmosphereFrameBody {
  readonly id: BodyIdHex;
  /** Its regime on the view (R07's `planLitBodies`): wanted unless a `point`. */
  readonly regime: LitRegime;
  /** The stars lighting it, `lightsAt`'s, brightest first, at most `LIGHT_SLOT_LIMIT` (2). */
  readonly lights: ReadonlyArray<BodyLight>;
  /** A kept scene's setting; absent for a server body, whose record is asked. */
  readonly kept?: KeptBodyAtmosphere;
}

/** One frame of a view, as {@link AtmosphereView.noteFrame} takes it. */
export interface AtmosphereFrame {
  /** A monotonic time, ms, such as `performance.now()`: the pending line's hold reads it. */
  readonly nowMs: number;
  /** The server scene; `null` for a kept scene, whose bodies are all kept. */
  readonly scene: AtmosphereScene | null;
  readonly bodies: ReadonlyArray<AtmosphereFrameBody>;
}

/** A body's atmosphere as drawn. */
export interface AtmosphereEntry<T> {
  /** `atmosphereKey`'s, or `kept/<kind>/v<version>` for a kept scene's setting. */
  readonly key: string;
  /** Its labels, each once, in `ATMOSPHERE_LABELS`' order. */
  readonly labels: ReadonlyArray<AtmosphereLabel>;
  /** Why it carries `atmosphereApproximate`, as player text; empty without it. */
  readonly reasons: ReadonlyArray<string>;
  /**
   * How its medium is drawn, `classifyRegime`'s (R08.T13.d): the assembly's for a server body, the
   * cache's own for a kept scene's medium; `null` where it draws no medium.
   */
  readonly regime: AtmosphereRegime | null;
  /**
   * What its tables are made from; `null` where it draws no medium, or holds an absorber and no
   * light lights it, so that no curve exists.
   */
  readonly built: BuiltAtmosphere | null;
  /** Its per-planet tables; `null` with no `built`, after an eviction, or while the device is lost. */
  readonly tables: T | null;
}

/** What a view's frame draws and says of its wanted bodies. */
export interface AtmosphereViewReport {
  /**
   * The labels of its wanted bodies drawn, each once, in `ATMOSPHERE_LABELS`' order, with
   * `atmospherePending` while the view's {@link PendingLineHold} shows it.
   */
  readonly labels: ReadonlyArray<AtmosphereLabel>;
  /** Each wanted body's problem, where it has one. */
  readonly problems: ReadonlyMap<BodyIdHex, AtmosphereProblem>;
  /**
   * Why each wanted body drawn with `atmosphereApproximate` carries it, as player text, in its
   * entry's order (R08.T10.c; `decision-r08-approximate-plate.md`, item 1b), so that the label block
   * names each reason and its bodies: none for a body without the label (one with
   * `atmosphereComputing` in its place among them), with a problem, pending, or whose entry holds
   * no reason.
   */
  readonly reasons: ReadonlyMap<BodyIdHex, ReadonlyArray<string>>;
  /** Whether some wanted body is pending this frame, before the hold. */
  readonly pending: boolean;
}

/** One photorealistic view's handle on its device's cache. */
export interface AtmosphereView {
  /**
   * Notes the view's frame: asks for, builds and draws its wanted bodies, and returns what it says
   * of them. Does nothing once the view is closed or the cache disposed.
   */
  noteFrame(frame: AtmosphereFrame): AtmosphereViewReport;
  /**
   * Reports that the view's drawing of `body`'s atmosphere threw (`decision-r08-t10b-leans.md`
   * §2): item 6's failure, which holds the body `failed` for every view, with one `console.error`
   * naming `viewName`, and none for a body already failed. A lost device's throw is not reported.
   */
  reportDrawFailure(body: BodyIdHex, error: unknown, viewName: string): void;
  /** Lets the view's bodies go, as no longer wanted by it. */
  close(): void;
}

/**
 * Schedules `run` after `delayMs`, returning what cancels it: `setTimeout`'s by default, which
 * tests control with fake timers.
 */
export type AtmosphereSchedule = (run: () => void, delayMs: number) => () => void;

/** What a cache is made with. */
export interface AtmosphereCacheOptions<T extends CachedTables> {
  readonly requests: AtmosphereRequests;
  /** The optics pool (`opticsPool.ts`'s `OpticsWorkerPool`), which the cache owns and terminates. */
  readonly builder: AtmosphereBuilder;
  readonly tables: AtmosphereTablesFactory<T>;
  /** How the busy re-asks wait; `setTimeout` if absent. */
  readonly schedule?: AtmosphereSchedule;
  /**
   * Whether a body drawn in `regime` takes its thick bake (R08.T14.c): `thick/routing.ts`'s
   * `thickBakeRouted` if absent, which routes no regime until R08.T14.d's gates pass; a test's
   * routing otherwise.
   */
  readonly routes?: (regime: AtmosphereRegime) => boolean;
}

/**
 * The hold of a view's `ATMOSPHERE: PENDING` line, as R05's terrain annunciation holds its own
 * (`terrain/annunciation.ts`; `decision-r08-t10a-leans.md` item 9): it shows once some wanted body
 * has been pending for `ANNUNCIATION_ONSET_MS` (250 ms) on end and goes once none has been for
 * `ANNUNCIATION_CLEAR_MS` (1 s), so that it comes on at most once in any 1.25 s, well under the
 * guide's three flashes a second, and a reply drawn within 250 ms shows nothing.
 */
export class PendingLineHold {
  #shown = false;
  #holds = false;
  /** When the condition last changed, ms. */
  #sinceMs = Number.NEGATIVE_INFINITY;

  /**
   * Takes one frame's condition and returns whether the line shows.
   *
   * @param nowMs - A monotonic time, ms.
   */
  update(holds: boolean, nowMs: number): boolean {
    if (holds !== this.#holds) {
      this.#holds = holds;
      this.#sinceMs = nowMs;
    }
    const heldMs = nowMs - this.#sinceMs;
    if (!this.#shown && holds && heldMs >= ANNUNCIATION_ONSET_MS) {
      this.#shown = true;
    } else if (this.#shown && !holds && heldMs >= ANNUNCIATION_CLEAR_MS) {
      this.#shown = false;
    }
    return this.#shown;
  }
}

/** The regimes whose bodies are wanted (`decision-r08-t10a-leans.md` item 8). */
const WANTED_REGIMES: ReadonlySet<LitRegime> = new Set<LitRegime>(["disc", "mesh", "terrain"]);

/** Where a server body's record stands. */
type Fetch =
  | { readonly kind: "idle" }
  /** Asked, the answer awaited. */
  | { readonly kind: "asking"; readonly cancel: () => void }
  /** To be asked once the link is up. */
  | { readonly kind: "waiting" }
  /** Answered busy: asked again when its wait ends, or when wanted again after it stopped. */
  | BusyFetch;

/** A busy answer's wait. */
interface BusyFetch {
  readonly kind: "busy";
  /** Cancels the wait; `null` once it stopped because no view wanted the body. */
  cancel: (() => void) | null;
}

/** An entry's labels and the reasons behind its `atmosphereApproximate`. */
interface Labelled {
  readonly labels: ReadonlyArray<AtmosphereLabel>;
  readonly reasons: ReadonlyArray<string>;
}

/** An entry to draw, its tables not yet made. */
interface ToDraw extends Labelled {
  readonly key: string;
  readonly regime: AtmosphereRegime | null;
  readonly built: BuiltAtmosphere | null;
  /**
   * The input a server body was built from, which its thick bake asks with; `null` for a kept
   * scene's body, whose bake carries its own lights.
   */
  readonly input: BodyAtmosphereInput | null;
}

/** A server body's build under way. */
interface Building {
  readonly kind: "building";
  readonly key: string;
  readonly input: BodyAtmosphereInput;
  readonly handle: OpticsBuild;
}

/** Where a body's build stands. */
type Build =
  | Building
  /** Built, to be drawn at the next frame that wants the body. */
  | { readonly kind: "landed"; readonly key: string; readonly entry: ToDraw };

/** Where a drawn entry's thick bake stands (R08.T14.c). */
type BakeState<T> =
  | { readonly kind: "baking"; readonly handle: ThickBake }
  | {
      readonly kind: "baked";
      /** The bake's tables, one a band of the tables' plan, kept for tables made again. */
      readonly tables: ReadonlyArray<MsSourceTable>;
      /** The tables that hold them; `null` before any did, or once another set stands. */
      given: T | null;
    }
  /** Its bake failed: `atmosphereApproximate` stands with every reason. */
  | { readonly kind: "failed" };

/** A drawn entry's thick bake, with the labels and reasons the entry was made with. */
interface HeldBake<T> {
  /** The entry's labels and reasons before the bake's state is read into them. */
  readonly made: Labelled;
  readonly state: BakeState<T>;
}

/** A body as the cache holds it. */
interface HeldBody<T> {
  readonly id: BodyIdHex;
  /** Whether it is a kept scene's body, whose setting comes in the frame. */
  readonly kept: boolean;
  /** The kept setting last made, or `null` for none yet (or forgotten failures). */
  keptSetting: KeptBodyAtmosphere | null;
  /** The scene it was last wanted in, which a busy re-ask takes. */
  scene: AtmosphereScene | null;
  fetch: Fetch;
  /** What it was last asked under; `null` before its first want, or to be asked afresh. */
  asked: { readonly arrival: object; readonly grant: DetailLevelDto | undefined } | null;
  eventPending: boolean;
  /** Busy answers in a row, which set the next wait. */
  busyAnswers: number;
  /** Its lights as last noted. */
  lights: ReadonlyArray<BodyLight>;
  /** The reply taken, for the scene, which a changed light set or an eviction builds from. */
  reply: BodyDetail | null;
  build: Build | null;
  drawn: AtmosphereEntry<T> | null;
  /** The drawn entry's thick bake; `null` where none is asked, or nothing is drawn. */
  bake: HeldBake<T> | null;
  problem: AtmosphereProblem | null;
}

const IDLE: Fetch = { kind: "idle" };
const WAITING: Fetch = { kind: "waiting" };
const FAILED: AtmosphereProblem = { kind: "failed" };
const LOAD_FAILED: AtmosphereProblem = { kind: "loadFailed" };
const BAKE_FAILED = { kind: "failed" } as const;

/**
 * What a kept body's thick bake carries as its input (R08.T14.c): no medium of its own, since a
 * scene's medium is handed to the cache whole and never assembled, so that no `KeptAtmosphere`
 * names it. The worker reads only its line species from it, none for a kept input, and takes the
 * body's absorber specs and light spectra from the request's `kept` lights in its place
 * (`opticsJobs.ts`'s `#bakeLights`). Its key, `kept/unset/v<version>`, is no body's: the cache
 * keys a kept medium itself (`kept/medium/v<version>`).
 */
const KEPT_BAKE_INPUT: BodyAtmosphereInput = { kind: "kept", atmosphere: { kind: "unset" } };

/**
 * The reference Sun's spectrum in the bake bins, under which the kept cases' curves of growth were
 * formed: R07's `SUN_BAKE_SPECTRUM`, of which `earth.ts`'s `R06_SOLAR_BAKE_SPECTRUM` is the same
 * values through `bakeSpectrumOf` (`reference/cases.ts`'s `referenceCaseBakeLights`), taken from
 * the host disc's module since `earth.ts` would load the absorbers' data on the render thread.
 */
const REFERENCE_SUN_BAKE_SPECTRUM: BakeSpectrum = SUN_BAKE_SPECTRUM;

/**
 * The bulk gravity a sphere's bake body is given, m s⁻² (standard gravity): a sphere with no spin
 * has one band at s = 1 whatever it is, which the bake reads nowhere (`thick/keptBake.ts`'s
 * `sphereBakeBody`, `hillaire.ts`'s `bodyOfFigure`).
 */
const SPHERE_BAKE_GRAVITY_MS2 = 9.806_65;

/** A report of nothing: a closed view's, or a disposed cache's. */
const NO_REPORT: AtmosphereViewReport = {
  labels: [],
  problems: new Map(),
  reasons: new Map(),
  pending: false,
};

/** What a `body_detail`'s outcome is to the cache. */
type DetailAnswer =
  | { readonly kind: "answer"; readonly detail: BodyDetail }
  | { readonly kind: "link" }
  | { readonly kind: "cancelled" }
  | { readonly kind: "busy" }
  | { readonly kind: "rejected"; readonly code: string; readonly message: string }
  | { readonly kind: "invalid"; readonly check: string; readonly detail: unknown };

/**
 * What a `body_detail`'s outcome is to the cache (`decision-r08-t10a-leans.md`, follow-up to item
 * 6): the record converted, a lost link, the client's own cancel, a server too busy, a refusal, or
 * an answer it cannot take, as `CoarseFieldStore` reads a field's.
 */
function detailAnswerOf(
  outcome: RequestOutcome<"body_detail">,
  scene: AtmosphereScene,
): DetailAnswer {
  if (outcome.ok) {
    const converted = toBodyDetail(outcome.response, scene.system, scene.designation);
    return converted.kind === "ok"
      ? { kind: "answer", detail: converted.detail }
      : { kind: "invalid", check: converted.fault, detail: converted.fault };
  }
  const { error } = outcome;
  const rejected: DetailAnswer = { kind: "rejected", code: error.code, message: error.message };
  // The client's own finding, an answer of another kind or of chunks not come whole: a check, not
  // the server's word.
  const unreadable: DetailAnswer = {
    kind: "invalid",
    check: ATMOSPHERE_ANSWER_UNREADABLE,
    detail: error,
  };
  // Every code by name, so that a code the protocol adds is a compile error here, not a refusal.
  let answer: DetailAnswer;
  switch (error.code) {
    case "link_lost":
      answer = { kind: "link" };
      break;
    case "aborted":
    case "superseded":
      answer = { kind: "cancelled" };
      break;
    case "too_many_requests":
    case "queue_full":
    case "cancelled":
      answer = { kind: "busy" };
      break;
    case "protocol_violation":
      answer = unreadable;
      break;
    case "internal":
      answer = isBulkAssemblyFailure(error) ? unreadable : rejected;
      break;
    case "bad_request":
    case "unsupported":
    case "hello_required":
    case "unknown_universe":
    case "unknown_system":
    case "unknown_body":
    case "generator_version_mismatch":
    case "unsupported_save_format":
    case "name_taken":
    case "universe_limit_reached":
    case "storage_failed":
      answer = rejected;
      break;
  }
  return answer;
}

/** A record's ground albedo: p q per channel of R07's photometry, clamped to [0, 1] (item 2). */
function groundAlbedoOf(detail: BodyDetail): Rgb {
  const { geometricAlbedo: p, phaseIntegral: q } = heldAppearanceOf(detail.record).photometry;
  return clampedGroundAlbedo([p[0] * q[0], p[1] * q[1], p[2] * q[2]]);
}

/** Whether two light sets are the same stars with the same spectra, in the same order. */
function sameLights(a: ReadonlyArray<BodyLight>, b: ReadonlyArray<BodyLight>): boolean {
  return (
    a === b ||
    (a.length === b.length &&
      a.every((light, k) => {
        const other = b[k];
        return (
          other !== undefined &&
          other.star === light.star &&
          (other.spectrum === light.spectrum ||
            light.spectrum.every((value, n) => Object.is(value, other.spectrum[n])))
        );
      }))
  );
}

/** Whether a kept setting is the one already made. */
function sameKept(a: KeptBodyAtmosphere | null, b: KeptBodyAtmosphere): boolean {
  if (a === null || a.kind !== b.kind) {
    return false;
  }
  return a.kind !== "medium" || (b.kind === "medium" && a.built === b.built && a.cover === b.cover);
}

/** Labels each once, in `ATMOSPHERE_LABELS`' order. */
function ordered(labels: Iterable<AtmosphereLabel>): ReadonlyArray<AtmosphereLabel> {
  const held = new Set(labels);
  return ATMOSPHERE_LABELS.filter((label) => held.has(label));
}

/**
 * What an assembly of `input` draws: no medium; or its medium, whose tables need each absorber's
 * curves under at least one light, with its terms' sources for its thick bake.
 */
function toDrawOf(key: string, input: BodyAtmosphereInput, assembled: AssembledMedium): ToDraw {
  const { labels, reasons } = assembled;
  if (assembled.medium === undefined) {
    return { key, labels, reasons, regime: null, built: null, input };
  }
  const { drawn } = assembled;
  const curved = drawn.absorbers.length === 0 || drawn.curves.length > 0;
  return {
    key,
    labels,
    reasons,
    regime: assembled.regime,
    built: curved
      ? {
          medium: assembled.medium,
          bottomRadiusM: drawn.bottomRadiusM,
          curves: drawn.curves,
          body: drawn.body,
          sources: drawn.sources,
        }
      : null,
    input,
  };
}

/**
 * An entry's labels and reasons under its thick bake's state (the module's remarks): while it
 * bakes, `atmosphereComputing` in `atmosphereApproximate`'s place and no reason; baked, the made
 * reasons but `THICK_REASON`, and `atmosphereApproximate` only where one remains; failed,
 * `atmosphereApproximate` with every made reason and `THICK_REASON`.
 */
function bakeLabelsOf(made: Labelled, state: BakeState<unknown>["kind"]): Labelled {
  const others = made.labels.filter((label) => label !== "atmosphereApproximate");
  let labelled: Labelled;
  switch (state) {
    case "baking":
      labelled = { labels: ordered([...others, "atmosphereComputing"]), reasons: [] };
      break;
    case "baked": {
      const reasons = made.reasons.filter((reason) => reason !== THICK_REASON);
      labelled = {
        labels: ordered(reasons.length > 0 ? [...others, "atmosphereApproximate"] : others),
        reasons,
      };
      break;
    }
    case "failed":
      labelled = {
        labels: ordered([...made.labels, "atmosphereApproximate"]),
        reasons: made.reasons.includes(THICK_REASON)
          ? made.reasons
          : [...made.reasons, THICK_REASON],
      };
      break;
  }
  return labelled;
}

/**
 * The body a thick bake of `tables` is for: their built body, its slicing widened over the 2 MB row
 * where `tables.ts`'s `rowBodyOf` widened it, on their ground R_ref; or, for tables of one slice
 * and one band (`body` `null`), a sphere of their ground's radius with no spin, as
 * `thick/keptBake.ts`'s `sphereBakeBody` makes it, whose one band stands at s = 1 on R_ref.
 */
function bakeBodyOf(tables: CachedTables): ThickBakeBody {
  const { body, bottomRadiusM } = tables;
  if (body !== null) {
    return { figure: body.figure, gravity: body.gravity, bottomRadiusM };
  }
  const figure = { equatorialRadiusM: bottomRadiusM, polarRadiusM: bottomRadiusM };
  return {
    figure,
    gravity: bodyGravity(
      {
        figure,
        massKg: 1,
        angularVelocityRadS: null,
        bulkGravityMS2: SPHERE_BAKE_GRAVITY_MS2,
        figureLaw: null,
      },
      bottomRadiusM,
    ),
    bottomRadiusM,
  };
}

/**
 * A routed body's thick bake request (R08.T14.c), every band and light slot in one, or `null`
 * where no light lights it to bake under.
 *
 * @remarks
 * - **The column** is `spectralColumnOf` of the medium and its terms' sources.
 * - **The body** is {@link bakeBodyOf}'s, and **the bands** every band of the tables' plan, in its
 *   order: `thick/bake.ts`'s `thickBakeBands(body)`, one a band of the same slicing by the same
 *   rule (held equal by `bake.test.ts`), so that `setThickTables` takes the bake as it lands.
 * - **The slots** are the light slots in the order the kernels number them, a source's slot k
 *   reading the table's layer b × slots + k (`thick/read.ts`). A server body's are its build's
 *   input's lights in `lightsAt`'s order: the assembly's `DrawnMedium.spectra[k]` is
 *   `input.lights[k]`'s spectrum, `DrawnMedium.curves[k]` that light's curves (the pool's join,
 *   one a slot), and the worker's `absorberSpecsOf(input).spectra`, which a request's slots index,
 *   the same list as `DrawnMedium.spectra`. So slot k is `input.lights[k]`.
 * - **A kept body's lights** go in `kept`: its medium's absorber specs (`keptBakeLightsOf`) and the
 *   spectra of the lights it is drawn under. Those are the scene's own suns where the frame gives
 *   the body lights, each `BodyLight.spectrum`, R06's bake spectrum, one slot a light in
 *   `lightsAt`'s order; with none, the reference Sun alone
 *   ({@link REFERENCE_SUN_BAKE_SPECTRUM}), under which the kept cases' curves were formed. Its
 *   input is {@link KEPT_BAKE_INPUT}.
 *
 * @param input - A server body's build's input; `null` for a kept body.
 * @param lights - The body's lights as last noted, a kept body's spectra.
 * @throws RangeError as `spectralColumnOf` refuses the sources: a bug in what the body was built
 *   from.
 */
function thickBakeAskOf(
  input: BodyAtmosphereInput | null,
  built: BuiltAtmosphere,
  sources: ReadonlyArray<SpectralSource>,
  tables: CachedTables,
  lights: ReadonlyArray<BodyLight>,
): ThickBakeAsk | null {
  const column = spectralColumnOf(built.medium, sources);
  const body = bakeBodyOf(tables);
  const bands = tables.plan.bands.map((_, band) => band);
  if (input !== null) {
    const slots = input.kind === "record" ? input.lights.map((_, slot) => slot) : [];
    return slots.length === 0 ? null : { input, slots, bands, column, body };
  }
  const spectra =
    lights.length > 0 ? lights.map((light) => light.spectrum) : [REFERENCE_SUN_BAKE_SPECTRUM];
  return {
    input: KEPT_BAKE_INPUT,
    slots: spectra.map((_, slot) => slot),
    bands,
    column,
    body,
    kept: keptBakeLightsOf(built.medium, spectra),
  };
}

/** `setTimeout`'s schedule. */
const scheduleTimeout: AtmosphereSchedule = (run, delayMs) => {
  const timer = setTimeout(run, delayMs);
  return () => {
    clearTimeout(timer);
  };
};

/** One device's atmospheres for every view (see the module's remarks). */
export class AtmosphereCache<T extends CachedTables = AtmosphereTables> {
  readonly #requests: AtmosphereRequests;
  readonly #builder: AtmosphereBuilder;
  readonly #tables: AtmosphereTablesFactory<T>;
  readonly #schedule: AtmosphereSchedule;
  readonly #routes: (regime: AtmosphereRegime) => boolean;
  /** Every body held, least recently wanted first. */
  readonly #bodies = new Map<BodyIdHex, HeldBody<T>>();
  /** Each open view's wanted bodies, as its latest frame noted them. */
  readonly #views = new Map<object, ReadonlySet<BodyIdHex>>();
  /** The arrival last noted. */
  #arrival: object | null = null;
  #disposed = false;

  constructor(opts: AtmosphereCacheOptions<T>) {
    this.#requests = opts.requests;
    this.#builder = opts.builder;
    this.#tables = opts.tables;
    this.#schedule = opts.schedule ?? scheduleTimeout;
    this.#routes = opts.routes ?? ((regime): boolean => thickBakeRouted(regime));
  }

  /** How many bodies the cache holds, drawn or not. */
  get size(): number {
    return this.#bodies.size;
  }

  /** The bytes of the tables held, B: each entry's `rowBytes` as built. */
  get bytes(): number {
    let bytes = 0;
    for (const held of this.#bodies.values()) {
      bytes += held.drawn?.tables?.rowBytes ?? 0;
    }
    return bytes;
  }

  /** Opens a view's handle, which notes its frames and holds its pending line. */
  openView(): AtmosphereView {
    const view = {};
    const hold = new PendingLineHold();
    let closed = false;
    this.#views.set(view, new Set());
    return {
      noteFrame: (frame) =>
        closed || this.#disposed ? NO_REPORT : this.#noteFrame(view, hold, frame),
      reportDrawFailure: (body, error, viewName) => {
        if (!closed && !this.#disposed) {
          this.#drawFailed(body, error, viewName);
        }
      },
      close: () => {
        if (closed) {
          return;
        }
        closed = true;
        this.#views.delete(view);
        this.#stopUnwantedWaits();
      },
    };
  }

  /** A body's atmosphere as drawn; `null` with nothing drawn, or with a problem. */
  entryOf(body: BodyIdHex): AtmosphereEntry<T> | null {
    const held = this.#bodies.get(body);
    return held === undefined || held.problem !== null ? null : held.drawn;
  }

  /** A body's labels: its entry's, `atmospherePending` while pending, none with a problem. */
  labelsOf(body: BodyIdHex): ReadonlyArray<AtmosphereLabel> {
    const held = this.#bodies.get(body);
    if (held === undefined || held.problem !== null) {
      return [];
    }
    if (held.drawn !== null) {
      return held.drawn.labels;
    }
    return isPending(held) ? ["atmospherePending"] : [];
  }

  /** A body's problem, where it has one. */
  problemOf(body: BodyIdHex): AtmosphereProblem | null {
    return this.#bodies.get(body)?.problem ?? null;
  }

  /**
   * Notes an event on a body (P14.T31's `body_events`, which no server sends today): the body is
   * asked again at the next frame that wants it.
   */
  noteBodyEvent(body: BodyIdHex): void {
    const held = this.#bodies.get(body);
    if (held !== undefined && !held.kept) {
      held.eventPending = true;
    }
  }

  /**
   * Forgets every body's problem, as R08.T10.b does when `VIEW` is shown again: a server body is
   * asked afresh, and a kept one's tables made again, at the next frame that wants it.
   */
  forgetFailures(): void {
    for (const held of this.#bodies.values()) {
      if (held.problem === null) {
        continue;
      }
      held.problem = null;
      if (held.kept) {
        held.keptSetting = null;
      } else {
        held.asked = null;
      }
    }
  }

  /** Cancels every request and wait, terminates the builder and disposes every table. */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    for (const held of this.#bodies.values()) {
      this.#release(held);
    }
    this.#bodies.clear();
    this.#views.clear();
    this.#builder.terminate();
  }

  #noteFrame(view: object, hold: PendingLineHold, frame: AtmosphereFrame): AtmosphereViewReport {
    const wanted = new Map<BodyIdHex, AtmosphereFrameBody>();
    for (const body of frame.bodies) {
      if (WANTED_REGIMES.has(body.regime)) {
        wanted.set(body.id, body);
      }
    }
    this.#views.set(view, new Set(wanted.keys()));
    const { scene } = frame;
    if (scene !== null && scene.arrival !== this.#arrival) {
      this.#arrival = scene.arrival;
      this.#letUnwantedGo();
    }
    for (const body of wanted.values()) {
      const held = this.#heldFor(body);
      if (body.kept !== undefined) {
        // A kept body's lights, which its scene's medium does not depend on, are its bake's.
        held.lights = body.lights;
        this.#keep(held, body.kept);
      } else if (scene !== null) {
        this.#want(held, body, scene);
      }
      this.#draw(held);
    }
    this.#stopUnwantedWaits();
    this.#evict();

    const labels = new Set<AtmosphereLabel>();
    const problems = new Map<BodyIdHex, AtmosphereProblem>();
    const reasons = new Map<BodyIdHex, ReadonlyArray<string>>();
    let anyPending = false;
    for (const id of wanted.keys()) {
      const held = this.#bodies.get(id);
      if (held === undefined) {
        continue;
      }
      if (held.problem !== null) {
        problems.set(id, held.problem);
      } else if (held.drawn !== null) {
        for (const label of held.drawn.labels) {
          labels.add(label);
        }
        if (held.drawn.labels.includes("atmosphereApproximate") && held.drawn.reasons.length > 0) {
          reasons.set(id, held.drawn.reasons);
        }
      } else if (isPending(held)) {
        anyPending = true;
      }
    }
    if (hold.update(anyPending, frame.nowMs)) {
      labels.add("atmospherePending");
    }
    return { labels: ordered(labels), problems, reasons, pending: anyPending };
  }

  /** The body held for a frame's body, made most recently wanted; made anew if its kind changed. */
  #heldFor(body: AtmosphereFrameBody): HeldBody<T> {
    const kept = body.kept !== undefined;
    let held = this.#bodies.get(body.id);
    if (held !== undefined && held.kept !== kept) {
      this.#release(held);
      held = undefined;
    }
    this.#bodies.delete(body.id);
    held ??= {
      id: body.id,
      kept,
      keptSetting: null,
      scene: null,
      fetch: IDLE,
      asked: null,
      eventPending: false,
      busyAnswers: 0,
      lights: body.lights,
      reply: null,
      build: null,
      drawn: null,
      bake: null,
      problem: null,
    };
    this.#bodies.set(body.id, held);
    return held;
  }

  /** Takes a kept scene's setting of a body, its entry made at once where it changed. */
  #keep(held: HeldBody<T>, kept: KeptBodyAtmosphere): void {
    if (sameKept(held.keptSetting, kept)) {
      return;
    }
    held.keptSetting = kept;
    held.problem = null;
    let entry: ToDraw;
    switch (kept.kind) {
      case "unset":
      case "airless":
        entry = {
          key: atmosphereKey({ kind: "kept", atmosphere: kept }),
          labels: kept.kind === "unset" ? ["atmosphereNotYetModelled"] : [],
          reasons: [],
          regime: null,
          built: null,
          input: null,
        };
        break;
      case "loading":
        held.build = null;
        this.#dropDrawn(held);
        return;
      case "failed":
        // The scene logged its failed load once; the cache holds item 6's fault and its line.
        held.build = null;
        this.#dropDrawn(held);
        held.problem = LOAD_FAILED;
        return;
      case "medium": {
        // The router's note carries its reason, so that no APPROXIMATE on a kept scene stands
        // without one; a scene's own labels carry none (R08.T10.c).
        const regime = classifyRegime(kept.built.medium, kept.cover ?? CLEAR_COVER);
        const approximate = approximateRegime(regime);
        entry = {
          key: `kept/medium/v${ATMOSPHERE_OPTICS_VERSION}`,
          labels: ordered([
            ...kept.labels,
            ...(approximate ? (["atmosphereApproximate"] as const) : []),
          ]),
          reasons: approximate ? [THICK_REASON] : [],
          regime,
          built: kept.built,
          input: null,
        };
        break;
      }
    }
    held.build = { kind: "landed", key: entry.key, entry };
  }

  /** Asks for a server body where a trigger calls for it, and rebuilds it for a changed light set. */
  #want(held: HeldBody<T>, body: AtmosphereFrameBody, scene: AtmosphereScene): void {
    held.scene = scene;
    const grant = scene.grants.get(body.id);
    const { asked, fetch } = held;
    if (
      asked === null ||
      asked.arrival !== scene.arrival ||
      asked.grant !== grant ||
      held.eventPending
    ) {
      held.eventPending = false;
      held.asked = { arrival: scene.arrival, grant };
      this.#ask(held, scene);
    } else if (fetch.kind === "waiting" && scene.linkUp) {
      this.#ask(held, scene);
    } else if (fetch.kind === "busy" && fetch.cancel === null) {
      // Its wait stopped while no view wanted it: asked again, a busy re-ask and not a trigger.
      this.#ask(held, scene);
    }
    if (!sameLights(held.lights, body.lights)) {
      held.lights = body.lights;
      if (held.problem === null) {
        this.#rebuild(held);
      }
    }
  }

  /** Asks for a body's record, cancelling a request or wait standing, or waits for the link. */
  #ask(held: HeldBody<T>, scene: AtmosphereScene): void {
    this.#stopFetch(held);
    if (!scene.linkUp) {
      held.fetch = WAITING;
      return;
    }
    const pending = this.#requests.request(
      toBodyDetailRequest(scene.universe, held.id, scene.time),
    );
    const asking: Fetch = {
      kind: "asking",
      cancel: () => {
        pending.cancel();
      },
    };
    held.fetch = asking;
    const take = async (): Promise<void> => {
      const outcome = await pending.outcome;
      if (this.#disposed || this.#bodies.get(held.id) !== held || held.fetch !== asking) {
        return;
      }
      this.#take(held, scene, detailAnswerOf(outcome, scene));
    };
    void take().catch((error: unknown) => {
      // A bug in taking the answer: the body is held failed, with the error in the log.
      if (!this.#disposed && this.#bodies.get(held.id) === held) {
        held.fetch = IDLE;
        this.#fail(held, error);
      } else {
        console.error(`the atmosphere's record of ${held.id} could not be taken:`, error);
      }
    });
  }

  /** Takes a body's answer (see the module's remarks). */
  #take(held: HeldBody<T>, scene: AtmosphereScene, answer: DetailAnswer): void {
    switch (answer.kind) {
      case "answer":
        held.fetch = IDLE;
        held.busyAnswers = 0;
        if (held.problem?.kind === "rejected" || held.problem?.kind === "invalid") {
          held.problem = null;
        }
        held.reply = answer.detail;
        this.#rebuild(held);
        return;
      case "link":
        held.fetch = WAITING;
        return;
      case "cancelled":
        held.fetch = IDLE;
        return;
      case "busy":
        held.busyAnswers += 1;
        this.#askLater(held, scene);
        return;
      case "rejected": {
        held.fetch = IDLE;
        held.busyAnswers = 0;
        const { code, message } = answer;
        this.#refuse(held, { kind: "rejected", code, message, refusal: isRefusal(code) });
        return;
      }
      case "invalid":
        held.fetch = IDLE;
        held.busyAnswers = 0;
        console.error(`the atmosphere's record of ${held.id} is not valid:`, answer.detail);
        this.#refuse(held, { kind: "invalid", check: answer.check });
        return;
    }
  }

  /** Asks for a body again after the server was too busy, waiting longer each time. */
  #askLater(held: HeldBody<T>, scene: AtmosphereScene): void {
    const delayMs = Math.min(
      FIELD_RETRY_MAX_DELAY_MS,
      FIELD_RETRY_DELAY_MS * 2 ** (held.busyAnswers - 1),
    );
    const busy: BusyFetch = { kind: "busy", cancel: null };
    held.fetch = busy;
    busy.cancel = this.#schedule(() => {
      if (this.#disposed || held.fetch !== busy) {
        return;
      }
      busy.cancel = null;
      this.#ask(held, held.scene ?? scene);
    }, delayMs);
  }

  /** Holds a refusal or an answer not taken: no reply, no build, nothing drawn. */
  #refuse(held: HeldBody<T>, problem: AtmosphereProblem): void {
    held.problem = problem;
    held.reply = null;
    this.#cancelBuild(held);
    this.#dropDrawn(held);
  }

  /** Builds a body from its reply, its lights and its ground, unless that is drawn or building. */
  #rebuild(held: HeldBody<T>): void {
    const { reply } = held;
    if (reply === null) {
      return;
    }
    let input: BodyAtmosphereInput;
    let key: string;
    try {
      input = {
        kind: "record",
        detail: reply,
        lights: held.lights,
        groundAlbedo: groundAlbedoOf(reply),
      };
      key = atmosphereKey(input);
    } catch (error: unknown) {
      this.#fail(held, error);
      return;
    }
    if (held.drawn?.key === key && held.problem === null) {
      this.#cancelBuild(held);
      return;
    }
    if (held.build?.key === key) {
      return;
    }
    this.#cancelBuild(held);
    let handle: OpticsBuild;
    try {
      handle = this.#builder.build(input);
    } catch (error: unknown) {
      this.#fail(held, error);
      return;
    }
    const building: Building = { kind: "building", key, input, handle };
    held.build = building;
    void this.#land(held, building, handle).catch((error: unknown) => {
      // A bug in taking the build: the body is held failed, with the error in the log, rather
      // than left pending on a build that will never land.
      if (!this.#disposed && this.#bodies.get(held.id) === held && held.build === building) {
        held.build = null;
        this.#fail(held, error);
      } else {
        console.error(`the atmosphere of ${held.id} could not be taken:`, error);
      }
    });
  }

  /** Takes a build's outcome, unless the body has moved on from it. */
  async #land(held: HeldBody<T>, building: Building, handle: OpticsBuild): Promise<void> {
    const outcome = await handle.outcome;
    if (this.#disposed || this.#bodies.get(held.id) !== held || held.build !== building) {
      return;
    }
    switch (outcome.kind) {
      case "built":
        held.build = {
          kind: "landed",
          key: building.key,
          entry: toDrawOf(building.key, building.input, outcome.assembled),
        };
        break;
      case "failed":
        held.build = null;
        this.#fail(held, new Error(outcome.message));
        break;
      case "cancelled":
        held.build = null;
        break;
    }
  }

  /**
   * Draws a wanted body's landed build, asking its thick bake where it is routed, or makes its
   * tables again after an eviction; and gives its tables its bake where they do not hold it.
   */
  #draw(held: HeldBody<T>): void {
    const { build, drawn } = held;
    if (build?.kind === "landed") {
      const { entry } = build;
      let tables: T | null = null;
      if (entry.built !== null) {
        const made = this.#make(held, entry.built);
        if (made === "unavailable" || made === "failed") {
          // Kept landed to be made again at a later frame, or dropped with the body's problem.
          return;
        }
        tables = made;
      }
      if (drawn !== null && drawn.tables !== null) {
        drawn.tables.dispose();
      }
      // The entry replaced takes its bake with it: the new one is another computation.
      this.#cancelBake(held);
      held.build = null;
      held.problem = null;
      held.drawn = { ...entry, tables };
      if (
        tables !== null &&
        entry.built !== null &&
        entry.regime !== null &&
        this.#routes(entry.regime)
      ) {
        this.#bake(held, entry, entry.built, tables);
      }
    } else if (drawn !== null && drawn.built !== null && drawn.tables === null) {
      const made = this.#make(held, drawn.built);
      if (made !== "unavailable" && made !== "failed") {
        held.drawn = { ...drawn, tables: made };
      }
    }
    this.#giveBake(held);
  }

  /**
   * Asks a routed entry's thick bake (the module's remarks), drawn meanwhile under
   * `atmosphereComputing`; a kept medium with no sources to bake from, or a request that cannot be
   * formed or asked, is a failed bake, with one `console.warn` or `console.error`.
   */
  #bake(held: HeldBody<T>, entry: ToDraw, built: BuiltAtmosphere, tables: T): void {
    const made: Labelled = { labels: entry.labels, reasons: entry.reasons };
    if (built.sources === undefined) {
      console.warn(
        `the atmosphere of ${held.id} has no sources to bake: drawn by the thin-atmosphere method`,
      );
      this.#showBake(held, { made, state: BAKE_FAILED });
      return;
    }
    let ask: ThickBakeAsk | null;
    let handle: ThickBake;
    try {
      ask = thickBakeAskOf(entry.input, built, built.sources, tables, held.lights);
      if (ask === null) {
        // No light to bake under: drawn as a body the router does not route.
        return;
      }
      handle = this.#builder.bakeThick(ask);
    } catch (error: unknown) {
      console.error(`the thick bake of ${held.id} could not be asked:`, error);
      this.#showBake(held, { made, state: BAKE_FAILED });
      return;
    }
    const bake: HeldBake<T> = { made, state: { kind: "baking", handle } };
    this.#showBake(held, bake);
    void this.#bakeLanded(held, bake, handle).catch((error: unknown) => {
      // A bug in taking the bake: the bake failed, and the body is drawn by the analytic term.
      console.error(`the thick bake of ${held.id} could not be taken:`, error);
      if (!this.#disposed && this.#bodies.get(held.id) === held && held.bake === bake) {
        this.#showBake(held, { made, state: BAKE_FAILED });
      }
    });
  }

  /** Takes a thick bake's outcome, unless the body has moved on from it. */
  async #bakeLanded(held: HeldBody<T>, bake: HeldBake<T>, handle: ThickBake): Promise<void> {
    const outcome = await handle.outcome;
    if (this.#disposed || this.#bodies.get(held.id) !== held || held.bake !== bake) {
      return;
    }
    switch (outcome.kind) {
      case "baked":
        this.#showBake(held, {
          made: bake.made,
          state: { kind: "baked", tables: outcome.tables, given: null },
        });
        this.#giveBake(held);
        break;
      case "failed":
        // The bake's failure alone: the body stays drawn, under APPROXIMATE, with no fault.
        console.error(`the thick bake of ${held.id} failed: ${outcome.message}`);
        this.#showBake(held, { made: bake.made, state: BAKE_FAILED });
        break;
      case "cancelled":
        this.#cancelBake(held);
        break;
    }
  }

  /**
   * Gives a baked body's tables its bake where they do not hold it: tables made again after an
   * eviction, or that refused it while the device was lost, which a later frame gives it again. A
   * bake its tables refuse otherwise is a failed bake, with one `console.error`.
   */
  #giveBake(held: HeldBody<T>): void {
    const { bake } = held;
    const tables = held.drawn?.tables ?? null;
    if (bake === null || bake.state.kind !== "baked" || tables === null) {
      return;
    }
    const { state } = bake;
    if (state.given === tables) {
      return;
    }
    try {
      tables.setThickTables(state.tables);
      state.given = tables;
    } catch (error: unknown) {
      if (error instanceof EngineUnavailable) {
        return;
      }
      console.error(`the thick bake of ${held.id} could not be given to its tables:`, error);
      this.#showBake(held, { made: bake.made, state: BAKE_FAILED });
    }
  }

  /** Sets a body's thick bake, its drawn entry's labels and reasons read from its state. */
  #showBake(held: HeldBody<T>, bake: HeldBake<T>): void {
    held.bake = bake;
    if (held.drawn !== null) {
      held.drawn = { ...held.drawn, ...bakeLabelsOf(bake.made, bake.state.kind) };
    }
  }

  /** Cancels a body's thick bake under way, or drops one held, its entry's labels as made. */
  #cancelBake(held: HeldBody<T>): void {
    const { bake } = held;
    if (bake === null) {
      return;
    }
    if (bake.state.kind === "baking") {
      bake.state.handle.cancel();
    }
    held.bake = null;
    if (held.drawn !== null) {
      held.drawn = { ...held.drawn, labels: bake.made.labels, reasons: bake.made.reasons };
    }
  }

  /** A body's tables, or `unavailable` while the device is lost, or `failed` (its problem set). */
  #make(held: HeldBody<T>, built: BuiltAtmosphere): T | "unavailable" | "failed" {
    try {
      return this.#tables(built);
    } catch (error: unknown) {
      if (error instanceof EngineUnavailable) {
        return "unavailable";
      }
      this.#fail(held, error);
      return "failed";
    }
  }

  /**
   * Holds a body whose drawing threw in a view (`decision-r08-t10b-leans.md` §2) as a failed
   * build is held, logged once: a body already failed, or not held, takes nothing.
   */
  #drawFailed(body: BodyIdHex, error: unknown, viewName: string): void {
    const held = this.#bodies.get(body);
    if (held === undefined || held.problem !== null) {
      return;
    }
    console.error(`the atmosphere of ${body} could not be drawn in ${viewName}:`, error);
    held.problem = FAILED;
    this.#cancelBuild(held);
    this.#dropDrawn(held);
  }

  /** Holds a failed build (item 6): one `console.error`, nothing drawn, no label. */
  #fail(held: HeldBody<T>, error: unknown): void {
    console.error(`the atmosphere of ${held.id} could not be computed:`, error);
    held.problem = FAILED;
    this.#cancelBuild(held);
    this.#dropDrawn(held);
  }

  /** Disposes the tables of the least recently wanted bodies no view wants, past the size. */
  #evict(): void {
    let bytes = this.bytes;
    if (bytes <= ATMOSPHERE_CACHE_BYTES) {
      return;
    }
    const wanted = this.#wantedByAnyView();
    for (const held of this.#bodies.values()) {
      if (bytes <= ATMOSPHERE_CACHE_BYTES) {
        return;
      }
      const { drawn } = held;
      if (drawn === null || drawn.tables === null || wanted.has(held.id)) {
        continue;
      }
      bytes -= drawn.tables.rowBytes;
      drawn.tables.dispose();
      held.drawn = { ...drawn, tables: null };
    }
  }

  /** Lets go every body no view wants: a new arrival's. */
  #letUnwantedGo(): void {
    const wanted = this.#wantedByAnyView();
    // A map's iteration goes on past the entries deleted from it.
    for (const held of this.#bodies.values()) {
      if (!wanted.has(held.id)) {
        this.#release(held);
        this.#bodies.delete(held.id);
      }
    }
  }

  /** Stops the busy waits of the bodies no view wants. */
  #stopUnwantedWaits(): void {
    const wanted = this.#wantedByAnyView();
    for (const held of this.#bodies.values()) {
      const { fetch } = held;
      if (fetch.kind === "busy" && fetch.cancel !== null && !wanted.has(held.id)) {
        fetch.cancel();
        fetch.cancel = null;
      }
    }
  }

  /** The bodies every open view's latest frame wanted. */
  #wantedByAnyView(): ReadonlySet<BodyIdHex> {
    const wanted = new Set<BodyIdHex>();
    for (const bodies of this.#views.values()) {
      for (const id of bodies) {
        wanted.add(id);
      }
    }
    return wanted;
  }

  /** Cancels a body's request and its wait. */
  #stopFetch(held: HeldBody<T>): void {
    const { fetch } = held;
    switch (fetch.kind) {
      case "asking":
        fetch.cancel();
        break;
      case "busy":
        fetch.cancel?.();
        break;
      case "idle":
      case "waiting":
        break;
    }
    held.fetch = IDLE;
  }

  /** Cancels a body's build under way, or drops one landed. */
  #cancelBuild(held: HeldBody<T>): void {
    if (held.build?.kind === "building") {
      held.build.handle.cancel();
    }
    held.build = null;
  }

  /** Disposes a body's tables, cancels its thick bake and draws nothing of it. */
  #dropDrawn(held: HeldBody<T>): void {
    this.#cancelBake(held);
    held.drawn?.tables?.dispose();
    held.drawn = null;
  }

  /** Releases everything a body holds: its request, its wait, its build and its tables. */
  #release(held: HeldBody<T>): void {
    this.#stopFetch(held);
    this.#cancelBuild(held);
    this.#dropDrawn(held);
  }
}

/**
 * Whether a body is pending, with nothing drawn and no problem: a server body asked, waiting or
 * building; a kept one whose scene is still building its medium (R08.T10.b).
 */
function isPending(held: HeldBody<unknown>): boolean {
  if (held.problem !== null || held.drawn !== null) {
    return false;
  }
  return held.kept
    ? held.keptSetting?.kind === "loading"
    : held.fetch.kind !== "idle" || held.build !== null;
}
