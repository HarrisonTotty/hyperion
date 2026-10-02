/**
 * How the `VIEW` display stands towards the server's scene (plan R02, R02.T17): whether it can draw
 * it, whether what it draws is stale, and the words that say so. Pure.
 */
import type { SystemIdHex } from "@hyperion/protocol";

import { isRefusal } from "../../components/RequestStatus";
import type { StatusStanding } from "../../components/StatusLine";
import {
  type CameraReportFault,
  SCENE_SILENCE_MS,
  type SceneSnapshot,
  type SceneStaleReason,
} from "../../lib/scene/useScene";
import type { SystemPlace } from "../../lib/scene/model";
import { serverSceneGap } from "../../view/scene/fromServer";
import type { SceneProvenance } from "../../view/scene/model";
import { SERVER_SCENE_NAME } from "./viewRun";

/** A status line's words and standing, and whether it offers `RETRY`. */
export interface SceneAnnunciation {
  readonly text: string;
  readonly standing: StatusStanding;
  /** Whether the line offers `RETRY`: the scene was refused or went unanswered. */
  readonly retry?: true;
}

/** The server's scene as the display can use it. */
export interface ServerSceneStanding {
  /** Whether the scene can be drawn: a model holding a system with its tidal radius and the ship. */
  readonly drawable: boolean;
  /** Whether the scene drawn is stale: the link is down, it is being reopened, or it fell silent. */
  readonly stale: boolean;
  /** What the status line under the `SCENE` selector says, or `null` for a live scene. */
  readonly annunciation: SceneAnnunciation | null;
}

/** Why the server's scene is not drawn: its first words, then the cause. */
const NOT_AVAILABLE = "SCENE NOT AVAILABLE";

/**
 * Where the server's scene stands, from `useScene`'s snapshot.
 *
 * @remarks
 * While the scene cannot be drawn the display draws a kept scene in its place, under the training
 * banner, and the line says why: no universe open, the subscription pending, the link down, the
 * server's refusal (plain, as every refusal is) or a fault, an unanswered subscription, or a scene
 * with the ship in no system or without its tidal radius. A refused or unanswered scene offers
 * `RETRY`. A scene held through a stale period stays drawn, its time held, and the line says it is
 * stale and why: the link is down (`NO CARRIER`), the scene is being opened again (after the server
 * ended it with `subscription_ended`, or after a push the client could not apply), or nothing has
 * arrived for twice the heartbeat. Every annunciation is three words or fewer before its colon.
 */
export function serverSceneStanding(snapshot: SceneSnapshot): ServerSceneStanding {
  const { status, model } = snapshot;
  const gap = model === null ? null : serverSceneGap(model);
  const drawable = model !== null && gap === null;
  let annunciation: SceneAnnunciation | null;
  let stale = false;
  switch (status.kind) {
    case "idle":
      annunciation = { text: `${NOT_AVAILABLE}: no universe open`, standing: "waiting" };
      break;
    case "pending":
      annunciation = { text: "SCENE PENDING", standing: "waiting" };
      break;
    case "link_down":
      annunciation = { text: `${NOT_AVAILABLE}: NO CARRIER`, standing: "waiting" };
      break;
    case "rejected":
      annunciation = {
        text: `SCENE REJECTED: ${status.reason}`,
        standing: isRefusal(status.code) ? "refused" : "fault",
        retry: true,
      };
      break;
    case "timed_out":
      annunciation = { text: "SCENE TIMED OUT", standing: "fault", retry: true };
      break;
    case "stale":
      // A stale scene is drawn as it was held, if it could be drawn.
      stale = drawable;
      annunciation = drawable ? staleAnnunciation(status.reason) : null;
      break;
    case "live":
      annunciation = null;
      break;
  }
  // A refusal or a timeout ends the scene: what it held is no longer drawn.
  if (status.kind === "live" || status.kind === "stale") {
    if (drawable) {
      return { drawable, stale, annunciation };
    }
  } else {
    return { drawable: false, stale: false, annunciation };
  }
  return {
    drawable: false,
    stale: false,
    annunciation:
      gap === "no_tidal_radius"
        ? { text: `${NOT_AVAILABLE}: the system's tidal radius was not sent`, standing: "fault" }
        : { text: `${NOT_AVAILABLE}: the ship is in no system`, standing: "waiting" },
  };
}

function staleAnnunciation(reason: SceneStaleReason): SceneAnnunciation {
  let text: string;
  switch (reason) {
    case "link_down":
      text = "SCENE STALE: NO CARRIER";
      break;
    case "resubscribing":
      text = "SCENE STALE: reopening the scene";
      break;
    case "silent":
      text = `SCENE STALE: nothing received for ${String(SCENE_SILENCE_MS / 1_000)} s`;
      break;
  }
  return { text, standing: "waiting" };
}

/**
 * The line for a camera report the server did not accept: refused, with its reason, as a refusal;
 * unanswered, as a fault.
 */
export function cameraAnnunciation(fault: CameraReportFault): SceneAnnunciation {
  let shown: SceneAnnunciation;
  switch (fault.kind) {
    case "refused":
      shown = { text: `CAMERA REPORT REFUSED: ${fault.reason}`, standing: "refused" };
      break;
    case "timed_out":
      shown = { text: "CAMERA REPORT UNANSWERED", standing: "fault" };
      break;
  }
  return shown;
}

/**
 * Where the scene `VIEW` draws comes from: the server's, when it is chosen and can be drawn, else
 * the kept scene standing in. The header strip's `TRAINING` banner follows it (delegated decision 3).
 */
export function viewProvenance(
  sceneName: string,
  snapshot: SceneSnapshot,
): SceneProvenance["kind"] {
  return sceneName === SERVER_SCENE_NAME && serverSceneStanding(snapshot).drawable
    ? "server"
    : "kept";
}

/**
 * Where the client places a server scene's system: the place the scene states (R03.T16); from a
 * server that does not state it, the system the client was told of (the `SYSTEM` display's
 * opening, whose target carries the chart's designation and position); else the system's ID with
 * no position.
 */
export function systemPlace(
  system: SystemIdHex,
  stated: SystemPlace | null,
  known: SystemPlace | null,
): SystemPlace {
  if (stated !== null) {
    return stated;
  }
  return known?.system === system ? known : { kind: "unknown", system, designation: system };
}
