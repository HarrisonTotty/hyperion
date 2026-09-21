import {
  PROTOCOL_VERSION,
  type RequestBody,
  type RequestError,
  type RequestId,
  type RequestKind,
  type RequestOf,
  type ResponseBody,
  type ResponseFor,
  type ServerMessage,
} from "@hyperion/protocol";

/** A `request` message the client sent, with its ID and body. */
export interface SentRequest<K extends RequestKind> {
  readonly id: RequestId;
  readonly body: RequestOf<K>;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/**
 * Whether a sent message is a `request`.
 *
 * @remarks
 * Only the envelope is checked. The body is the client's own, typed by the generated bindings
 * when it was encoded, so a body with a `kind` is trusted to be a {@link RequestBody}.
 */
function isRequestMessage(
  message: unknown,
): message is { type: "request"; id: RequestId; body: RequestBody } {
  return (
    isRecord(message) &&
    message["type"] === "request" &&
    typeof message["id"] === "number" &&
    isRecord(message["body"]) &&
    typeof message["body"]["kind"] === "string"
  );
}

function isOfKind<K extends RequestKind>(body: RequestBody, kind: K): body is RequestOf<K> {
  return body.kind === kind;
}

/**
 * In-memory stand-in for the browser `WebSocket`, letting tests play the
 * server's side of the conversation.
 */
export class FakeWebSocket extends EventTarget {
  static instances: FakeWebSocket[] = [];

  readonly sent: unknown[] = [];

  /** IDs of the requests the server has ended, with a response or an error. */
  readonly #answered = new Set<RequestId>();

  constructor(readonly url: string) {
    super();
    FakeWebSocket.instances.push(this);
  }

  static latest(): FakeWebSocket {
    const socket = FakeWebSocket.instances.at(-1);
    if (socket === undefined) {
      throw new Error("no WebSocket has been opened");
    }
    return socket;
  }

  send(data: string): void {
    const message: unknown = JSON.parse(data);
    this.sent.push(message);
  }

  close(): void {
    this.dispatchEvent(new Event("close"));
  }

  serverOpens(): void {
    this.dispatchEvent(new Event("open"));
  }

  serverSends(message: ServerMessage): void {
    if (message.type === "response" || message.type === "request_error") {
      this.#answered.add(message.id);
    }
    this.dispatchEvent(new MessageEvent("message", { data: JSON.stringify(message) }));
  }

  /** Opens the socket and welcomes the client at this client's protocol version. */
  serverWelcomes(generatorVersion = 2): void {
    this.serverOpens();
    this.serverSends({
      type: "welcome",
      server_version: "9.9.9",
      protocol_version: PROTOCOL_VERSION,
      generator_version: generatorVersion,
    });
  }

  /** Ends the request with this ID with a response. */
  serverResponds(id: RequestId, body: ResponseBody): void {
    this.serverSends({ type: "response", id, body });
  }

  /** Ends the request with this ID with an error. */
  serverRejects(id: RequestId, error: RequestError): void {
    this.serverSends({ type: "request_error", id, error });
  }

  /** The `request` messages sent on this socket whose body has `kind`, oldest first. */
  requestsOfKind<K extends RequestKind>(kind: K): Array<SentRequest<K>> {
    const found: Array<SentRequest<K>> = [];
    for (const message of this.sent) {
      if (isRequestMessage(message) && isOfKind(message.body, kind)) {
        found.push({ id: message.id, body: message.body });
      }
    }
    return found;
  }

  /** The IDs of the `cancel` messages sent on this socket, oldest first. */
  cancelledIds(): RequestId[] {
    const ids: RequestId[] = [];
    for (const message of this.sent) {
      if (isRecord(message) && message["type"] === "cancel" && typeof message["id"] === "number") {
        ids.push(message["id"]);
      }
    }
    return ids;
  }

  /**
   * Answers the latest request of `kind` that the server has not yet ended, with the response
   * `build` makes from its body.
   *
   * @returns The ID of the request answered.
   * @throws Error naming `kind` when every request of that kind has been answered, or none was
   *   sent.
   */
  serverAnswers<K extends RequestKind>(
    kind: K,
    build: (body: RequestOf<K>) => ResponseFor<K>,
  ): RequestId {
    const request = this.requestsOfKind(kind)
      .toReversed()
      .find(({ id }) => !this.#answered.has(id));
    if (request === undefined) {
      throw new Error(`no unanswered ${kind} request has been sent`);
    }
    this.serverResponds(request.id, build(request.body));
    return request.id;
  }
}
