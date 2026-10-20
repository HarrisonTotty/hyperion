/**
 * The client's coarse-field store: which bodies' fields the views want, R09's `surface_field`
 * asked and asked again for what is new, each payload block forwarded to every height worker as it
 * completes, and each body's coverage (plan R10, T7; Design note 16).
 *
 * @remarks
 * A body's field is wanted once it is a view's camera's local body or within its hand-over range
 * (Design note 13) of any view's camera, and released once every view has left twice that range
 * ({@link fieldDemand}). The store asks `surface_field` with the revision it holds as
 * `have_revision`, `null` for none, and the server answers the cells surveyed since: a `ready`
 * answer with its header, revision and coverage, after its payload's binary chunks, or
 * `not_modelled`. The render thread never decodes the payload: it cuts R03's frames into R09's
 * self-contained blocks by each block's length, the `u32` at offset 6 of its 10-byte frame
 * ({@link BlockSplitter}), and posts each block to every worker the moment it completes, through
 * `requestBulk`'s per-chunk option, without waiting for the answer. A body's first block ever is
 * its `field_init`, R09's block 0, which carries the whole header; every later block, a later
 * payload's block 0 included, is a `field_chunk`.
 *
 * A rise in a body's revision on the scene topic ({@link CoarseFieldStore.noteSurfaceRevisions})
 * asks again with the revision held, so only the new cells travel: R09.T19.b's delta, whose
 * coverage names its own cells alone, new ones or ones surveyed again more finely, merged here
 * into what is held. A lost link keeps everything received, in the workers and here, and marks the
 * coverage stale; when the link returns, every body whose field is kept is asked again with the
 * revision it holds, since a revision announced while the link was down is not replayed (R03
 * Design note 4). A `have_revision` the server no longer knows (`bad_request` naming it) drops the
 * field and asks for it whole. A server too busy to answer is asked again later, waiting longer
 * each time ({@link FIELD_RETRY_DELAY_MS}). Any other refusal is held, and not asked again until the
 * body is wanted afresh; a first payload that fails leaves nothing in the workers.
 */

import type {
  BodyIdHex,
  BulkChunk,
  LiquidBasisDto,
  LiquidDto,
  PaletteEntryDto,
  PaletteRoleDto,
  RecordSectionDto,
  RequestClient,
  RequestFailure,
  ResponseFor,
  SurfaceHeaderDto,
  SurfaceRevisionDto,
  UniverseIdHex,
} from "@hyperion/protocol";

import { CoverageView, isFieldLevel } from "./coverage";
import type { HeightWorkerPool } from "./workers/pool";

/** The largest payload block, bytes, its header included: 1 MiB (R09's `wire::MAX_BLOCK_BYTES`). */
export const MAX_BLOCK_BYTES = 1 << 20;

/** The first four bytes of every block, `HYSF` (R09's `wire::BLOCK_MAGIC`). */
export const BLOCK_MAGIC: readonly [number, number, number, number] = [0x48, 0x59, 0x53, 0x46];

/**
 * The bytes of the frame every block format keeps: the magic, the format (`u16`) and the block's
 * length (`u32` at offset 6), little-endian (R09's `wire` module documentation, `FRAME_BYTES`).
 */
export const BLOCK_FRAME_BYTES = 10;

/** The offset of a block's length in its frame (R09's `wire` module documentation). */
const BLOCK_LENGTH_OFFSET = 6;

/** The fewest bytes a block has: its fixed header (R09's `wire::BLOCK_HEADER_BYTES`). */
export const BLOCK_HEADER_BYTES = 33;

/** What a chunk completed: the blocks, each its own buffer, or why the payload cannot be split. */
export type BlockSplit =
  | { readonly ok: true; readonly blocks: ReadonlyArray<ArrayBuffer> }
  | { readonly ok: false; readonly message: string };

/**
 * Cuts a payload's bytes, as its chunks arrive, into R09's blocks, without decoding them.
 *
 * @remarks
 * Only each block's frame is read: its magic, and its length, which must be at least the fixed
 * header's and at most {@link MAX_BLOCK_BYTES}. Each block is copied into a buffer of its own,
 * since a block spans R03's 256 KiB frames, and the worker decodes it. Once a frame is refused the
 * splitter refuses everything after it, with the same message.
 */
export class BlockSplitter {
  readonly #frame = new Uint8Array(BLOCK_FRAME_BYTES);
  #frameFill = 0;
  #block: Uint8Array | null = null;
  /** The buffer {@link BlockSplitter.#block} views, handed over whole once the block is. */
  #blockBuffer = new ArrayBuffer(0);
  #blockFill = 0;
  #refused: string | null = null;
  #offset = 0;

  /** Whether the bytes so far end at a block's end, as a whole payload does. */
  get atBoundary(): boolean {
    return this.#refused === null && this.#block === null && this.#frameFill === 0;
  }

  /** Takes the next chunk of the payload, returning the blocks it completed, in order. */
  push(chunk: Uint8Array): BlockSplit {
    if (this.#refused !== null) {
      return { ok: false, message: this.#refused };
    }
    const blocks: ArrayBuffer[] = [];
    let at = 0;
    while (at < chunk.length) {
      if (this.#block === null) {
        const take = Math.min(BLOCK_FRAME_BYTES - this.#frameFill, chunk.length - at);
        this.#frame.set(chunk.subarray(at, at + take), this.#frameFill);
        this.#frameFill += take;
        at += take;
        if (this.#frameFill < BLOCK_FRAME_BYTES) {
          break;
        }
        const refusal = this.#startBlock();
        if (refusal !== null) {
          this.#refused = refusal;
          return { ok: false, message: refusal };
        }
        continue;
      }
      const block = this.#block;
      const take = Math.min(block.length - this.#blockFill, chunk.length - at);
      block.set(chunk.subarray(at, at + take), this.#blockFill);
      this.#blockFill += take;
      at += take;
      if (this.#blockFill === block.length) {
        blocks.push(this.#blockBuffer);
        this.#offset += block.length;
        this.#block = null;
        this.#blockFill = 0;
      }
    }
    return { ok: true, blocks };
  }

  /** Starts the block whose frame is held, or says why its frame is refused. */
  #startBlock(): string | null {
    const view = new DataView(this.#frame.buffer);
    if (BLOCK_MAGIC.some((byte, at) => view.getUint8(at) !== byte)) {
      return `the payload's block at byte ${this.#offset} does not start with HYSF`;
    }
    const length = view.getUint32(BLOCK_LENGTH_OFFSET, true);
    if (length < BLOCK_HEADER_BYTES || length > MAX_BLOCK_BYTES) {
      return (
        `the payload's block at byte ${this.#offset} states ${length} bytes, outside ` +
        `${BLOCK_HEADER_BYTES} to ${MAX_BLOCK_BYTES}`
      );
    }
    this.#blockBuffer = new ArrayBuffer(length);
    const block = new Uint8Array(this.#blockBuffer);
    block.set(this.#frame);
    this.#block = block;
    this.#blockFill = BLOCK_FRAME_BYTES;
    this.#frameFill = 0;
    return null;
  }
}

/** Why a body's field is wanted (Design note 16). */
export type FieldWantReason = "local-body" | "in-range";

/** A body one view may want the field of, as that view's camera sees it now. */
export interface FieldCandidate {
  readonly body: BodyIdHex;
  /** Whether it is the view's camera's local body. */
  readonly local: boolean;
  /**
   * The view's camera's distance to the body's limb over the body's hand-over range in that view's
   * style, dimensionless (`handover.ts`' `handoverRangeRatio`, R10.T10.e): the range is where the
   * body's relief subtends the style's τ pixels (Design note 13: 1 px high, 2 px low, 4 px in the
   * wireframe), so each view has its own.
   */
  readonly rangeRatio: number;
}

/** What the views' cameras ask of the store: the bodies to want, with why, and those to release. */
export interface FieldDemand {
  readonly want: ReadonlyArray<{ readonly body: BodyIdHex; readonly reason: FieldWantReason }>;
  readonly release: ReadonlyArray<BodyIdHex>;
}

/** How many hand-over ranges every view must leave before a body's field is released. */
export const FIELD_RELEASE_RANGE_FACTOR = 2;

/**
 * The bodies the views want the fields of, and those to release, with Design note 16's
 * hysteresis.
 *
 * @remarks
 * `candidates` holds a body once for each view that sees it. A body is wanted as some view's
 * camera's local body, or once some view's camera is within the body's hand-over range in that
 * view's style (a {@link FieldCandidate.rangeRatio} of 1 or less). A body wanted already is kept
 * until every view has left {@link FIELD_RELEASE_RANGE_FACTOR} times its own range, so a camera
 * moving to and fro inside the band asks nothing. A wanted body that no view sees has left the
 * scene and is released. Each body is named once in the answer, in the order first seen.
 *
 * @param wanted - The bodies wanted now.
 */
export function fieldDemand(
  candidates: ReadonlyArray<FieldCandidate>,
  wanted: ReadonlySet<BodyIdHex>,
): FieldDemand {
  // Each body's views folded: local in any, and the least ratio of all.
  const bodies = new Map<BodyIdHex, { local: boolean; rangeRatio: number }>();
  for (const { body, local, rangeRatio } of candidates) {
    const seen = bodies.get(body);
    if (seen === undefined) {
      bodies.set(body, { local, rangeRatio });
    } else {
      seen.local ||= local;
      seen.rangeRatio = Math.min(seen.rangeRatio, rangeRatio);
    }
  }
  const want: Array<{ readonly body: BodyIdHex; readonly reason: FieldWantReason }> = [];
  const release: BodyIdHex[] = [];
  for (const [body, { local, rangeRatio }] of bodies) {
    if (local) {
      want.push({ body, reason: "local-body" });
    } else if (rangeRatio <= 1) {
      want.push({ body, reason: "in-range" });
    } else if (wanted.has(body) && !(rangeRatio <= FIELD_RELEASE_RANGE_FACTOR)) {
      release.push(body);
    }
  }
  for (const body of wanted) {
    if (!bodies.has(body)) {
      release.push(body);
    }
  }
  return { want, release };
}

/**
 * The number of spectral bins of a palette entry's liquid record: R08's fifteen bake bins, 380 to
 * 760 nm (`hyperion-protocol`'s `SURFACE_LIQUID_BINS`; decision-r11-palette-liquids §1.2).
 */
export const LIQUID_BINS: LiquidDto["refractive_index"]["length"] = 15;

/**
 * The bases a liquid record may name, as `hyperion-surface`'s `LiquidBasis` codes them: a record,
 * so that a basis the protocol gains fails to compile here.
 */
const LIQUID_BASES: Readonly<Record<LiquidBasisDto, true>> = {
  measured: true,
  derived: true,
  stand_in: true,
};

/** Whether `value` is finite and above 0. */
function positive(value: number): boolean {
  return Number.isFinite(value) && value > 0;
}

/** Whether `value` is finite and at least +0: a −0 is refused, as R09's codec refuses it. */
function atLeastZero(value: number): boolean {
  return Number.isFinite(value) && value >= 0 && !Object.is(value, -0);
}

/** Whether `value` is a fraction, finite from +0 to 1. */
function isFraction(value: number): boolean {
  return atLeastZero(value) && value <= 1;
}

/** Whether `values` holds `count` values, each passing `test`. */
function holds(
  values: ReadonlyArray<number>,
  count: number,
  test: (value: number) => boolean,
): boolean {
  return values.length === count && values.every(test);
}

/** Whether `bins` holds one value a liquid bin, each passing `test`. */
function byBin(bins: ReadonlyArray<number>, test: (value: number) => boolean): boolean {
  return holds(bins, LIQUID_BINS, test);
}

/** Whether a by-bin member that may be absent is absent, `null`, or passes `test` in every bin. */
function absentOrByBin(
  bins: ReadonlyArray<number> | null | undefined,
  test: (value: number) => boolean,
): boolean {
  return bins === undefined || bins === null || byBin(bins, test);
}

/**
 * Whether an entry of each role must carry a liquid record, may, or must not
 * (decision-r11-palette-liquids §1.3): a sea's or a lake's must, a crust's may where the body
 * melts it, and an ice, a cover or a deposit never does.
 */
const LIQUID_RULES: Readonly<Record<PaletteRoleDto, "required" | "allowed" | "refused">> = {
  primary_crust: "allowed",
  secondary_crust: "allowed",
  tertiary_crust: "allowed",
  province: "allowed",
  ice: "refused",
  liquid: "required",
  cover: "refused",
  deposit: "refused",
  bulk: "allowed",
};

/** Why a liquid record cannot be taken, by its first member out of range, or `null`. */
function liquidFailure(record: LiquidDto): string | null {
  const bins = `${LIQUID_BINS} finite values`;
  if (!positive(record.temperature_k)) {
    return `temperature, ${record.temperature_k} K, is not positive`;
  }
  if (!positive(record.density_kg_m3)) {
    return `density, ${record.density_kg_m3} kg/m³, is not positive`;
  }
  if (!Object.hasOwn(LIQUID_BASES, record.basis)) {
    return `basis, "${record.basis}", is not one this client knows`;
  }
  if (!byBin(record.refractive_index, positive)) {
    return `refractive index is not ${bins} above 0`;
  }
  if (!absentOrByBin(record.absorption_per_m, atLeastZero)) {
    return `absorption is not ${bins} of at least 0`;
  }
  if (!absentOrByBin(record.scattering_per_m, atLeastZero)) {
    return `scattering is not ${bins} of at least 0`;
  }
  const tension = record.surface_tension_n_per_m;
  if (tension !== undefined && tension !== null && !positive(tension)) {
    return `surface tension, ${tension} N/m, is not positive`;
  }
  const viscosity = record.viscosity_pa_s;
  if (viscosity !== undefined && viscosity !== null && !positive(viscosity)) {
    return `viscosity, ${viscosity} Pa s, is not positive`;
  }
  return null;
}

/** Whether a member that may be absent is present. */
function present(value: number | null | undefined): value is number {
  return value !== undefined && value !== null;
}

/** Why a palette entry's liquid record, or its absence, does not suit its role, or `null`. */
function recordFailure(entry: PaletteEntryDto): string | null {
  const rule = LIQUID_RULES[entry.role];
  if (entry.liquid === null) {
    return rule === "required" ? "has no liquid record" : null;
  }
  if (rule === "refused") {
    return "has a liquid record, which no ice, cover or deposit carries";
  }
  const failure = liquidFailure(entry.liquid);
  return failure === null ? null : `has a liquid record whose ${failure}`;
}

/**
 * Why a palette entry's FeO mass fraction cannot be taken, or `null`: one on a liquid, which only
 * a solid carries, or one that is not finite from +0 to 1 (decision-p14-t51d-weathering §1.6).
 */
function feoFailure(entry: PaletteEntryDto): string | null {
  const feo = entry.feo_mass_fraction;
  if (!present(feo)) {
    return null;
  }
  if (entry.role === "liquid") {
    return "has an FeO mass fraction, which no liquid carries";
  }
  return isFraction(feo) ? null : `has an FeO mass fraction, ${feo}, that is not from 0 to 1`;
}

/**
 * Why a palette entry cannot be taken, or `null`: its albedo, its phase row, its liquid record and
 * then its FeO mass fraction, in R09's codec's order.
 */
function entryFailure(entry: PaletteEntryDto): string | null {
  if (!holds(entry.normal_albedo_bvr, 3, atLeastZero)) {
    return "has a normal albedo that is not three finite values of at least 0";
  }
  if (!Number.isInteger(entry.phase_row) || entry.phase_row < 0 || entry.phase_row > 255) {
    return `has a phase row, ${entry.phase_row}, that is not a byte`;
  }
  return recordFailure(entry) ?? feoFailure(entry);
}

/**
 * Why a field's header cannot be taken, by the first of its members out of range, or `null`.
 *
 * @remarks
 * It holds what R09's codec holds of the same members (decision-r11-palette-liquids §1.6 and §2):
 * each palette entry's normal albedo finite and at least 0 and its phase row a byte; a liquid
 * record on every liquid entry and on no ice, cover or deposit, each of its members in range and
 * each by-bin member {@link LIQUID_BINS} long, a member not modelled absent; each entry's FeO mass
 * fraction, where present, on no liquid and from 0 to 1; the main liquid, where there is one, a
 * liquid entry of the palette; the two fractions plan 14 fills, the maturity and the organic
 * cover, from 0 to 1 where present; and the weathering rate, finite and at least 0 where present,
 * and present exactly when the maturity is, each part's range checked before the pairing
 * (decision-p14-t51d-weathering §1.6 and §2.3). The server sends only headers its own checks pass,
 * so a header refused here is a damaged answer or a server of another build.
 */
export function headerFailure(header: SurfaceHeaderDto): string | null {
  for (const [index, entry] of header.palette.entries()) {
    const failure = entryFailure(entry);
    if (failure !== null) {
      return `the palette's entry ${index}, ${entry.substance} as ${entry.role}, ${failure}`;
    }
  }
  const main = header.main_liquid;
  if (main !== null && header.palette[main]?.role !== "liquid") {
    return `the field's main liquid, entry ${main}, is not a liquid of its palette`;
  }
  const fractions = [
    ["maturity", header.maturity],
    ["organic cover", header.organic_cover],
  ] as const;
  for (const [name, value] of fractions) {
    if (present(value) && !isFraction(value)) {
      return `the field's ${name}, ${value}, is not from 0 to 1`;
    }
  }
  const rate = header.weathering_rate;
  if (present(rate) && !atLeastZero(rate)) {
    return `the field's weathering rate, ${rate}, is not finite and at least 0`;
  }
  if (present(rate) !== present(header.maturity)) {
    return present(rate)
      ? "the field's weathering rate is present without its maturity"
      : "the field's maturity is present without its weathering rate";
  }
  return null;
}

/** The part of R05's pool the store posts to. */
export type FieldPool = Pick<
  HeightWorkerPool,
  "fieldInit" | "fieldChunk" | "fieldRelease" | "onFieldRefused"
>;

/** What makes a store. */
export interface CoarseFieldStoreOptions {
  /** The universe the bodies are in; a store serves one. */
  readonly universe: UniverseIdHex;
  /** The link's request client. */
  readonly requests: Pick<RequestClient, "requestBulk">;
  /** The height workers' pool. */
  readonly pool: FieldPool;
  /** Whether the link is up as the store starts; {@link CoarseFieldStore.setLinkUp} follows it. */
  readonly linkUp: boolean;
}

/** Where a wanted body's field stands, besides its coverage and header. */
export interface FieldStatus {
  readonly reason: FieldWantReason;
  /**
   * Whether a request for the field is in flight, waits for the link to return, or waits to be
   * asked again after the server was too busy to answer.
   */
  readonly pending: boolean;
  /** For a body whose field R09 cannot compute yet, the plan 14 section it needs; else `null`. */
  readonly notModelled: RecordSectionDto | null;
  /**
   * Why the body's field is not being asked for: the server's refusal, a payload that could not
   * be read, or a block a worker refused. It is held until the body is wanted afresh; else `null`.
   */
  readonly failure: string | null;
}

/** How long the store waits before asking again after the server was too busy, ms, at first. */
export const FIELD_RETRY_DELAY_MS = 2_000;

/** The longest wait between asks after the server was too busy, ms: the delay doubles to it. */
export const FIELD_RETRY_MAX_DELAY_MS = 32_000;

/** A request in flight for one body. */
interface Fetch {
  /** Cancels the request; set once it is sent. */
  cancel: () => void;
  /** Why the store gave the request up itself, if it did. */
  failure: string | null;
}

/** One wanted body. */
interface Entry {
  reason: FieldWantReason;
  header: SurfaceHeaderDto | null;
  coverage: CoverageView | null;
  notModelled: RecordSectionDto | null;
  failure: string | null;
  fetch: Fetch | null;
  /** Whether the body's first block has been posted, as `field_init`. */
  started: boolean;
  /** The newest revision the scene has announced since the field was last dropped, or `null`. */
  announced: number | null;
  /** Whether the body is to be asked once the link returns. */
  waiting: boolean;
  /** The timer that asks again after the server was too busy, or `null`. */
  retry: ReturnType<typeof setTimeout> | null;
  /** How many asks in a row the server was too busy for. */
  busyAnswers: number;
  /**
   * Whether the field is to be asked again once the request in flight settles with an answer: a
   * resync came while it was out ({@link CoarseFieldStore.resync}), so its answer may be behind a
   * rise the scene announced where the client could not hear it.
   */
  recheck: boolean;
}

/** The ending of a request that is the link's, not the server's: asked again when it returns. */
function isLinkFailure(failure: RequestFailure): boolean {
  return failure.code === "link_lost";
}

/** Whether a request ended because the client cancelled it. */
function isCancelled(failure: RequestFailure): boolean {
  return failure.code === "aborted" || failure.code === "superseded";
}

/**
 * Whether the server refused the revision asked from (R09.T19.b): one above the body's own, as a
 * server whose survey log is not the one the field came from answers.
 */
function isUnknownRevision(failure: RequestFailure): boolean {
  return failure.code === "bad_request" && "field" in failure && failure.field === "have_revision";
}

/**
 * Whether the server was too busy to answer, which passes: its bulk queue full (`queue_full`), the
 * connection's request limit reached (`too_many_requests`), or its job cancelled on its side
 * (`cancelled`; the client's own cancel ends as `aborted`).
 */
function isBusy(failure: RequestFailure): boolean {
  return (
    failure.code === "queue_full" ||
    failure.code === "too_many_requests" ||
    failure.code === "cancelled"
  );
}

/**
 * The coarse fields the client holds, for every view and every height worker (Design note 16).
 *
 * @remarks
 * Each frame the views' wiring calls {@link CoarseFieldStore.follow} with the bodies their cameras
 * see, the scene's notifications pass their `surface_revisions` to
 * {@link CoarseFieldStore.noteSurfaceRevisions}, the scene topic opened again reaches
 * {@link CoarseFieldStore.resync}, and the link's state reaches
 * {@link CoarseFieldStore.setLinkUp} (R10.T16.a's `TerrainSession` wires all four). Consumers read
 * {@link CoarseFieldStore.coverage} each frame,
 * or listen with {@link CoarseFieldStore.onRevision} for each new coverage. Call
 * {@link CoarseFieldStore.dispose} before the pool is terminated.
 */
export class CoarseFieldStore {
  readonly #universe: UniverseIdHex;
  readonly #requests: Pick<RequestClient, "requestBulk">;
  readonly #pool: FieldPool;
  readonly #entries = new Map<BodyIdHex, Entry>();
  readonly #revisionListeners = new Set<(body: BodyIdHex, coverage: CoverageView) => void>();
  readonly #offRefused: () => void;
  #linkUp: boolean;
  #disposed = false;

  constructor(opts: CoarseFieldStoreOptions) {
    this.#universe = opts.universe;
    this.#requests = opts.requests;
    this.#pool = opts.pool;
    this.#linkUp = opts.linkUp;
    this.#offRefused = this.#pool.onFieldRefused((refusal) => {
      this.#refused(refusal.body, refusal.message);
    });
  }

  /** The bodies wanted, in the order first wanted. */
  get wanted(): ReadonlySet<BodyIdHex> {
    return new Set(this.#entries.keys());
  }

  /**
   * Wants `body`'s field: asks for it the first time, and otherwise only records the reason.
   *
   * @throws Error once the store is disposed.
   */
  want(body: BodyIdHex, reason: FieldWantReason): void {
    this.#assertLive();
    const held = this.#entries.get(body);
    if (held !== undefined) {
      held.reason = reason;
      return;
    }
    const entry: Entry = {
      reason,
      header: null,
      coverage: null,
      notModelled: null,
      failure: null,
      fetch: null,
      started: false,
      announced: null,
      waiting: false,
      retry: null,
      busyAnswers: 0,
      recheck: false,
    };
    this.#entries.set(body, entry);
    this.#ask(body, entry);
  }

  /**
   * Releases `body`'s field: its request in flight is cancelled, and every worker drops it. A body
   * not wanted is no error.
   */
  release(body: BodyIdHex): void {
    const entry = this.#entries.get(body);
    if (entry === undefined) {
      return;
    }
    this.#entries.delete(body);
    entry.fetch?.cancel();
    entry.fetch = null;
    if (entry.retry !== null) {
      clearTimeout(entry.retry);
      entry.retry = null;
    }
    if (entry.started) {
      this.#pool.fieldRelease(body);
    }
  }

  /** Wants and releases bodies as {@link fieldDemand} says of `candidates`. */
  follow(candidates: ReadonlyArray<FieldCandidate>): void {
    const demand = fieldDemand(candidates, this.wanted);
    for (const body of demand.release) {
      this.release(body);
    }
    for (const { body, reason } of demand.want) {
      this.want(body, reason);
    }
  }

  /** `body`'s coverage, or `null` until a `ready` answer is held and after its field is dropped. */
  coverage(body: BodyIdHex): CoverageView | null {
    return this.#entries.get(body)?.coverage ?? null;
  }

  /** `body`'s field header, as its newest `ready` answer gave it, or `null` before one. */
  header(body: BodyIdHex): SurfaceHeaderDto | null {
    return this.#entries.get(body)?.header ?? null;
  }

  /** Where `body`'s field stands, or `null` for a body not wanted. */
  status(body: BodyIdHex): FieldStatus | null {
    const entry = this.#entries.get(body);
    if (entry === undefined) {
      return null;
    }
    return {
      reason: entry.reason,
      pending: entry.fetch !== null || entry.waiting || entry.retry !== null,
      notModelled: entry.notModelled,
      failure: entry.failure,
    };
  }

  /**
   * Listens for each new coverage of a body, once its answer is held; returns the unsubscribe.
   *
   * @remarks
   * A listener may release the body or want others. One that throws is reported on the console
   * and leaves the field and the other listeners as they were.
   */
  onRevision(cb: (body: BodyIdHex, coverage: CoverageView) => void): () => void {
    this.#revisionListeners.add(cb);
    return () => {
      this.#revisionListeners.delete(cb);
    };
  }

  /**
   * Takes a scene notification's `surface_revisions`: each wanted body whose revision rose past the
   * one held is asked again for the new cells. A body with a request in flight is asked once it
   * settles, if the answer is still behind. The scene names every surveyed body of the universe;
   * those not wanted are passed over.
   */
  noteSurfaceRevisions(revisions: ReadonlyArray<SurfaceRevisionDto>): void {
    for (const { body, revision } of revisions) {
      const entry = this.#entries.get(body);
      if (entry === undefined) {
        continue;
      }
      entry.announced = Math.max(entry.announced ?? revision, revision);
      const held = entry.coverage?.revision ?? null;
      if (held !== null && revision > held && entry.failure === null) {
        this.#ask(body, entry);
      }
    }
  }

  /**
   * Follows the link: down marks every coverage stale, and up again asks for every field kept, with
   * the revision held, and every one whose request the link cut off.
   *
   * @remarks
   * The revisions announced before the link went down are forgotten, since the return asks every
   * kept field again anyway, and the server answers with its latest.
   */
  setLinkUp(up: boolean): void {
    const wasUp = this.#linkUp;
    this.#linkUp = up;
    if (!up) {
      for (const entry of this.#entries.values()) {
        entry.coverage = entry.coverage?.withStale(true) ?? null;
        entry.announced = null;
      }
      return;
    }
    if (wasUp) {
      return;
    }
    for (const [body, entry] of this.#entries) {
      const kept = entry.coverage !== null && entry.failure === null;
      if (entry.waiting || kept) {
        this.#ask(body, entry);
      }
    }
  }

  /**
   * Asks every kept field again with the revision held, as the link's return does: for the scene
   * topic opened again after a sequence fault, a push it could not use or the server's ending it,
   * whose notifications meanwhile are not replayed (R03 Design note 4), so that a rise announced
   * in them is not missed (plan R10, R10.T16.a).
   *
   * @remarks
   * A body whose request is in flight is asked again once it is answered, since that answer may
   * predate the rise; one waiting for the link, or for a busy server, is asked as it was to be. A
   * field held failed, or not modelled, is not asked. Nothing once the store is disposed.
   */
  resync(): void {
    if (this.#disposed) {
      return;
    }
    for (const [body, entry] of this.#entries) {
      if (entry.fetch !== null) {
        entry.recheck = true;
      } else if (entry.coverage !== null && entry.failure === null) {
        this.#ask(body, entry);
      }
    }
  }

  /** Cancels every request and releases every body from the workers; the store is then unusable. */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    // A Map's iteration visits each key once though `release` deletes it.
    for (const body of this.#entries.keys()) {
      this.release(body);
    }
    this.#offRefused();
    this.#revisionListeners.clear();
  }

  #assertLive(): void {
    if (this.#disposed) {
      throw new Error("the coarse-field store was disposed");
    }
  }

  /**
   * Asks for `body`'s field, with the revision held, or waits for the link; nothing when the store
   * is disposed, the body is no longer wanted as `entry`, or a request for it is in flight already,
   * whose answer asks again if it is behind.
   */
  #ask(body: BodyIdHex, entry: Entry): void {
    if (this.#disposed || this.#entries.get(body) !== entry || entry.fetch !== null) {
      return;
    }
    if (entry.retry !== null) {
      clearTimeout(entry.retry);
      entry.retry = null;
    }
    if (!this.#linkUp) {
      entry.waiting = true;
      return;
    }
    entry.waiting = false;
    entry.recheck = false;
    const splitter = new BlockSplitter();
    const started: Fetch = { cancel: () => undefined, failure: null };
    const have = entry.coverage?.revision ?? null;
    const onChunk = (chunk: BulkChunk): void => {
      if (entry.fetch !== started || started.failure !== null) {
        return;
      }
      const split = splitter.push(chunk.payload);
      if (!split.ok) {
        started.failure = split.message;
        started.cancel();
        return;
      }
      for (const block of split.blocks) {
        if (entry.started) {
          this.#pool.fieldChunk(body, block);
        } else {
          this.#pool.fieldInit(body, block);
          entry.started = true;
        }
      }
    };
    const pending = this.#requests.requestBulk<"surface_field">(
      {
        kind: "surface_field",
        universe: this.#universe,
        body,
        have_revision: have,
      },
      (response) => (response.state === "ready" ? response.bulk : null),
      { onChunk },
    );
    started.cancel = () => {
      pending.cancel();
    };
    entry.fetch = started;
    const finish = async (): Promise<void> => {
      const outcome = await pending.outcome;
      if (this.#entries.get(body) !== entry || entry.fetch !== started) {
        return;
      }
      entry.fetch = null;
      if (started.failure !== null) {
        this.#fail(body, entry, started.failure);
      } else if (outcome.ok) {
        this.#answered(body, entry, outcome.response, splitter);
      } else if (isLinkFailure(outcome.error)) {
        entry.waiting = true;
        entry.coverage = entry.coverage?.withStale(true) ?? null;
      } else if (have !== null && isUnknownRevision(outcome.error)) {
        // What is held is not of the server's history: dropped, and the field asked for whole.
        this.#drop(body, entry);
        this.#ask(body, entry);
      } else if (isBusy(outcome.error)) {
        this.#askLater(body, entry);
      } else if (!isCancelled(outcome.error)) {
        this.#fail(body, entry, `${outcome.error.code}: ${outcome.error.message}`);
      }
    };
    void finish().catch((error: unknown) => {
      // A bug in taking the answer: the body is held failed, with the reason, and reported.
      console.error(`the coarse field of ${body} could not be taken:`, error);
      entry.failure = error instanceof Error ? error.message : String(error);
    });
  }

  /** Asks for `body`'s field again after the server was too busy, waiting longer each time. */
  #askLater(body: BodyIdHex, entry: Entry): void {
    entry.busyAnswers += 1;
    const delayMs = Math.min(
      FIELD_RETRY_MAX_DELAY_MS,
      FIELD_RETRY_DELAY_MS * 2 ** (entry.busyAnswers - 1),
    );
    entry.retry = setTimeout(() => {
      entry.retry = null;
      this.#ask(body, entry);
    }, delayMs);
  }

  /** Takes an answer: a field's header and coverage merged in, or a field not modelled. */
  #answered(
    body: BodyIdHex,
    entry: Entry,
    response: ResponseFor<"surface_field">,
    splitter: BlockSplitter,
  ): void {
    entry.busyAnswers = 0;
    if (response.state === "not_modelled") {
      this.#drop(body, entry);
      entry.notModelled = response.section;
      entry.failure = null;
      return;
    }
    const failure = this.#readyFailure(entry, response.header, splitter);
    if (failure !== null) {
      this.#fail(body, entry, failure);
      return;
    }
    const base = entry.coverage ?? CoverageView.empty(response.header.level);
    const merged = base.merged(response.coverage, response.revision);
    if (!merged.ok) {
      this.#fail(body, entry, merged.message);
      return;
    }
    const coverage = merged.coverage.withStale(!this.#linkUp);
    entry.coverage = coverage;
    entry.header = response.header;
    entry.notModelled = null;
    entry.failure = null;
    // A listener removed by another during the call is not told; one added is.
    for (const listener of this.#revisionListeners) {
      try {
        listener(body, coverage);
      } catch (error: unknown) {
        console.error(`a coarse-field listener failed on the revision of ${body}:`, error);
      }
    }
    if (entry.recheck || (entry.announced !== null && entry.announced > response.revision)) {
      this.#ask(body, entry);
    }
  }

  /** Why a `ready` answer cannot be taken, or `null`. */
  #readyFailure(entry: Entry, header: SurfaceHeaderDto, splitter: BlockSplitter): string | null {
    if (!splitter.atBoundary) {
      return "the payload ended inside a block";
    }
    if (!isFieldLevel(header.level)) {
      return `the field's level ${header.level} is not 5 to 8`;
    }
    if (entry.coverage !== null && entry.coverage.level !== header.level) {
      return `the field's level moved from ${entry.coverage.level} to ${header.level}`;
    }
    return headerFailure(header);
  }

  /**
   * Holds `message` as the body's failure. A field with no answer held yet is dropped from the
   * workers too, so that no part of a payload never taken stays there; one held keeps what it has.
   */
  #fail(body: BodyIdHex, entry: Entry, message: string): void {
    if (entry.coverage === null) {
      this.#drop(body, entry);
    }
    entry.failure = message;
  }

  /** A worker refused one of `body`'s blocks: the field is dropped and the refusal held. */
  #refused(body: BodyIdHex, message: string): void {
    const entry = this.#entries.get(body);
    if (entry === undefined || !entry.started) {
      return;
    }
    entry.fetch?.cancel();
    entry.fetch = null;
    this.#drop(body, entry);
    entry.failure = `a height worker refused a block of the field: ${message}`;
  }

  /**
   * Drops what is held of `body`'s field, here and in every worker, keeping the body wanted, and
   * forgets the revisions announced for it, which a server of another history may never reach.
   */
  #drop(body: BodyIdHex, entry: Entry): void {
    if (entry.started) {
      this.#pool.fieldRelease(body);
      entry.started = false;
    }
    entry.coverage = null;
    entry.header = null;
    entry.announced = null;
  }
}
