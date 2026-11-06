/**
 * A served scene's system's host discs on their own (plans R06 and R07): asked of the server's
 * `host_discs` once the system is known, so that the view lights its bodies before its sky's first
 * reply, which may take tens of seconds for a system newly arrived at.
 */

import type { RequestFailure, SystemIdHex, UniverseIdHex, UniverseTime } from "@hyperion/protocol";
import { useEffect, useState } from "react";

import { useServerLink } from "../../lib/serverLink";
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
  /**
   * The discs last answered, or `null` before any: of the system asked, or of one asked before it,
   * which the lighting's selector (`heldHostDiscs`) tells apart by its system.
   */
  readonly held: SystemHostDiscs | null;
  /** Whether the system asked has its discs asked and neither answered nor refused. */
  readonly pending: boolean;
}

/** Whether `discs` are the discs of `ask`'s system. */
function holdsFor(
  discs: Pick<SystemHostDiscs, "universe" | "system"> | null,
  ask: HostDiscsAsk,
): boolean {
  return discs !== null && discs.universe === ask.universe && discs.system === ask.system;
}

/**
 * Whether a failure is the link's or the client's own, after which the discs are asked again once
 * the link is up; any other, the server's refusal (`unknown_system`, or `unsupported` from a server
 * older than the kind), is held, and the view waits for its sky's `hosts` as before the kind.
 */
function isTransient(failure: RequestFailure): boolean {
  return (
    failure.code === "link_lost" || failure.code === "aborted" || failure.code === "superseded"
  );
}

/**
 * The view's system's discs on their own (`host_discs`; plans R06 and R07).
 *
 * @param ask - The universe and system whose discs the view lights its scene by, at the scene's
 *   time, or `null` where the view asks none (a kept scene of its own, no universe open).
 * @remarks
 * Kept by universe and system: a system is asked once, at the time it is first asked about, and
 * asked again only for another system or universe, or after a link lost before its answer. The
 * request of a system left is cancelled. The discs held stay until another system's arrive, so that
 * returning to a system just left asks nothing.
 */
export function useHostDiscs(ask: HostDiscsAsk | null): HostDiscsView {
  const { status, requests } = useServerLink();
  const [held, setHeld] = useState<SystemHostDiscs | null>(null);
  // The system whose discs the server refused, which is not asked again until another is.
  const [refused, setRefused] = useState<Pick<SystemHostDiscs, "universe" | "system"> | null>(null);
  // The request asked, kept by universe and system: a later time of the same system asks nothing.
  const [asked, setAsked] = useState<HostDiscsAsk | null>(null);
  if (ask === null ? asked !== null : !holdsFor(asked, ask)) {
    setAsked(ask);
  }
  const current = ask === null || !holdsFor(asked, ask) ? null : asked;
  const answered = current !== null && (holdsFor(held, current) || holdsFor(refused, current));
  const connected = status === "connected";

  useEffect(() => {
    if (current === null || answered || !connected) {
      return undefined;
    }
    let live = true;
    const pending = requests.request({ kind: "host_discs", ...current });
    const settle = async (): Promise<void> => {
      const outcome = await pending.outcome;
      if (!live) {
        return;
      }
      if (outcome.ok) {
        setHeld({
          universe: current.universe,
          system: current.system,
          discs: outcome.response.hosts,
        });
      } else if (!isTransient(outcome.error)) {
        setRefused({ universe: current.universe, system: current.system });
      }
    };
    void settle().catch((error: unknown) => {
      console.error("the system's host discs could not be read:", error);
    });
    return () => {
      live = false;
      pending.cancel();
    };
  }, [current, answered, connected, requests]);

  return { held, pending: current !== null && !answered };
}
