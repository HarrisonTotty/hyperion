/**
 * The descent spike's trace over the Chrome DevTools Protocol (plan R05, T14.i;
 * decision-r05-trace-windows-2.md, rulings 1, 5 and 6): each window recorded through CDP's
 * `Tracing` domain on the spike window's own `webContents.debugger`, returned as a Perfetto
 * protobuf stream, and read into the window's file.
 *
 * @remarks
 * Electron's `contentTracing` writes JSON only, which Chromium's tracing service builds whole in
 * memory at the stop, 4.3–5.7 times the buffer's bytes; the protobuf stream is about 1.2 times the
 * buffer and is read in seconds (T14.f's measurements). The session sends only `Tracing` and `IO`
 * commands ({@link CDP_TRACE_COMMANDS}), which the browser process's DevTools handlers answer, so
 * the page's renderer sees only the attach and the detach. It attaches once, at the first window's
 * start, and detaches once, after the last stop ({@link CdpTracing.close}), so no measured frame
 * sees either. A detach the trace did not ask for (the page gone, DevTools opened on it) ends the
 * trace, not the run: every later command is refused with its reason, so the next cycle fails as a
 * refused one does.
 *
 * Chromium spools the stream to a file in the browser's temporary directory before `IO.read`
 * returns it, which the recipe puts on disk (`descentSpike.sh`'s `TMPDIR`).
 */

import { open } from "node:fs/promises";

import type { TraceConfig } from "electron";

import type { SpikeTracing, TraceWindowStop } from "./spike";

/** The protocol version the trace's session attaches with. */
export const CDP_PROTOCOL_VERSION = "1.3";

/**
 * The only commands the trace sends: `Tracing`'s start and end, and `IO`'s read and close of the
 * stream a stop returns (ruling 5a).
 */
export const CDP_TRACE_COMMANDS = ["Tracing.start", "Tracing.end", "IO.read", "IO.close"] as const;

/** One of {@link CDP_TRACE_COMMANDS}. */
export type CdpTraceCommand = (typeof CDP_TRACE_COMMANDS)[number];

/** How much of a stopped window's stream one `IO.read` asks for, bytes: 8 MiB. */
export const CDP_READ_BYTES = 8 * 1024 * 1024;

/** How often Chromium reports the buffer's use while a window records, ms. */
export const CDP_BUFFER_REPORT_MS = 1000;

/** A detached session's reason, as the commands it refuses give it. */
function detachedReason(reason: string): string {
  return `the trace's debugger session detached: ${reason}`;
}

/**
 * The part of Electron's `webContents.debugger` the trace uses.
 *
 * @remarks
 * Its events are Electron's: `message` for each protocol event, and `detach` when the session ends
 * for any reason, the trace's own {@link TraceDebugger.detach} included.
 */
export interface TraceDebugger {
  attach(protocolVersion: string): void;
  detach(): void;
  sendCommand(method: string, params: Readonly<Record<string, unknown>>): Promise<unknown>;
  on(
    event: "message",
    listener: (event: unknown, method: string, params: unknown) => void,
  ): unknown;
  on(event: "detach", listener: (event: unknown, reason: string) => void): unknown;
  off(
    event: "message",
    listener: (event: unknown, method: string, params: unknown) => void,
  ): unknown;
  off(event: "detach", listener: (event: unknown, reason: string) => void): unknown;
}

/** A window's file, open for writing. */
export interface TraceFileWriter {
  write(data: Uint8Array): Promise<unknown>;
  close(): Promise<void>;
}

/** What the trace needs besides the debugger. */
export interface CdpTracingDeps {
  /** The run's log, one line a call. */
  readonly log: (line: string) => void;
  /** `performance.now()`, ms, which times the stops' phases. */
  readonly nowMs: () => number;
  /** Opens a window's file for writing, truncating it; Node's by default. */
  readonly openFile?: (path: string) => Promise<TraceFileWriter>;
}

/**
 * A file opened with Node, each write whole: `FileHandle.writeFile` writes all its bytes from the
 * file's current position, where one `write` may write fewer.
 */
async function openForWriting(path: string): Promise<TraceFileWriter> {
  const handle = await open(path, "w");
  return {
    write: (data) => handle.writeFile(data),
    close: () => handle.close(),
  };
}

/** CDP's name for each of Chromium's recording modes (`TraceConfig.recordMode`). */
const CDP_RECORD_MODES: Readonly<Record<NonNullable<TraceConfig["recording_mode"]>, string>> = {
  "record-until-full": "recordUntilFull",
  "record-continuously": "recordContinuously",
  "record-as-much-as-possible": "recordAsMuchAsPossible",
  "trace-to-console": "echoToConsole",
};

/**
 * `Tracing.start`'s parameters for a window recorded with `config`: its buffer, mode and
 * categories, returned as an uncompressed protobuf stream, the buffer's use reported every
 * {@link CDP_BUFFER_REPORT_MS}.
 */
export function cdpStartParams(config: TraceConfig): Readonly<Record<string, unknown>> {
  return {
    traceConfig: {
      // Chromium's default when none is given.
      recordMode: CDP_RECORD_MODES[config.recording_mode ?? "record-until-full"],
      ...(config.trace_buffer_size_in_kb === undefined
        ? {}
        : { traceBufferSizeInKb: config.trace_buffer_size_in_kb }),
      includedCategories: [...(config.included_categories ?? [])],
      excludedCategories: [...(config.excluded_categories ?? [])],
    },
    transferMode: "ReturnAsStream",
    streamFormat: "proto",
    streamCompression: "none",
    bufferUsageReportingInterval: CDP_BUFFER_REPORT_MS,
  };
}

/** What `Tracing.tracingComplete` says of a stopped window. */
interface TracingComplete {
  /** The stream's handle, or `null` when Chromium returned none. */
  readonly stream: string | null;
  readonly dataLossOccurred: boolean;
}

function readComplete(params: unknown): TracingComplete {
  if (typeof params !== "object" || params === null) {
    return { stream: null, dataLossOccurred: false };
  }
  const stream: unknown = Reflect.get(params, "stream");
  return {
    stream: typeof stream === "string" ? stream : null,
    dataLossOccurred: Reflect.get(params, "dataLossOccurred") === true,
  };
}

/** One `IO.read`'s answer. */
interface StreamChunk {
  readonly data: Uint8Array;
  readonly eof: boolean;
}

/**
 * An `IO.read` answer's bytes.
 *
 * @throws Error if the answer is not a read's: CDP's `{ data, eof, base64Encoded? }`.
 */
function readChunk(answer: unknown): StreamChunk {
  if (typeof answer !== "object" || answer === null) {
    throw new Error("IO.read answered without the stream's data");
  }
  const text: unknown = Reflect.get(answer, "data");
  const eof: unknown = Reflect.get(answer, "eof");
  if (typeof text !== "string" || typeof eof !== "boolean") {
    throw new Error("IO.read answered without the stream's data");
  }
  const base64 = Reflect.get(answer, "base64Encoded") === true;
  return { data: Buffer.from(text, base64 ? "base64" : "utf8"), eof };
}

/** Where the trace's debugger session is. */
type Session =
  | { readonly kind: "unattached" }
  | { readonly kind: "attached" }
  /** It ended without the trace asking: every later command is refused with the reason. */
  | { readonly kind: "detached"; readonly reason: string }
  /** The trace detached it after its last stop. */
  | { readonly kind: "closed" };

/** Why a session that has not attached, or has closed, takes no command. */
const REFUSALS: Readonly<Record<"unattached" | "closed", string>> = {
  unattached: "the trace's debugger session is not attached",
  closed: "the trace's debugger session is closed",
};

/** A stop's wait for `Tracing.tracingComplete`. */
interface CompleteWaiter {
  readonly resolve: (complete: TracingComplete) => void;
  readonly reject: (error: Error) => void;
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * The spike's trace transport over the spike window's `webContents.debugger`: one CDP session for
 * the run, one `Tracing` recording a window.
 *
 * @remarks
 * {@link SpikeTrace} keeps one operation at a time; this class keeps the session. Each stop logs
 * its two phases: `Tracing.end` until `Tracing.tracingComplete`, and the stream's read.
 */
export class CdpTracing implements SpikeTracing {
  readonly #debugger: TraceDebugger;
  readonly #log: (line: string) => void;
  readonly #nowMs: () => number;
  readonly #openFile: (path: string) => Promise<TraceFileWriter>;
  #session: Session = { kind: "unattached" };
  /**
   * The window's last `Tracing.bufferUsage.percentFull`, a fraction from 0 to 1, or `null` before
   * one.
   */
  #bufferFraction: number | null = null;
  #complete: CompleteWaiter | null = null;
  /** Stops made so far, which number the log's lines. */
  #stops = 0;

  readonly #onMessage = (_event: unknown, method: string, params: unknown): void => {
    if (method === "Tracing.bufferUsage") {
      const percentFull: unknown =
        typeof params === "object" && params !== null
          ? Reflect.get(params, "percentFull")
          : undefined;
      if (typeof percentFull === "number" && Number.isFinite(percentFull) && percentFull >= 0) {
        this.#bufferFraction = percentFull;
      }
    } else if (method === "Tracing.tracingComplete") {
      const waiter = this.#complete;
      this.#complete = null;
      waiter?.resolve(readComplete(params));
    }
  };

  readonly #onDetach = (_event: unknown, reason: string): void => {
    // The trace's own detach has closed the session first.
    if (this.#session.kind !== "attached") {
      return;
    }
    this.#session = { kind: "detached", reason };
    this.#log(`descent spike: ${detachedReason(reason)}`);
    const waiter = this.#complete;
    this.#complete = null;
    waiter?.reject(new Error(detachedReason(reason)));
  };

  constructor(debuggerSession: TraceDebugger, deps: CdpTracingDeps) {
    this.#debugger = debuggerSession;
    this.#log = deps.log;
    this.#nowMs = deps.nowMs;
    this.#openFile = deps.openFile ?? openForWriting;
  }

  /**
   * Starts a window's recording, attaching the session first if this is the first window.
   *
   * @throws Error if the session cannot attach (another debugger holds the page), has detached or
   * is closed, or if Chromium refuses the start.
   */
  async start(config: TraceConfig): Promise<void> {
    this.#attach();
    this.#bufferFraction = null;
    await this.#send("Tracing.start", cdpStartParams(config));
  }

  /**
   * Ends the window's recording and writes its stream to `path`.
   *
   * @returns Whether Chromium lost data, and the buffer's last reported use, %.
   * @throws Error if the session has detached or is closed, if Chromium refuses the end or returns
   * no stream, or if the stream cannot be read or written; the file may then be partial.
   */
  async stop(path: string): Promise<TraceWindowStop> {
    this.#stops += 1;
    const k = this.#stops;
    const fromMs = this.#nowMs();
    const complete = this.#tracingComplete();
    let done: TracingComplete;
    try {
      // The wait is registered first, so the event cannot pass before it.
      [done] = await Promise.all([complete, this.#send("Tracing.end")]);
    } finally {
      this.#complete = null;
    }
    const completeMs = this.#nowMs() - fromMs;
    const bufferPercent = this.#bufferFraction === null ? null : this.#bufferFraction * 100;
    if (done.stream === null) {
      throw new Error("Chromium returned no stream for the trace");
    }
    const bytes = await this.#readStream(done.stream, path);
    const readMs = this.#nowMs() - fromMs - completeMs;
    this.#log(
      `descent spike: trace window ${String(k)} stopped: complete after ${completeMs.toFixed(0)} ms, ` +
        `${String(bytes)} B read in ${readMs.toFixed(0)} ms${done.dataLossOccurred ? ", data lost" : ""}`,
    );
    return { lostData: done.dataLossOccurred, bufferPercent };
  }

  /**
   * Ends the session after the last stop: detaches it, if it is attached, and stops listening.
   * Every later start is refused, and a stop still waiting for the trace's end fails.
   */
  close(): void {
    const attached = this.#session.kind === "attached";
    if (this.#session.kind !== "detached") {
      this.#session = { kind: "closed" };
    }
    const waiter = this.#complete;
    this.#complete = null;
    waiter?.reject(new Error(REFUSALS.closed));
    if (attached) {
      this.#debugger.detach();
    }
    this.#debugger.off("message", this.#onMessage);
    this.#debugger.off("detach", this.#onDetach);
  }

  /** Attaches the session once; a detached or closed one stays so. */
  #attach(): void {
    switch (this.#session.kind) {
      case "attached":
        return;
      case "unattached":
        this.#debugger.on("message", this.#onMessage);
        this.#debugger.on("detach", this.#onDetach);
        try {
          this.#debugger.attach(CDP_PROTOCOL_VERSION);
        } catch (error: unknown) {
          this.#debugger.off("message", this.#onMessage);
          this.#debugger.off("detach", this.#onDetach);
          throw new Error(`the trace's debugger session did not attach: ${messageOf(error)}`, {
            cause: error,
          });
        }
        this.#session = { kind: "attached" };
        return;
      case "detached":
        throw new Error(detachedReason(this.#session.reason));
      case "closed":
        throw new Error(REFUSALS.closed);
    }
  }

  /**
   * Sends one of the trace's commands.
   *
   * @throws Error if the session is not attached, or if Chromium refuses the command.
   */
  #send(method: CdpTraceCommand, params: Readonly<Record<string, unknown>> = {}): Promise<unknown> {
    const session = this.#session;
    if (session.kind === "attached") {
      return this.#debugger.sendCommand(method, params);
    }
    return Promise.reject(
      new Error(
        session.kind === "detached" ? detachedReason(session.reason) : REFUSALS[session.kind],
      ),
    );
  }

  /** Waits for the window's `Tracing.tracingComplete`, or for the session's detach. */
  #tracingComplete(): Promise<TracingComplete> {
    return new Promise((resolve, reject) => {
      this.#complete = { resolve, reject };
    });
  }

  /** Reads the stream `handle` into `path` until its end, then closes it; returns its bytes. */
  async #readStream(handle: string, path: string): Promise<number> {
    try {
      const file = await this.#openFile(path);
      let bytes = 0;
      try {
        for (;;) {
          // One read at a time, in order: each answer is the next part of the one stream.
          // oxlint-disable-next-line no-await-in-loop
          const chunk = readChunk(await this.#send("IO.read", { handle, size: CDP_READ_BYTES }));
          if (chunk.data.length > 0) {
            // Each chunk is written before the next is read, so one is held at a time.
            // oxlint-disable-next-line no-await-in-loop
            await file.write(chunk.data);
            bytes += chunk.data.length;
          }
          if (chunk.eof) {
            break;
          }
        }
      } catch (error: unknown) {
        // The read's error is the window's, not the close's that may follow it.
        await file.close().catch((closeError: unknown) => {
          this.#log(`descent spike: a trace window's file did not close: ${messageOf(closeError)}`);
        });
        throw error;
      }
      await file.close();
      return bytes;
    } finally {
      await this.#closeStream(handle);
    }
  }

  /**
   * Closes the stream in the browser. A failure is logged, not thrown: after a whole read the file
   * stands, and after a failed one the read's error is the window's.
   */
  async #closeStream(handle: string): Promise<void> {
    if (this.#session.kind !== "attached") {
      return;
    }
    try {
      await this.#send("IO.close", { handle });
    } catch (error: unknown) {
      this.#log(`descent spike: the trace's stream was not closed: ${messageOf(error)}`);
    }
  }
}
