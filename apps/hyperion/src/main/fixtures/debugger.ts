import { EventEmitter } from "node:events";

import type { TraceDebugger, TraceFileWriter } from "../cdpTracing";

/** One `IO.read` answer, as CDP gives it. */
export interface FakeChunk {
  readonly data: string;
  readonly base64Encoded: boolean;
  readonly eof: boolean;
}

/** A chunk of `bytes`, base64-encoded as Chromium sends a binary stream. */
export function chunkOf(bytes: ReadonlyArray<number>, eof: boolean): FakeChunk {
  return { data: Buffer.from(bytes).toString("base64"), base64Encoded: true, eof };
}

/** The stream handle the fake's `Tracing.tracingComplete` names. */
export const FAKE_STREAM = "stream-1";

/**
 * A fake of Electron's `webContents.debugger`: it records every attach, detach and command in
 * order, answers `Tracing.end` with `Tracing.tracingComplete` at once (before the end's own answer,
 * the worse order) unless held, and serves each window's stream from {@link FakeDebugger.chunks}.
 * Its own `detach` emits `detach`, as Electron's does.
 */
export class FakeDebugger extends EventEmitter implements TraceDebugger {
  /** Every call in order: `attach <version>`, `detach` and each command's method. */
  readonly calls: string[] = [];
  /** Each command with its parameters, in order. */
  readonly sent: Array<{
    readonly method: string;
    readonly params: Readonly<Record<string, unknown>>;
  }> = [];
  /** The chunks each window's stream answers with, in order; an empty, ended one past them. */
  chunks: FakeChunk[] = [chunkOf([1, 2, 3], false), chunkOf([4, 5, 6], true)];
  /** Whether the next `Tracing.tracingComplete` says data was lost. */
  dataLoss = false;
  /** Holds `Tracing.tracingComplete` until {@link FakeDebugger.complete}. */
  holdComplete = false;
  /** Thrown by the next attach, as Electron's is when another debugger holds the page. */
  failAttach: Error | null = null;
  /** Commands that the fake refuses, with the error each rejects with. */
  readonly refuse = new Map<string, Error>();

  attach(protocolVersion: string): void {
    const failure = this.failAttach;
    if (failure !== null) {
      this.failAttach = null;
      throw failure;
    }
    this.calls.push(`attach ${protocolVersion}`);
  }

  detach(): void {
    this.calls.push("detach");
    this.emit("detach", {}, "target closed");
  }

  sendCommand(method: string, params: Readonly<Record<string, unknown>>): Promise<unknown> {
    this.calls.push(method);
    this.sent.push({ method, params });
    const refused = this.refuse.get(method);
    if (refused !== undefined) {
      return Promise.reject(refused);
    }
    switch (method) {
      case "Tracing.end":
        if (!this.holdComplete) {
          this.complete();
        }
        return Promise.resolve({});
      case "IO.read":
        return Promise.resolve(this.chunks.shift() ?? { data: "", base64Encoded: true, eof: true });
      default:
        return Promise.resolve({});
    }
  }

  /** Sends `Tracing.tracingComplete` for the window, naming {@link FAKE_STREAM}. */
  complete(): void {
    this.emit("message", {}, "Tracing.tracingComplete", {
      dataLossOccurred: this.dataLoss,
      stream: FAKE_STREAM,
      traceFormat: "proto",
      streamCompression: "none",
    });
  }

  /** Sends `Tracing.bufferUsage` with `percentFull`, a fraction. */
  bufferUsage(percentFull: number): void {
    this.emit("message", {}, "Tracing.bufferUsage", {
      percentFull,
      eventCount: 1000,
      value: percentFull,
    });
  }

  /** Ends the session as Chromium does when the page goes or DevTools opens on it. */
  lose(reason: string): void {
    this.emit("detach", {}, reason);
  }
}

/** Files written in memory: each path's bytes in write order, and the paths closed. */
export function memoryWriter(): {
  readonly files: Map<string, number[]>;
  readonly closed: string[];
  readonly openFile: (path: string) => Promise<TraceFileWriter>;
} {
  const files = new Map<string, number[]>();
  const closed: string[] = [];
  return {
    files,
    closed,
    openFile: (path) => {
      const bytes: number[] = [];
      files.set(path, bytes);
      return Promise.resolve({
        write: (data) => {
          bytes.push(...data);
          return Promise.resolve();
        },
        close: () => {
          closed.push(path);
          return Promise.resolve();
        },
      });
    },
  };
}
