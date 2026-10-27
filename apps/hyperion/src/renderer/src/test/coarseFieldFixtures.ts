/**
 * Builders for R09's `surface_field` answers over R03's fake socket, and for the height workers'
 * answers to a payload's blocks (plan R10, R10.T16.a's terrain session tests).
 *
 * @remarks
 * A block here is R09's frame (`HYSF`, its format, its length at offset 6) followed by a marker
 * byte, which the store cuts at its length and no worker here decodes: the scripted workers hold
 * it as a stand-in's, and a test tells a worker what the block surveyed with
 * {@link answerBlock}.
 */
import type {
  BodyIdHex,
  BulkManifestDto,
  CoverDto,
  ResponseFor,
  SurfaceHeaderDto,
} from "@hyperion/protocol";

import { BLOCK_MAGIC } from "../view/terrain/coarseFieldStore";
import type { FieldBlockAnswer } from "../view/terrain/workers/messages";
import { binaryFrame } from "./binaryFrames";
import type { FakeWebSocket } from "./FakeWebSocket";
import type { ScriptedHeightWorker } from "./scriptedHeightWorker";

/**
 * A Moon-like body's field header at level 5: its mean radius the Moon's 1,737.4 km (IAU WGCCRE
 * 2015, Archinal et al. 2018), its equatorial and polar radii NASA's Moon fact sheet's 1,738.1 and
 * 1,736.0 km (the store's tests' figures), and a realised relief of 18 km, a stand-in of the
 * Moon's order (its highest point less its lowest is about 20 km by LOLA's topography, Smith et al.
 * 2010), which sets the tests' hand-over ranges; its other figures plausible stand-ins, which no
 * test of the session reads.
 */
export function moonLikeHeader(body: BodyIdHex): SurfaceHeaderDto {
  return {
    body,
    level: 5,
    radius_m: 1_737_400,
    equatorial_radius_m: 1_738_100,
    polar_radius_m: 1_736_000,
    boundary_diameter_m: 120_000,
    sea_level_m: -2_000,
    lapse_rate_k_per_m: 0,
    realised_sigma_h_m: 1_900,
    realised_relief_m: 18_000,
    months: 1,
    season_eccentricity: 0,
    reference_temperature_k: 250,
    surface_age_gyr: 4.4,
    surface_pressure_pa: 0,
    albedo_scale: null,
    climate_model: "radiative_equilibrium",
    precipitation: "heuristic",
    palette: [],
    main_liquid: null,
  };
}

/** A payload block of `length` bytes: R09's frame (HYSF, format 1, its length), then `marker`. */
export function fieldBlock(length: number, marker: number): Uint8Array {
  const bytes = new Uint8Array(length).fill(marker);
  bytes.set(BLOCK_MAGIC, 0);
  const view = new DataView(bytes.buffer);
  view.setUint16(4, 1, true);
  view.setUint32(6, length, true);
  return bytes;
}

/** `blocks` end to end, as a payload. */
export function fieldPayload(blocks: ReadonlyArray<Uint8Array>): Uint8Array {
  const payload = new Uint8Array(blocks.reduce((n, b) => n + b.length, 0));
  let at = 0;
  for (const b of blocks) {
    payload.set(b, at);
    at += b.length;
  }
  return payload;
}

/** Sends `payload` as request `id`'s binary frames, cut every `frameBytes`, and gives its manifest. */
export function sendFieldFrames(
  socket: FakeWebSocket,
  id: number,
  payload: Uint8Array,
  frameBytes = 64,
): BulkManifestDto {
  const count = Math.ceil(payload.length / frameBytes);
  for (let k = 0; k < count; k += 1) {
    const part = payload.subarray(k * frameBytes, (k + 1) * frameBytes);
    socket.serverSendsBinary(binaryFrame(id, k, count, [...part]));
  }
  return { chunks: count, bytes: payload.length };
}

/** A cover of `[start, end, resolution code]` runs of cells. */
export function fieldCover(...ranges: ReadonlyArray<readonly [number, number, number]>): CoverDto {
  return { ranges: ranges.map(([start, end, resolution]) => ({ start, end, resolution })) };
}

/** The `ready` answer of a field at `revision`, over `coverage`, with `header`'s figures. */
export function readyField(
  header: SurfaceHeaderDto,
  revision: number,
  coverage: CoverDto,
  bulk: BulkManifestDto,
): ResponseFor<"surface_field"> {
  return { kind: "surface_field", state: "ready", header, revision, coverage, bulk };
}

/** The latest `surface_field` request's ID, body and `have_revision`. */
export function latestFieldAsk(socket: FakeWebSocket): {
  readonly id: number;
  readonly body: BodyIdHex;
  readonly have: number | null;
} {
  const sent = socket.requestsOfKind("surface_field").at(-1);
  if (sent === undefined) {
    throw new Error("no surface_field request was sent");
  }
  return { id: sent.id, body: sent.body.body, have: sent.body.have_revision };
}

/**
 * What a payload's block changed in a worker's field at `fieldLevel`: the cells it surveyed anew and
 * those it brought, each as flat runs of cell indices, and a level table of no height (25 rows)
 * with no σ.
 */
export function blockChange(
  fieldLevel: number,
  newlySurveyed: ReadonlyArray<number>,
  newlyHeld: ReadonlyArray<number> = newlySurveyed,
): FieldBlockAnswer {
  return {
    fieldLevel,
    newlySurveyed: Uint32Array.from(newlySurveyed),
    newlyHeld: Uint32Array.from(newlyHeld),
    levels: new Float64Array(25 * 4),
    sigmaM: new Float64Array(25),
  };
}

/**
 * Answers `worker`'s field message number `index` (from 0, in the order received) as taken, with
 * what its block changed, as the real worker answers a payload's block.
 */
export function answerBlock(
  worker: ScriptedHeightWorker,
  index: number,
  block: FieldBlockAnswer,
  heldBytes = 0,
): void {
  const message = worker.fieldMessages[index];
  if (message === undefined || message.kind === "field-release") {
    throw new Error(`the worker received no block as its field message ${String(index)}`);
  }
  worker.reply({ kind: "field-held", id: message.id, body: message.body, heldBytes, block });
}
