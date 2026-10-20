/**
 * `VIEW`'s terrain session, made above its stage (plan R10, R10.T16.a; decision-r10-live-path, D1):
 * one {@link TerrainSession} a source and engine, for the server's scene of the open universe over
 * the link's request client, or for a kept scene that carries a terrain fixture over the fixture's,
 * and for no other kept scene.
 *
 * @remarks
 * The hook lives in `ViewPanels`, so that a stage remounted on a new scene of the same source keeps
 * the session and its fields. A session is disposed with the engine (which `Activity` disposes
 * when `VIEW` is hidden, so a re-show fetches the fields again), the universe, the link's request
 * client or a change of source. The link's state reaches it as it changes, and the scene topic's
 * surface events (`useScene`'s `onSurface`, a fixture's own) as they come, neither rendering
 * anything.
 */
import type { UniverseIdHex } from "@hyperion/protocol";
import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";

import type { ServerLink } from "../../lib/serverLink";
import type { SceneSurfaceEvent, SceneView } from "../../lib/scene/useScene";
import type { ViewId } from "../../view/camera/state";
import type { TerrainFault } from "../../view/terrain/annunciation";
import type { TerrainFailureGuard } from "../../view/terrain/failureGuard";
import { TerrainSession, type TerrainRequests } from "../../view/terrain/session";
import type { WorkerLike } from "../../view/terrain/workers/pool";
import type { ViewEngineState } from "./useViewEngine";
import type { SceneOption } from "./viewRun";

/** Listens for a scene's surface events (`SceneView.onSurface`'s shape); returns the unsubscribe. */
export type SurfaceEvents = SceneView["onSurface"];

/**
 * A kept scene's terrain fixture (R10.T16.d's `TERRAIN TEST`): the universe its bodies are in, and
 * the link that serves their fields, for which `VIEW` makes a terrain session.
 */
export interface TerrainFixture {
  /** The universe its bodies' fields are asked in. */
  readonly universe: UniverseIdHex;
  /** Opens its link for one session, which closes it after the session is disposed. */
  readonly open: () => TerrainFixtureLink;
  /**
   * Takes the time, s, to which the stage has played the kept scene's script (`ViewRun.tS`), each
   * primary frame: `TERRAIN TEST`'s fixture raises its field's revision the first time the script
   * passes the delta's time (R10.T16.d; decision-r10-t16-contracts §5). Absent, a fixture follows
   * no script.
   */
  readonly notePlayed?: (scriptS: number) => void;
}

/** A terrain fixture's link for one session: always up. */
export interface TerrainFixtureLink {
  /** Answers the session's `surface_field` and `body_detail` from the fixture's payloads. */
  readonly requests: TerrainRequests;
  /** Its scene's surface events: each revision rise of a payload, as the scene topic's. */
  readonly onSurface: SurfaceEvents;
  /** Ends what it started: its answers in flight and its timers. */
  readonly close: () => void;
}

/** What a stage draws, as the terrain session's source reads it. */
export type TerrainStage =
  | { readonly kind: "kept"; readonly option: SceneOption }
  | {
      readonly kind: "server";
      /** The open universe, or `null` with none open. */
      readonly universe: UniverseIdHex | null;
      readonly link: ServerLink;
      /** The scene subscription's surface events (`SceneView.onSurface`). */
      readonly onSurface: SurfaceEvents;
    };

/** Where a session's fields come from: the server's scene of a universe, or a kept fixture. */
export type TerrainSessionSource =
  | {
      readonly kind: "server";
      readonly universe: UniverseIdHex;
      readonly requests: TerrainRequests;
      readonly linkUp: boolean;
      readonly onSurface: SurfaceEvents;
    }
  | { readonly kind: "fixture"; readonly fixture: TerrainFixture };

/**
 * The terrain session's source for what a stage draws: the server's scene of the open universe,
 * over the link's request client; a kept scene's terrain fixture; or `null`, no session, for any
 * other kept scene and for a server scene with no universe open.
 */
export function terrainSessionSource(stage: TerrainStage): TerrainSessionSource | null {
  let source: TerrainSessionSource | null;
  switch (stage.kind) {
    case "server":
      source =
        stage.universe === null
          ? null
          : {
              kind: "server",
              universe: stage.universe,
              requests: stage.link.requests,
              linkUp: stage.link.status === "connected",
              onSurface: stage.onSurface,
            };
      break;
    case "kept":
      source =
        stage.option.terrain === undefined
          ? null
          : { kind: "fixture", fixture: stage.option.terrain };
      break;
  }
  return source;
}

/**
 * Starts one height worker on the renderer's own module (R04.T10.c's rule): the browser's, which
 * `VIEW` uses unless a test gives its own.
 */
export function browserHeightWorker(): WorkerLike {
  return new Worker(new URL("../../view/terrain/workers/height.worker.ts", import.meta.url), {
    type: "module",
    name: "height",
  });
}

/** What `useTerrainSession` reads. */
export interface TerrainSessionInput {
  readonly engineState: ViewEngineState;
  /** The stage's source ({@link terrainSessionSource}), or `null` for none. */
  readonly source: TerrainSessionSource | null;
  /** Starts one height worker, stable in identity: {@link browserHeightWorker} in the app. */
  readonly createWorker: () => WorkerLike;
  /** The workers' count; Design note 11's default where absent (`TerrainSessionOptions.workers`). */
  readonly workers?: number | undefined;
}

/** The session the hook holds, outside React, so that its making is an effect's and renders it. */
class SessionHolder {
  readonly #listeners = new Set<() => void>();
  #session: TerrainSession | null = null;

  readonly subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  readonly get = (): TerrainSession | null => this.#session;

  set(session: TerrainSession | null): void {
    if (session === this.#session) {
      return;
    }
    this.#session = session;
    for (const listener of this.#listeners) {
      listener();
    }
  }
}

/** Takes one surface event into `session`. */
function takeSurface(session: TerrainSession, event: SceneSurfaceEvent): void {
  switch (event.kind) {
    case "revisions":
      session.noteSurfaceRevisions(event.revisions);
      break;
    case "resync":
      session.resync();
      break;
  }
}

/**
 * `VIEW`'s terrain session for its stage's source, or `null` with none (no source, or no engine
 * ready): made in an effect, kept while the engine, the source's universe and request client (or
 * fixture), the worker factory and count stay, and disposed when any changes or the caller
 * unmounts.
 */
export function useTerrainSession(input: TerrainSessionInput): TerrainSession | null {
  const { engineState, source, createWorker, workers } = input;
  const engine = engineState.kind === "ready" ? engineState.engine : null;
  const universe = source?.kind === "server" ? source.universe : null;
  const requests = source?.kind === "server" ? source.requests : null;
  const onSurface = source?.kind === "server" ? source.onSurface : null;
  const fixture = source?.kind === "fixture" ? source.fixture : null;
  const linkUp = source?.kind === "server" ? source.linkUp : true;
  const [holder] = useState(() => new SessionHolder());
  const session = useSyncExternalStore(holder.subscribe, holder.get);
  // The link's state when a session is made, read through a ref so that its changes, which the
  // effect below follows, make no session anew.
  const linkUpRef = useRef(linkUp);
  useLayoutEffect(() => {
    linkUpRef.current = linkUp;
  }, [linkUp]);

  useEffect(() => {
    if (engine === null) {
      return undefined;
    }
    const counted = workers === undefined ? {} : { workers };
    if (universe !== null && requests !== null) {
      const made = new TerrainSession({
        engine,
        universe,
        requests,
        linkUp: linkUpRef.current,
        createWorker,
        ...counted,
      });
      holder.set(made);
      return () => {
        holder.set(null);
        made.dispose();
      };
    }
    if (fixture === null) {
      return undefined;
    }
    const link = fixture.open();
    const made = new TerrainSession({
      engine,
      universe: fixture.universe,
      requests: link.requests,
      linkUp: true,
      createWorker,
      ...counted,
    });
    const unsubscribe = link.onSurface((event) => {
      takeSurface(made, event);
    });
    holder.set(made);
    return () => {
      holder.set(null);
      unsubscribe();
      made.dispose();
      link.close();
    };
  }, [engine, universe, requests, fixture, createWorker, workers, holder]);

  useEffect(() => {
    if (session !== null && fixture === null) {
      session.setLinkUp(linkUp);
    }
  }, [session, linkUp, fixture]);

  useEffect(() => {
    if (session === null || onSurface === null) {
      return undefined;
    }
    return onSurface((event) => {
      takeSurface(session, event);
    });
  }, [session, onSurface]);

  return session;
}

/** No view shows the terrain's fault: the snapshot with no session. */
const NO_TERRAIN_FAULTS: ReadonlyMap<ViewId, TerrainFault> = new Map();

/** The subscription with no session, which nothing changes. */
function noFaults(): () => void {
  return () => undefined;
}

/**
 * The views whose label blocks show the terrain's fault, each with its text (plan R10, R10.T16.b and
 * R10.T16.e; `TerrainSession.terrainFaults`): every view that hands a body over once every height
 * worker is lost or a guarded computation has kept failing; none with no session.
 */
export function useTerrainFaults(
  session: TerrainSession | null,
): ReadonlyMap<ViewId, TerrainFault> {
  return useSyncExternalStore(
    session?.subscribeFaults ?? noFaults,
    () => session?.terrainFaults ?? NO_TERRAIN_FAULTS,
  );
}

/**
 * Steps a view's terrain session once (`TerrainSession.step`), through `guard` (plan R10, R10.T16
 * and R10.T16.e; the batch review's S5; decision-r10-t16-leans §8): a device loss
 * (`EngineUnavailable`) passes in silence, and anything else, a bug, is reported once until a step
 * succeeds again, so that no throw stops the view's loop; once its steps have failed for
 * `ANNUNCIATION_ONSET_MS`, the session's terrain is faulted (`TerrainSession.failComputation`).
 *
 * @param guard - The loop's guard of its session's step, one a session.
 * @param nowMs - The frame's monotonic time, ms.
 */
export function stepTerrainSession(
  session: Pick<TerrainSession, "step" | "failComputation"> | null,
  guard: TerrainFailureGuard,
  nowMs: number,
): void {
  guard.run(
    () => {
      session?.step();
    },
    undefined,
    nowMs,
    {
      describe: "the terrain session's step failed:",
      onFault: () => {
        session?.failComputation();
      },
    },
  );
}
