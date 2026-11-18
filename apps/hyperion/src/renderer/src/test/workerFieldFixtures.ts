/**
 * The surface crate's worker fixture fields (`tests/golden/worker/<world>.golden`) for the rocks'
 * tests on the real module (plan R11, T4.a and T4.c): a fixture's body, seed, payload blocks and
 * first pinned patch, held in a worker's fields through the worker's own answers or in the
 * module's own fields on the test's thread.
 *
 * @remarks
 * The module is the caller's: only workers and tests import `generated/surface/`
 * (`wasm/surfaceImports.test.ts`), so a test builds its `HeightModule` and passes it in.
 */

import type { PatchKey } from "../view/terrain/patchKey";
import {
  answerRequest,
  type HeightModule,
  WASM_SIGMA_SURVEY,
  type WasmSurfaceFields,
  WorkerFields,
} from "../view/terrain/workers/heightBake";
import type { HeightWorkerRequest } from "../view/terrain/workers/messages";

/** A fixture's body, seed and blocks, and the first patch it pins, on surveyed ground. */
export interface WorkerFixture {
  readonly body: string;
  readonly seed: string;
  readonly blocks: ReadonlyArray<ArrayBuffer>;
  readonly anchor: PatchKey;
}

/** The bytes of a hexadecimal string, in a buffer of their own. */
function bytesOf(hex: string): ArrayBuffer {
  const bytes = new Uint8Array(hex.length / 2);
  for (let n = 0; n < bytes.length; n += 1) {
    bytes[n] = Number.parseInt(hex.slice(2 * n, 2 * n + 2), 16);
  }
  return bytes.buffer;
}

/**
 * The fixture of a worker golden's `text`.
 *
 * @throws Error where it pins no patch, or its first names no face.
 */
export function parseWorkerFixture(text: string): WorkerFixture {
  let body = "";
  let seed = "";
  let anchor: PatchKey | null = null;
  const blocks: ArrayBuffer[] = [];
  for (const line of text.split("\n")) {
    const [head, ...rest] = line.split(" ");
    if (head === "body") {
      body = rest[0] ?? "";
    } else if (head === "seed") {
      seed = rest[0] ?? "";
    } else if (head === "block") {
      blocks.push(bytesOf(rest[2] ?? ""));
    } else if (head === "patch" && anchor === null) {
      const [face, level, i, j] = rest.slice(1, 5).map(Number);
      if (face !== 0 && face !== 1 && face !== 2 && face !== 3 && face !== 4 && face !== 5) {
        throw new Error(`the fixture's patch names no face, ${String(face)}`);
      }
      anchor = { face, level: level ?? 0, i: i ?? 0, j: j ?? 0 };
    }
  }
  if (anchor === null) {
    throw new Error("the fixture pins no patch");
  }
  return { body, seed, blocks, anchor };
}

/**
 * A worker's fields holding every block of `fixture`, through the worker's answers.
 *
 * @throws Error for a block the worker refuses.
 */
export function workerFieldsOf(module: HeightModule, fixture: WorkerFixture): WorkerFields {
  const fields = new WorkerFields(module);
  fixture.blocks.forEach((block, n) => {
    const request: HeightWorkerRequest =
      n === 0
        ? {
            kind: "field-init",
            id: n,
            body: fixture.body,
            block,
            form: "payload",
            sigma: "skipped",
          }
        : { kind: "field-chunk", id: n, body: fixture.body, block };
    const { reply } = answerRequest(module, request, fields);
    if (reply.kind !== "field-held") {
      throw new Error(`block ${String(n)} was refused: ${JSON.stringify(reply)}`);
    }
  });
  return fields;
}

/**
 * The module's own fields, made by `make` (the generated `new SurfaceFields()`), holding every
 * block of `fixture`, with its body's and seed's arguments.
 */
export function directFieldsOf<Fields extends WasmSurfaceFields>(
  make: () => Fields,
  fixture: WorkerFixture,
): {
  readonly direct: Fields;
  readonly args: readonly [bigint, number];
  readonly seed: bigint;
} {
  const direct = make();
  const [system, index] = fixture.body.split(".");
  const args = [BigInt(`0x${system ?? ""}`), Number.parseInt(index ?? "", 16)] as const;
  fixture.blocks.forEach((block, n) => {
    const bytes = new Uint8Array(block);
    const taken =
      n === 0
        ? direct.fieldInit(...args, bytes, WASM_SIGMA_SURVEY.skipped)
        : direct.fieldChunk(...args, bytes);
    taken.free();
  });
  return { direct, args, seed: BigInt(`0x${fixture.seed}`) };
}

/**
 * The finest patch `step` of a 5 × 5 block about the middle of the fixture's first pinned patch,
 * at `finestLevel`, row by row.
 */
export function finestAbout(fixture: WorkerFixture, finestLevel: number, step: number): PatchKey {
  const { anchor } = fixture;
  const down = finestLevel - anchor.level;
  return {
    face: anchor.face,
    level: finestLevel,
    i: anchor.i * 2 ** down + Math.floor(2 ** down / 2) + (step % 5) - 2,
    j: anchor.j * 2 ** down + Math.floor(2 ** down / 2) + Math.floor(step / 5) - 2,
  };
}
