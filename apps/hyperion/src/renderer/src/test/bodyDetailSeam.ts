import {
  type BodyIdHex,
  type ClientMessage,
  type ErrorCode,
  RequestClient,
  type ResponseBody,
  type ResponseFor,
  type UniverseTime,
} from "@hyperion/protocol";

/** One `body_detail` request as the seam saw it sent. */
export interface AskedBodyDetail {
  /** The request's ID, which answers it. */
  readonly id: number;
  readonly body: BodyIdHex;
  readonly time: UniverseTime;
}

/**
 * A request seam for the atmosphere cache's tests (plan R08, R08.T10.a;
 * `decision-r08-t10a-leans.md` item 4): a real `RequestClient` over a recording `send`, so that
 * each `body_detail` the cache sends is counted where it leaves, and each answer, refusal, lost
 * link or cancel takes the client's own path, as the server's link would.
 */
export class BodyDetailSeam {
  /** The client the cache asks through, as `Pick<RequestClient, "request">`. */
  readonly client: RequestClient;
  /** Every `body_detail` request sent, in order. */
  readonly asked: AskedBodyDetail[] = [];
  /** The IDs of the requests cancelled, in order. */
  readonly cancelled: number[] = [];

  constructor() {
    this.client = new RequestClient((message) => {
      this.#receive(message);
      return true;
    });
  }

  /** How many `body_detail` requests have been sent for `body`. */
  askedFor(body: BodyIdHex): number {
    return this.asked.filter((each) => each.body === body).length;
  }

  /**
   * The latest request sent for `body`.
   *
   * @throws Error where none was: a test's bug.
   */
  latestFor(body: BodyIdHex): AskedBodyDetail {
    const found = this.asked.findLast((each) => each.body === body);
    if (found === undefined) {
      throw new Error(`no body_detail was sent for ${body}`);
    }
    return found;
  }

  /** Answers request `id` with `response`. */
  answer(id: number, response: ResponseFor<"body_detail">): void {
    this.client.handleServerMessage({ type: "response", id, body: response });
  }

  /** Answers request `id` with a response of another kind, which the client takes as a violation. */
  answerWith(id: number, response: ResponseBody): void {
    this.client.handleServerMessage({ type: "response", id, body: response });
  }

  /** Refuses request `id` as the server words a refusal. */
  refuse(id: number, code: ErrorCode, message: string): void {
    this.client.handleServerMessage({
      type: "request_error",
      id,
      error: { code, message, field: null },
    });
  }

  /** Drops the link: every request in flight ends `link_lost`. */
  loseLink(): void {
    this.client.linkLost();
  }

  #receive(message: ClientMessage): void {
    switch (message.type) {
      case "request":
        if (message.body.kind === "body_detail") {
          const { body, time } = message.body;
          this.asked.push({ id: message.id, body, time });
        }
        break;
      case "cancel":
        this.cancelled.push(message.id);
        break;
      case "hello":
      case "ping":
        break;
    }
  }
}
