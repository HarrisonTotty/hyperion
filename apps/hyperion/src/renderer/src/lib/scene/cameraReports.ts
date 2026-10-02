/**
 * The reports of a client's local cameras to its scene subscription (rendering plan R03, R03.T14;
 * Design note 6).
 *
 * @remarks
 * Each local view hands its camera's pose to one {@link CameraReporter}, which sends the whole set
 * at most at 4 Hz and at once when a camera's frame changes or a view appears or goes. The server
 * keeps the set it was last sent, replacing the previous one whole, so every report carries every
 * camera. Cameras are never ship state: the server neither saves nor echoes them.
 */
import type { CameraReportDto } from "@hyperion/protocol";

import type { ViewId } from "../../view/camera/state";
import type { SceneKinematics, ScenePosition } from "./model";
import { toKinematicsDto } from "./sceneWire";

/**
 * The shortest interval between two reports that no frame change forced, ms: 4 Hz, which keeps a
 * report within a quarter-second of a camera's pose for a few hundred bytes a second.
 */
export const CAMERA_REPORT_INTERVAL_MS = 250;

/** The most cameras one scene subscription reports, the server's `MAX_SCENE_CAMERAS`. */
export const MAX_SCENE_CAMERAS = 8;

/** Sends a whole set of camera reports, replacing the set the server holds. */
export type SendCameraReports = (cameras: ReadonlyArray<CameraReportDto>) => void;

/** Whether two positions are in the same frame: the same kind, and the same system or body. */
function sameFrame(a: ScenePosition, b: ScenePosition): boolean {
  let same: boolean;
  switch (a.kind) {
    case "galactic":
      same = b.kind === "galactic";
      break;
    case "system":
      same = b.kind === "system" && b.system === a.system;
      break;
    case "body":
      same = b.kind === "body" && b.body === a.body;
      break;
  }
  return same;
}

/** One local view's camera as last handed over, with the slot it is reported under. */
interface Camera {
  readonly slot: number;
  readonly pose: SceneKinematics;
}

/**
 * Collects the local views' camera poses and sends them as one set, at most at 4 Hz and at once on
 * a change of frame.
 *
 * @remarks
 * A view is reported under a slot from 0 to 7, the wire's `view`, held from its first pose until
 * {@link CameraReporter.remove}. A pose in the frame the view's last report used is sent no sooner
 * than {@link CAMERA_REPORT_INTERVAL_MS} after the previous report, the latest pose then; a new
 * view, a removed one or a change of frame sends the set at once. While the reporter is detached
 * (no subscription is live) poses are kept and nothing is sent; {@link CameraReporter.attach} sends
 * the set held, if any, at once. {@link CameraReporter.detach} stops its timer.
 */
export class CameraReporter {
  readonly #cameras = new Map<ViewId, Camera>();
  /** The frame each view was last reported in, by which a change of frame is told. */
  readonly #reported = new Map<ViewId, ScenePosition>();
  #send: SendCameraReports | null = null;
  #timer: ReturnType<typeof setTimeout> | undefined;
  /** Whether a pose has changed since the last report, to be sent when the interval ends. */
  #due = false;

  /**
   * Hands over `view`'s camera pose.
   *
   * @throws RangeError when a ninth view reports, more than one subscription carries: a wiring
   *   bug, since a client has at most eight local views.
   */
  report(view: ViewId, pose: SceneKinematics): void {
    const held = this.#cameras.get(view);
    let slot = held?.slot;
    if (slot === undefined) {
      slot = this.#freeSlot();
      if (slot === undefined) {
        throw new RangeError(`a scene subscription reports at most ${MAX_SCENE_CAMERAS} cameras`);
      }
    }
    this.#cameras.set(view, { slot, pose });
    const reported = this.#reported.get(view);
    if (reported === undefined || !sameFrame(reported, pose.position)) {
      this.#sendNow();
    } else {
      this.#sendSoon();
    }
  }

  /** Stops reporting `view`'s camera, which the next report, sent at once, leaves out. */
  remove(view: ViewId): void {
    if (this.#cameras.delete(view)) {
      this.#reported.delete(view);
      this.#sendNow();
    }
  }

  /** The set of cameras as it would be sent now, in slot order. */
  cameras(): CameraReportDto[] {
    return [...this.#cameras.values()]
      .toSorted((a, b) => a.slot - b.slot)
      .map(({ slot, pose }) => ({ view: slot, pose: toKinematicsDto(pose) }));
  }

  /**
   * Starts sending reports through `send`, at once with the set held if there is one: a new
   * subscription holds no cameras until it is told them.
   */
  attach(send: SendCameraReports): void {
    this.detach();
    this.#send = send;
    if (this.#cameras.size > 0) {
      this.#sendNow();
    }
  }

  /** Stops sending, and stops the timer; the poses held are kept for the next {@link attach}. */
  detach(): void {
    this.#send = null;
    this.#due = false;
    if (this.#timer !== undefined) {
      clearTimeout(this.#timer);
      this.#timer = undefined;
    }
  }

  #freeSlot(): number | undefined {
    const taken = new Set([...this.#cameras.values()].map(({ slot }) => slot));
    for (let slot = 0; slot < MAX_SCENE_CAMERAS; slot += 1) {
      if (!taken.has(slot)) {
        return slot;
      }
    }
    return undefined;
  }

  /** Sends the set now and starts the interval before the next report that is not forced. */
  #sendNow(): void {
    const send = this.#send;
    if (send === null) {
      return;
    }
    this.#due = false;
    if (this.#timer !== undefined) {
      clearTimeout(this.#timer);
    }
    for (const [view, { pose }] of this.#cameras) {
      this.#reported.set(view, pose.position);
    }
    send(this.cameras());
    this.#timer = setTimeout(() => {
      this.#timer = undefined;
      if (this.#due) {
        this.#sendNow();
      }
    }, CAMERA_REPORT_INTERVAL_MS);
  }

  /** Sends the set now if the interval has passed, else once it does. */
  #sendSoon(): void {
    if (this.#send === null) {
      return;
    }
    if (this.#timer === undefined) {
      this.#sendNow();
    } else {
      this.#due = true;
    }
  }
}
