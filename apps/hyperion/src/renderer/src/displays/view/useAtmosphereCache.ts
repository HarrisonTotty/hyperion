/**
 * `VIEW`'s atmosphere cache, made above its stage (plan R08, R08.T10.b; `AtmosphereCache`'s
 * remarks): one a device, which every photorealistic view of the stage draws its bodies' air from,
 * and its `body_detail` fetch joined with the terrain session's ({@link useBodyDetailShare}).
 *
 * @remarks
 * The cache lives as long as the engine and the requests it asks through: `Activity` disposes the
 * engine when `VIEW` is hidden, so a `VIEW` shown again makes a new cache, which holds no failure
 * of the old and asks every body afresh, as `decision-r08-t10a-leans.md` item 6 has "reopen VIEW to
 * retry" do.
 */
import type { RequestClient } from "@hyperion/protocol";
import { useEffect, useState, useSyncExternalStore } from "react";

import {
  type AtmosphereRequests,
  atmosphereTablesOn,
  AtmosphereCache,
} from "../../view/atmosphere/AtmosphereCache";
import { BodyDetailShare } from "../../view/atmosphere/bodyDetailShare";
import {
  defaultOpticsWorkers,
  type OpticsWorkerLike,
  OpticsWorkerPool,
} from "../../view/atmosphere/opticsPool";
import type { ViewEngineState } from "./useViewEngine";

/**
 * The requests of a cache whose scene is kept: its bodies bring their own settings and are never
 * asked, so an ask is a bug.
 */
const KEPT_SCENE_REQUESTS: AtmosphereRequests = {
  request: () => {
    throw new Error("a kept scene's atmosphere cache asks the server for nothing");
  },
};

/** What `useAtmosphereCache` reads. */
export interface AtmosphereCacheInput {
  readonly engineState: ViewEngineState;
  /** The cache's requests: the share's (`BodyDetailShare.atmosphere`), or `null` for a kept scene. */
  readonly requests: AtmosphereRequests | null;
  /** Starts one optics worker, stable in identity: `startOpticsWorker` in the app. */
  readonly createWorker: () => OpticsWorkerLike;
}

/** The cache the hook holds, outside React, so that its making is an effect's and renders it. */
class CacheHolder {
  readonly #listeners = new Set<() => void>();
  #cache: AtmosphereCache | null = null;

  readonly subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  readonly get = (): AtmosphereCache | null => this.#cache;

  set(cache: AtmosphereCache | null): void {
    if (cache === this.#cache) {
      return;
    }
    this.#cache = cache;
    for (const listener of this.#listeners) {
      listener();
    }
  }
}

/**
 * `VIEW`'s atmosphere cache on its engine, or `null` with no engine ready: made in an effect, kept
 * while the engine, the requests and the worker factory stay, and disposed, its workers terminated
 * and its tables released, when any changes or the caller unmounts.
 */
export function useAtmosphereCache(input: AtmosphereCacheInput): AtmosphereCache | null {
  const { engineState, requests, createWorker } = input;
  const engine = engineState.kind === "ready" ? engineState.engine : null;
  const [holder] = useState(() => new CacheHolder());
  const cache = useSyncExternalStore(holder.subscribe, holder.get);
  useEffect(() => {
    if (engine === null) {
      return undefined;
    }
    const made = new AtmosphereCache({
      requests: requests ?? KEPT_SCENE_REQUESTS,
      builder: new OpticsWorkerPool({
        workers: defaultOpticsWorkers(navigator.hardwareConcurrency),
        createWorker,
      }),
      tables: atmosphereTablesOn(engine),
    });
    holder.set(made);
    return () => {
      holder.set(null);
      made.dispose();
    };
  }, [engine, requests, createWorker, holder]);
  return cache;
}

/**
 * The `body_detail` share over a request client (`BodyDetailShare`), made again for another
 * client, so that the terrain session and the atmosphere cache ask one fetch.
 */
export function useBodyDetailShare(client: RequestClient): BodyDetailShare {
  const [held, setHeld] = useState(() => ({ client, share: new BodyDetailShare(client) }));
  if (held.client !== client) {
    const next = { client, share: new BodyDetailShare(client) };
    setHeld(next);
    return next.share;
  }
  return held.share;
}
