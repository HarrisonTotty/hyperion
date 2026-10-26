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
 *   wanted. Its fields' bulk requests (`surface_field`) pass through whole.
 * - Both seams take `body_detail` alone, the one kind either asks of its own (`AtmosphereRequests`,
 *   `TerrainRequests`), so that the share hands back each request under its own kind.
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
  RequestKind,
  RequestOf,
  RequestOutcome,
  ResponseFor,
  UniverseIdHex,
} from "@hyperion/protocol";

import type { TerrainRequests } from "../terrain/session";
import type { AtmosphereRequests } from "./AtmosphereCache";

/**
 * The share's client: the link's request client, or a fixture's, as the terrain session takes one
 * (`TerrainRequests`: `body_detail` and the fields' bulk requests).
 */
export type BodyDetailClient = TerrainRequests;

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

  /** The atmosphere cache's requests (`AtmosphereRequests`): each ask a trigger's. */
  readonly atmosphere: AtmosphereRequests = {
    request: (body: RequestOf<"body_detail">): PendingRequest<"body_detail"> => {
      const key = keyOf(body.universe, body.body);
      this.#replies.delete(key);
      return this.#ask(key, body);
    },
  };

  /**
   * The terrain session's requests (`TerrainRequests`): a body's record taken from the cache's
   * where it can be, and its fields' bulk requests passed to the client whole.
   */
  readonly terrain: TerrainRequests = {
    request: (body: RequestOf<"body_detail">): PendingRequest<"body_detail"> => {
      const key = keyOf(body.universe, body.body);
      const kept = this.#replies.get(key);
      if (kept !== undefined && !this.#inFlight.has(key)) {
        return settled(kept);
      }
      return this.#ask(key, body);
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
