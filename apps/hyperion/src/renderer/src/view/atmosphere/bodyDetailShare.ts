/**
 * One `body_detail` fetch for the atmosphere cache and the terrain session (plan R08, R08.T10.b;
 * `decision-r08-t10a-leans.md` item 4): R10.T16.a's terrain session asks a body's record for its
 * detail seed and its seasons, and R08.T10.a's cache for its atmosphere, of the same request
 * client. The share sits between them and the client, so that one request a body serves both.
 *
 * @remarks
 * - **The cache's asks** ({@link BodyDetailShare.atmosphere}) are its triggers: a body's first want,
 *   a new arrival, a new grant, an event on it. Each is sent, or joins a request for the same body
 *   still in flight, which no answer has yet taken: the terrain's, as a rule, asked at the body's
 *   want, or the cache's own that the terrain joined and a trigger then cancelled. Its reply is
 *   kept for the terrain.
 * - **The terrain's asks** ({@link BodyDetailShare.terrain}) join a request in flight for the body,
 *   or take the latest reply kept for it, at once, or else are sent: the session asks once a want,
 *   and the cache has as a rule asked first, the body being lit in a view before its field is
 *   wanted. Every other request, `surface_field` among them, passes through.
 * - **A joined request** settles with the one it joined. Cancelling it leaves the request to its
 *   other askers; the last to cancel cancels it at the client, which then settles every asker's.
 * - **The replies kept** are the latest {@link BODY_DETAIL_SHARE_REPLIES} bodies' answered `ok`, the
 *   least recently answered let go first; a body's reply is let go when the cache asks it afresh,
 *   so that the terrain never takes a reply a trigger has made stale.
 * - A request's time is its asker's own, the scene's: a join answers at its first asker's, a few
 *   frames of the scene's time apart, which neither the seed, the seasons nor the medium reads.
 */

import type {
  BodyIdHex,
  BulkManifestDto,
  BulkRequestOptions,
  PendingBulkRequest,
  PendingRequest,
  RequestClient,
  RequestKind,
  RequestOf,
  RequestOutcome,
  ResponseFor,
  UniverseIdHex,
} from "@hyperion/protocol";

/** The share's client: the link's request client, or a kept scene's terrain fixture's. */
export type BodyDetailClient = Pick<RequestClient, "request" | "requestBulk">;

/** The most bodies whose latest reply the share keeps for the terrain's asks. */
export const BODY_DETAIL_SHARE_REPLIES = 64;

/** A `body_detail` in flight, with every asker that has not cancelled it. */
interface InFlight {
  readonly pending: PendingRequest<"body_detail">;
  askers: number;
}

/** A request's key: its universe and body. */
function keyOf(universe: UniverseIdHex, body: BodyIdHex): string {
  return `${universe}/${body}`;
}

/** Whether a request is a `body_detail`. */
function isBodyDetail(body: RequestOf<RequestKind>): body is RequestOf<"body_detail"> {
  return body.kind === "body_detail";
}

/**
 * A `body_detail`'s pending request as the generic `request` returns it: `K` is `"body_detail"`
 * wherever it is called, {@link isBodyDetail} having narrowed the request, which TypeScript does not
 * carry to `K`; the one cast of the module.
 */
function asRequested<K extends RequestKind>(
  pending: PendingRequest<"body_detail">,
): PendingRequest<K> {
  // The protocol's generic `request` cannot see its kind's narrowing through `K`; the one place a
  // body_detail's request is handed back under the caller's own `K`.
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return pending as unknown as PendingRequest<K>;
}

/** An outcome already settled, as a pending request that cancels nothing. */
function settled(outcome: RequestOutcome<"body_detail">): PendingRequest<"body_detail"> {
  return { outcome: Promise.resolve(outcome), cancel: () => undefined };
}

/** One request client's `body_detail` shared by the atmosphere cache and the terrain session. */
export class BodyDetailShare {
  readonly #client: BodyDetailClient;
  readonly #inFlight = new Map<string, InFlight>();
  /** The latest reply answered `ok` by body, the least recently answered first. */
  readonly #replies = new Map<string, RequestOutcome<"body_detail">>();

  constructor(client: BodyDetailClient) {
    this.#client = client;
  }

  /** The atmosphere cache's requests: each ask a trigger's (the module's remarks). */
  readonly atmosphere: Pick<RequestClient, "request"> = {
    request: <K extends RequestKind>(body: RequestOf<K>): PendingRequest<K> => {
      if (!isBodyDetail(body)) {
        return this.#client.request(body);
      }
      const key = keyOf(body.universe, body.body);
      this.#replies.delete(key);
      return asRequested(this.#ask(key, body));
    },
  };

  /** The terrain session's requests: a body's record taken from the cache's where it can be. */
  readonly terrain: BodyDetailClient = {
    request: <K extends RequestKind>(body: RequestOf<K>): PendingRequest<K> => {
      if (!isBodyDetail(body)) {
        return this.#client.request(body);
      }
      const key = keyOf(body.universe, body.body);
      const kept = this.#replies.get(key);
      if (kept !== undefined && !this.#inFlight.has(key)) {
        return asRequested(settled(kept));
      }
      return asRequested(this.#ask(key, body));
    },
    requestBulk: <K extends RequestKind>(
      body: RequestOf<K>,
      manifestOf: (response: ResponseFor<K>) => BulkManifestDto | null,
      options?: BulkRequestOptions<K>,
    ): PendingBulkRequest<K> => this.#client.requestBulk(body, manifestOf, options),
  };

  /** How many bodies' replies are kept, for the tests. */
  get repliesKept(): number {
    return this.#replies.size;
  }

  /** Joins the request in flight for `key`, or sends one, keeping its reply where it is `ok`. */
  #ask(key: string, body: RequestOf<"body_detail">): PendingRequest<"body_detail"> {
    let flight = this.#inFlight.get(key);
    if (flight === undefined) {
      const pending = this.#client.request(body);
      const made: InFlight = { pending, askers: 0 };
      flight = made;
      this.#inFlight.set(key, made);
      const settle = async (): Promise<void> => {
        const outcome = await pending.outcome;
        if (this.#inFlight.get(key) === made) {
          this.#inFlight.delete(key);
        }
        if (outcome.ok) {
          this.#keep(key, outcome);
        }
      };
      void settle().catch((error: unknown) => {
        console.error("a shared body_detail's answer could not be kept:", error);
      });
    }
    const joined = flight;
    joined.askers += 1;
    let cancelled = false;
    return {
      outcome: joined.pending.outcome,
      cancel: () => {
        if (cancelled) {
          return;
        }
        cancelled = true;
        joined.askers -= 1;
        if (joined.askers === 0) {
          if (this.#inFlight.get(key) === joined) {
            this.#inFlight.delete(key);
          }
          joined.pending.cancel();
        }
      },
    };
  }

  /** Keeps a body's latest reply, letting the least recently answered go past the cap. */
  #keep(key: string, outcome: RequestOutcome<"body_detail">): void {
    this.#replies.delete(key);
    this.#replies.set(key, outcome);
    while (this.#replies.size > BODY_DETAIL_SHARE_REPLIES) {
      const oldest = this.#replies.keys().next();
      if (oldest.done === true) {
        return;
      }
      this.#replies.delete(oldest.value);
    }
  }
}
