/**
 * The descent spike's Perfetto protobuf trace decoder (plan R05, T14.h;
 * decision-r05-trace-windows-2.md): a trace file in, the JSON-shaped events `TraceReducer.add`
 * reads out, so that one reducer serves both formats.
 *
 * @remarks
 * Hand-written and dependency-free, it decodes only the messages the reducer needs and skips every
 * other field. The field numbers are those of Perfetto's protos at revision
 * `9c7ce42050379be8c24a270f395e00c7347965cb`, the `src/third_party/perfetto` that Chromium
 * 152.0.7977.130 (Electron 44.4.3) pins in its `DEPS`; each constant below names its file under
 * `protos/perfetto/` (or `protos/third_party/chromium/`) and its message. The test's fixtures,
 * recorded from Electron 44.4.3, pin them.
 *
 * What the decoder keeps of a trace:
 *
 * - **Sequences.** Each packet belongs to its writer's sequence (`trusted_packet_sequence_id`).
 *   A sequence's interned categories, event names and debug-annotation names and values, and its
 *   packet defaults are its incremental state, reset when a packet clears it (Chromium clears each
 *   busy sequence's every 0.5 s). A packet that needs incremental state its sequence has not had
 *   since it began is dropped, as Perfetto's own importer drops it. A sequence that lost packets
 *   (`previous_packet_dropped` past its first) fails the file, since a lost end would pair its
 *   begin with another slice's.
 * - **Clocks.** Every timestamp is converted to the trace's clock, the snapshot's
 *   `primary_trace_clock` (`MONOTONIC` in Chromium's traces, the clock its JSON's `ts` is on), and
 *   given in µs. A sequence-scoped clock (ids 64–127, Chromium's incremental µs clock and its
 *   absolute twin) is placed by its sequence's last snapshot, which also gives the trace clock's
 *   time; an incremental clock's timestamps are deltas from the sequence's last one. A builtin
 *   clock other than the trace's is placed by the last snapshot holding both. So the main thread's
 *   and the compositor's sequences share one clock.
 * - **Tracks.** Process tracks give `process_name` metadata and thread tracks `thread_name`
 *   metadata, with Chrome's process and thread types standing in for a missing name. A slice's
 *   track is its event's `track_uuid` or its sequence's default; its process and thread are the
 *   nearest thread track up its parents, else the nearest process track's with the sequence's
 *   thread.
 * - **Slices.** Begins and ends pair per track as a stack. A slice on a thread track becomes one
 *   complete event (`ph: "X"`) at its end, as Chromium's JSON writes `RunTask`, `GPUTask` and the
 *   GC slices. A slice on any other track (a span of `performance.measure` on its own child of the
 *   main thread's track, a `PipelineReporter` on a child of the renderer's process track, V8's
 *   GC-state tracks) becomes an async begin and end (`ph: "b"` and `"e"`), with an ID unique to the
 *   slice, so that the reducer's (process, ID, name) matching pairs them as before.
 * - **Legacy events** (`legacy_event`: V8's `Profile` and `ProfileChunk` samples, among others)
 *   keep their phase and IDs, as Chromium's JSON writes them.
 * - **Arguments.** Debug annotations become `args`: scalars, pointers as hex, nested values and
 *   dictionaries as objects, arrays, and legacy JSON values parsed (`startTime`, a `ProfileChunk`'s
 *   `data`); the frame reporter becomes `args.frame_reporter` with the JSON's `STATE_*` names.
 *
 * An event of a kind the reducer reads that arrives in an encoding the decoder does not know fails
 * the file, naming the event. Counter events, which the reducer does not read, are skipped.
 */

import { createReadStream } from "node:fs";

/** The first byte of a Perfetto trace: `Trace.packet`'s tag, field 1, length-delimited. */
export const PROTO_TRACE_FIRST_BYTE = 0x0a;

/** How many bytes the file reader reads at a time by default. */
export const DEFAULT_PROTO_READ_BYTES = 4 * 1024 * 1024;

/** `trace/trace.proto`, `Trace`: `packet`. */
const TRACE_PACKET = 1;

/** `trace/trace_packet.proto`, `TracePacket`. */
const PACKET = {
  timestamp: 8,
  timestampClockId: 58,
  trustedPacketSequenceId: 10,
  sequenceFlags: 13,
  incrementalStateCleared: 41,
  firstPacketOnSequence: 87,
  previousPacketDropped: 42,
  internedData: 12,
  tracePacketDefaults: 59,
  trackDescriptor: 60,
  trackEvent: 11,
  clockSnapshot: 6,
  processDescriptor: 43,
  threadDescriptor: 44,
} as const;

/** `TracePacket.SequenceFlags`. */
const SEQ_INCREMENTAL_STATE_CLEARED = 1;
const SEQ_NEEDS_INCREMENTAL_STATE = 2;

/** `trace/trace_packet_defaults.proto`, `TracePacketDefaults`. */
const DEFAULTS = { timestampClockId: 58, trackEventDefaults: 11 } as const;

/** `trace/track_event/track_event.proto`, `TrackEventDefaults`: `track_uuid`. */
const TRACK_EVENT_DEFAULTS_TRACK_UUID = 11;

/** `trace/clock_snapshot.proto`, `ClockSnapshot` and its `Clock`. */
const CLOCK_SNAPSHOT = { clocks: 1, primaryTraceClock: 2 } as const;
const CLOCK = { clockId: 1, timestamp: 2, isIncremental: 3, unitMultiplierNs: 4 } as const;

/**
 * `common/builtin_clock.proto`: `BUILTIN_CLOCK_BOOTTIME`, the clock of a packet that names none
 * (`trace/trace_packet.proto`, `timestamp_clock_id`) and of a trace whose snapshots name no
 * primary clock.
 */
const BOOTTIME = 6;

/** `trace/clock_snapshot.proto`: clock IDs 64 to 127 are scoped to their sequence. */
const FIRST_SEQUENCE_CLOCK = 64;
const LAST_SEQUENCE_CLOCK = 127;

/** `trace/interned_data/interned_data.proto`, `InternedData`. */
const INTERNED = {
  eventCategories: 1,
  eventNames: 2,
  debugAnnotationNames: 3,
  debugAnnotationStringValues: 29,
} as const;

/**
 * The `iid` and string fields of the interned entries: `EventCategory` and `EventName`
 * (`trace/track_event/track_event.proto`), `DebugAnnotationName`
 * (`trace/track_event/debug_annotation.proto`) and `InternedString`
 * (`trace/profiling/profile_common.proto`, its `str`), each `iid = 1` and its string `= 2`.
 */
const INTERNED_ENTRY = { iid: 1, value: 2 } as const;

/** `trace/track_event/track_descriptor.proto`, `TrackDescriptor`. */
const TRACK = {
  uuid: 1,
  parentUuid: 5,
  process: 3,
  chromeProcess: 6,
  thread: 4,
  chromeThread: 7,
} as const;

/** `trace/track_event/process_descriptor.proto`, `ProcessDescriptor`. */
const PROCESS = { pid: 1, processName: 6, chromeProcessType: 4 } as const;

/** `trace/track_event/thread_descriptor.proto`, `ThreadDescriptor`. */
const THREAD = {
  pid: 1,
  tid: 2,
  threadName: 5,
  chromeThreadType: 4,
  referenceTimestampUs: 6,
} as const;

/**
 * `trace/track_event/chrome_process_descriptor.proto` and `chrome_thread_descriptor.proto`:
 * `ChromeProcessDescriptor.process_type` and `ChromeThreadDescriptor.thread_type`, both 1.
 */
const CHROME_DESCRIPTOR_TYPE = 1;

/**
 * The names Chromium's JSON gives the processes the reducer reads, by Chrome's process type
 * (`protos/third_party/chromium/chrome_enums.proto`, `ProcessType`, whose values
 * `ProcessDescriptor.ChromeProcessType` shares).
 */
const PROCESS_NAMES: ReadonlyMap<number, string> = new Map([
  [1, "Browser"],
  [2, "Renderer"],
  [6, "GPU Process"],
]);

/**
 * The names Chromium's JSON gives the threads the reducer reads, by Chrome's thread type
 * (`protos/third_party/chromium/chrome_enums.proto`, `ThreadType`).
 */
const THREAD_NAMES: ReadonlyMap<number, string> = new Map([
  [15, "CrBrowserMain"],
  [16, "CrRendererMain"],
  [18, "CrGpuMain"],
]);

/** `trace/track_event/track_event.proto`, `TrackEvent`. */
const EVENT = {
  categoryIids: 3,
  categories: 22,
  nameIid: 10,
  name: 23,
  type: 9,
  trackUuid: 11,
  debugAnnotations: 4,
  legacyEvent: 6,
  timestampDeltaUs: 1,
  timestampAbsoluteUs: 16,
  chromeFrameReporter: 32,
} as const;

/**
 * `protos/third_party/chromium/chrome_track_event.proto`: `ChromeTrackEvent.frame_reporter`, the
 * extension of `TrackEvent` that Chromium 152 writes the frame reporter in, in place of the
 * deprecated `TrackEvent.chrome_frame_reporter`.
 */
const CHROME_TRACK_EVENT_FRAME_REPORTER = 1075;

/** `TrackEvent.Type`. */
const TYPE_UNSPECIFIED = 0;
const TYPE_SLICE_BEGIN = 1;
const TYPE_SLICE_END = 2;
const TYPE_INSTANT = 3;
const TYPE_COUNTER = 4;

/** `TrackEvent.LegacyEvent`. */
const LEGACY = {
  nameIid: 1,
  phase: 2,
  durationUs: 3,
  unscopedId: 6,
  localId: 10,
  globalId: 11,
  idScope: 7,
  pidOverride: 18,
  tidOverride: 19,
} as const;

/** `trace/track_event/debug_annotation.proto`, `DebugAnnotation`. */
const ANNOTATION = {
  nameIid: 1,
  name: 10,
  boolValue: 2,
  uintValue: 3,
  intValue: 4,
  doubleValue: 5,
  stringValue: 6,
  pointerValue: 7,
  nestedValue: 8,
  legacyJsonValue: 9,
  stringValueIid: 17,
  dictEntries: 11,
  arrayValues: 12,
} as const;

/** `DebugAnnotation.NestedValue`, and its `NestedType`'s `DICT` and `ARRAY`. */
const NESTED = {
  nestedType: 1,
  dictKeys: 2,
  dictValues: 3,
  arrayValues: 4,
  intValue: 5,
  doubleValue: 6,
  boolValue: 7,
  stringValue: 8,
} as const;
const NESTED_DICT = 1;
const NESTED_ARRAY = 2;

/**
 * `trace/track_event/chrome_frame_reporter.proto`, `ChromeFrameReporter` (and
 * `ChromeFrameReporter2`, the same numbers): `state` and `layer_tree_host_id`.
 */
const FRAME_REPORTER = { state: 1, layerTreeHostId: 11 } as const;

/** `ChromeFrameReporter.State`, by number: the names Chromium's JSON writes. */
const FRAME_STATES: ReadonlyArray<string> = [
  "STATE_NO_UPDATE_DESIRED",
  "STATE_PRESENTED_ALL",
  "STATE_PRESENTED_PARTIAL",
  "STATE_DROPPED",
];

/** Protobuf wire types. */
const VARINT = 0;
const FIXED64 = 1;
const LENGTH_DELIMITED = 2;
const FIXED32 = 5;

/** A JSON-shaped trace event, as Chromium's JSON writes one and `TraceReducer.add` reads it. */
export interface ProtoTraceEvent {
  readonly name: string;
  /** The categories, joined by commas. */
  readonly cat: string;
  readonly ph: string;
  readonly pid: number;
  readonly tid: number;
  /** On the trace's clock, µs. */
  readonly ts: number;
  /** A complete event's duration, µs. */
  readonly dur?: number;
  /** An async or sampled event's ID. */
  readonly id?: string;
  readonly args: Readonly<Record<string, unknown>>;
}

/** One packet decoded. */
export interface DecodedPacket {
  /** The byte offset of its tag in the trace. */
  readonly offset: number;
  /** Its `trusted_packet_sequence_id`, 0 without one. */
  readonly sequence: number;
  /** Its timestamp on the trace's clock, µs, or `null` for a packet without one. */
  readonly timeUs: number | null;
  /**
   * How it stands to its sequence's incremental state: it clears it, it needs it, or neither (a
   * packet that stands alone).
   */
  readonly state: "clears" | "needs" | "none";
  /** The events it completes: its begins, ends, instants and samples, and its tracks' metadata. */
  readonly events: ReadonlyArray<ProtoTraceEvent>;
}

/** An error in a trace's bytes: a message that is not one. */
class WireError extends Error {}

/** A field that runs past the bytes there are: a message cut short, or a trace not yet all read. */
class ShortRead extends WireError {}

const TEXT = new TextDecoder();

/** A cursor over one protobuf message's bytes. */
class Reader {
  readonly #bytes: Uint8Array;
  #pos: number;
  readonly #end: number;

  constructor(bytes: Uint8Array, start = 0) {
    this.#bytes = bytes;
    this.#pos = start;
    this.#end = bytes.length;
  }

  get done(): boolean {
    return this.#pos >= this.#end;
  }

  get pos(): number {
    return this.#pos;
  }

  #byte(): number {
    const byte = this.#bytes[this.#pos];
    if (byte === undefined || this.#pos >= this.#end) {
      throw new ShortRead("a field runs past its message's end");
    }
    this.#pos += 1;
    return byte;
  }

  /** The low and high 32 bits of the varint read last by {@link Reader.#words}. */
  #lo = 0;
  #hi = 0;

  /** Reads a varint exactly into {@link Reader.#lo} and {@link Reader.#hi}, unsigned. */
  #words(): void {
    let lo = 0;
    let hi = 0;
    for (let i = 0; i < 10; i += 1) {
      const byte = this.#byte();
      const bits = byte & 0x7f;
      if (i < 4) {
        lo |= bits << (7 * i);
      } else if (i === 4) {
        lo |= (bits & 0x0f) << 28;
        hi |= bits >>> 4;
      } else {
        hi |= bits << (7 * i - 32);
      }
      if (byte < 0x80) {
        this.#lo = lo >>> 0;
        this.#hi = hi >>> 0;
        return;
      }
    }
    throw new WireError("a varint runs past ten bytes");
  }

  /** An unsigned varint, exact to 2^53. */
  varint(): number {
    let byte = this.#byte();
    if (byte < 0x80) {
      return byte;
    }
    let value = byte & 0x7f;
    let scale = 0x80;
    for (let i = 1; i < 10; i += 1) {
      byte = this.#byte();
      value += (byte & 0x7f) * scale;
      if (byte < 0x80) {
        return value;
      }
      scale *= 0x80;
    }
    throw new WireError("a varint runs past ten bytes");
  }

  /**
   * A varint as a 64-bit two's-complement integer (`int64`, `int32`, whose negative values take
   * ten bytes), exact for magnitudes to 2^53.
   */
  signed(): number {
    this.#words();
    // The high word as signed: the sum then holds a negative value without passing near 2^64.
    return (this.#hi | 0) * TWO_32 + this.#lo;
  }

  /** A varint exactly, in decimal: a 64-bit track UUID or ID exceeds a double's 53 bits. */
  key(): string {
    this.#words();
    return this.#hi === 0
      ? String(this.#lo)
      : (BigInt(this.#hi) * TWO_32_BIG + BigInt(this.#lo)).toString();
  }

  /** A little-endian `double`. */
  double(): number {
    const start = this.#advance(8);
    for (let i = 0; i < 8; i += 1) {
      DOUBLE_BYTES[i] = this.#bytes[start + i] ?? 0;
    }
    return DOUBLE[0] ?? Number.NaN;
  }

  /** A length-delimited field's bytes, as a view. */
  bytes(): Uint8Array {
    const length = this.varint();
    const start = this.#advance(length);
    return this.#bytes.subarray(start, start + length);
  }

  string(): string {
    return TEXT.decode(this.bytes());
  }

  /** Skips a field of the given wire type. */
  skip(wireType: number): void {
    switch (wireType) {
      case VARINT:
        this.varint();
        return;
      case FIXED64:
        this.#advance(8);
        return;
      case LENGTH_DELIMITED:
        this.bytes();
        return;
      case FIXED32:
        this.#advance(4);
        return;
      default:
        throw new WireError(`a field of unknown wire type ${wireType}`);
    }
  }

  /** Moves past `n` bytes, returning where they start. */
  #advance(n: number): number {
    const start = this.#pos;
    if (start + n > this.#end) {
      throw new ShortRead(`a field of ${n} bytes runs past its message's end`);
    }
    this.#pos += n;
    return start;
  }
}

const TWO_32 = 2 ** 32;
const TWO_32_BIG = 4_294_967_296n;
/** A little-endian `double`'s bytes and its value (the platforms Electron runs on). */
const DOUBLE = new Float64Array(1);
const DOUBLE_BYTES = new Uint8Array(DOUBLE.buffer);

/** A message's packed or unpacked repeated varint field, appended to `out`. */
function repeatedVarint(reader: Reader, wireType: number, out: number[]): void {
  if (wireType !== LENGTH_DELIMITED) {
    out.push(reader.varint());
    return;
  }
  const packed = new Reader(reader.bytes());
  while (!packed.done) {
    out.push(packed.varint());
  }
}

/** A sequence-scoped clock as its sequence's last snapshot placed it. */
interface SequenceClock {
  /** Its value at the snapshot, in its units. */
  readonly atSnapshot: number;
  /** The trace's clock at the snapshot, ns. */
  readonly traceNsAtSnapshot: number;
  readonly unitNs: number;
  readonly incremental: boolean;
  /** Its last value on the sequence, in its units. */
  last: number;
}

/** A sequence's incremental state, and its clocks. */
interface Sequence {
  /** Whether its incremental state is whole: cleared since the sequence began. */
  readonly valid: boolean;
  readonly categories: Map<number, string>;
  readonly names: Map<number, string>;
  readonly annotationNames: Map<number, string>;
  readonly annotationStrings: Map<number, string>;
  /**
   * Its sequence-scoped clocks, which outlive a clearing of its incremental state: the packet
   * that clears it gives them again in a new snapshot.
   */
  readonly clocks: Map<number, SequenceClock>;
  defaultClockId: number | undefined;
  defaultTrack: string | undefined;
  /** Its thread by the legacy `thread_descriptor`, for a sequence without a default track. */
  legacyThread: Place | undefined;
  /** The legacy events' µs clock: the thread descriptor's reference, moved by their deltas. */
  legacyUs: number;
}

function newSequence(valid: boolean, clocks: Map<number, SequenceClock>): Sequence {
  return {
    valid,
    categories: new Map(),
    names: new Map(),
    annotationNames: new Map(),
    annotationStrings: new Map(),
    clocks,
    defaultClockId: undefined,
    defaultTrack: undefined,
    legacyThread: undefined,
    legacyUs: 0,
  };
}

/** A process and thread. */
interface Place {
  readonly pid: number;
  readonly tid: number;
}

/** A described track: its parent, and its process and thread if it is a process or thread track. */
type Track =
  | { readonly kind: "thread"; readonly parent: string | undefined; readonly place: Place }
  | { readonly kind: "process"; readonly parent: string | undefined; readonly pid: number }
  | { readonly kind: "other"; readonly parent: string | undefined };

/** Where an event lies: its process and thread, and whether its track is the thread's own. */
interface EventPlace extends Place {
  readonly onThread: boolean;
}

/** A slice begun and not yet ended. */
interface OpenSlice extends EventPlace {
  readonly name: string;
  readonly cat: string;
  readonly ts: number;
  readonly id: string;
  readonly args: Readonly<Record<string, unknown>>;
}

/** The packet's fields the decoder reads, read whole before any is applied. */
interface RawPacket {
  timestamp: number | undefined;
  clockId: number | undefined;
  sequence: number;
  flags: number;
  cleared: boolean;
  first: boolean;
  dropped: boolean;
  interned: Uint8Array | undefined;
  defaults: Uint8Array | undefined;
  track: Uint8Array | undefined;
  event: Uint8Array | undefined;
  snapshot: Uint8Array | undefined;
  process: Uint8Array | undefined;
  thread: Uint8Array | undefined;
}

function readPacket(bytes: Uint8Array): RawPacket {
  const raw: RawPacket = {
    timestamp: undefined,
    clockId: undefined,
    sequence: 0,
    flags: 0,
    cleared: false,
    first: false,
    dropped: false,
    interned: undefined,
    defaults: undefined,
    track: undefined,
    event: undefined,
    snapshot: undefined,
    process: undefined,
    thread: undefined,
  };
  const reader = new Reader(bytes);
  while (!reader.done) {
    const tag = reader.varint();
    switch (tag >>> 3) {
      case PACKET.timestamp:
        raw.timestamp = reader.varint();
        break;
      case PACKET.timestampClockId:
        raw.clockId = reader.varint();
        break;
      case PACKET.trustedPacketSequenceId:
        raw.sequence = reader.varint();
        break;
      case PACKET.sequenceFlags:
        raw.flags = reader.varint();
        break;
      case PACKET.incrementalStateCleared:
        raw.cleared = reader.varint() !== 0;
        break;
      case PACKET.firstPacketOnSequence:
        raw.first = reader.varint() !== 0;
        break;
      case PACKET.previousPacketDropped:
        raw.dropped = reader.varint() !== 0;
        break;
      case PACKET.internedData:
        raw.interned = reader.bytes();
        break;
      case PACKET.tracePacketDefaults:
        raw.defaults = reader.bytes();
        break;
      case PACKET.trackDescriptor:
        raw.track = reader.bytes();
        break;
      case PACKET.trackEvent:
        raw.event = reader.bytes();
        break;
      case PACKET.clockSnapshot:
        raw.snapshot = reader.bytes();
        break;
      case PACKET.processDescriptor:
        raw.process = reader.bytes();
        break;
      case PACKET.threadDescriptor:
        raw.thread = reader.bytes();
        break;
      default:
        reader.skip(tag & 7);
    }
  }
  return raw;
}

/** A track event's fields, read whole before it is applied. */
interface RawEvent {
  type: number;
  trackUuid: string | undefined;
  readonly categoryIids: number[];
  readonly categories: string[];
  nameIid: number | undefined;
  name: string | undefined;
  readonly annotations: Uint8Array[];
  legacy: Uint8Array | undefined;
  frameReporter: Uint8Array | undefined;
  deltaUs: number | undefined;
  absoluteUs: number | undefined;
}

function readEvent(bytes: Uint8Array): RawEvent {
  const raw: RawEvent = {
    type: TYPE_UNSPECIFIED,
    trackUuid: undefined,
    categoryIids: [],
    categories: [],
    nameIid: undefined,
    name: undefined,
    annotations: [],
    legacy: undefined,
    frameReporter: undefined,
    deltaUs: undefined,
    absoluteUs: undefined,
  };
  const reader = new Reader(bytes);
  while (!reader.done) {
    const tag = reader.varint();
    switch (tag >>> 3) {
      case EVENT.type:
        raw.type = reader.varint();
        break;
      case EVENT.trackUuid:
        raw.trackUuid = reader.key();
        break;
      case EVENT.categoryIids:
        repeatedVarint(reader, tag & 7, raw.categoryIids);
        break;
      case EVENT.categories:
        raw.categories.push(reader.string());
        break;
      case EVENT.nameIid:
        raw.nameIid = reader.varint();
        break;
      case EVENT.name:
        raw.name = reader.string();
        break;
      case EVENT.debugAnnotations:
        raw.annotations.push(reader.bytes());
        break;
      case EVENT.legacyEvent:
        raw.legacy = reader.bytes();
        break;
      case EVENT.chromeFrameReporter:
      case CHROME_TRACK_EVENT_FRAME_REPORTER:
        raw.frameReporter = reader.bytes();
        break;
      case EVENT.timestampDeltaUs:
        raw.deltaUs = reader.signed();
        break;
      case EVENT.timestampAbsoluteUs:
        raw.absoluteUs = reader.signed();
        break;
      default:
        reader.skip(tag & 7);
    }
  }
  return raw;
}

/** A `LegacyEvent`'s fields. */
interface Legacy {
  nameIid: number | undefined;
  /** Its phase, a character code. */
  phase: number | undefined;
  durationUs: number | undefined;
  /** Its ID as the reducer keys Chromium's JSON IDs, its scope in front. */
  id: string | undefined;
  pid: number | undefined;
  tid: number | undefined;
}

function readLegacy(bytes: Uint8Array): Legacy {
  const legacy: Legacy = {
    nameIid: undefined,
    phase: undefined,
    durationUs: undefined,
    id: undefined,
    pid: undefined,
    tid: undefined,
  };
  let scope = "";
  let id = "";
  const reader = new Reader(bytes);
  while (!reader.done) {
    const tag = reader.varint();
    switch (tag >>> 3) {
      case LEGACY.nameIid:
        legacy.nameIid = reader.varint();
        break;
      case LEGACY.phase:
        legacy.phase = reader.signed();
        break;
      case LEGACY.durationUs:
        legacy.durationUs = reader.signed();
        break;
      // Chromium's JSON writes these as `id`, `id2.local` and `id2.global`, in hex.
      case LEGACY.unscopedId:
        id = hex(reader.key());
        break;
      case LEGACY.localId:
        id = `local:${hex(reader.key())}`;
        break;
      case LEGACY.globalId:
        id = `global:${hex(reader.key())}`;
        break;
      case LEGACY.idScope:
        scope = reader.string();
        break;
      case LEGACY.pidOverride:
        legacy.pid = reader.signed();
        break;
      case LEGACY.tidOverride:
        legacy.tid = reader.signed();
        break;
      default:
        reader.skip(tag & 7);
    }
  }
  if (id.length > 0) {
    legacy.id = scope.length > 0 ? `${scope}:${id}` : id;
  }
  return legacy;
}

/** A decimal key in hex, as Chromium's JSON writes IDs and pointers. */
function hex(key: string): string {
  return `0x${BigInt(key).toString(16)}`;
}

/** An error naming an event of a kind the reducer reads that the decoder cannot decode. */
function unknownEncoding(name: string, detail: string): Error {
  return new Error(
    `the trace's ${name} arrives in an encoding the decoder does not know: ${detail}`,
  );
}

/**
 * Whether the reducer reads an event of this name and categories (`reduceTrace.ts`), which the
 * decoder must then decode or refuse, never skip.
 *
 * @remarks
 * A copy of the reducer's names, since `reduceTrace.ts` imports this module; the test pins it to
 * the reducer's `GPU_PROCESS_SLICES` and `FRAME_MEASURE`'s category.
 */
export function readByReducer(name: string, cat: string): boolean {
  return (
    name === "RunTask" ||
    name === "MinorGC" ||
    name === "MajorGC" ||
    name === "PipelineReporter" ||
    name === "GPUTask" ||
    name === "WebGPU" ||
    name === "VulkanQueueSubmitHook" ||
    name === "Profile" ||
    name === "ProfileChunk" ||
    cat === "blink.user_timing" ||
    cat.split(",").some((category) => category.endsWith("v8.gc"))
  );
}

/** Where a track's events lie, or why they cannot be placed. */
type Placed = EventPlace | "undescribed" | "no thread";

/**
 * Decodes a trace's packets in order, keeping the state they build: each sequence's incremental
 * state and clocks, the tracks, the trace's clock, and the slices begun.
 */
export class ProtoTraceDecoder {
  readonly #sequences = new Map<number, Sequence>();
  readonly #tracks = new Map<string, Track>();
  readonly #open = new Map<string, OpenSlice[]>();
  /** Per track, the slices and instants begun on it so far, for the async IDs. */
  readonly #begun = new Map<string, number>();
  readonly #processNames = new Map<number, string>();
  readonly #threadNames = new Map<string, string>();
  #traceClock: number | undefined;
  /**
   * Each builtin clock's offset to the trace's clock, ns (trace's clock less it), by the last
   * snapshot that held both.
   */
  readonly #offsetsNs = new Map<number, number>();

  /**
   * Decodes one packet's bytes.
   *
   * @param offset - Where its tag lies in the trace, for the errors.
   * @throws Error naming the offset when the packet's bytes are not a packet, its sequence lost
   * packets before it (`previous_packet_dropped` on other than its first), its state is broken (a
   * name not interned, a clock without a snapshot), or an event the reducer reads arrives in an
   * encoding the decoder does not know.
   */
  decode(bytes: Uint8Array, offset: number): DecodedPacket {
    try {
      return this.#decode(bytes, offset);
    } catch (error: unknown) {
      if (error instanceof WireError) {
        throw new Error(`the trace's packet at byte ${offset} is not a packet: ${error.message}`, {
          cause: error,
        });
      }
      const message = error instanceof Error ? error.message : String(error);
      throw new Error(`${message}, in the packet at byte ${offset}`, { cause: error });
    }
  }

  #decode(bytes: Uint8Array, offset: number): DecodedPacket {
    const raw = readPacket(bytes);
    if (raw.dropped && !raw.first) {
      // A lost packet could hold any slice's end, and its begin would then pair with another's.
      throw new Error(`the trace lost packets of sequence ${raw.sequence} before this one`);
    }
    const events: ProtoTraceEvent[] = [];
    const before = this.#sequences.get(raw.sequence);
    let sequence = before ?? newSequence(false, new Map());
    if (raw.first && before !== undefined) {
      sequence = newSequence(false, new Map());
    }
    if (raw.cleared || (raw.flags & SEQ_INCREMENTAL_STATE_CLEARED) !== 0) {
      sequence = newSequence(true, sequence.clocks);
    }
    this.#sequences.set(raw.sequence, sequence);
    const clears = raw.cleared || (raw.flags & SEQ_INCREMENTAL_STATE_CLEARED) !== 0;
    const needs = (raw.flags & SEQ_NEEDS_INCREMENTAL_STATE) !== 0;
    const state = clears ? "clears" : needs ? "needs" : "none";
    if (needs && !sequence.valid) {
      // Perfetto's importer drops a packet whose sequence lacks the state it needs.
      return { offset, sequence: raw.sequence, timeUs: null, state, events };
    }
    if (raw.snapshot !== undefined) {
      this.#snapshot(raw.snapshot, sequence);
    }
    if (raw.defaults !== undefined) {
      this.#defaults(raw.defaults, sequence);
    }
    if (raw.interned !== undefined) {
      this.#interned(raw.interned, sequence);
    }
    const timeUs =
      raw.timestamp === undefined
        ? null
        : this.#traceNs(
            raw.timestamp,
            raw.clockId ?? sequence.defaultClockId ?? BOOTTIME,
            sequence,
          ) / 1000;
    if (raw.track !== undefined) {
      this.#track(raw.track, events);
    }
    if (raw.process !== undefined) {
      this.#legacyProcess(raw.process, events);
    }
    if (raw.thread !== undefined) {
      this.#legacyThread(raw.thread, sequence, events);
    }
    if (raw.event !== undefined) {
      this.#event(raw.event, timeUs, sequence, events);
    }
    return { offset, sequence: raw.sequence, timeUs, state, events };
  }

  #snapshot(bytes: Uint8Array, sequence: Sequence): void {
    const clocks: Array<{ id: number; value: number; incremental: boolean; unitNs: number }> = [];
    const reader = new Reader(bytes);
    while (!reader.done) {
      const tag = reader.varint();
      switch (tag >>> 3) {
        case CLOCK_SNAPSHOT.primaryTraceClock:
          this.#primary(reader.varint());
          break;
        case CLOCK_SNAPSHOT.clocks:
          clocks.push(readClock(reader.bytes()));
          break;
        default:
          reader.skip(tag & 7);
      }
    }
    const traceClock = this.#traceClockInUse();
    const builtin = clocks.filter(({ id }) => id < FIRST_SEQUENCE_CLOCK);
    const traceNs = builtin.find(({ id }) => id === traceClock)?.value;
    if (traceNs !== undefined) {
      for (const { id, value } of builtin) {
        this.#offsetsNs.set(id, traceNs - value);
      }
    }
    let atTraceNs = traceNs;
    for (const { id, value } of builtin) {
      const offsetNs = this.#offsetsNs.get(id);
      atTraceNs ??= offsetNs === undefined ? undefined : value + offsetNs;
    }
    for (const { id, value, incremental, unitNs } of clocks) {
      if (id < FIRST_SEQUENCE_CLOCK || id > LAST_SEQUENCE_CLOCK) {
        continue;
      }
      if (atTraceNs === undefined) {
        throw new Error(
          `the trace's sequence clock ${id} is given in a snapshot without the trace's clock`,
        );
      }
      sequence.clocks.set(id, {
        atSnapshot: value,
        traceNsAtSnapshot: atTraceNs,
        unitNs,
        incremental,
        last: value,
      });
    }
  }

  #primary(clockId: number): void {
    if (this.#traceClock !== undefined && this.#traceClock !== clockId) {
      throw new Error(
        `the trace's clock changed from ${this.#traceClock} to ${clockId} after it was used`,
      );
    }
    this.#traceClock = clockId;
  }

  /** The trace's clock, BOOTTIME when no snapshot has named one before it is first needed. */
  #traceClockInUse(): number {
    this.#traceClock ??= BOOTTIME;
    return this.#traceClock;
  }

  /** A packet's timestamp on the trace's clock, ns. */
  #traceNs(timestamp: number, clockId: number, sequence: Sequence): number {
    if (clockId >= FIRST_SEQUENCE_CLOCK && clockId <= LAST_SEQUENCE_CLOCK) {
      const clock = sequence.clocks.get(clockId);
      if (clock === undefined) {
        throw new Error(`the trace's sequence clock ${clockId} is used before its snapshot`);
      }
      const value = clock.incremental ? clock.last + timestamp : timestamp;
      clock.last = value;
      return clock.traceNsAtSnapshot + (value - clock.atSnapshot) * clock.unitNs;
    }
    if (clockId === this.#traceClockInUse()) {
      return timestamp;
    }
    const offsetNs = this.#offsetsNs.get(clockId);
    if (offsetNs === undefined) {
      throw new Error(`the trace's clock ${clockId} has no snapshot beside the trace's clock`);
    }
    return timestamp + offsetNs;
  }

  #defaults(bytes: Uint8Array, sequence: Sequence): void {
    const reader = new Reader(bytes);
    while (!reader.done) {
      const tag = reader.varint();
      switch (tag >>> 3) {
        case DEFAULTS.timestampClockId:
          sequence.defaultClockId = reader.varint();
          break;
        case DEFAULTS.trackEventDefaults: {
          const inner = new Reader(reader.bytes());
          while (!inner.done) {
            const innerTag = inner.varint();
            if (innerTag >>> 3 === TRACK_EVENT_DEFAULTS_TRACK_UUID) {
              sequence.defaultTrack = inner.key();
            } else {
              inner.skip(innerTag & 7);
            }
          }
          break;
        }
        default:
          reader.skip(tag & 7);
      }
    }
  }

  #interned(bytes: Uint8Array, sequence: Sequence): void {
    const reader = new Reader(bytes);
    while (!reader.done) {
      const tag = reader.varint();
      const table = internedTable(tag >>> 3, sequence);
      if (table === undefined) {
        reader.skip(tag & 7);
        continue;
      }
      const entry = new Reader(reader.bytes());
      let iid = -1;
      let value = "";
      while (!entry.done) {
        const entryTag = entry.varint();
        if (entryTag >>> 3 === INTERNED_ENTRY.iid) {
          iid = entry.varint();
        } else if (entryTag >>> 3 === INTERNED_ENTRY.value) {
          value = entry.string();
        } else {
          entry.skip(entryTag & 7);
        }
      }
      table.set(iid, value);
    }
  }

  #track(bytes: Uint8Array, events: ProtoTraceEvent[]): void {
    let uuid = "";
    let parent: string | undefined;
    let process: Described | undefined;
    let thread: Described | undefined;
    let chromeType: number | undefined;
    const reader = new Reader(bytes);
    while (!reader.done) {
      const tag = reader.varint();
      switch (tag >>> 3) {
        case TRACK.uuid:
          uuid = reader.key();
          break;
        case TRACK.parentUuid:
          parent = reader.key();
          break;
        case TRACK.process:
          process = readDescribed(reader.bytes(), "process");
          break;
        case TRACK.thread:
          thread = readDescribed(reader.bytes(), "thread");
          break;
        case TRACK.chromeProcess:
        case TRACK.chromeThread:
          // `ChromeProcessDescriptor.process_type` and `ChromeThreadDescriptor.thread_type`.
          chromeType = typeOf(reader.bytes(), CHROME_DESCRIPTOR_TYPE);
          break;
        default:
          reader.skip(tag & 7);
      }
    }
    if (thread !== undefined) {
      const parentTrack = parent === undefined ? undefined : this.#tracks.get(parent);
      const pid = thread.pid ?? (parentTrack?.kind === "process" ? parentTrack.pid : undefined);
      if (pid === undefined || thread.tid === undefined) {
        throw new Error(`the trace's thread track ${uuid} has no process or thread ID`);
      }
      const place = { pid, tid: thread.tid };
      this.#tracks.set(uuid, { kind: "thread", parent, place });
      this.#threadName(
        place,
        thread.name ?? THREAD_NAMES.get(chromeType ?? thread.type ?? -1),
        events,
      );
    } else if (process !== undefined) {
      if (process.pid === undefined) {
        throw new Error(`the trace's process track ${uuid} has no process ID`);
      }
      this.#tracks.set(uuid, { kind: "process", parent, pid: process.pid });
      this.#processName(
        process.pid,
        process.name ?? PROCESS_NAMES.get(chromeType ?? process.type ?? -1),
        events,
      );
    } else {
      this.#tracks.set(uuid, { kind: "other", parent });
    }
  }

  #legacyProcess(bytes: Uint8Array, events: ProtoTraceEvent[]): void {
    const process = readDescribed(bytes, "process");
    if (process.pid !== undefined) {
      this.#processName(process.pid, process.name ?? PROCESS_NAMES.get(process.type ?? -1), events);
    }
  }

  #legacyThread(bytes: Uint8Array, sequence: Sequence, events: ProtoTraceEvent[]): void {
    const thread = readDescribed(bytes, "thread");
    if (thread.pid === undefined || thread.tid === undefined) {
      return;
    }
    const place = { pid: thread.pid, tid: thread.tid };
    sequence.legacyThread = place;
    if (thread.referenceUs !== undefined) {
      sequence.legacyUs = thread.referenceUs;
    }
    this.#threadName(place, thread.name ?? THREAD_NAMES.get(thread.type ?? -1), events);
  }

  #processName(pid: number, name: string | undefined, events: ProtoTraceEvent[]): void {
    if (name === undefined || this.#processNames.get(pid) === name) {
      return;
    }
    this.#processNames.set(pid, name);
    events.push({
      name: "process_name",
      cat: "__metadata",
      ph: "M",
      pid,
      tid: 0,
      ts: 0,
      args: { name },
    });
  }

  #threadName({ pid, tid }: Place, name: string | undefined, events: ProtoTraceEvent[]): void {
    const key = `${pid}:${tid}`;
    if (name === undefined || this.#threadNames.get(key) === name) {
      return;
    }
    this.#threadNames.set(key, name);
    events.push({
      name: "thread_name",
      cat: "__metadata",
      ph: "M",
      pid,
      tid,
      ts: 0,
      args: { name },
    });
  }

  /** A sequence's thread: its default track's, or its legacy thread descriptor's. */
  #sequenceThread(sequence: Sequence): Place | undefined {
    const track =
      sequence.defaultTrack === undefined ? undefined : this.#tracks.get(sequence.defaultTrack);
    return track?.kind === "thread" ? track.place : sequence.legacyThread;
  }

  /**
   * Where a typed event lies: on a thread track, that thread; on another track, the nearest thread
   * up its parents, else the nearest process's, with the sequence's thread if it is that
   * process's. With no track, the sequence's thread; on the global track (UUID 0), the sequence's
   * thread, but not as its own track.
   */
  #placeOf(trackKey: string | undefined, sequence: Sequence): Placed {
    const thread = this.#sequenceThread(sequence);
    if (trackKey === undefined || trackKey === "0") {
      return thread === undefined ? "no thread" : { ...thread, onThread: trackKey === undefined };
    }
    let key: string | undefined = trackKey;
    for (let depth = 0; key !== undefined && depth < MAX_TRACK_DEPTH; depth += 1) {
      const track = this.#tracks.get(key);
      if (track === undefined) {
        return "undescribed";
      }
      switch (track.kind) {
        case "thread":
          return { ...track.place, onThread: depth === 0 };
        case "process":
          return {
            pid: track.pid,
            tid: thread?.pid === track.pid ? thread.tid : 0,
            onThread: false,
          };
        case "other":
          key = track.parent;
          break;
      }
    }
    return thread === undefined ? "no thread" : { ...thread, onThread: false };
  }

  #event(
    bytes: Uint8Array,
    packetUs: number | null,
    sequence: Sequence,
    events: ProtoTraceEvent[],
  ): void {
    const raw = readEvent(bytes);
    const ts = this.#eventUs(raw, packetUs, sequence);
    const trackKey = raw.trackUuid ?? sequence.defaultTrack;
    if (raw.type === TYPE_SLICE_END) {
      this.#end(trackKey ?? "", ts, events);
      return;
    }
    const legacy = raw.legacy === undefined ? undefined : readLegacy(raw.legacy);
    const nameIid = raw.nameIid ?? legacy?.nameIid;
    const name = raw.name ?? (nameIid === undefined ? undefined : sequence.names.get(nameIid));
    if (name === undefined) {
      if (nameIid !== undefined) {
        throw new Error(`the trace's event name ${nameIid} is not interned on its sequence`);
      }
      return;
    }
    const cat = categoriesOf(raw, sequence);
    switch (raw.type) {
      case TYPE_UNSPECIFIED:
        if (legacy === undefined) {
          if (readByReducer(name, cat)) {
            throw unknownEncoding(name, "neither a typed nor a legacy event");
          }
          return;
        }
        this.#legacy(legacy, name, cat, ts, this.#args(raw, sequence, name), sequence, events);
        return;
      case TYPE_SLICE_BEGIN:
      case TYPE_INSTANT:
        break;
      case TYPE_COUNTER:
        // The reducer reads no counter.
        if (readByReducer(name, cat)) {
          throw unknownEncoding(name, "a counter");
        }
        return;
      default:
        if (readByReducer(name, cat)) {
          throw unknownEncoding(name, `an event of type ${raw.type}`);
        }
        return;
    }
    const place = this.#placeOf(raw.trackUuid ?? sequence.defaultTrack, sequence);
    if (typeof place === "string") {
      if (readByReducer(name, cat)) {
        throw unknownEncoding(name, `an event on track ${trackKey ?? "(none)"}: ${place}`);
      }
      return;
    }
    const { pid, tid, onThread } = place;
    const key = trackKey ?? "";
    const args = this.#args(raw, sequence, name);
    if (raw.type === TYPE_INSTANT) {
      events.push(
        onThread
          ? { name, cat, ph: "I", pid, tid, ts, args }
          : { name, cat, ph: "n", pid, tid, ts, id: this.#asyncId(key), args },
      );
      return;
    }
    const id = onThread ? "" : this.#asyncId(key);
    let stack = this.#open.get(key);
    if (stack === undefined) {
      stack = [];
      this.#open.set(key, stack);
    }
    stack.push({ name, cat, pid, tid, onThread, ts, id, args });
    if (!onThread) {
      events.push({ name, cat, ph: "b", pid, tid, ts, id, args });
    }
  }

  /** An event's time: its packet's, else its own µs fields (the legacy encoding), µs. */
  #eventUs(raw: RawEvent, packetUs: number | null, sequence: Sequence): number {
    if (packetUs !== null) {
      return packetUs;
    }
    if (raw.absoluteUs !== undefined) {
      sequence.legacyUs = raw.absoluteUs;
    } else if (raw.deltaUs !== undefined) {
      sequence.legacyUs += raw.deltaUs;
    }
    return sequence.legacyUs;
  }

  /** A new ID for a slice or instant on a track that is not a thread's own. */
  #asyncId(trackKey: string): string {
    const n = (this.#begun.get(trackKey) ?? 0) + 1;
    this.#begun.set(trackKey, n);
    return `${trackKey}.${n}`;
  }

  #end(trackKey: string, ts: number, events: ProtoTraceEvent[]): void {
    const slice = this.#open.get(trackKey)?.pop();
    if (slice === undefined) {
      // Its begin came before the trace did.
      return;
    }
    const { name, cat, pid, tid, id } = slice;
    events.push(
      slice.onThread
        ? { name, cat, ph: "X", pid, tid, ts: slice.ts, dur: ts - slice.ts, args: slice.args }
        : { name, cat, ph: "e", pid, tid, ts, id, args: {} },
    );
  }

  #legacy(
    legacy: Legacy,
    name: string,
    cat: string,
    ts: number,
    args: Readonly<Record<string, unknown>>,
    sequence: Sequence,
    events: ProtoTraceEvent[],
  ): void {
    const thread = this.#sequenceThread(sequence);
    const pid = legacy.pid ?? thread?.pid;
    const tid = legacy.tid ?? thread?.tid;
    if (pid === undefined || tid === undefined || legacy.phase === undefined) {
      if (readByReducer(name, cat)) {
        throw unknownEncoding(name, "a legacy event without a phase or a thread");
      }
      return;
    }
    const ph = String.fromCharCode(legacy.phase);
    switch (ph) {
      case "C":
        // The reducer reads no counter.
        return;
      case "X":
        if (legacy.durationUs === undefined) {
          throw unknownEncoding(name, "a complete legacy event without a duration");
        }
        events.push({ name, cat, ph, pid, tid, ts, dur: legacy.durationUs, args });
        return;
      case "B": {
        const key = `legacy:${pid}:${tid}`;
        const stack = this.#open.get(key) ?? [];
        stack.push({ name, cat, pid, tid, onThread: true, ts, id: "", args });
        this.#open.set(key, stack);
        return;
      }
      case "E":
        this.#end(`legacy:${pid}:${tid}`, ts, events);
        return;
      default:
        events.push(
          legacy.id === undefined
            ? { name, cat, ph, pid, tid, ts, args }
            : { name, cat, ph, pid, tid, ts, id: legacy.id, args },
        );
    }
  }

  #args(raw: RawEvent, sequence: Sequence, name: string): Readonly<Record<string, unknown>> {
    const args: Record<string, unknown> = {};
    for (const bytes of raw.annotations) {
      const { name: key, value } = annotation(bytes, sequence);
      if (key !== undefined) {
        args[key] = value;
      }
    }
    if (raw.frameReporter !== undefined) {
      args["frame_reporter"] = frameReporter(raw.frameReporter, name);
    }
    return args;
  }
}

/** How far up its parents a track's process or thread is looked for. */
const MAX_TRACK_DEPTH = 64;

/** The interning table of an `InternedData` field, or `undefined` for one the decoder skips. */
function internedTable(field: number, sequence: Sequence): Map<number, string> | undefined {
  switch (field) {
    case INTERNED.eventCategories:
      return sequence.categories;
    case INTERNED.eventNames:
      return sequence.names;
    case INTERNED.debugAnnotationNames:
      return sequence.annotationNames;
    case INTERNED.debugAnnotationStringValues:
      return sequence.annotationStrings;
    default:
      return undefined;
  }
}

/** An event's categories, interned and inline, joined by commas as Chromium's JSON writes them. */
function categoriesOf(raw: RawEvent, sequence: Sequence): string {
  const names = raw.categoryIids.map((iid) => {
    const category = sequence.categories.get(iid);
    if (category === undefined) {
      throw new Error(`the trace's event category ${iid} is not interned on its sequence`);
    }
    return category;
  });
  return [...names, ...raw.categories].join(",");
}

/** A `ClockSnapshot.Clock`. */
function readClock(bytes: Uint8Array): {
  id: number;
  value: number;
  incremental: boolean;
  unitNs: number;
} {
  const clock = { id: 0, value: 0, incremental: false, unitNs: 1 };
  const reader = new Reader(bytes);
  while (!reader.done) {
    const tag = reader.varint();
    switch (tag >>> 3) {
      case CLOCK.clockId:
        clock.id = reader.varint();
        break;
      case CLOCK.timestamp:
        clock.value = reader.varint();
        break;
      case CLOCK.isIncremental:
        clock.incremental = reader.varint() !== 0;
        break;
      case CLOCK.unitMultiplierNs:
        clock.unitNs = reader.varint();
        break;
      default:
        reader.skip(tag & 7);
    }
  }
  return clock;
}

/** A process or thread descriptor's fields. */
interface Described {
  pid: number | undefined;
  tid: number | undefined;
  name: string | undefined;
  /** Its Chrome process or thread type. */
  type: number | undefined;
  /** A thread's reference timestamp for the legacy events' µs fields. */
  referenceUs: number | undefined;
}

function readDescribed(bytes: Uint8Array, kind: "process" | "thread"): Described {
  const described: Described = {
    pid: undefined,
    tid: undefined,
    name: undefined,
    type: undefined,
    referenceUs: undefined,
  };
  const reader = new Reader(bytes);
  while (!reader.done) {
    const tag = reader.varint();
    const field = tag >>> 3;
    if (field === (kind === "process" ? PROCESS.pid : THREAD.pid)) {
      described.pid = reader.signed();
    } else if (kind === "thread" && field === THREAD.tid) {
      described.tid = reader.signed();
    } else if (field === (kind === "process" ? PROCESS.processName : THREAD.threadName)) {
      described.name = reader.string();
    } else if (
      field === (kind === "process" ? PROCESS.chromeProcessType : THREAD.chromeThreadType)
    ) {
      described.type = reader.signed();
    } else if (kind === "thread" && field === THREAD.referenceTimestampUs) {
      described.referenceUs = reader.signed();
    } else {
      reader.skip(tag & 7);
    }
  }
  return described;
}

/** A Chrome descriptor's type field. */
function typeOf(bytes: Uint8Array, number: number): number | undefined {
  let type: number | undefined;
  const reader = new Reader(bytes);
  while (!reader.done) {
    const tag = reader.varint();
    if (tag >>> 3 === number) {
      type = reader.signed();
    } else {
      reader.skip(tag & 7);
    }
  }
  return type;
}

/** A legacy JSON value parsed, or its text when it does not parse. */
function parsedJson(text: string): unknown {
  try {
    const value: unknown = JSON.parse(text);
    return value;
  } catch {
    return text;
  }
}

/** A debug annotation's name and value, as Chromium's JSON writes it in `args`. */
function annotation(
  bytes: Uint8Array,
  sequence: Sequence,
): { readonly name: string | undefined; readonly value: unknown } {
  let name: string | undefined;
  let value: unknown = null;
  let entries: Record<string, unknown> | undefined;
  let items: unknown[] | undefined;
  const reader = new Reader(bytes);
  while (!reader.done) {
    const tag = reader.varint();
    switch (tag >>> 3) {
      case ANNOTATION.nameIid: {
        const iid = reader.varint();
        name = sequence.annotationNames.get(iid);
        if (name === undefined) {
          throw new Error(
            `the trace's debug-annotation name ${iid} is not interned on its sequence`,
          );
        }
        break;
      }
      case ANNOTATION.name:
        name = reader.string();
        break;
      case ANNOTATION.boolValue:
        value = reader.varint() !== 0;
        break;
      case ANNOTATION.uintValue:
        value = reader.varint();
        break;
      case ANNOTATION.intValue:
        value = reader.signed();
        break;
      case ANNOTATION.doubleValue:
        value = reader.double();
        break;
      case ANNOTATION.stringValue:
        value = reader.string();
        break;
      case ANNOTATION.pointerValue:
        value = hex(reader.key());
        break;
      case ANNOTATION.nestedValue:
        value = nested(reader.bytes());
        break;
      case ANNOTATION.legacyJsonValue:
        value = parsedJson(reader.string());
        break;
      case ANNOTATION.stringValueIid: {
        const iid = reader.varint();
        value = sequence.annotationStrings.get(iid);
        if (value === undefined) {
          throw new Error(
            `the trace's debug-annotation string ${iid} is not interned on its sequence`,
          );
        }
        break;
      }
      case ANNOTATION.dictEntries: {
        const entry = annotation(reader.bytes(), sequence);
        entries ??= {};
        if (entry.name !== undefined) {
          entries[entry.name] = entry.value;
        }
        break;
      }
      case ANNOTATION.arrayValues:
        items ??= [];
        items.push(annotation(reader.bytes(), sequence).value);
        break;
      default:
        reader.skip(tag & 7);
    }
  }
  return { name, value: entries ?? items ?? value };
}

/** A `NestedValue` as JSON: a dictionary, an array or a scalar. */
function nested(bytes: Uint8Array): unknown {
  let type = 0;
  const keys: string[] = [];
  const values: unknown[] = [];
  const items: unknown[] = [];
  let scalar: unknown = null;
  const reader = new Reader(bytes);
  while (!reader.done) {
    const tag = reader.varint();
    switch (tag >>> 3) {
      case NESTED.nestedType:
        type = reader.varint();
        break;
      case NESTED.dictKeys:
        keys.push(reader.string());
        break;
      case NESTED.dictValues:
        values.push(nested(reader.bytes()));
        break;
      case NESTED.arrayValues:
        items.push(nested(reader.bytes()));
        break;
      case NESTED.intValue:
        scalar = reader.signed();
        break;
      case NESTED.doubleValue:
        scalar = reader.double();
        break;
      case NESTED.boolValue:
        scalar = reader.varint() !== 0;
        break;
      case NESTED.stringValue:
        scalar = reader.string();
        break;
      default:
        reader.skip(tag & 7);
    }
  }
  if (type === NESTED_DICT) {
    return Object.fromEntries(keys.map((key, i) => [key, values[i] ?? null]));
  }
  return type === NESTED_ARRAY ? items : scalar;
}

/** The frame reporter's state and compositor, as Chromium's JSON writes them. */
function frameReporter(bytes: Uint8Array, name: string): Readonly<Record<string, unknown>> {
  let state = 0;
  let host: number | undefined;
  const reader = new Reader(bytes);
  while (!reader.done) {
    const tag = reader.varint();
    switch (tag >>> 3) {
      case FRAME_REPORTER.state:
        state = reader.varint();
        break;
      case FRAME_REPORTER.layerTreeHostId:
        host = reader.varint();
        break;
      default:
        reader.skip(tag & 7);
    }
  }
  const stateName = FRAME_STATES[state];
  if (stateName === undefined) {
    throw unknownEncoding(name, `a frame reporter's state ${state}`);
  }
  return host === undefined ? { state: stateName } : { state: stateName, layer_tree_host_id: host };
}

/** One packet's place in a trace: its tag's offset, and its bytes with and without its framing. */
export interface PacketBytes {
  readonly offset: number;
  /** The packet's message. */
  readonly bytes: Uint8Array;
  /** The packet as the trace holds it: tag, length and message. */
  readonly framed: Uint8Array;
}

/**
 * The packets of a whole trace in memory, in order.
 *
 * @throws Error naming its offset when the trace ends inside a packet, or holds bytes that begin no
 * field.
 */
export function* tracePackets(trace: Uint8Array): Generator<PacketBytes, void, undefined> {
  const framing = new Framing();
  yield* framing.push(trace, true);
}

/** Splits a trace's bytes, which may arrive in pieces, into packets. */
class Framing {
  #pending: Uint8Array = new Uint8Array(0);
  /** The trace offset of `#pending`'s first byte. */
  #offset = 0;

  /**
   * The packets completed by `chunk`. With `last`, a packet left incomplete is an error.
   *
   * @throws Error naming its offset when the trace ends inside a packet, or holds bytes that begin
   * no field.
   */
  *push(chunk: Uint8Array, last: boolean): Generator<PacketBytes, void, undefined> {
    let bytes: Uint8Array;
    if (this.#pending.length === 0) {
      bytes = chunk;
    } else {
      bytes = new Uint8Array(this.#pending.length + chunk.length);
      bytes.set(this.#pending, 0);
      bytes.set(chunk, this.#pending.length);
    }
    let pos = 0;
    while (pos < bytes.length) {
      const reader = new Reader(bytes, pos);
      let field: number;
      let length: number;
      try {
        const tag = reader.varint();
        field = tag >>> 3;
        const wireType = tag & 7;
        if (wireType !== LENGTH_DELIMITED) {
          // `Trace` holds only packets; skip anything else whole.
          reader.skip(wireType);
          pos = reader.pos;
          continue;
        }
        length = reader.varint();
      } catch (error: unknown) {
        if (error instanceof ShortRead) {
          // The packet's tag or length runs into the bytes not yet read.
          break;
        }
        if (error instanceof WireError) {
          throw new Error(
            `the trace at byte ${this.#offset + pos} is not a packet: ${error.message}`,
            {
              cause: error,
            },
          );
        }
        throw error;
      }
      const start = reader.pos;
      if (start + length > bytes.length) {
        break;
      }
      if (field === TRACE_PACKET) {
        yield {
          offset: this.#offset + pos,
          bytes: bytes.subarray(start, start + length),
          framed: bytes.subarray(pos, start + length),
        };
      }
      pos = start + length;
    }
    // Copied, since the chunk's buffer may be reused by the stream.
    this.#pending = bytes.slice(pos);
    this.#offset += pos;
    if (last && this.#pending.length > 0) {
      throw new Error(`the trace ends inside a packet at byte ${this.#offset}`);
    }
  }
}

/**
 * The packets of a trace in memory, decoded in order.
 *
 * @throws Error as {@link tracePackets} and {@link ProtoTraceDecoder.decode} do.
 */
export function* decodeProtoPackets(trace: Uint8Array): Generator<DecodedPacket, void, undefined> {
  const decoder = new ProtoTraceDecoder();
  for (const { offset, bytes } of tracePackets(trace)) {
    yield decoder.decode(bytes, offset);
  }
}

/**
 * The events of a trace in memory, in order.
 *
 * @throws Error as {@link tracePackets} and {@link ProtoTraceDecoder.decode} do.
 */
export function* decodeProtoTrace(trace: Uint8Array): Generator<ProtoTraceEvent, void, undefined> {
  for (const packet of decodeProtoPackets(trace)) {
    yield* packet.events;
  }
}

/** Options of {@link readProtoTraceEvents}. */
export interface ProtoReadOptions {
  /** How many bytes to read at a time; {@link DEFAULT_PROTO_READ_BYTES} by default. */
  readonly readBytes?: number;
}

/**
 * The events of a Perfetto protobuf trace file, read a piece at a time: those of each piece's
 * packets, in order.
 *
 * @remarks
 * One array a piece rather than one `yield` an event, since a window holds millions of events and
 * each asynchronous step costs more than reducing one.
 * @throws Error naming the file and the packet's offset when the trace ends inside a packet, a
 * packet is not one, or an event the reducer reads arrives in an encoding the decoder does not know.
 */
export async function* readProtoTraceBatches(
  path: string,
  options: ProtoReadOptions = {},
): AsyncGenerator<ReadonlyArray<ProtoTraceEvent>, void, undefined> {
  const input = createReadStream(path, {
    highWaterMark: options.readBytes ?? DEFAULT_PROTO_READ_BYTES,
  });
  const framing = new Framing();
  const decoder = new ProtoTraceDecoder();
  const decodeAll = (chunk: Uint8Array, last: boolean): ProtoTraceEvent[] => {
    const events: ProtoTraceEvent[] = [];
    for (const { offset, bytes } of framing.push(chunk, last)) {
      for (const event of decoder.decode(bytes, offset).events) {
        events.push(event);
      }
    }
    return events;
  };
  try {
    for await (const chunk of input) {
      if (!(chunk instanceof Uint8Array)) {
        throw new Error("the file was read as text");
      }
      yield decodeAll(chunk, false);
    }
    yield decodeAll(new Uint8Array(0), true);
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`${path}: ${message}`, { cause: error });
  } finally {
    input.destroy();
  }
}

/**
 * The events of a Perfetto protobuf trace file, read a piece at a time, in order.
 *
 * @throws Error as {@link readProtoTraceBatches} does.
 */
export async function* readProtoTraceEvents(
  path: string,
  options: ProtoReadOptions = {},
): AsyncGenerator<ProtoTraceEvent, void, undefined> {
  for await (const events of readProtoTraceBatches(path, options)) {
    yield* events;
  }
}

/**
 * A trace cut at `untilUs`, and from `fromUs` if given: each sequence's packets from its first
 * (with `fromUs`, from its first that clears its incremental state at or after `fromUs`) up to, and
 * not including, its first packet later than `untilUs` on the trace's clock.
 *
 * @remarks
 * A sequence's incremental state (its interning, defaults and incremental clock) builds up packet
 * by packet from a packet that clears it, which also gives the clocks and is followed by the
 * sequence's track descriptors again. So a stretch of each sequence that starts at such a packet
 * keeps it whole, and the result is a valid trace. With `fromUs`, a sequence's packets that stand
 * alone (neither clearing nor needing incremental state: the tracing service's clock snapshots
 * among them) are kept from `fromUs`, or wherever they have no timestamp, and a sequence that
 * clears its state at no time from `fromUs` keeps none of the packets that need it. Used to cut
 * the test's fixtures.
 * @param untilUs - On the trace's clock, µs.
 * @param fromUs - On the trace's clock, µs; the trace's start by default.
 * @throws Error as {@link tracePackets} and {@link ProtoTraceDecoder.decode} do.
 */
export function trimProtoTrace(trace: Uint8Array, untilUs: number, fromUs?: number): Uint8Array {
  const decoder = new ProtoTraceDecoder();
  const ended = new Set<number>();
  const started = new Set<number>();
  const kept: Uint8Array[] = [];
  let length = 0;
  for (const { offset, bytes, framed } of tracePackets(trace)) {
    const { sequence, timeUs, state } = decoder.decode(bytes, offset);
    if (ended.has(sequence)) {
      continue;
    }
    if (timeUs !== null && timeUs > untilUs) {
      ended.add(sequence);
      continue;
    }
    if (fromUs !== undefined && !started.has(sequence)) {
      const fromOn = timeUs === null || timeUs >= fromUs;
      if (state === "clears" && fromOn) {
        started.add(sequence);
      } else if (state !== "none" || !fromOn) {
        continue;
      }
    }
    kept.push(framed);
    length += framed.length;
  }
  const out = new Uint8Array(length);
  let at = 0;
  for (const framed of kept) {
    out.set(framed, at);
    at += framed.length;
  }
  return out;
}
