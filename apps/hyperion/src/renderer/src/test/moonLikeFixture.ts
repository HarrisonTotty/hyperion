/**
 * The Moon-like world's worker fixture (`hyperion_surface`'s `tests/golden/worker/moon_like.golden`):
 * its body, its detail seed and its payloads' field blocks, which R10's lit-terrain and survey-edge
 * smoke checks and the survey edge's and the hand-over's wasm tests take into a worker's fields.
 *
 * @remarks
 * The fixture's lines are `body <id>`, `seed <hex>` and `block <payload> <index> <hex>`, the first
 * payload's blocks the first survey's. Only the module's own loading stays in each test: workers and
 * tests alone import `generated/surface/` (`wasm/surfaceImports.test.ts`).
 */
import fixture from "../../../../../../crates/hyperion-surface/tests/golden/worker/moon_like.golden?raw";

/** A worker fixture's body, its detail seed and its field blocks, in the fixture's order. */
export interface WorkerFixture {
  readonly body: string;
  readonly seed: string;
  /** Each block in a buffer of its own, which a caller may transfer. */
  readonly blocks: ReadonlyArray<ArrayBuffer>;
}

/** The bytes of a hexadecimal string, in a buffer of their own. */
export function bytesOf(hex: string): ArrayBuffer {
  const bytes = new Uint8Array(hex.length / 2);
  for (let n = 0; n < bytes.length; n += 1) {
    bytes[n] = Number.parseInt(hex.slice(2 * n, 2 * n + 2), 16);
  }
  return bytes.buffer;
}

/**
 * The Moon-like fixture, parsed afresh: its first payload's blocks alone (`first`, the first
 * survey's), or both payloads' (`all`), in its order.
 */
export function moonLikeFixture(payloads: "first" | "all"): WorkerFixture {
  let body = "";
  let seed = "";
  const blocks: ArrayBuffer[] = [];
  for (const line of fixture.split("\n")) {
    const [head, first, , third] = line.split(" ");
    if (head === "body") {
      body = first ?? "";
    } else if (head === "seed") {
      seed = first ?? "";
    } else if (head === "block" && (payloads === "all" || first === "1")) {
      blocks.push(bytesOf(third ?? ""));
    }
  }
  return { body, seed, blocks };
}
