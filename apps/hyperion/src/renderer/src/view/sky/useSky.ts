/**
 * The sky a view draws: asked of the server as the request rule says, decoded off the render
 * thread, and held, marked stale while the link is down (plan R06, T12; Design note 13).
 */

import type { ResponseFor, SkyRequest } from "@hyperion/protocol";
import { useEffect, useState } from "react";

import { useServerLink } from "../../lib/serverLink";
import type { SkyDecodeReply, SkyDecodeRequest } from "./decodePayload";
import { type SkyCamera, type SkyModel, skyRequestReason } from "./model";

/** Decodes a payload, off the render thread in the app. */
export interface SkyDecoder {
  /** Settles with the decoded sky or why it was refused; never rejects. */
  decode(request: SkyDecodeRequest): Promise<SkyDecodeReply>;
  /** Stops the decoder; a decode in flight then never settles. */
  dispose(): void;
}

/**
 * A decoder over the sky's module worker (`decode.worker.ts`), started as a same-origin module
 * file, as the surface worker is.
 */
export function createWorkerSkyDecoder(): SkyDecoder {
  const worker = new Worker(new URL("./decode.worker.ts", import.meta.url), {
    type: "module",
    name: "sky decode",
  });
  const waiting = new Map<number, (reply: SkyDecodeReply) => void>();
  worker.addEventListener("message", (event: MessageEvent<SkyDecodeReply>) => {
    const settle = waiting.get(event.data.id);
    waiting.delete(event.data.id);
    settle?.(event.data);
  });
  worker.addEventListener("error", (event: Event) => {
    const message =
      event instanceof ErrorEvent ? event.message : "the decode worker failed to load";
    for (const [id, settle] of waiting) {
      settle({ id, ok: false, message });
    }
    waiting.clear();
  });
  let next = 0;
  return {
    decode: (request) =>
      new Promise((resolve) => {
        next += 1;
        const id = next;
        waiting.set(id, resolve);
        worker.postMessage(
          { ...request, id },
          request.chunks.flatMap((chunk) =>
            chunk.buffer instanceof ArrayBuffer ? [chunk.buffer] : [],
          ),
        );
      }),
    dispose: () => {
      waiting.clear();
      worker.terminate();
    },
  };
}

/** What a view has of the sky. */
export interface SkyView {
  /** The sky held, or `null` before the first arrives. */
  readonly model: SkyModel | null;
  /** Why the last request of this arrival failed, or `null`. */
  readonly failure: string | null;
  /** Whether a request is in flight. */
  readonly pending: boolean;
}

/** What `useSky` takes besides its request. */
export interface UseSkyOptions {
  /** Makes the decoder: the worker's by default, a test's stand-in otherwise. */
  readonly createDecoder?: () => SkyDecoder;
}

/** A decoded sky, before the link's state marks it stale. */
type Held = Omit<SkyModel, "stale">;

/** A failed request and its reason. */
interface Failure {
  readonly request: SkyRequest;
  readonly message: string;
}

/** Whether two requests are for one arrival: the same universe and the same own system. */
function sameArrival(a: SkyRequest, b: SkyRequest): boolean {
  return a.universe === b.universe && a.exclude_system === b.exclude_system;
}

/**
 * The view's sky (plan R06, T12).
 *
 * @param request - The request as it would be sent now (observer, time, limits, N_max), or `null`
 *   where none can be asked (no universe, or the system's position unknown): nothing is asked.
 * @param cameras - Every open view's camera, for the parallax rule.
 * @remarks
 * A request is sent when {@link skyRequestReason} names a reason and none is in flight; a request
 * for another arrival supersedes one in flight, which is cancelled. A failure is held and not
 * retried until the next arrival. The sky held is kept through a lost link, marked stale, and
 * nothing is asked until the link returns.
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
  const model: SkyModel | null = held === null ? null : { ...held, stale };

  // Adjusted during render, as the request rule answers for what is held now.
  if (request !== null && !stale) {
    if (asked !== null && !sameArrival(asked, request)) {
      setAsked(request);
    } else if (
      asked === null &&
      !(failure !== null && sameArrival(failure.request, request)) &&
      skyRequestReason(model, { request, cameras }) !== null
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
    const decoder = createDecoder();
    const pending = requests.requestBulk<"sky">(
      { kind: "sky", ...asked },
      (response) => response.bulk,
    );
    const settle = (result: { readonly held: Held } | { readonly failure: Failure }): void => {
      if (!live) {
        return;
      }
      if ("held" in result) {
        setHeld(result.held);
        setFailure(null);
      } else {
        setFailure(result.failure);
      }
      setAsked(null);
    };
    pending.outcome
      .then(async (outcome) => {
        if (!outcome.ok) {
          settle({ failure: { request: asked, message: outcome.error.message } });
          return undefined;
        }
        const response: ResponseFor<"sky"> = outcome.response;
        const reply = await decoder.decode({
          id: 0,
          chunks: outcome.chunks,
          starsBytes: response.stars_bytes,
          bandBytes: response.band_bytes,
        });
        settle(
          reply.ok
            ? { held: { request: asked, response, stars: reply.stars, band: reply.band } }
            : { failure: { request: asked, message: reply.message } },
        );
        return undefined;
      })
      .catch((error: unknown) => {
        console.error("the sky's request failed to settle:", error);
      });
    return () => {
      live = false;
      pending.cancel();
      decoder.dispose();
    };
  }, [asked, requests, createDecoder]);

  return { model, failure: failure?.message ?? null, pending: asked !== null };
}
