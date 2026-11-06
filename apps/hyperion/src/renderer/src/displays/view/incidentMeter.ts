/**
 * Where a view's `INCIDENT` meter reads the light, and its reading each frame (ux-calls-view, item
 * 3b, the owner's delegated call, 2026-11-01).
 */
import type { BodyIdHex } from "@hyperion/protocol";

import type { CameraTarget } from "../../view/camera/state";
import {
  heldHostDiscs,
  type HostsOfSky,
  sceneHostDiscs,
  type SystemHostDiscs,
} from "../../view/lighting/hostLights";
import { bringsOwnSettings } from "../../view/scene/model";
import { incidentIlluminanceLx } from "../../view/lighting/incident";
import { isLitBody } from "../../view/lighting/retarded";
import { frameBody, runPose, type ViewRun } from "./viewRun";

/**
 * The body a view's `INCIDENT` meter reads the light at: its `FRAME` body ({@link frameBody}) where
 * that is a lit body (`isLitBody`), else the selected body where that is one, else none.
 */
export function incidentBodyOf(run: ViewRun, selection: CameraTarget | null): BodyIdHex | null {
  const framed = frameBody(run.camera.pose.frame, run.scene);
  if (framed !== null && isLitBody(framed)) {
    return framed.id;
  }
  if (selection?.kind !== "body") {
    return null;
  }
  const selected = run.scene.bodies.find((body) => body.id === selection.body);
  return selected !== undefined && isLitBody(selected) ? selected.id : null;
}

/**
 * A view's `INCIDENT` reading this frame, lx: the illuminance on a flat receptor at its body
 * ({@link incidentBodyOf}) facing each host disc (`incidentIlluminanceLx`); 0 where no disc
 * reaches the body, and where the view has no body to read at, so that both read `NO LIGHT`;
 * `null` while the scene's lighting is awaited, a served scene none of whose system's discs is
 * held (`LIGHTING: PENDING`; `heldHostDiscs`), so that the meter is not yet metered rather than
 * `NO LIGHT`.
 *
 * @param sky - The view's held sky, or `null` where it holds none; a kept scene of its own brings
 *   its own discs.
 * @param asked - The scene's system's discs on their own (`host_discs`), or `null`.
 */
export function incidentReadingLx(
  run: ViewRun,
  selection: CameraTarget | null,
  sky: HostsOfSky | null,
  asked: SystemHostDiscs | null,
): number | null {
  if (!bringsOwnSettings(run.scene) && heldHostDiscs(run.scene, sky, asked) === null) {
    return null;
  }
  const body = incidentBodyOf(run, selection);
  return body === null
    ? 0
    : (incidentIlluminanceLx(
        run.scene,
        body,
        runPose(run),
        sceneHostDiscs(run.scene, sky, asked),
      ) ?? 0);
}
