import { EventEmitter } from "node:events";

import { describe, expect, it } from "vitest";

import { ClientBytes, frameAt, relayConnection } from "./holdingRelay";

/** A WebSocket frame, masked as a client's are unless `masked` is false (RFC 6455 §5.2). */
function frame(opcode: number, payload: Buffer, masked = true): Buffer {
  const length = payload.length;
  const head =
    length < 126
      ? Buffer.from([0x80 | opcode, (masked ? 0x80 : 0) | length])
      : length < 0x1_0000
        ? Buffer.concat([
            Buffer.from([0x80 | opcode, (masked ? 0x80 : 0) | 126]),
            Buffer.from([length >> 8, length & 0xff]),
          ])
        : Buffer.concat([
            Buffer.from([0x80 | opcode, (masked ? 0x80 : 0) | 127]),
            ((): Buffer => {
              const bytes = Buffer.alloc(8);
              bytes.writeBigUInt64BE(BigInt(length));
              return bytes;
            })(),
          ]);
  const key = Buffer.from([1, 2, 3, 4]);
  const body = masked ? payload.map((byte, i) => byte ^ (key[i % 4] ?? 0)) : payload;
  return Buffer.concat([head, masked ? key : Buffer.alloc(0), body]);
}

const UPGRADE = Buffer.from(
  "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
  "latin1",
);
const TEXT = frame(0x1, Buffer.from('{"kind":"sky"}'));
const CLOSE = frame(0x8, Buffer.from([0x03, 0xe9]));

/** A socket standing in for one of the relay's two, recording what is written and destroyed. */
class FakeSocket extends EventEmitter {
  readonly written: Buffer[] = [];
  destroyed = false;

  write(chunk: Buffer): boolean {
    this.written.push(chunk);
    return true;
  }

  destroy(): void {
    this.destroyed = true;
    this.emit("close");
  }
}

describe("a WebSocket frame's bounds", () => {
  it("reads a short masked frame's length from its header, mask and payload", () => {
    expect(frameAt(TEXT)).toEqual({ opcode: 0x1, length: 2 + 4 + 14 });
  });

  it("reads the 16- and 64-bit extended lengths", () => {
    expect(frameAt(frame(0x2, Buffer.alloc(300)))).toEqual({ opcode: 0x2, length: 4 + 4 + 300 });
    expect(frameAt(frame(0x2, Buffer.alloc(70_000), false))).toEqual({
      opcode: 0x2,
      length: 10 + 70_000,
    });
  });

  it("waits for a header split across chunks", () => {
    expect(frameAt(TEXT.subarray(0, 1))).toBeNull();
    expect(frameAt(frame(0x2, Buffer.alloc(300)).subarray(0, 5))).toBeNull();
  });
});

describe("the client's bytes", () => {
  it("passes the upgrade's head and each whole frame on as they came", () => {
    const bytes = new ClientBytes();
    const whole = Buffer.concat([UPGRADE, TEXT, TEXT]);
    const out = [3, 40, 85, whole.length].flatMap((end, i, ends) =>
      bytes.take(whole.subarray(i === 0 ? 0 : ends[i - 1], end)),
    );
    expect(Buffer.concat(out)).toEqual(whole);
    expect(out).toHaveLength(3);
    expect(bytes.closed).toBe(false);
  });

  it("keeps the close frame and everything after it from the server", () => {
    const bytes = new ClientBytes();
    expect(Buffer.concat(bytes.take(Buffer.concat([UPGRADE, TEXT, CLOSE, TEXT])))).toEqual(
      Buffer.concat([UPGRADE, TEXT]),
    );
    expect(bytes.closed).toBe(true);
    expect(bytes.take(TEXT)).toEqual([]);
  });
});

describe("a relayed connection", () => {
  it("relays both ways while the client is there", () => {
    const client = new FakeSocket();
    const upstream = new FakeSocket();
    const relayed = relayConnection(client, upstream);
    client.emit("data", Buffer.concat([UPGRADE, TEXT]));
    upstream.emit("data", Buffer.from("HTTP/1.1 101 Switching Protocols\r\n\r\n"));
    expect(Buffer.concat(upstream.written)).toEqual(Buffer.concat([UPGRADE, TEXT]));
    expect(Buffer.concat(client.written).toString("latin1")).toContain("101");
    expect(relayed.state()).toBe("open");
  });

  it("holds the server's socket open once the client has sent its close and gone", () => {
    const client = new FakeSocket();
    const upstream = new FakeSocket();
    const relayed = relayConnection(client, upstream);
    client.emit("data", Buffer.concat([UPGRADE, TEXT, CLOSE]));
    client.emit("end");
    upstream.emit("data", Buffer.from("the sky's final reply"));
    expect(relayed.state()).toBe("held");
    expect(client.destroyed).toBe(true);
    expect(upstream.destroyed).toBe(false);
    expect(Buffer.concat(upstream.written)).toEqual(Buffer.concat([UPGRADE, TEXT]));
    expect(client.written).toEqual([]);
  });

  it("holds the server's socket open when the client goes without a close", () => {
    const client = new FakeSocket();
    const upstream = new FakeSocket();
    const relayed = relayConnection(client, upstream);
    client.emit("data", Buffer.concat([UPGRADE, TEXT]));
    client.emit("error", new Error("ECONNRESET"));
    expect(relayed.state()).toBe("held");
    expect(upstream.destroyed).toBe(false);
  });

  it("closes the server's socket when it is ended", () => {
    const client = new FakeSocket();
    const upstream = new FakeSocket();
    const relayed = relayConnection(client, upstream);
    client.emit("end");
    relayed.end();
    expect(relayed.state()).toBe("ended");
    expect(upstream.destroyed).toBe(true);
  });

  it("lets the client go when the server closes", () => {
    const client = new FakeSocket();
    const upstream = new FakeSocket();
    const relayed = relayConnection(client, upstream);
    upstream.emit("end");
    expect(relayed.state()).toBe("ended");
    expect(client.destroyed).toBe(true);
  });
});
