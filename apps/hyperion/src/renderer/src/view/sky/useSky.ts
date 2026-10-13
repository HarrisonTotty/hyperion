/**
 * The sky a view draws: asked of the server as the request rule says, decoded off the render
 * thread, and held, marked stale while the link is down, and kept through a request asked again
 * until that request's replies hold as much (plan R06, T12 and T12.b; Design note 13).
 */

import type { PartialBulkAnswer, RequestFailure, SkyRequest } from "@hyperion/protocol";
import { useEffect, useMemo, useState } from "react";

import { useServerLink } from "../../lib/serverLink";
import type { SkyDecodeReply, SkyDecodeRequest } from "./decodePayload";
import {
  keepsHeldSky,
  type SkyCamera,
  type SkyModel,
  type SkyRequestReason,
  skyRequestReason,
} from "./model";

/** A payload to decode: the worker's request without its number, which the decoder assigns. */
export type SkyDecodeJob = Omit<SkyDecodeRequest, "id">;

/** Decodes a payload, off the render thread in the app. */
export interface SkyDecoder {
  /** Settles with the decoded sky or why it was refused; never rejects. */
  decode(job: SkyDecodeJob): Promise<SkyDecodeReply>;
  /** Stops the decoder; a decode in flight then never settles. */
  dispose(): void;
}

/** The part of a `Worker` the decoder uses, so that a test can stand in for one. */
export interface SkyDecodeWorker {
  addEventListener(type: "message", listener: (event: MessageEvent<SkyDecodeReply>) => void): void;
  addEventListener(type: "error" | "messageerror", listener: (event: Event) => void): void;
  postMessage(message: SkyDecodeRequest, transfer: Transferable[]): void;
  terminate(): void;
}

/** Starts the sky's module worker, a same-origin module file as the surface worker is. */
function startDecodeWorker(): SkyDecodeWorker {
  return new Worker(new URL("./decode.worker.ts", import.meta.url), {
    type: "module",
    name: "sky decode",
  });
}

/**
 * A decoder over the sky's decode worker (`decode.worker.ts`).
 *
 * @param start - Starts the worker; the module worker's by default.
 * @remarks
 * Each decode is numbered and its reply matched by number. A worker that fails to load, or a
 * reply that cannot be read, settles every decode waiting with the reason, as does a message the
 * worker cannot take.
 */
export function createWorkerSkyDecoder(
  start: () => SkyDecodeWorker = startDecodeWorker,
): SkyDecoder {
  const worker = start();
  const waiting = new Map<number, (reply: SkyDecodeReply) => void>();
  const failAll = (message: string): void => {
    for (const [id, settle] of waiting) {
      settle({ id, ok: false, message });
    }
    waiting.clear();
  };
  worker.addEventListener("message", (event: MessageEvent<SkyDecodeReply>) => {
    const settle = waiting.get(event.data.id);
    waiting.delete(event.data.id);
    settle?.(event.data);
  });
  worker.addEventListener("error", (event: Event) => {
    failAll(event instanceof ErrorEvent ? event.message : "the decode worker failed to load");
  });
  worker.addEventListener("messageerror", () => {
    failAll("the decode worker's reply could not be read");
  });
  let next = 0;
  return {
    decode: (job) =>
      new Promise((resolve) => {
        next += 1;
        const id = next;
        waiting.set(id, resolve);
        try {
          worker.postMessage(
            { ...job, id },
            job.chunks.flatMap((chunk) =>
              chunk.buffer instanceof ArrayBuffer ? [chunk.buffer] : [],
            ),
          );
        } catch (error: unknown) {
          waiting.delete(id);
          resolve({
            id,
            ok: false,
            message: error instanceof Error ? error.message : "the payload could not be posted",
          });
        }
      }),
    dispose: () => {
      waiting.clear();
      worker.terminate();
    },
  };
}

/** What a view has of the sky. */
export interface SkyView {
  /**
   * The sky held for the current arrival, or `null` before it arrives: a sky of another system is
   * not this one's, so a view draws its stand-in meanwhile.
   */
  readonly model: SkyModel | null;
  /** Why the last request of this arrival failed, or `null`. */
  readonly failure: string | null;
  /**
   * Whether a request is in flight and no sky of its arrival is held: once the request's first
   * reply is, the sky arriving nearest first is held, and its later replies replace it
   * (R06.T11.d). A sky asked again within its arrival is never pending, since the sky before it
   * stays held meanwhile (R06.T12.b).
   */
  readonly pending: boolean;
}

/** What `useSky` takes besides its request. */
export interface UseSkyOptions {
  /** Makes the decoder: the worker's by default; a test's stand-in, stable in identity, otherwise. */
  readonly createDecoder?: () => SkyDecoder;
}

/** A decoded sky, before the link's state marks it stale. */
type Decoded = Omit<SkyModel, "stale">;

/** A request asked, and why: the hold reads the reason (R06.T12.b). One per request sent. */
interface Asked {
  readonly request: SkyRequest;
  readonly reason: SkyRequestReason;
}

/** A decoded reply and the request it answers, by which one request's replies are told apart. */
interface Answer {
  readonly sky: Decoded;
  readonly asked: Asked;
}

/** A failed request and its reason. */
interface Failure {
  readonly request: SkyRequest;
  readonly message: string;
}

/**
 * Whether a failure is the link's or the client's own, for which a request is not held back: it
 * is asked again once the link is up.
 */
function isTransient(failure: RequestFailure): boolean {
  return (
    failure.code === "link_lost" || failure.code === "aborted" || failure.code === "superseded"
  );
}

/** Whether two requests are for one arrival: the same universe and the same own system. */
function sameArrival(a: SkyRequest, b: SkyRequest): boolean {
  return a.universe === b.universe && a.exclude_system === b.exclude_system;
}

/**
 * Whether a request's newest reply takes the held sky's place now: with none held, for another
 * arrival, once a reply of its request is held, or where the hold gives way ({@link keepsHeldSky},
 * R06.T12.b).
 */
function takesHeldPlace(
  held: Answer | null,
  newest: Answer,
  cameras: ReadonlyArray<SkyCamera>,
): boolean {
  return (
    held === null ||
    held.asked === newest.asked ||
    !sameArrival(held.asked.request, newest.asked.request) ||
    !keepsHeldSky(newest.asked.reason, held.sky, newest.sky.response, cameras)
  );
}

/** What a settled request leaves. */
type Settled =
  | { readonly kind: "held" }
  | { readonly kind: "failed"; readonly failure: Failure }
  | { readonly kind: "transient" };

/** One reply of a sky request, as it arrived: its response and its payload's chunks. */
type Reply = PartialBulkAnswer<"sky">;

/**
 * The view's sky (plan R06, T12; R06.T11.d; R06.T12.b).
 *
 * @param request - The request as it would be sent now (observer, time, limits, N_max), or `null`
 *   where none can be asked (no universe, or the system's position unknown): nothing is asked.
 * @param cameras - Every open view's camera, for the parallax rule and the hold's pixel.
 * @remarks
 * A request is sent when {@link skyRequestReason} names a reason and none is in flight; a request
 * for another arrival supersedes one in flight, which is cancelled. A refusal is held and not
 * retried until the next arrival; a request the link cut off is asked again when it returns. The
 * sky held is kept through a lost link and marked stale.
 *
 * A sky arrives nearest first, as several replies, the last final (R06.T11.d). Each reply is
 * decoded off the render thread and then held in place of the one before, whole: the model
 * changes from one reply's to the next's in one render, never through an empty or half-decoded
 * sky, so the view bakes the new cube before it draws it. Replies are decoded one at a time; a
 * reply that is not final, still waiting when a later one arrives, is passed over for it, and the
 * final reply is always held.
 *
 * A sky asked again within its arrival for the held sky's own time (`partial`, `limits` or
 * `parallax`) keeps the sky held: the request's newest decoded reply waits behind it until that
 * reply is final or as complete in every layer, or until the held sky's parallax for an open
 * camera reaches a pixel ({@link keepsHeldSky}), and then it and each later reply is held
 * (R06.T12.b). The hold is judged at each render, for the cameras as they are then.
 */
export function useSky(
  request: SkyRequest | null,
  cameras: ReadonlyArray<SkyCamera>,
  options: UseSkyOptions = {},
): SkyView {
  const { status, requests } = useServerLink();
  const [held, setHeld] = useState<Answer | null>(null);
  // The newest decoded reply of the request last asked: held at once, or once the hold allows.
  const [newest, setNewest] = useState<Answer | null>(null);
  const [asked, setAsked] = useState<Asked | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const stale = status !== "connected";
  const heldModel = useMemo<SkyModel | null>(
    () => (held === null ? null : { ...held.sky, stale }),
    [held, stale],
  );

  // Adjusted during render: the newest reply takes the held sky's place where the hold allows, for
  // the cameras now; else the request rule answers for what is held. A request asked drops any
  // reply still waiting behind the hold, which it supersedes.
  if (newest !== null && newest !== held && takesHeldPlace(held, newest, cameras)) {
    setHeld(newest);
  } else if (request !== null && !stale) {
    if (asked !== null && !sameArrival(asked.request, request)) {
      // Back in the held sky's arrival before another's first reply, the request rule answers for
      // the sky held, as it does for any re-ask: often nothing, which ends the request in flight.
      const reason = skyRequestReason(heldModel, { request, cameras });
      setAsked(reason === null ? null : { request, reason });
      setNewest(null);
    } else if (asked === null && !(failure !== null && sameArrival(failure.request, request))) {
      const reason = skyRequestReason(heldModel, { request, cameras });
      if (reason !== null) {
        setAsked({ request, reason });
        setNewest(null);
      }
    }
  }

  const createDecoder = options.createDecoder ?? createWorkerSkyDecoder;
  useEffect(() => {
    if (asked === null) {
      return undefined;
    }
    let live = true;
    // Made once a payload is in hand, so that a request never answered starts no worker.
    let decoder: SkyDecoder | null = null;
    let draining = false;
    // The latest reply not yet decoded: a partial one waiting is passed over for a later one.
    let waiting: Reply | null = null;
    const settle = (settled: Settled): void => {
      if (!live) {
        return;
      }
      switch (settled.kind) {
        case "held":
          setFailure(null);
          break;
        case "failed":
          setFailure(settled.failure);
          break;
        case "transient":
          break;
      }
      setAsked(null);
    };
    const failed = (error: unknown): void => {
      const message = error instanceof Error ? error.message : "the sky could not be read";
      settle({ kind: "failed", failure: { request: asked.request, message } });
    };
    // Decodes the reply waiting and offers it whole to take the place of the one held, then the
    // next, until none waits, the final is decoded, or the request has ended.
    const drain = async (): Promise<void> => {
      const reply = waiting;
      if (reply === null) {
        draining = false;
        return;
      }
      waiting = null;
      if (decoder === null) {
        decoder = createDecoder();
      }
      const decoded = await decoder.decode({
        chunks: reply.chunks,
        starsBytes: reply.response.stars_bytes,
        bandBytes: reply.response.band_bytes,
      });
      if (!live) {
        return;
      }
      if (!decoded.ok) {
        draining = false;
        failed(new Error(decoded.message));
        return;
      }
      setNewest({
        sky: {
          request: asked.request,
          response: reply.response,
          stars: decoded.stars,
          band: decoded.band,
        },
        asked,
      });
      if (reply.response.final) {
        draining = false;
        settle({ kind: "held" });
        return;
      }
      await drain();
    };
    const offer = (reply: Reply): void => {
      // Cleaned up while the payload arrived: start no worker that nothing would stop.
      if (!live) {
        return;
      }
      waiting = reply;
      if (!draining) {
        draining = true;
        void drain().catch(failed);
      }
    };
    const pending = requests.requestBulk<"sky">({ kind: "sky", ...asked.request }, (r) => r.bulk, {
      onPartial: offer,
    });
    const finish = async (): Promise<void> => {
      const outcome = await pending.outcome;
      if (outcome.ok) {
        offer({ response: outcome.response, chunks: outcome.chunks });
      } else if (isTransient(outcome.error)) {
        settle({ kind: "transient" });
      } else {
        settle({
          kind: "failed",
          failure: { request: asked.request, message: outcome.error.message },
        });
      }
    };
    void finish().catch(failed);
    return () => {
      live = false;
      pending.cancel();
      decoder?.dispose();
    };
  }, [asked, requests, createDecoder]);

  const current = request ?? asked?.request ?? null;
  const model =
    heldModel !== null && current !== null && !sameArrival(heldModel.request, current)
      ? null
      : heldModel;
  const failed =
    failure !== null && (current === null || sameArrival(failure.request, current))
      ? failure.message
      : null;
  // Pending until a sky of the arrival in flight is held: a sky arriving nearest first is held
  // from its first reply, and one asked again keeps the sky before it meanwhile (R06.T12.b).
  const pending =
    asked !== null && (held === null || !sameArrival(held.asked.request, asked.request));
  return { model, failure: failed, pending };
}
