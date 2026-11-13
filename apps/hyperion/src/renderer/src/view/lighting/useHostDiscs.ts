/**
 * A served scene's system's host discs on their own (plans R06 and R07): asked of the server's
 * `host_discs` once the system is known, so that the view lights its bodies before its sky's first
 * reply, which may take tens of seconds for a system newly arrived at.
 */

import type { RequestFailure, SystemIdHex, UniverseIdHex, UniverseTime } from "@hyperion/protocol";
import { useEffect, useState } from "react";

import { useServerLink } from "../../lib/serverLink";
import { FIELD_RETRY_DELAY_MS, FIELD_RETRY_MAX_DELAY_MS } from "../terrain/coarseFieldStore";
import type { SystemHostDiscs } from "./hostLights";

/** What a view asks the discs of: its universe and its scene's system, at the scene's time. */
export interface HostDiscsAsk {
  readonly universe: UniverseIdHex;
  readonly system: SystemIdHex;
  /** The scene's time as the system is first asked about: the stars' state then. */
  readonly time: UniverseTime;
}

/** What a view holds of its system's discs. */
export interface HostDiscsView {
  /** The discs of the system asked, once answered, or `null`. */
  readonly held: SystemHostDiscs | null;
  /**
   * Whether the system asked has its discs neither answered nor refused: asked of the server, or,
   * through a lost link, to be asked again once it returns, which clears by itself (the guide's
   * `LIGHTING: PENDING`, ruled 2026-11-13).
   */
  readonly pending: boolean;
}

/** A universe and a system, by which discs are kept. */
type DiscsKey = Pick<SystemHostDiscs, "universe" | "system">;

/** The busy answers a system's discs have had in a row: each asks again a little later. */
interface Busy extends DiscsKey {
  readonly answers: number;
}

/** Whether `discs` are of `ask`'s universe and system. */
function holdsFor(discs: DiscsKey | null, ask: DiscsKey): boolean {
  return discs !== null && discs.universe === ask.universe && discs.system === ask.system;
}

/**
 * Whether a failure is the link's or the client's own, after which the discs are asked again once
 * the link is up.
 */
function isTransient(failure: RequestFailure): boolean {
  return (
    failure.code === "link_lost" || failure.code === "aborted" || failure.code === "superseded"
  );
}

/**
 * Whether a failure is a busy server's, which refuses nothing: the discs are asked again by
 * themselves after {@link FIELD_RETRY_DELAY_MS}, doubling to {@link FIELD_RETRY_MAX_DELAY_MS}, as a
 * body's field and air are (`coarseFieldStore`'s `isBusy`).
 */
function isBusy(failure: RequestFailure): boolean {
  return (
    failure.code === "queue_full" ||
    failure.code === "too_many_requests" ||
    failure.code === "cancelled"
  );
}

/**
 * The view's system's discs on their own (`host_discs`; plans R06 and R07).
 *
 * @param ask - The universe and system whose discs the view lights its scene by, at the scene's
 *   time, or `null` where the view asks none (a kept scene of its own, no universe open).
 * @remarks
 * Kept by universe and system: a system is asked once, at the time it is first asked about, and
 * asked again for another system or universe, after a link lost before its answer, and after a busy
 * server's answer (`isBusy`). Any other refusal, the server's `unknown_system` or an older server's
 * `unsupported`, is held, and the view waits for its sky's `hosts` as before the kind. The request
 * of a system left is cancelled. Discs held are kept through a lost link, and asked nothing again:
 * the scene stays lit, its staleness the link's to say (`SCENE STALE: NO CARRIER`).
 */
export function useHostDiscs(ask: HostDiscsAsk | null): HostDiscsView {
  const { status, requests } = useServerLink();
  const [held, setHeld] = useState<SystemHostDiscs | null>(null);
  // The system whose discs the server refused, which is not asked again until another is.
  const [refused, setRefused] = useState<DiscsKey | null>(null);
  const [busy, setBusy] = useState<Busy | null>(null);
  // The request asked, kept by universe and system: a later time of the same system asks nothing.
  const [asked, setAsked] = useState<HostDiscsAsk | null>(null);
  if (ask === null ? asked !== null : !holdsFor(asked, ask)) {
    setAsked(ask);
  }
  const current = ask === null || !holdsFor(asked, ask) ? null : asked;
  const answered = current !== null && (holdsFor(held, current) || holdsFor(refused, current));
  const busyAnswers =
    current !== null && busy !== null && holdsFor(busy, current) ? busy.answers : 0;
  const connected = status === "connected";

  useEffect(() => {
    if (current === null || answered || !connected) {
      return undefined;
    }
    let live = true;
    let retry: ReturnType<typeof setTimeout> | null = null;
    const pending = requests.request({ kind: "host_discs", ...current });
    const settle = async (): Promise<void> => {
      const outcome = await pending.outcome;
      if (!live) {
        return;
      }
      const key = { universe: current.universe, system: current.system };
      if (outcome.ok) {
        setHeld({ ...key, discs: outcome.response.hosts });
      } else if (isBusy(outcome.error)) {
        const delayMs = Math.min(FIELD_RETRY_DELAY_MS * 2 ** busyAnswers, FIELD_RETRY_MAX_DELAY_MS);
        retry = setTimeout(() => {
          setBusy({ ...key, answers: busyAnswers + 1 });
        }, delayMs);
      } else if (!isTransient(outcome.error)) {
        setRefused(key);
      }
    };
    void settle().catch((error: unknown) => {
      console.error("the system's host discs could not be read:", error);
    });
    return () => {
      live = false;
      pending.cancel();
      if (retry !== null) {
        clearTimeout(retry);
      }
    };
  }, [current, answered, connected, requests, busyAnswers]);

  return {
    held: current !== null && holdsFor(held, current) ? held : null,
    pending: current !== null && !answered,
  };
}
