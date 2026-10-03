/**
 * The `VIEW` display's sky (plan R06, T13.c, with T12's view wiring): asked about the scene's
 * system where its position is known, culled to the view's limit, and split between sprites and
 * the bake.
 */

import type { UniverseIdHex } from "@hyperion/protocol";
import { useMemo } from "react";

import { barycentreAt } from "../../lib/scene/place";
import type { SystemPlace } from "../../lib/scene/model";
import type { ExposureControl } from "../../view/photometry/exposure";
import { SETTINGS } from "../../view/quality/qualitySetting";
import { cameraGalacticPosition } from "../../view/sky/camera";
import type { BakeInput } from "../../view/sky/bake";
import { cullSky } from "../../view/sky/cull";
import { starIlluminanceRgbLx } from "../../view/sky/photometry";
import { skyLabelValue } from "../../view/sky/label";
import type { SkyCamera, SkyModel } from "../../view/sky/model";
import { selectSkySprites, type SkySelection } from "../../view/sky/select";
import { useSky } from "../../view/sky/useSky";
import { viewSkyLimit, viewSkyRequest } from "../../view/sky/viewSky";
import { runPose, type ViewRun } from "./viewRun";

/** The sky as the drawing loop draws it: the model and which of its stars are sprites. */
export interface DrawnSky {
  readonly model: SkyModel;
  readonly selection: SkySelection;
}

/** The view's sky, or `null` while R02's interim field stands in. */
export interface ViewSky {
  /** What the loop draws, or `null` before the sky arrives. */
  readonly drawn: DrawnSky | null;
  /** The label block's `STARS` reading, or `null` while the interim field's stands. */
  readonly labelValue: string | null;
  /** Whether a sky has been asked and not yet answered (R07's lighting label reads it). */
  readonly pending: boolean;
}

/** The baked stars of a view's sky as a bake takes them: directions and light per channel. */
export function bakeInputOf(sky: DrawnSky, faceSizePx: number): BakeInput {
  const { stars } = sky.model;
  const count = sky.selection.baked.length;
  const directions = new Float32Array(count * 3);
  const illuminanceLx = new Float32Array(count * 3);
  sky.selection.baked.forEach((index, at) => {
    directions.set(stars.directions.subarray(index * 3, index * 3 + 3), at * 3);
    illuminanceLx.set(
      starIlluminanceRgbLx(
        stars.vMag[index] ?? Number.POSITIVE_INFINITY,
        stars.chroma[index * 2] ?? 0,
        stars.chroma[index * 2 + 1] ?? 0,
      ),
      at * 3,
    );
  });
  return { directions, illuminanceLx, faceSizePx, name: "sky cube: view" };
}

/** What the view's sky is made from. */
export interface ViewSkyInput {
  /** The open universe, or `null`: nothing is asked. */
  readonly universe: UniverseIdHex | null;
  /** The server scene's system place, or `null` for a kept scene: nothing is asked. */
  readonly place: SystemPlace | null;
  /** The run as last published: its scene's time, its camera's pose, role and field of view. */
  readonly run: ViewRun;
  readonly exposure: ExposureControl;
  /** The view's width, device px, or `null` before it is measured. */
  readonly widthPx: number | null;
}

/** The view's width the parallax rule reads before the view is measured: 1080p's. */
const DEFAULT_WIDTH_PX = 1_920;

/**
 * The view's sky (plan R06, T13.c).
 *
 * @remarks
 * The high setting's N_max and sprite budget until the view takes a quality setting. Asked on the
 * published run (4 Hz), which is often enough: the request rule holds a sky for a year or until a
 * camera moves its nearest baked star by a tenth of a pixel.
 */
export function useViewSky(input: ViewSkyInput): ViewSky {
  const { universe, place, run, exposure } = input;
  const settings = SETTINGS.high.sky;
  const widthPx = input.widthPx ?? DEFAULT_WIDTH_PX;
  const time = run.scene.time;
  const observer = place === null ? null : barycentreAt(place, time);
  const request =
    universe === null || place === null || observer === null
      ? null
      : viewSkyRequest({
          universe,
          observer,
          system: place.system,
          time,
          role: run.camera.role,
          exposure,
          fovDeg: run.camera.fovDeg,
          nMax: settings.nMax,
        });
  const position = cameraGalacticPosition(runPose(run), run.scene);
  const cameras: ReadonlyArray<SkyCamera> =
    position === null ? [] : [{ position, fovDeg: run.camera.fovDeg, widthPx }];
  const { model, pending } = useSky(request, cameras);
  const role = run.camera.role;
  const fovDeg = run.camera.fovDeg;
  // A cull of up to 3 × 10⁵ stars, kept until the sky, the view's limit or its size changes, so
  // that the drawn sky keeps its identity from one published run to the next and is baked once.
  const culled = useMemo(() => {
    if (model === null) {
      return null;
    }
    const { limit, labelV } = viewSkyLimit(model, role, exposure, fovDeg);
    const kept = cullSky(model.stars, limit, model.response.band.face_texels);
    const selection = selectSkySprites(model.stars, kept.kept, settings.spriteBudget, {
      fovDeg,
      widthPx,
    });
    return {
      drawn: { model, selection },
      labelValue: skyLabelValue(labelV, role, model.response.not_modelled),
    };
  }, [model, role, exposure, fovDeg, widthPx, settings.spriteBudget]);
  // A sky is this view's only for the system it was asked about, whose position is known.
  const ours =
    culled !== null &&
    request !== null &&
    culled.drawn.model.request.exclude_system === request.exclude_system;
  return ours
    ? { drawn: culled.drawn, labelValue: culled.labelValue, pending }
    : { drawn: null, labelValue: null, pending };
}
