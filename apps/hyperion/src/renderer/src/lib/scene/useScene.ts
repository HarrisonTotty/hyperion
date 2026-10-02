/**
 * The scene of a universe as a display holds it: the subscription, the model it keeps, the time to
 * render at and the local cameras' reports (rendering plan R03, R03.T14; Design notes 4, 6 and 9).
 */
import type {
  CameraReportDto,
  DetailLevelDto,
  NotificationOf,
  PendingSubscription,
  RequestClient,
  RequestFailure,
  SceneNotificationDto,
  SceneStateDto,
  StateOf,
  Subscription,
  SystemIdHex,
  UniverseIdHex,
} from "@hyperion/protocol";
import { useEffect, useState, useSyncExternalStore } from "react";

import type { ViewId } from "../../view/camera/state";
import { RECONNECT_DELAY_MS } from "../connection";
import { useServerLink } from "../serverLink";
import { followRequest, REQUEST_TIMEOUT_MS } from "../useServerRequest";
import { type SceneFrame, sceneAt, shipObserver } from "./apparent";
import { CameraReporter } from "./cameraReports";
import type { SceneKinematics, SceneModel } from "./model";
import { renderTime } from "./sceneClock";
import { applySceneNotification, type Designate, toSceneModel } from "./sceneWire";

/**
 * Why a scene could not be opened: the server's refusal, or `unusable` for an opening state the
 * client cannot read or a run of pushes it could not apply.
 */
export type SceneRefusalCode =
  Exclude<RequestFailure["code"], "aborted" | "superseded" | "link_lost"> | "unusable";

/**
 * Why the scene on show is stale: the link is down; the scene is being opened again after a
 * numbering error, an unusable push or the server's ending it; or nothing has arrived for twice
 * the heartbeat.
 */
export type SceneStaleReason = "link_down" | "resubscribing" | "silent";

/**
 * Where a display's scene subscription stands.
 *
 * @remarks
 * `stale` keeps the last scene on show, for one of {@link SceneStaleReason}'s reasons, and holds
 * its clock at the moment it went stale; the next push makes it `live` again. `link_down` is the
 * link being down before any scene arrived. `rejected` and `timed_out` (a subscription unanswered in {@link REQUEST_TIMEOUT_MS}) stay until
 * the link comes back or the scene asked for changes.
 */
export type SceneStatus =
  | { readonly kind: "idle" }
  | { readonly kind: "pending" }
  | { readonly kind: "live" }
  | { readonly kind: "stale"; readonly reason: SceneStaleReason }
  | { readonly kind: "link_down" }
  | { readonly kind: "rejected"; readonly code: SceneRefusalCode; readonly reason: string }
  | { readonly kind: "timed_out" };

/** What the scene subscription holds at one moment, as a render reads it. */
export interface SceneSnapshot {
  readonly status: SceneStatus;
  /** The scene after the latest push applied, or `null` before any arrived. */
  readonly model: SceneModel | null;
  /**
   * What became of the latest camera report settled, when it was not accepted: refused, with the
   * server's reason in its own words, or unanswered in {@link REQUEST_TIMEOUT_MS}; `null` once a
   * report is accepted.
   */
  readonly cameraFault: CameraReportFault | null;
}

/** A camera report the server did not accept: refused with its reason, or timed out. */
export type CameraReportFault =
  { readonly kind: "refused"; readonly reason: string } | { readonly kind: "timed_out" };

/** The scene of a universe, for R02's view: its state, its frames and its camera reports. */
export interface SceneView extends SceneSnapshot {
  /**
   * The scene as the ship sees it at the `performance.now()` of a frame being drawn: `sceneAt` at
   * the pushed time extrapolated by the time since the push arrived times its rate (Design
   * note 9), carrying the previous frame's local body; `null` with no scene or no system.
   *
   * @remarks
   * Meant to be called from a drawing loop, not during a render: it keeps the frame it returns as
   * the next one's `previous`. Stable for the life of the subscription.
   */
  readonly frameAt: (nowMs: number) => SceneFrame | null;
  /** Hands over a local view's camera pose for the reports (Design note 6). */
  readonly reportCamera: (view: ViewId, pose: SceneKinematics) => void;
  /** Stops reporting a local view's camera, as when the view closes. */
  readonly removeCamera: (view: ViewId) => void;
  /**
   * Opens the scene again after it was refused or went unanswered (the operator's `RETRY`); does
   * nothing in any other state.
   */
  readonly retry: () => void;
}

/** What a scene is asked for, besides its universe. */
export interface SceneOptions {
  /** The detail level asked for every body; the server grants each its own. */
  readonly detail: DetailLevelDto;
  /**
   * Names a system whose scene states no place (R03.T16), the fallback for a server that does not
   * send it. The latest one given is used from the next push on; a new one neither reopens the
   * scene nor relabels what is held.
   */
  readonly designate: Designate;
}

/** Resubscriptions in a row, with no push applied between them, after which the scene gives up. */
const MAX_RESUBSCRIPTIONS = 3;

/**
 * How long the scene may go without a push before it is shown stale, ms: twice the server's 1 s
 * heartbeat (`SCENE_HEARTBEAT`), the guide's "twice its expected period".
 */
export const SCENE_SILENCE_MS = 2_000;

const IDLE: SceneSnapshot = { status: { kind: "idle" }, model: null, cameraFault: null };

/** The opening state as the scene's adapter reads it, without the envelope's `topic`. */
function sceneState(state: StateOf<"scene">): SceneStateDto {
  const { topic: _topic, ...scene } = state;
  return scene;
}

/** A notification as the scene's adapter reads it, without the envelope's `topic`. */
function sceneNotification(notification: NotificationOf<"scene">): SceneNotificationDto {
  const { topic: _topic, ...scene } = notification;
  return scene;
}

/** The system a model holds, by which a frame's predecessor is kept or dropped. */
function systemOf(model: SceneModel | null): SystemIdHex | null {
  return model?.system?.model.system ?? null;
}

/** Whether two statuses say the same, so that setting one over the other changes nothing. */
function sameStatus(a: SceneStatus, b: SceneStatus): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/**
 * One scene subscription and what it holds, outside React: `useScene` reads it through
 * `useSyncExternalStore` and starts and stops it from an effect.
 */
class SceneStore {
  readonly #listeners = new Set<() => void>();
  readonly #reporter = new CameraReporter();
  #designate: Designate;
  #snapshot: SceneSnapshot;
  /** The `performance.now()` at which the push stating the model's clock arrived. */
  #receivedMs = 0;
  /** The `performance.now()` at which the scene went stale, at which its clock is held. */
  #staleSinceMs: number | null = null;
  #previousFrame: SceneFrame | null = null;
  #previousSystem: SystemIdHex | null = null;
  /** Bumped by every opening and closing, so that a callback of an earlier one does nothing. */
  #generation = 0;
  /** Resubscriptions since a push was last applied. */
  #resubscriptions = 0;
  #pending: PendingSubscription<"scene"> | null = null;
  #subscription: Subscription<"scene"> | null = null;
  #stopOpening: (() => void) | null = null;
  /** Stops following the camera report in flight; `null` with none in flight. */
  #stopCameraReport: (() => void) | null = null;
  /** The latest set of cameras handed over while a report was in flight, sent once it settles. */
  #waitingCameras: ReadonlyArray<CameraReportDto> | null = null;
  /** The watchdog that marks the scene stale when no push has arrived for a while. */
  #silence: ReturnType<typeof setTimeout> | undefined;
  /** The wait before opening the scene again after the server ended it. */
  #retry: ReturnType<typeof setTimeout> | undefined;

  constructor(
    readonly requests: RequestClient,
    readonly universe: UniverseIdHex | null,
    readonly detail: DetailLevelDto,
    designate: Designate,
  ) {
    this.#designate = designate;
    this.#snapshot = universe === null ? IDLE : { ...IDLE, status: { kind: "pending" } };
  }

  /** Whether this store serves these inputs, or `useScene` must replace it. */
  serves(requests: RequestClient, universe: UniverseIdHex | null, detail: DetailLevelDto): boolean {
    return this.requests === requests && this.universe === universe && this.detail === detail;
  }

  /** Names systems with `designate` from the next push on. */
  setDesignate(designate: Designate): void {
    this.#designate = designate;
  }

  /** Adds a listener for every change of the snapshot, as `useSyncExternalStore` asks. */
  readonly subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  /** The current snapshot, the same object until it changes. */
  readonly getSnapshot = (): SceneSnapshot => this.#snapshot;

  /** Opens the subscription, the link being up. */
  open(): void {
    const universe = this.universe;
    if (universe === null) {
      return;
    }
    this.#stop();
    const generation = this.#generation;
    const pending = this.requests.subscribe(universe, {
      topic: "scene",
      detail: this.detail,
      // Cameras held from an earlier subscription may be out of the new scene's reach, which
      // would refuse the whole subscription; they are sent once it is open.
      cameras: [],
    });
    this.#pending = pending;
    if (this.#snapshot.model === null) {
      this.#set({ status: { kind: "pending" } });
    }
    const timer = setTimeout(() => {
      if (generation === this.#generation) {
        this.#stop();
        this.#set({ status: { kind: "timed_out" } });
      }
    }, REQUEST_TIMEOUT_MS);
    this.#stopOpening = () => {
      clearTimeout(timer);
    };
    pending.outcome
      .then((outcome) => {
        if (generation !== this.#generation) {
          if (outcome.ok) {
            outcome.subscription.unsubscribe();
          }
          return undefined;
        }
        clearTimeout(timer);
        this.#pending = null;
        if (outcome.ok) {
          this.#adopt(outcome.subscription, generation);
          return undefined;
        }
        const { code, message } = outcome.error;
        if (code === "link_lost") {
          this.#goStale("link_down");
        } else if (code !== "aborted" && code !== "superseded") {
          this.#set({ status: { kind: "rejected", code, reason: message } });
        }
        return undefined;
      })
      .catch((error: unknown) => {
        console.error("a scene subscription's outcome failed to settle:", error);
      });
  }

  /** See {@link SceneView.retry}. */
  readonly retry = (): void => {
    const status = this.#snapshot.status.kind;
    if (status !== "rejected" && status !== "timed_out") {
      return;
    }
    this.#resubscriptions = 0;
    this.#set({ status: { kind: "pending" } });
    this.open();
  };

  /** Stops the subscription; what it held stays for the link's return. */
  close(): void {
    this.#stop();
  }

  /** Marks the scene as cut off by the link, the subscription having ended with it. */
  linkDown(): void {
    this.#stop();
    this.#goStale("link_down");
  }

  /** See {@link SceneView.frameAt}. */
  readonly frameAt = (nowMs: number): SceneFrame | null => {
    const model = this.#snapshot.model;
    if (model === null) {
      return null;
    }
    const heldMs = this.#staleSinceMs === null ? nowMs : Math.min(nowMs, this.#staleSinceMs);
    const time = renderTime(model.clock, this.#receivedMs, heldMs);
    const observer = shipObserver(model, time);
    const system = systemOf(model);
    if (system !== this.#previousSystem) {
      this.#previousFrame = null;
      this.#previousSystem = system;
    }
    const frame = observer === null ? null : sceneAt(model, observer, time, this.#previousFrame);
    this.#previousFrame = frame;
    return frame;
  };

  /** See {@link SceneView.reportCamera}. */
  readonly reportCamera = (view: ViewId, pose: SceneKinematics): void => {
    this.#reporter.report(view, pose);
  };

  /** See {@link SceneView.removeCamera}. */
  readonly removeCamera = (view: ViewId): void => {
    this.#reporter.remove(view);
  };

  #adopt(subscription: Subscription<"scene">, generation: number): void {
    const opened = toSceneModel(sceneState(subscription.state), this.#designate);
    if (opened.kind === "fault") {
      subscription.unsubscribe();
      this.#set({ status: { kind: "rejected", code: "unusable", reason: opened.fault } });
      return;
    }
    this.#subscription = subscription;
    this.#receive(opened.model);
    subscription.onNotification((notification) => {
      this.#apply(notification);
    });
    subscription.onEnd((end) => {
      if (generation !== this.#generation) {
        return;
      }
      switch (end.kind) {
        case "link_lost":
          this.#subscription = null;
          this.#reporter.detach();
          this.#goStale("link_down");
          break;
        case "ended":
          this.#subscription = null;
          this.#resubscribe(
            `the server ended the scene (${end.error.code}: ${end.error.message})`,
            end.error.code,
            RECONNECT_DELAY_MS,
          );
          break;
        case "unsubscribed":
          break;
      }
    });
    this.#reporter.attach((cameras) => {
      this.#reportCameras(subscription.id, cameras);
    });
  }

  /**
   * Sends a set of cameras, or holds it until the report in flight settles: one report is in
   * flight at a time, so that each is answered, and a refusal is never lost to a newer report.
   */
  #reportCameras(subscription: number, cameras: ReadonlyArray<CameraReportDto>): void {
    if (this.#stopCameraReport !== null) {
      this.#waitingCameras = cameras;
      return;
    }
    this.#stopCameraReport = followRequest(
      this.requests.request({ kind: "scene_cameras", subscription, cameras: [...cameras] }),
      REQUEST_TIMEOUT_MS,
      (state) => {
        this.#stopCameraReport = null;
        switch (state.kind) {
          case "ok":
            this.#set({ cameraFault: null });
            break;
          case "rejected":
            this.#set({ cameraFault: { kind: "refused", reason: state.reason } });
            break;
          case "timed_out":
            this.#set({ cameraFault: { kind: "timed_out" } });
            break;
          case "link_down":
            break;
        }
        const waiting = this.#waitingCameras;
        this.#waitingCameras = null;
        if (waiting !== null && state.kind !== "link_down") {
          this.#reportCameras(subscription, waiting);
        }
      },
    );
  }

  #apply(notification: NotificationOf<"scene">): void {
    const model = this.#snapshot.model;
    if (model === null) {
      return;
    }
    const update = applySceneNotification(model, sceneNotification(notification), this.#designate);
    switch (update.kind) {
      case "ok":
        this.#resubscriptions = 0;
        this.#receive(update.model);
        break;
      case "sequence":
        this.#resubscribe(
          `scene notification ${update.received} arrived where ${update.expected} was due`,
          "unusable",
          0,
        );
        break;
      case "fault":
        this.#resubscribe(`scene notification unusable (${update.fault})`, "unusable", 0);
        break;
    }
  }

  /** Takes `model` as the scene, its clock stated as of now, and restarts the watchdog. */
  #receive(model: SceneModel): void {
    this.#receivedMs = performance.now();
    this.#staleSinceMs = null;
    this.#snapshot = { ...this.#snapshot, model, status: { kind: "live" } };
    clearTimeout(this.#silence);
    this.#silence = setTimeout(() => {
      this.#silence = undefined;
      this.#goStale("silent");
    }, SCENE_SILENCE_MS);
    this.#notify();
  }

  /**
   * Opens the scene again, after `delayMs`, once it could not go on: a push it could not apply or
   * the server's ending it. After {@link MAX_RESUBSCRIPTIONS} in a row with no push applied
   * between, it gives up as `rejected` with `code`.
   */
  #resubscribe(why: string, code: SceneRefusalCode, delayMs: number): void {
    this.#stop();
    this.#resubscriptions += 1;
    if (this.#resubscriptions > MAX_RESUBSCRIPTIONS) {
      console.error(`${why}; giving up after ${MAX_RESUBSCRIPTIONS} resubscriptions`);
      this.#set({ status: { kind: "rejected", code, reason: why } });
      return;
    }
    console.error(`${why}; resubscribing`);
    this.#goStale("resubscribing");
    if (delayMs <= 0) {
      this.open();
      return;
    }
    this.#retry = setTimeout(() => {
      this.#retry = undefined;
      this.open();
    }, delayMs);
  }

  /** Marks the scene held as stale, or the link as down with none held. */
  #goStale(reason: SceneStaleReason): void {
    if (reason !== "silent") {
      // The scene is stale for a reason of its own now; silence must not relabel it.
      clearTimeout(this.#silence);
      this.#silence = undefined;
    }
    if (this.#snapshot.model === null) {
      this.#set({ status: reason === "link_down" ? { kind: "link_down" } : { kind: "pending" } });
      return;
    }
    this.#staleSinceMs ??= performance.now();
    this.#set({ status: { kind: "stale", reason } });
  }

  /** Ends every opening, subscription and report in progress. */
  #stop(): void {
    this.#generation += 1;
    this.#stopOpening?.();
    this.#stopOpening = null;
    this.#pending?.cancel();
    this.#pending = null;
    this.#subscription?.unsubscribe();
    this.#subscription = null;
    this.#reporter.detach();
    this.#stopCameraReport?.();
    this.#stopCameraReport = null;
    this.#waitingCameras = null;
    clearTimeout(this.#silence);
    this.#silence = undefined;
    clearTimeout(this.#retry);
    this.#retry = undefined;
  }

  /** Changes the snapshot's status or camera fault, notifying only for a real change. */
  #set(change: {
    readonly status?: SceneStatus;
    readonly cameraFault?: CameraReportFault | null;
  }): void {
    const current = this.#snapshot;
    const status = change.status ?? current.status;
    const cameraFault = change.cameraFault === undefined ? current.cameraFault : change.cameraFault;
    if (
      sameStatus(status, current.status) &&
      JSON.stringify(cameraFault) === JSON.stringify(current.cameraFault)
    ) {
      return;
    }
    this.#snapshot = { ...current, status, cameraFault };
    this.#notify();
  }

  #notify(): void {
    for (const listener of this.#listeners) {
      listener();
    }
  }
}

/**
 * Subscribes a display to the scene of `universe` and keeps it: the model after every push, the
 * time to render at, and the reports of the display's cameras.
 *
 * @remarks
 * Subscribes when the link is up, and again when it returns, after a gap or a step back in the
 * pushes' numbering, after a push the client cannot use (Design note 4), and
 * {@link RECONNECT_DELAY_MS} after the server ends the scene with `subscription_ended`, giving up
 * as `rejected` after {@link MAX_RESUBSCRIPTIONS} in a row with no push applied between. While the
 * link is down, or once no push has arrived for {@link SCENE_SILENCE_MS}, the last scene stays on
 * show as `stale`, its clock held. A camera report goes out at most at 4 Hz and at once on
 * a change of frame, through `scene_cameras` once the subscription is open, one in flight at a
 * time. Every push re-renders the caller; a drawing loop reads positions through
 * {@link SceneView.frameAt}, which needs no render.
 *
 * @param universe - The universe whose scene to show, or `null` for none (`idle`).
 */
export function useScene(universe: UniverseIdHex | null, options: SceneOptions): SceneView {
  const { status, requests } = useServerLink();
  const connected = status === "connected";
  const { detail, designate } = options;
  const [held, setHeld] = useState(() => new SceneStore(requests, universe, detail, designate));
  let store = held;
  if (!held.serves(requests, universe, detail)) {
    store = new SceneStore(requests, universe, detail, designate);
    setHeld(store);
  }
  const snapshot = useSyncExternalStore(store.subscribe, store.getSnapshot);

  useEffect(() => {
    store.setDesignate(designate);
  }, [store, designate]);

  useEffect(() => {
    if (store.universe === null) {
      return undefined;
    }
    if (!connected) {
      store.linkDown();
      return undefined;
    }
    store.open();
    return () => {
      store.close();
    };
  }, [store, connected]);

  return {
    ...snapshot,
    frameAt: store.frameAt,
    reportCamera: store.reportCamera,
    removeCamera: store.removeCamera,
    retry: store.retry,
  };
}
