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
 * `thickBakeRouted`, which routes a `thickScattering` world within the range R08.T14.d.4's gates
 * pass, and no other) has its bake asked once its tables are made, every band in one ask
 * ({@link AtmosphereBuilder.bakeThick}), and the bake given to its tables when it lands
 * (`AtmosphereTables.setThickTables`). Its labels follow the bake, at most one of
 * `ATMOSPHERE: COMPUTING` and `ATMOSPHERE: APPROXIMATE` (`labels.ts`'s precedence; the guide's
 * rows):
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
 *
 * **The polarisation correction** (R08.T14.f; Design note 10). Every drawn body whose medium needs
 * it ({@link AtmosphereCacheOptions.corrects}: `thick/routing.ts`'s `correctionRouted`, every world
 * with air, thin or thick, since R08.T14.d.4's route; a thick world the route does not route asks
 * for none) has its correction asked once its tables are made, beside any thick bake, every band
 * in one ask ({@link AtmosphereBuilder.bakePolarisation}), and given to its tables when it
 * lands (`AtmosphereTables.setPolarisationCorrection`, which reads it beside Ψ_ms, or summed into
 * the thick bake where they hold one). Its labels follow it as a thick bake's do (the plan's "as
 * for any gated bake"), but `atmosphereComputing` takes `atmosphereApproximate`'s place for it only
 * where nothing the correction leaves holds `atmosphereApproximate` (the guide's row: the computed
 * light replaces the approximation):
 *
 * - while the thick bake runs, `atmosphereComputing` with no reason, as before;
 * - while the correction alone runs, `atmosphereComputing` with no reason where the entry's labels
 *   with its bakes landed carry no `atmosphereApproximate`, and those labels and reasons where they
 *   do;
 * - once both have landed, the entry's labels as each bake's outcome leaves them;
 * - a failed correction adds `atmosphereApproximate` with {@link POLARISATION_REASON}, since the sky
 *   it draws then carries the scalar error the correction takes out, until the body's atmosphere is
 *   next computed. Like a failed thick bake, it is never item 6's fault.
 *
 * A medium with no sources to bake from asks for none, and is drawn as before.
 *
 * **Deep twilight** (R08.T14.d.4; main's ruling with T14.d.2's step 3). The route's gates pass to
 * the sun at the horizon, s90, and s95 lies past the pseudo-spherical model's limit. So where a
 * view gives a routed body's observer ({@link AtmosphereFrameBody.observer}), its bake landed, the
 * camera within its air and its brightest light below the camera's local horizontal, the view's
 * report adds `atmosphereApproximate` with `thick/routing.ts`'s `TWILIGHT_REASON` for it, per view,
 * never in its entry. It is a reason the correction leaves, so it stands in the correction's
 * `atmosphereComputing`'s place; while the bake itself runs, `atmosphereComputing` stands as
 * before.
 *
 * **The disc's table** (R08.T16.b; `thick/discBake.ts`). Every drawn body with air that has sources
 * to bake from ({@link AtmosphereCacheOptions.discs}) has its disc reflectance table asked once its
 * tables are made and its thick bake and polarisation correction, where asked, have landed or
 * failed, so that its J is read from them as the march reads it ({@link AtmosphereBuilder.bakeDisc}),
 * and given to its tables when it lands (`AtmosphereTables.setDiscReflectance`), kept with the entry
 * as a bake is. R07's disc reads it beyond the 10⁹ m boundary in place of the full passes. It
 * changes no label: until it lands, and where it fails (one `console.error`, never item 6's fault),
 * the body is drawn by the full passes at every distance, as before.
 *
 * **The cloud deck** (R08.T15.a.2; Design note 9's split). A drawn body whose regime is
 * `cloudDeck` and which the router routes (`routes`, which routes no `cloudDeck` until R08.T15.b.2)
 * is drawn over its deck: its tables are made of its upper medium on the deck's datum
 * (`thick/deckRead.ts`'s `upperMediumOf` and `deckDatumOf`), its deck's bake asked in their thick
 * bake's place, every band in one ask ({@link AtmosphereBuilder.bakeDeck}), on the body's own
 * figure, its split the regime's `deckAltitudeM` over a floor of the ground, or of an envelope where
 * the body's cover is one's, and given to its tables when it lands (`AtmosphereTables`'
 * `setDeckTables`, with the correction over the deck the same job bakes). Its labels and its
 * failures are a thick bake's; it asks no correction of its own.
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
import type {
  AtmosphereBuilder,
  DeckBake,
  DeckBakeAsk,
  DiscBake,
  DiscBakeAsk,
  OpticsBuild,
  PolarisationBake,
  ThickBake,
  ThickBakeAsk,
} from "./opticsPool";
import type { DiscReflectanceTable } from "./discTable";
import type { BakeSpectrum } from "./spectralColour";
import { groundTemperatureFieldOf, type SurfaceTemperatureField } from "./thermal";
import type { MediumTemperature } from "./thermalSource";
import {
  type AbsorberCurveSlots,
  type AtmosphereBody,
  AtmosphereTables,
  kernelMedium,
} from "./tables";
import type { MsSourceTable, ThickBakeBody } from "./thick/bake";
import { spectralColumnOf } from "./thick/columnLayout";
import type { DeckSplit, DeckTables } from "./thick/deck";
import { deckCorrectionAbove, deckDatumOf, upperMediumOf } from "./thick/deckRead";
import type { PolarisationTable } from "./thick/polarisation";
import {
  approximateRegime,
  type AtmosphereRegime,
  type BodyCover,
  CLEAR_COVER,
  classifyRegime,
  THICK_REASON,
} from "./thick/regime";
import {
  correctionRouted,
  discTableRouted,
  thickBakeRouted,
  TWILIGHT_REASON,
} from "./thick/routing";
import type { SpectralColumn, SpectralSource } from "./thick/spectralColumn";

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
  | "rowBytes"
  | "dispose"
  | "setThickTables"
  | "setPolarisationCorrection"
  | "setDiscReflectance"
  | "setDeckTables"
  | "plan"
  | "body"
  | "bottomRadiusM"
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
  /**
   * The column's temperature of a self-luminous medium (R08.T18.b; `AtmosphereRegime`'s
   * `selfLuminous`), from which its tables make its thermal source and its thick bake its thermal
   * field; absent below the thermal cut, where nothing of it is computed.
   */
  readonly temperature?: MediumTemperature;
}

/**
 * Makes a body's per-planet tables, `thickSlots` the light slots of the thick table the body's
 * route gives it, one a light, or 0 where it takes none, which their 2 MB row counts when it
 * decides their slicing (R08.T14.d.4, T14.d's precondition 4; `tables.ts`'s `rowBodyOf`).
 *
 * @throws `EngineUnavailable` while the engine has no device, which the cache passes in silence
 *   and asks again at a later frame; anything else is the body's `failed` problem.
 */
export type AtmosphereTablesFactory<T extends CachedTables> = (
  built: BuiltAtmosphere,
  thickSlots: number,
) => T;

/**
 * The app's factory: `new AtmosphereTables(engine, …)` on the engine's device, which rebuilds them
 * itself after a device loss (`AtmosphereTables`' remarks).
 */
export function atmosphereTablesOn(
  engine: RenderEngine,
): AtmosphereTablesFactory<AtmosphereTables> {
  return (built, thickSlots) => {
    const tables = new AtmosphereTables(
      engine,
      built.medium,
      built.bottomRadiusM,
      built.curves,
      built.body,
      "read",
      thickSlots,
    );
    if (built.temperature !== undefined) {
      try {
        tables.setThermalSource(built.temperature);
      } catch (error: unknown) {
        tables.dispose();
        throw error;
      }
    }
    return tables;
  };
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

/**
 * Where a view's camera stands at a lit body (plan R08, R08.T14.d.4): what a routed world's
 * twilight reason reads (`thick/routing.ts`'s `TWILIGHT_REASON`).
 */
export interface AtmosphereObserver {
  /** The camera's geodetic height above the body's datum, m. */
  readonly heightM: number;
  /**
   * The sine of the body's brightest light's elevation at the camera over the datum's tangent plane
   * there: below 0, the light stands below the camera's horizon.
   */
  readonly lightElevationSine: number;
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
  /** Where the view's camera stands at it (R08.T14.d.4); absent where the view gives none. */
  readonly observer?: AtmosphereObserver;
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
   * Whether the body's cover is a gas envelope's, whose deck reads to the envelope's floor
   * (`thick/deck.ts`'s `DeckFloor`; R08.T15.a.2): a kept scene's cover's, never a server body's.
   */
  readonly envelope: boolean;
  /**
   * What its tables are made from; `null` where it draws no medium, or holds an absorber and no
   * light lights it, so that no curve exists. Over a routed deck its tables are made of its upper
   * medium instead (the module's remarks, "The cloud deck").
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
   * entry's order and then the view's own, deep twilight's (R08.T10.c;
   * `decision-r08-approximate-plate.md`, item 1b; R08.T14.d.4), so that the label block names each
   * reason and its bodies: none for a body without the label (one with `atmosphereComputing` in its
   * place among them), with a problem, pending, or whose entry holds no reason.
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
   * Whether a body drawn in `regime` from `medium` takes its thick bake (R08.T14.c):
   * `thick/routing.ts`'s `thickBakeRouted` if absent, which routes a `thickScattering` world within
   * the range its gates pass over (R08.T14.d.4); a test's routing otherwise.
   */
  readonly routes?: (regime: AtmosphereRegime, medium: AtmosphereMedium) => boolean;
  /**
   * Whether a body drawn from `column` takes its polarisation correction (R08.T14.f):
   * `thick/routing.ts`'s `correctionRouted` if absent, every world with air since R08.T14.d.4's
   * route (`CORRECTION_ROUTED`); a test's choice otherwise. A thick world the route does not route
   * takes none, whatever this says (main, with T14.d's split).
   */
  readonly corrects?: (column: SpectralColumn) => boolean;
  /**
   * Whether a body drawn from `column` takes its disc's table (R08.T16.b): `thick/routing.ts`'s
   * `discTableRouted` if absent, every world with air whose column has a term; a test's choice
   * otherwise.
   */
  readonly discs?: (column: SpectralColumn) => boolean;
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
  /**
   * Whether the body's cover is a gas envelope's (R08.T15.a.2): a kept scene's cover's; never a
   * server body's, whose assembly draws a surface's air alone (`assemble.ts`'s `bodyCoverOf`).
   */
  readonly envelope: boolean;
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
  | { readonly kind: "baking"; readonly handle: ThickBake | DeckBake }
  | {
      readonly kind: "baked";
      /** The bake's tables, one a band of the tables' plan, kept for tables made again. */
      readonly tables: BakedTables;
      /** The tables that hold them; `null` before any did, or once another set stands. */
      given: T | null;
    }
  /** Its bake failed: `atmosphereApproximate` stands with every reason. */
  | { readonly kind: "failed" };

/**
 * What a bake gave: a thick bake's tables, or a deck's (R08.T15.a.2) with the correction over it,
 * its altitudes the bake's, and the split they stand on.
 */
type BakedTables =
  | { readonly kind: "thick"; readonly tables: ReadonlyArray<MsSourceTable> }
  | {
      readonly kind: "deck";
      readonly tables: ReadonlyArray<DeckTables>;
      readonly correction: ReadonlyArray<PolarisationTable> | null;
      readonly splitM: number;
    };

/** A drawn entry's thick bake, with the labels and reasons the entry was made with. */
interface HeldBake<T> {
  /** The entry's labels and reasons before the bake's state is read into them. */
  readonly made: Labelled;
  readonly state: BakeState<T>;
}

/** Where a drawn entry's polarisation correction stands (R08.T14.f), as a thick bake's state. */
type CorrectionState<T> =
  | { readonly kind: "baking"; readonly handle: PolarisationBake }
  | {
      readonly kind: "baked";
      /** The correction's tables, one a band of the tables' plan, kept for tables made again. */
      readonly tables: ReadonlyArray<PolarisationTable>;
      /** The tables that hold them; `null` before any did, or once another set stands. */
      given: T | null;
    }
  /** Its bake failed: `atmosphereApproximate` stands with {@link POLARISATION_REASON}. */
  | { readonly kind: "failed" };

/** A drawn entry's polarisation correction, with the labels and reasons the entry was made with. */
interface HeldCorrection<T> {
  /** The entry's labels and reasons before either bake's state is read into them. */
  readonly made: Labelled;
  readonly state: CorrectionState<T>;
}

/** Where a drawn entry's disc table stands (R08.T16.b). */
type DiscState<T> =
  /** Asked once the entry's thick bake and correction under way have settled. */
  | { readonly kind: "waiting"; readonly entry: ToDraw; readonly built: BuiltAtmosphere }
  | { readonly kind: "baking"; readonly handle: DiscBake }
  | {
      readonly kind: "baked";
      readonly table: DiscReflectanceTable;
      /** The tables that hold it; `null` before any did, or once another set stands. */
      given: T | null;
    }
  /** Its bake failed: the body is drawn by the full passes at every distance. */
  | { readonly kind: "failed" };

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
  /** The ground's temperature field of the reply it was read from (R08.T18.a); `null` before. */
  ground: { readonly reply: BodyDetail; readonly field: SurfaceTemperatureField | null } | null;
  build: Build | null;
  drawn: AtmosphereEntry<T> | null;
  /**
   * The light slots of the thick table the drawn entry's tables were made to count (R08.T14.d.4),
   * so that its tables made again after an eviction or a loss are sliced as they were; 0 before.
   */
  drawnThickSlots: number;
  /** The drawn entry's thick bake; `null` where none is asked, or nothing is drawn. */
  bake: HeldBake<T> | null;
  /** The drawn entry's polarisation correction; `null` where none is asked, or nothing is drawn. */
  correction: HeldCorrection<T> | null;
  /** The drawn entry's disc table (R08.T16.b); `null` where none is asked, or nothing is drawn. */
  disc: DiscState<T> | null;
  problem: AtmosphereProblem | null;
}

const IDLE: Fetch = { kind: "idle" };
const WAITING: Fetch = { kind: "waiting" };
const FAILED: AtmosphereProblem = { kind: "failed" };
const LOAD_FAILED: AtmosphereProblem = { kind: "loadFailed" };
const BAKE_FAILED = { kind: "failed" } as const;
const DISC_FAILED = { kind: "failed" } as const;

/**
 * Why a body whose polarisation correction failed carries `atmosphereApproximate` (R08.T14.f), as
 * player text, describing the drawing as `THICK_REASON` does: its multiple scattering is drawn by
 * the scalar method alone, whose error the correction takes out (Design note 10's polarisation: up
 * to −5.2% to +4.6% in an Earth-like blue, past the metric's 5% in a thicker air). Pending the
 * guide's sign-off (plan R08's Risks, "Preconditions of the correction's route").
 */
export const POLARISATION_REASON = "multiple scattering: drawn without its polarisation";

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
    case "unknown_feature":
    case "unknown_body":
    case "generator_version_mismatch":
    case "unsupported_save_format":
    case "name_taken":
    case "universe_limit_reached":
    case "storage_failed":
    // A code this build does not know, which a newer server may send: the server's word, read as a
    // failed system.
    case "unknown":
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
    return { key, labels, reasons, regime: null, envelope: false, built: null, input };
  }
  const { drawn } = assembled;
  const curved = drawn.absorbers.length === 0 || drawn.curves.length > 0;
  return {
    key,
    labels,
    reasons,
    regime: assembled.regime,
    envelope: false,
    built: curved
      ? {
          medium: assembled.medium,
          bottomRadiusM: drawn.bottomRadiusM,
          curves: drawn.curves,
          body: drawn.body,
          sources: drawn.sources,
          ...(assembled.regime.selfLuminous === true && drawn.temperature !== undefined
            ? { temperature: drawn.temperature }
            : {}),
        }
      : null,
    input,
  };
}

/**
 * Whether a view stands in a routed thick world's deep twilight (R08.T14.d.4; the module's
 * remarks): the world's bake landed, the camera in its air, and its brightest light below the
 * camera's local horizontal, where the route's gates pass no view (`thick/routing.ts`'s
 * `TWILIGHT_REASON`).
 */
function inDeepTwilight(
  held: HeldBody<unknown>,
  observer: AtmosphereObserver | undefined,
): boolean {
  const built = held.drawn?.built ?? null;
  return (
    observer !== undefined &&
    built !== null &&
    held.bake?.state.kind === "baked" &&
    observer.heightM < built.medium.topHeightM &&
    observer.lightElevationSine < 0
  );
}

/**
 * An entry's labels and reasons under its landed thick bake's state (the module's remarks): baked,
 * the made reasons but `THICK_REASON`, and `atmosphereApproximate` only where one remains; failed,
 * `atmosphereApproximate` with every made reason and `THICK_REASON`.
 */
function thickLabelsOf(made: Labelled, state: "baked" | "failed"): Labelled {
  const others = made.labels.filter((label) => label !== "atmosphereApproximate");
  let labelled: Labelled;
  switch (state) {
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
 * An entry's labels and reasons under its thick bake's and its polarisation correction's states,
 * `null` for one not asked (the module's remarks): while the thick bake runs,
 * `atmosphereComputing` in `atmosphereApproximate`'s place and no reason; while the correction alone
 * runs, the same where its landing would leave no `atmosphereApproximate`, and the labels its
 * landing leaves where it would; once both have landed, the thick bake's outcome read into the made
 * labels ({@link thickLabelsOf}), and a failed correction's `atmosphereApproximate` with
 * {@link POLARISATION_REASON}.
 */
function bakeLabelsOf(
  made: Labelled,
  thick: BakeState<unknown>["kind"] | null,
  correction: CorrectionState<unknown>["kind"] | null,
): Labelled {
  const computing = (): Labelled => ({
    labels: ordered([
      ...made.labels.filter((label) => label !== "atmosphereApproximate"),
      "atmosphereComputing",
    ]),
    reasons: [],
  });
  if (thick === "baking") {
    return computing();
  }
  let labelled = thick === null ? made : thickLabelsOf(made, thick);
  if (correction === "baking") {
    return labelled.labels.includes("atmosphereApproximate") ? labelled : computing();
  }
  if (correction === "failed") {
    labelled = {
      labels: ordered([...labelled.labels, "atmosphereApproximate"]),
      reasons: labelled.reasons.includes(POLARISATION_REASON)
        ? labelled.reasons
        : [...labelled.reasons, POLARISATION_REASON],
    };
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
  // A self-luminous medium's column carries its temperature, so that the bake solves its thermal
  // field beside its beams (R08.T18.b).
  const column = spectralColumnOf(built.medium, sources, built.temperature);
  const body = bakeBodyOf(tables);
  const bands = tables.plan.bands.map((_, band) => band);
  if (input !== null) {
    // The tables' row counted these slots when they were made (`AtmosphereCache`'s
    // `#thickSlotsOf`).
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

/**
 * A drawn body's disc table request (R08.T16.b): its thick bake's request (`thickBakeAskOf`), every
 * band and light slot in one, with the medium as the tables hold it, its kernels' medium, the
 * tables' curves, and the routed bake and correction that have landed, which the job reads J from
 * in place of baking its own; `null` where no light lights it to bake under.
 *
 * @throws RangeError as `thickBakeAskOf`.
 */
function discBakeAskOf(
  input: BodyAtmosphereInput | null,
  built: BuiltAtmosphere,
  tables: CachedTables,
  lights: ReadonlyArray<BodyLight>,
  landed: {
    readonly bake: ReadonlyArray<MsSourceTable> | undefined;
    readonly correction: ReadonlyArray<PolarisationTable> | undefined;
  },
): DiscBakeAsk | null {
  if (built.sources === undefined) {
    return null;
  }
  const ask = thickBakeAskOf(input, built, built.sources, tables, lights);
  if (ask === null) {
    return null;
  }
  return {
    ...ask,
    air: {
      medium: built.medium,
      kernelMedium: kernelMedium(built.medium),
      curves: built.curves,
      ...(landed.bake === undefined ? {} : { bake: landed.bake }),
      ...(landed.correction === undefined ? {} : { correction: landed.correction }),
      // A self-luminous medium's table holds its own light (R08.T18.b).
      ...(built.temperature === undefined ? {} : { temperature: built.temperature }),
    },
  };
}

/**
 * What a routed deck's tables are made of (the module's remarks, "The cloud deck"; R08.T15.a.2): the
 * upper medium of `built` over the split at `splitM` (`thick/deckRead.ts`'s `upperMediumOf`), on the
 * deck's datum (`deckDatumOf`), its R_ref raised with the datum's radii, (2a + c) ÷ 3; a body of one
 * slice and one band on its sphere raised by the split.
 */
function deckBuiltOf(built: BuiltAtmosphere, splitM: number): BuiltAtmosphere {
  const medium = upperMediumOf(built.medium, splitM);
  const { body } = built;
  if (body === null) {
    return { ...built, medium, bottomRadiusM: built.bottomRadiusM + splitM };
  }
  const figure = deckDatumOf(body.figure, body.gravity, splitM);
  const raisedM =
    (2 * (figure.equatorialRadiusM - body.figure.equatorialRadiusM) +
      (figure.polarRadiusM - body.figure.polarRadiusM)) /
    3;
  return {
    ...built,
    medium,
    body: { ...body, figure },
    bottomRadiusM: built.bottomRadiusM + raisedM,
  };
}

/**
 * The body a routed deck's bake is made on: the body's own figure and R_ref (`built`'s), the split
 * being the bake's (`thick/deck.ts`'s `prepareDeck`), with its tables' gravity, whose slicing their
 * plan's bands are of; a sphere of R_ref, as {@link bakeBodyOf} makes one, for tables of one band.
 */
function deckBakeBodyOf(built: BuiltAtmosphere, tables: CachedTables): ThickBakeBody {
  const { body } = built;
  const made = tables.body;
  if (body === null || made === null) {
    return bakeBodyOf({ ...tables, body: null, bottomRadiusM: built.bottomRadiusM });
  }
  return { figure: body.figure, gravity: made.gravity, bottomRadiusM: built.bottomRadiusM };
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
  readonly #routes: (regime: AtmosphereRegime, medium: AtmosphereMedium) => boolean;
  readonly #corrects: (column: SpectralColumn) => boolean;
  readonly #discs: (column: SpectralColumn) => boolean;
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
    this.#routes = opts.routes ?? ((regime, medium): boolean => thickBakeRouted(regime, medium));
    this.#corrects = opts.corrects ?? ((column): boolean => correctionRouted(column));
    this.#discs = opts.discs ?? ((column): boolean => discTableRouted(column));
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

  /**
   * A body's ground temperature field (plan R08, R08.T18.a; `thermal.ts`' `groundTemperatureFieldOf`)
   * from the record the cache holds for it, read once a reply, a body with a medium's too since
   * R08.T18.b: `null` for one whose surface section is not `ok`, or one of no record held.
   */
  groundFieldOf(body: BodyIdHex): SurfaceTemperatureField | null {
    const held = this.#bodies.get(body);
    const reply = held?.reply ?? null;
    if (held === undefined || reply === null) {
      return null;
    }
    if (held.ground?.reply !== reply) {
      const { surface } = reply.record;
      held.ground = {
        reply,
        field: surface.state === "ok" ? groundTemperatureFieldOf(surface.value) : null,
      };
    }
    return held.ground.field;
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
        const { drawn } = held;
        const shown = drawn.labels.includes("atmosphereApproximate") ? drawn.reasons : [];
        if (inDeepTwilight(held, wanted.get(id)?.observer)) {
          // A reason the correction leaves: it stands in the correction's COMPUTING's place.
          for (const label of drawn.labels) {
            if (label !== "atmosphereComputing") {
              labels.add(label);
            }
          }
          labels.add("atmosphereApproximate");
          reasons.set(id, [...shown, TWILIGHT_REASON]);
        } else {
          for (const label of drawn.labels) {
            labels.add(label);
          }
          if (shown.length > 0) {
            reasons.set(id, shown);
          }
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
      ground: null,
      build: null,
      drawn: null,
      drawnThickSlots: 0,
      bake: null,
      correction: null,
      disc: null,
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
          envelope: false,
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
        const cover = kept.cover ?? CLEAR_COVER;
        const regime = classifyRegime(kept.built.medium, cover);
        const approximate = approximateRegime(regime);
        entry = {
          key: `kept/medium/v${ATMOSPHERE_OPTICS_VERSION}`,
          labels: ordered([
            ...kept.labels,
            ...(approximate ? (["atmosphereApproximate"] as const) : []),
          ]),
          reasons: approximate ? [THICK_REASON] : [],
          regime,
          envelope: cover.envelope,
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
      let thickSlots = 0;
      if (entry.built !== null) {
        thickSlots = this.#thickSlotsOf(held, entry, entry.built);
        const made = this.#make(held, this.#tablesBuilt(entry, entry.built), thickSlots);
        if (made === "unavailable" || made === "failed") {
          // Kept landed to be made again at a later frame, or dropped with the body's problem.
          return;
        }
        tables = made;
      }
      if (drawn !== null && drawn.tables !== null) {
        drawn.tables.dispose();
      }
      // The entry replaced takes its bakes with it: the new ones are another computation.
      this.#cancelBake(held);
      this.#cancelCorrection(held);
      this.#cancelDisc(held);
      held.build = null;
      held.problem = null;
      held.drawn = { ...entry, tables };
      held.drawnThickSlots = thickSlots;
      const deck = entry.built === null ? null : this.#deckSplitOf(entry, entry.built.medium);
      if (tables !== null && entry.built !== null && deck !== null) {
        // A routed deck's bake carries the correction over it: none is asked beside it. Nor is a
        // disc table (R08.T16.b), whose bake reads no deck: beyond the boundary the full passes
        // draw the deck's body, as they draw any body whose table has not landed.
        this.#bakeDeck(held, entry, entry.built, tables, deck);
      } else {
        if (
          tables !== null &&
          entry.built !== null &&
          entry.regime !== null &&
          this.#routes(entry.regime, entry.built.medium)
        ) {
          this.#bake(held, entry, entry.built, tables);
        }
        if (tables !== null && entry.built !== null) {
          this.#correct(held, entry, entry.built, tables);
          this.#disc(held, entry, entry.built);
        }
      }
    } else if (drawn !== null && drawn.built !== null && drawn.tables === null) {
      const made = this.#make(held, this.#tablesBuilt(drawn, drawn.built), held.drawnThickSlots);
      if (made !== "unavailable" && made !== "failed") {
        held.drawn = { ...drawn, tables: made };
      }
    }
    this.#giveBake(held);
    this.#giveCorrection(held);
    this.#giveDisc(held);
    // A table that waited on tables taken away (an eviction, a lost device) is asked once they're back.
    this.#askWaitingDisc(held);
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

  /**
   * A routed deck's split (the module's remarks, "The cloud deck"): its regime's `deckAltitudeM`
   * over a floor of the ground, or of an envelope where the body's cover is one's; `null` for an
   * entry that is not `cloudDeck`, or not routed with its own (whole) medium, `medium`.
   */
  #deckSplitOf(
    entry: Pick<ToDraw, "regime" | "envelope">,
    medium: AtmosphereMedium,
  ): DeckSplit | null {
    const { regime } = entry;
    // An envelope whose τ never reaches the split's is split at its ground: no deck stands there.
    if (
      regime === null ||
      regime.kind !== "cloudDeck" ||
      !(regime.deckAltitudeM > 0) ||
      !this.#routes(regime, medium)
    ) {
      return null;
    }
    return { altitudeM: regime.deckAltitudeM, floor: entry.envelope ? "envelope" : "ground" };
  }

  /** What an entry's tables are made of: its own, or over a routed deck its upper medium's. */
  #tablesBuilt(
    entry: Pick<ToDraw, "regime" | "envelope">,
    built: BuiltAtmosphere,
  ): BuiltAtmosphere {
    const deck = this.#deckSplitOf(entry, built.medium);
    return deck === null ? built : deckBuiltOf(built, deck.altitudeM);
  }

  /**
   * Asks a routed deck's bake (the module's remarks), as {@link #bake} asks a thick one, on the
   * body's own figure, drawn meanwhile under `atmosphereComputing`.
   */
  #bakeDeck(
    held: HeldBody<T>,
    entry: ToDraw,
    built: BuiltAtmosphere,
    tables: T,
    split: DeckSplit,
  ): void {
    const made: Labelled = { labels: entry.labels, reasons: entry.reasons };
    if (built.sources === undefined) {
      console.warn(
        `the atmosphere of ${held.id} has no sources to bake: drawn by the thin-atmosphere method`,
      );
      this.#showBake(held, { made, state: BAKE_FAILED });
      return;
    }
    let handle: DeckBake;
    try {
      const thick = thickBakeAskOf(entry.input, built, built.sources, tables, held.lights);
      if (thick === null) {
        return;
      }
      const ask: DeckBakeAsk = { ...thick, body: deckBakeBodyOf(built, tables), split };
      handle = this.#builder.bakeDeck(ask);
    } catch (error: unknown) {
      console.error(`the deck's bake of ${held.id} could not be asked:`, error);
      this.#showBake(held, { made, state: BAKE_FAILED });
      return;
    }
    const bake: HeldBake<T> = { made, state: { kind: "baking", handle } };
    this.#showBake(held, bake);
    void this.#deckLanded(held, bake, handle, split).catch((error: unknown) => {
      console.error(`the deck's bake of ${held.id} could not be taken:`, error);
      if (!this.#disposed && this.#bodies.get(held.id) === held && held.bake === bake) {
        this.#showBake(held, { made, state: BAKE_FAILED });
      }
    });
  }

  /** Takes a deck's bake's outcome, unless the body has moved on from it, as {@link #bakeLanded}. */
  async #deckLanded(
    held: HeldBody<T>,
    bake: HeldBake<T>,
    handle: DeckBake,
    split: DeckSplit,
  ): Promise<void> {
    const outcome = await handle.outcome;
    if (this.#disposed || this.#bodies.get(held.id) !== held || held.bake !== bake) {
      return;
    }
    switch (outcome.kind) {
      case "baked":
        this.#showBake(held, {
          made: bake.made,
          state: {
            kind: "baked",
            tables: {
              kind: "deck",
              tables: outcome.tables,
              correction: outcome.correction,
              splitM: split.altitudeM,
            },
            given: null,
          },
        });
        this.#giveBake(held);
        break;
      case "failed":
        console.error(`the deck's bake of ${held.id} failed: ${outcome.message}`);
        this.#showBake(held, { made: bake.made, state: BAKE_FAILED });
        break;
      case "cancelled":
        this.#cancelBake(held);
        break;
    }
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
          state: { kind: "baked", tables: { kind: "thick", tables: outcome.tables }, given: null },
        });
        this.#giveBake(held);
        this.#askWaitingDisc(held);
        break;
      case "failed":
        // The bake's failure alone: the body stays drawn, under APPROXIMATE, with no fault.
        console.error(`the thick bake of ${held.id} failed: ${outcome.message}`);
        this.#showBake(held, { made: bake.made, state: BAKE_FAILED });
        this.#askWaitingDisc(held);
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
      const baked = state.tables;
      switch (baked.kind) {
        case "thick":
          tables.setThickTables(baked.tables);
          break;
        case "deck":
          // The correction over the deck stands on the J's altitudes, from the split as J's do.
          if (baked.correction !== null) {
            tables.setPolarisationCorrection(deckCorrectionAbove(baked.correction, baked.splitM));
          }
          tables.setDeckTables(baked.tables);
          break;
      }
      state.given = tables;
    } catch (error: unknown) {
      if (error instanceof EngineUnavailable) {
        return;
      }
      console.error(`the thick bake of ${held.id} could not be given to its tables:`, error);
      this.#showBake(held, { made: bake.made, state: BAKE_FAILED });
    }
  }

  /** Sets a body's thick bake, its drawn entry's labels and reasons read from both bakes' states. */
  #showBake(held: HeldBody<T>, bake: HeldBake<T>): void {
    held.bake = bake;
    this.#relabel(held, bake.made);
  }

  /** Sets a body's polarisation correction, its drawn entry's labels read as {@link #showBake}'s. */
  #showCorrection(held: HeldBody<T>, correction: HeldCorrection<T>): void {
    held.correction = correction;
    this.#relabel(held, correction.made);
  }

  /** A drawn entry's labels and reasons, `made` read under its bakes' states (`bakeLabelsOf`). */
  #relabel(held: HeldBody<T>, made: Labelled): void {
    if (held.drawn !== null) {
      held.drawn = {
        ...held.drawn,
        ...bakeLabelsOf(made, held.bake?.state.kind ?? null, held.correction?.state.kind ?? null),
      };
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
    this.#relabel(held, bake.made);
  }

  /** Cancels a body's polarisation correction under way, or drops one held, as {@link #cancelBake}. */
  #cancelCorrection(held: HeldBody<T>): void {
    const { correction } = held;
    if (correction === null) {
      return;
    }
    if (correction.state.kind === "baking") {
      correction.state.handle.cancel();
    }
    held.correction = null;
    this.#relabel(held, correction.made);
  }

  /**
   * Asks a drawn entry's polarisation correction (the module's remarks), drawn meanwhile under
   * `atmosphereComputing`; a medium with no sources, or none listed, asks for none; a request that
   * cannot be formed or asked is a failed correction, with one `console.error`.
   */
  #correct(held: HeldBody<T>, entry: ToDraw, built: BuiltAtmosphere, tables: T): void {
    const made: Labelled = { labels: entry.labels, reasons: entry.reasons };
    if (built.sources === undefined) {
      return;
    }
    let handle: PolarisationBake;
    try {
      if (!this.#asksCorrection(entry, built)) {
        return;
      }
      const ask = thickBakeAskOf(entry.input, built, built.sources, tables, held.lights);
      if (ask === null) {
        // No light to bake under: drawn uncorrected, as a body that needs none.
        return;
      }
      handle = this.#builder.bakePolarisation(ask);
    } catch (error: unknown) {
      console.error(`the polarisation correction of ${held.id} could not be asked:`, error);
      this.#showCorrection(held, { made, state: BAKE_FAILED });
      return;
    }
    const correction: HeldCorrection<T> = { made, state: { kind: "baking", handle } };
    this.#showCorrection(held, correction);
    void this.#correctionLanded(held, correction, handle).catch((error: unknown) => {
      // A bug in taking the correction: it failed, and the body is drawn uncorrected.
      console.error(`the polarisation correction of ${held.id} could not be taken:`, error);
      if (!this.#disposed && this.#bodies.get(held.id) === held && held.correction === correction) {
        this.#showCorrection(held, { made, state: BAKE_FAILED });
      }
    });
  }

  /** Takes a polarisation correction's outcome, unless the body has moved on from it. */
  async #correctionLanded(
    held: HeldBody<T>,
    correction: HeldCorrection<T>,
    handle: PolarisationBake,
  ): Promise<void> {
    const outcome = await handle.outcome;
    if (this.#disposed || this.#bodies.get(held.id) !== held || held.correction !== correction) {
      return;
    }
    switch (outcome.kind) {
      case "baked":
        this.#showCorrection(held, {
          made: correction.made,
          state: { kind: "baked", tables: outcome.tables, given: null },
        });
        this.#giveCorrection(held);
        this.#askWaitingDisc(held);
        break;
      case "failed":
        // The correction's failure alone: the body stays drawn, under APPROXIMATE, with no fault.
        console.error(`the polarisation correction of ${held.id} failed: ${outcome.message}`);
        this.#showCorrection(held, { made: correction.made, state: BAKE_FAILED });
        this.#askWaitingDisc(held);
        break;
      case "cancelled":
        this.#cancelCorrection(held);
        break;
    }
  }

  /**
   * Gives a corrected body's tables its correction where they do not hold it, as
   * {@link #giveBake} gives a thick bake; tables that refuse it otherwise make it a failed
   * correction, with one `console.error`.
   */
  #giveCorrection(held: HeldBody<T>): void {
    const { correction } = held;
    const tables = held.drawn?.tables ?? null;
    if (correction === null || correction.state.kind !== "baked" || tables === null) {
      return;
    }
    const { state } = correction;
    if (state.given === tables) {
      return;
    }
    try {
      tables.setPolarisationCorrection(state.tables);
      state.given = tables;
    } catch (error: unknown) {
      if (error instanceof EngineUnavailable) {
        return;
      }
      console.error(
        `the polarisation correction of ${held.id} could not be given to its tables:`,
        error,
      );
      this.#showCorrection(held, { made: correction.made, state: BAKE_FAILED });
    }
  }

  /**
   * Asks a drawn entry's disc table (the module's remarks) where its column takes one, or waits for
   * its thick bake and correction under way; a request that cannot be formed or asked is a failed
   * table, with one `console.error`.
   */
  #disc(held: HeldBody<T>, entry: ToDraw, built: BuiltAtmosphere): void {
    if (built.sources === undefined || built.sources.length === 0) {
      return;
    }
    try {
      if (!this.#discs(spectralColumnOf(built.medium, built.sources))) {
        return;
      }
    } catch (error: unknown) {
      console.error(`the disc table of ${held.id} could not be asked:`, error);
      held.disc = DISC_FAILED;
      return;
    }
    held.disc = { kind: "waiting", entry, built };
    this.#askWaitingDisc(held);
  }

  /** Asks a waiting disc table once neither the thick bake nor the correction is under way. */
  #askWaitingDisc(held: HeldBody<T>): void {
    const { disc } = held;
    const tables = held.drawn?.tables ?? null;
    if (
      disc?.kind !== "waiting" ||
      tables === null ||
      held.bake?.state.kind === "baking" ||
      held.correction?.state.kind === "baking"
    ) {
      return;
    }
    const { entry, built } = disc;
    let handle: DiscBake;
    try {
      const ask = discBakeAskOf(entry.input, built, tables, held.lights, {
        // A deck's body asks no disc table (`#draw`), so only a thick bake's J is read here.
        bake:
          held.bake?.state.kind === "baked" && held.bake.state.tables.kind === "thick"
            ? held.bake.state.tables.tables
            : undefined,
        correction:
          held.correction?.state.kind === "baked" ? held.correction.state.tables : undefined,
      });
      if (ask === null) {
        // No light to bake under: drawn by the full passes at every distance.
        held.disc = null;
        return;
      }
      handle = this.#builder.bakeDisc(ask);
    } catch (error: unknown) {
      console.error(`the disc table of ${held.id} could not be asked:`, error);
      held.disc = DISC_FAILED;
      return;
    }
    const baking: DiscState<T> = { kind: "baking", handle };
    held.disc = baking;
    void this.#discLanded(held, baking, handle).catch((error: unknown) => {
      // A bug in taking the table: it failed, and the body is drawn by the full passes.
      console.error(`the disc table of ${held.id} could not be taken:`, error);
      if (!this.#disposed && this.#bodies.get(held.id) === held && held.disc === baking) {
        held.disc = DISC_FAILED;
      }
    });
  }

  /** Takes a disc table's outcome, unless the body has moved on from it. */
  async #discLanded(held: HeldBody<T>, baking: DiscState<T>, handle: DiscBake): Promise<void> {
    const outcome = await handle.outcome;
    if (this.#disposed || this.#bodies.get(held.id) !== held || held.disc !== baking) {
      return;
    }
    switch (outcome.kind) {
      case "baked":
        held.disc = { kind: "baked", table: outcome.table, given: null };
        this.#giveDisc(held);
        break;
      case "failed":
        // The table's failure alone: the body stays drawn by the full passes, with no fault.
        console.error(`the disc table of ${held.id} failed: ${outcome.message}`);
        held.disc = DISC_FAILED;
        break;
      case "cancelled":
        held.disc = null;
        break;
    }
  }

  /**
   * Gives a body's tables its disc table where they do not hold it, as {@link #giveBake} gives a
   * thick bake; tables that refuse it otherwise make it a failed table, with one `console.error`.
   */
  #giveDisc(held: HeldBody<T>): void {
    const { disc } = held;
    const tables = held.drawn?.tables ?? null;
    if (disc?.kind !== "baked" || tables === null || disc.given === tables) {
      return;
    }
    try {
      tables.setDiscReflectance(disc.table);
      disc.given = tables;
    } catch (error: unknown) {
      if (error instanceof EngineUnavailable) {
        return;
      }
      console.error(`the disc table of ${held.id} could not be given to its tables:`, error);
      held.disc = DISC_FAILED;
    }
  }

  /** Cancels a body's disc table under way, or drops one held. */
  #cancelDisc(held: HeldBody<T>): void {
    if (held.disc?.kind === "baking") {
      held.disc.handle.cancel();
    }
    held.disc = null;
  }

  /**
   * The light slots of the thick table a drawn entry's route gives it (R08.T14.d.4, T14.d's
   * precondition 4), which its tables' row counts: its bake's where the router routes it, or else
   * its correction's where it takes one, as {@link AtmosphereCache.#bake} and
   * {@link AtmosphereCache.#correct} ask them, one slot a light (`thickBakeAskOf`); 0 where it
   * takes neither. A kept body's are its lights as noted when its tables are made.
   */
  #thickSlotsOf(held: HeldBody<T>, entry: ToDraw, built: BuiltAtmosphere): number {
    if (built.sources === undefined) {
      return 0;
    }
    const routed = entry.regime !== null && this.#routes(entry.regime, built.medium);
    let corrected = false;
    if (!routed) {
      try {
        corrected = this.#asksCorrection(entry, built);
      } catch {
        // A column the correction cannot be asked of is its ask's failure, logged there.
      }
    }
    if (!routed && !corrected) {
      return 0;
    }
    // `thickBakeAskOf`'s slots, one a light: a kept body's lights, or the reference Sun alone.
    if (entry.input === null) {
      return Math.max(1, held.lights.length);
    }
    return entry.input.kind === "record" ? entry.input.lights.length : 0;
  }

  /**
   * Whether a drawn entry is asked its polarisation correction (the module's remarks): a world with
   * sources to bake from, thin or a thick one the route routes (an unrouted thick world takes none:
   * main, with R08.T14.d's split, question 3), whose column the product's choice takes
   * ({@link AtmosphereCacheOptions.corrects}).
   *
   * @throws RangeError as `spectralColumnOf` refuses the sources: a bug in what the body was built
   *   from.
   */
  #asksCorrection(entry: ToDraw, built: BuiltAtmosphere): boolean {
    if (built.sources === undefined || built.sources.length === 0) {
      return false;
    }
    if (
      entry.regime !== null &&
      entry.regime.kind !== "thin" &&
      !this.#routes(entry.regime, built.medium)
    ) {
      return false;
    }
    return this.#corrects(spectralColumnOf(built.medium, built.sources));
  }

  /**
   * A body's tables, or `unavailable` while the device is lost, or `failed` (its problem set).
   *
   * @param thickSlots - As the factory's ({@link AtmosphereTablesFactory}).
   */
  #make(
    held: HeldBody<T>,
    built: BuiltAtmosphere,
    thickSlots: number,
  ): T | "unavailable" | "failed" {
    try {
      return this.#tables(built, thickSlots);
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

  /** Disposes a body's tables, cancels its bakes and draws nothing of it. */
  #dropDrawn(held: HeldBody<T>): void {
    this.#cancelBake(held);
    this.#cancelCorrection(held);
    this.#cancelDisc(held);
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
