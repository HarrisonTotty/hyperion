/**
 * The sky a view draws: asked of the server as the request rule says, decoded off the render
 * thread, and held, marked stale while the link is down (plan R06, T12; Design note 13).
 */

import type { PartialBulkAnswer, RequestFailure, SkyRequest } from "@hyperion/protocol";
import { useEffect, useMemo, useState } from "react";

import { useServerLink } from "../../lib/serverLink";
import type { SkyDecodeReply, SkyDecodeRequest } from "./decodePayload";
import { type SkyCamera, type SkyModel, skyRequestReason } from "./model";

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
   * Whether a request is in flight and none of its replies is held yet: once its first reply is,
   * the sky arriving nearest first is held, and its later replies replace it (R06.T11.d).
   */
  readonly pending: boolean;
}

/** What `useSky` takes besides its request. */
export interface UseSkyOptions {
  /** Makes the decoder: the worker's by default; a test's stand-in, stable in identity, otherwise. */
  readonly createDecoder?: () => SkyDecoder;
}

/** A decoded sky, before the link's state marks it stale. */
type Held = Omit<SkyModel, "stale">;

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

/** What a settled request leaves. */
type Settled =
  | { readonly kind: "held" }
  | { readonly kind: "failed"; readonly failure: Failure }
  | { readonly kind: "transient" };

/** One reply of a sky request, as it arrived: its response and its payload's chunks. */
type Reply = PartialBulkAnswer<"sky">;

/**
 * The view's sky (plan R06, T12; R06.T11.d).
 *
 * @param request - The request as it would be sent now (observer, time, limits, N_max), or `null`
 *   where none can be asked (no universe, or the system's position unknown): nothing is asked.
 * @param cameras - Every open view's camera, for the parallax rule.
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
 */
export function useSky(
  request: SkyRequest | null,
  cameras: ReadonlyArray<SkyCamera>,
  options: UseSkyOptions = {},
): SkyView {
  const { status, requests } = useServerLink();
  const [held, setHeld] = useState<Held | null>(null);
  const [asked, setAsked] = useState<SkyRequest | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const stale = status !== "connected";
  const heldModel = useMemo<SkyModel | null>(
    () => (held === null ? null : { ...held, stale }),
    [held, stale],
  );

  // Adjusted during render, as the request rule answers for what is held now.
  if (request !== null && !stale) {
    if (asked !== null && !sameArrival(asked, request)) {
      setAsked(request);
    } else if (
      asked === null &&
      !(failure !== null && sameArrival(failure.request, request)) &&
      skyRequestReason(heldModel, { request, cameras }) !== null
    ) {
      setAsked(request);
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
      settle({ kind: "failed", failure: { request: asked, message } });
    };
    // Decodes the reply waiting and holds it whole in place of the one before, then the next, until
    // none waits, the final is held, or the request has ended.
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
      setHeld({
        request: asked,
        response: reply.response,
        stars: decoded.stars,
        band: decoded.band,
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
    const pending = requests.requestBulk<"sky">({ kind: "sky", ...asked }, (r) => r.bulk, {
      onPartial: offer,
    });
    const finish = async (): Promise<void> => {
      const outcome = await pending.outcome;
      if (outcome.ok) {
        offer({ response: outcome.response, chunks: outcome.chunks });
      } else if (isTransient(outcome.error)) {
        settle({ kind: "transient" });
      } else {
        settle({ kind: "failed", failure: { request: asked, message: outcome.error.message } });
      }
    };
    void finish().catch(failed);
    return () => {
      live = false;
      pending.cancel();
      decoder?.dispose();
    };
  }, [asked, requests, createDecoder]);

  const current = request ?? asked;
  const model =
    heldModel !== null && current !== null && !sameArrival(heldModel.request, current)
      ? null
      : heldModel;
  const failed =
    failure !== null && (current === null || sameArrival(failure.request, current))
      ? failure.message
      : null;
  // Pending until a reply of the request in flight is held: a sky arriving nearest first is held
  // from its first.
  return { model, failure: failed, pending: asked !== null && held?.request !== asked };
}
