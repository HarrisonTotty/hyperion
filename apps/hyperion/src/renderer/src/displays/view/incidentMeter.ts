/**
 * Where a view's `INCIDENT` meter reads the light, and its reading each frame (ux-calls-view, item
 * 3b, the owner's delegated call, 2026-11-01).
 */
import type { BodyIdHex, HostDiscDto } from "@hyperion/protocol";

import type { CameraTarget } from "../../view/camera/state";
import { sceneHostDiscs } from "../../view/lighting/hostLights";
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
 * `null` while the scene's lighting is awaited, a scene lit by its sky's discs before its sky is
 * held (`LIGHTING: PENDING`), so that the meter is not yet metered rather than `NO LIGHT`.
 *
 * @param skyHosts - The view's sky's host discs (its `response.hosts`), or `null` where it holds
 *   none; a kept scene of its own brings its own.
 */
export function incidentReadingLx(
  run: ViewRun,
  selection: CameraTarget | null,
  skyHosts: ReadonlyArray<HostDiscDto> | null,
): number | null {
  if (skyHosts === null && !bringsOwnSettings(run.scene)) {
    return null;
  }
  const body = incidentBodyOf(run, selection);
  return body === null
    ? 0
    : (incidentIlluminanceLx(run.scene, body, runPose(run), sceneHostDiscs(run.scene, skyHosts)) ??
        0);
}
