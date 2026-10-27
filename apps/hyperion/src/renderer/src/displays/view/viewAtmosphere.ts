/**
 * What a `VIEW` view tells the atmosphere cache of its frame, and what its label block says of the
 * cache's answer (plan R08, R08.T10.b; `AtmosphereCache`'s remarks): pure, called each frame by the
 * view's drawer and at each publish by its label block.
 */

import { type BodyIdHex, formatBodyId } from "@hyperion/protocol";

import type {
  AtmosphereCache,
  AtmosphereFrame,
  AtmosphereFrameBody,
  AtmosphereScene,
  AtmosphereView,
  AtmosphereViewReport,
  KeptBodyAtmosphere,
} from "../../view/atmosphere/AtmosphereCache";
import type { BodyLight } from "../../view/atmosphere/atmosphereInput";
import {
  atmosphereApproximateLines,
  type AtmosphereLabel,
  atmosphereProblemLines,
  type AtmosphereRejectedLine,
  type BodyApproximateReasons,
  type BodyAtmosphereProblem,
} from "../../view/atmosphere/labels";
import type { AtmosphereTables } from "../../view/atmosphere/tables";
import type { BodyLightSources } from "../../view/bodies/draw";
import type { LitRegime } from "../../view/bodies/regime";
import { isLitBody } from "../../view/lighting/retarded";
import type { ViewScene } from "../../view/scene/model";

/** A view's atmospheres for its drawer's frame (`ViewFrameInputs.atmosphere`). */
export interface ViewAtmosphereInput {
  /** The view's handle on the device's cache (`AtmosphereCache.openView`). */
  readonly view: AtmosphereView;
  /** A body's tables as the cache holds them now (`AtmosphereCache.entryOf(id).tables`). */
  readonly tablesOf: (body: BodyIdHex) => AtmosphereTables | null;
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
    return { view, tablesOf: (body) => cache.entryOf(body)?.tables ?? null, scene, nowMs };
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

/**
 * The frame a view notes with the atmosphere cache (`AtmosphereView.noteFrame`): each of the
 * scene's lit bodies, its regime on the view's last photorealistic plan (a point where it has none)
 * and the stars its record was shaded from, and on a kept scene its setting.
 *
 * @param regimes - The view's drawer's last plan's regimes (`ViewFrameDrawer.regimes`).
 * @param sources - That plan's sources by body (`BodyFramePlan.sources`).
 * @param drawsAtmosphere - Whether the frame is photorealistic: a wireframe frame draws none, and
 *   notes no body wanted.
 */
export function atmosphereFrameOf(
  scene: ViewScene,
  regimes: ReadonlyMap<BodyIdHex, LitRegime>,
  sources: ReadonlyMap<BodyIdHex, BodyLightSources>,
  input: Pick<ViewAtmosphereInput, "scene" | "nowMs">,
  drawsAtmosphere: boolean,
): AtmosphereFrame {
  const kept = scene.provenance.kind === "kept";
  const bodies: AtmosphereFrameBody[] = drawsAtmosphere
    ? scene.bodies.filter(isLitBody).map((body): AtmosphereFrameBody => {
        const regime = regimes.get(body.id) ?? "point";
        const lights = bodyLightsOf(scene, sources.get(body.id));
        return kept
          ? { id: body.id, regime, lights, kept: body.atmosphere ?? UNSET }
          : { id: body.id, regime, lights };
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
