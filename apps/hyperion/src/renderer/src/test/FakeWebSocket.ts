import type { RequestError, RequestId, ResponseBody, ServerMessage } from "@hyperion/protocol";

/**
 * In-memory stand-in for the browser `WebSocket`, letting tests play the
 * server's side of the conversation.
 */
export class FakeWebSocket extends EventTarget {
  static instances: FakeWebSocket[] = [];

  readonly sent: unknown[] = [];

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
    this.dispatchEvent(new MessageEvent("message", { data: JSON.stringify(message) }));
  }

  /** Ends the request with this ID with a response. */
  serverResponds(id: RequestId, body: ResponseBody): void {
    this.serverSends({ type: "response", id, body });
  }

  /** Ends the request with this ID with an error. */
  serverRejects(id: RequestId, error: RequestError): void {
    this.serverSends({ type: "request_error", id, error });
  }
}
