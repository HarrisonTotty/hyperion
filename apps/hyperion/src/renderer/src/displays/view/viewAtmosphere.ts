/**
 * What a `VIEW` view tells the atmosphere cache of its frame, and what its label block says of the
 * cache's answer (plan R08, R08.T10.b; `AtmosphereCache`'s remarks): pure, called each frame by the
 * view's drawer and at each publish by its label block.
 */

import { type BodyIdHex, formatBodyId } from "@hyperion/protocol";

import { add, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type {
  AtmosphereCache,
  AtmosphereFrame,
  AtmosphereFrameBody,
  AtmosphereObserver,
  AtmosphereScene,
  AtmosphereView,
  AtmosphereViewReport,
  KeptBodyAtmosphere,
} from "../../view/atmosphere/AtmosphereCache";
import type { BodyLight } from "../../view/atmosphere/atmosphereInput";
import { geodeticOf } from "../../view/atmosphere/oblate";
import {
  atmosphereApproximateLines,
  type AtmosphereLabel,
  atmosphereProblemLines,
  type AtmosphereRejectedLine,
  type BodyApproximateReasons,
  type BodyAtmosphereProblem,
} from "../../view/atmosphere/labels";
import type { AtmosphereTables } from "../../view/atmosphere/tables";
import type { SurfaceTemperatureField } from "../../view/atmosphere/thermal";
import type { BodyLightSources } from "../../view/bodies/draw";
import type { LitRegime } from "../../view/bodies/regime";
import type { CameraPose } from "../../view/camera/pose";
import { relativeToCamera } from "../../view/coords/relative";
import { isLitBody } from "../../view/lighting/retarded";
import {
  bringsOwnSettings,
  sceneOrigins,
  type ViewBody,
  type ViewScene,
} from "../../view/scene/model";
import type { BodyFigure } from "../../view/terrain/planet";

/** A view's atmospheres for its drawer's frame (`ViewFrameInputs.atmosphere`). */
export interface ViewAtmosphereInput {
  /** The view's handle on the device's cache (`AtmosphereCache.openView`). */
  readonly view: AtmosphereView;
  /** A body's tables as the cache holds them now (`AtmosphereCache.entryOf(id).tables`). */
  readonly tablesOf: (body: BodyIdHex) => AtmosphereTables | null;
  /**
   * A body's ground temperature field from the record the cache holds (plan R08, R08.T18.a;
   * `AtmosphereCache.groundFieldOf`).
   */
  readonly groundFieldOf: (body: BodyIdHex) => SurfaceTemperatureField | null;
  /** The server's scene as the cache's requests take it; `null` for a kept scene. */
  readonly scene: AtmosphereScene | null;
  /** A monotonic time, ms: the frame's, which `ATMOSPHERE: PENDING`'s hold reads. */
  readonly nowMs: number;
}

/**
 * A view's handle on the cache it is drawn with this frame, opened at its first frame with a cache
 * and anew when the stage's cache is remade, and closed with its view, so that its bodies are let
 * go as no longer wanted by it.
 */
export class ViewAtmosphereHandle {
  #cache: AtmosphereCache | null = null;
  #view: AtmosphereView | null = null;

  /**
   * The frame's input over `cache` (`ViewFrameInputs.atmosphere`), or `undefined` with no cache.
   *
   * @param scene - The server's scene as the cache's requests take it; `null` for a kept scene.
   * @param nowMs - The frame's monotonic time, ms.
   */
  inputFor(
    cache: AtmosphereCache | null,
    scene: AtmosphereScene | null,
    nowMs: number,
  ): ViewAtmosphereInput | undefined {
    if (cache !== this.#cache) {
      this.#view?.close();
      this.#view = cache?.openView() ?? null;
      this.#cache = cache;
    }
    const view = this.#view;
    if (cache === null || view === null) {
      return undefined;
    }
    return {
      view,
      tablesOf: (body) => cache.entryOf(body)?.tables ?? null,
      groundFieldOf: (body) => cache.groundFieldOf(body),
      scene,
      nowMs,
    };
  }

  /** Lets the view's bodies go, as its view goes. */
  close(): void {
    this.#view?.close();
    this.#view = null;
    this.#cache = null;
  }
}

/** No atmosphere setting on a kept scene's body: `ATMOSPHERE: NOT YET MODELLED`. */
const UNSET: KeptBodyAtmosphere = { kind: "unset" };

/**
 * A body's lights as the cache builds its medium for them (`BodyLight`): the stars of its record,
 * brightest first, each named by its scene body and with R06's bake spectrum.
 */
function bodyLightsOf(scene: ViewScene, sources: BodyLightSources | undefined): BodyLight[] {
  return (sources?.stars ?? []).map((light) => ({
    star: formatBodyId({ system: scene.system, bodyIndex: light.host.disc.star }),
    spectrum: light.host.disc.bake_spectrum,
  }));
}

/** The origin of a body's own frame: its centre. */
const CENTRE: Vec3 = vec3(0, 0, 0);

/**
 * Where a camera stands at a body (plan R08, R08.T14.d.4), for a routed world's twilight reason:
 * its geodetic height above the body's datum and the sine of a light's elevation over the datum's
 * tangent plane there.
 *
 * @param cameraFromCentreM - The camera's position from the body's centre, m, in the scene's axes.
 * @param figure - The body's datum; its pole the scene's +z where it has none.
 * @param towardLightM - The light's centre from the camera, m, in the same axes.
 */
export function atmosphereObserverOf(
  cameraFromCentreM: Vec3,
  figure: BodyFigure,
  towardLightM: Vec3,
): AtmosphereObserver {
  const pole = figure.pole ?? vec3(0, 0, 1);
  const along = dot(cameraFromCentreM, pole);
  const across = sub(cameraFromCentreM, scale(pole, along));
  const rho = norm(across);
  const geodetic = geodeticOf(vec3(rho, 0, along), figure);
  // On the axis the normal is the pole's own, whatever stands across it.
  const out = rho > 0 ? scale(across, 1 / rho) : vec3(0, 0, 0);
  const normal = add(scale(out, geodetic.normal.x), scale(pole, geodetic.normal.z));
  return {
    heightM: geodetic.heightM,
    lightElevationSine: dot(normal, normalise(towardLightM)),
  };
}

/**
 * Each body's observer under a camera's pose (R08.T14.d.4; {@link atmosphereObserverOf}): its
 * figure's, under its brightest light, or none for a body with no figure or no light.
 */
export function cameraObservers(
  scene: ViewScene,
  pose: CameraPose,
): (body: ViewBody, lights: ReadonlyArray<BodyLight>) => AtmosphereObserver | undefined {
  const origins = sceneOrigins(scene);
  return (body, lights) => {
    const figure = body.appearance?.figure ?? null;
    const star = lights[0]?.star;
    if (figure === null || star === undefined) {
      return undefined;
    }
    const toBody = relativeToCamera({ kind: "body", body: body.id, m: CENTRE }, pose, origins);
    const toStar = relativeToCamera({ kind: "body", body: star, m: CENTRE }, pose, origins);
    return atmosphereObserverOf(scale(toBody, -1), figure, toStar);
  };
}

/**
 * The frame a view notes with the atmosphere cache (`AtmosphereView.noteFrame`): each of the
 * scene's lit bodies, its regime on the view's last photorealistic plan (a point where it has none)
 * and the stars its record was shaded from, on a kept scene its setting, and where `observerOf`
 * gives one where the view's camera stands at it (R08.T14.d.4).
 *
 * @param regimes - The view's drawer's last plan's regimes (`ViewFrameDrawer.regimes`).
 * @param sources - That plan's sources by body (`BodyFramePlan.sources`).
 * @param drawsAtmosphere - Whether the frame is photorealistic: a wireframe frame draws none, and
 *   notes no body wanted.
 * @param observerOf - Where the camera stands at a body under its lights ({@link cameraObservers});
 *   none if absent.
 */
export function atmosphereFrameOf(
  scene: ViewScene,
  regimes: ReadonlyMap<BodyIdHex, LitRegime>,
  sources: ReadonlyMap<BodyIdHex, BodyLightSources>,
  input: Pick<ViewAtmosphereInput, "scene" | "nowMs">,
  drawsAtmosphere: boolean,
  observerOf?: (body: ViewBody, lights: ReadonlyArray<BodyLight>) => AtmosphereObserver | undefined,
): AtmosphereFrame {
  // A kept scene of served bodies has their records asked, as a server scene's (the owner's body
  // inspection, 2026-11-01).
  const kept = bringsOwnSettings(scene);
  const bodies: AtmosphereFrameBody[] = drawsAtmosphere
    ? scene.bodies.filter(isLitBody).map((body): AtmosphereFrameBody => {
        const regime = regimes.get(body.id) ?? "point";
        const lights = bodyLightsOf(scene, sources.get(body.id));
        const observer = observerOf?.(body, lights);
        if (kept) {
          const setting = body.atmosphere ?? UNSET;
          return observer === undefined
            ? { id: body.id, regime, lights, kept: setting }
            : { id: body.id, regime, lights, kept: setting, observer };
        }
        return observer === undefined
          ? { id: body.id, regime, lights }
          : { id: body.id, regime, lights, observer };
      })
    : [];
  return { nowMs: input.nowMs, scene: kept ? null : input.scene, bodies };
}

/** What a view's label block states of its atmospheres (plan R08, R08.T10.b). */
export interface ViewAtmosphereShown {
  /** Its bodies' labels, with `atmospherePending` while its hold shows it. */
  readonly labels: ReadonlyArray<AtmosphereLabel>;
  /** Its `ATMOSPHERE REJECTED` lines, among the atmosphere's notes. */
  readonly plate: ReadonlyArray<AtmosphereRejectedLine>;
  /**
   * The lines that follow `ATMOSPHERE: APPROXIMATE`, each reason with the bodies it holds for
   * (`atmosphereApproximateLines`; R08.T10.c).
   */
  readonly approximate: ReadonlyArray<string>;
  /** Its `ATMOSPHERE NOT AVAILABLE` line, then its `ATMOSPHERE DATA INVALID` lines, for the fault line. */
  readonly fault: ReadonlyArray<string>;
}

/** A view that states nothing of its atmospheres. */
export const NO_ATMOSPHERE_SHOWN: ViewAtmosphereShown = {
  labels: [],
  plate: [],
  approximate: [],
  fault: [],
};

/**
 * What a view's report from the cache says on its label block: its labels, its bodies' problems
 * worded with their designations (`atmosphereProblemLines`), and `ATMOSPHERE: APPROXIMATE`'s
 * reasons with theirs (`atmosphereApproximateLines`; R08.T10.c). A problem or a reason of a body
 * the scene no longer holds, a report the scene has moved on from, says nothing: no line names a
 * body the view does not draw.
 */
export function atmosphereShownOf(
  report: AtmosphereViewReport,
  scene: ViewScene,
): ViewAtmosphereShown {
  const designations = new Map(scene.bodies.map((body) => [body.id, body.designation]));
  const problems: BodyAtmosphereProblem[] = [];
  for (const [id, problem] of report.problems) {
    const designation = designations.get(id);
    if (designation !== undefined) {
      problems.push({ designation, problem });
    }
  }
  const approximate: BodyApproximateReasons[] = [];
  for (const [id, reasons] of report.reasons) {
    const designation = designations.get(id);
    if (designation !== undefined) {
      approximate.push({ designation, reasons });
    }
  }
  const lines = atmosphereProblemLines(problems);
  return {
    labels: report.labels,
    plate: lines.plate,
    approximate: atmosphereApproximateLines(approximate),
    fault: lines.fault,
  };
}
