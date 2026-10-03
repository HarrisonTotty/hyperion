/**
 * The smoke page's check of a height worker in the built app (plan R05, T10.b's built-app check).
 *
 * @remarks
 * The smoke page is a built page loaded with `loadFile` under a policy that, like the client's,
 * refuses WebAssembly on the render thread, so a height worker that loads its module and bakes here
 * shows that module workers load from `file://` and compile under the unchanged policy (Design note
 * 11; R04.T10.a). A policy refusal reaches this page as the worker's error, which `describeLoadFailure`
 * words as `csp-refused`, and fails the check.
 */

import type {
  BakedPatch,
  HeightWorkerReply,
  HeightWorkerRequest,
} from "../view/terrain/workers/messages";
import type { Checks } from "./harness";

/** How long the bake may take before the check fails, milliseconds. */
const BAKE_TIMEOUT_MS = 60_000;

/** Starts a height worker, bakes one level-12 patch, and checks what comes back. */
export async function checkHeightWorker(checks: Checks): Promise<void> {
  const worker = new Worker(new URL("../view/terrain/workers/height.worker.ts", import.meta.url), {
    type: "module",
  });
  try {
    const request: HeightWorkerRequest = {
      kind: "bake",
      id: 1,
      key: { face: 2, level: 12, i: 1_000, j: 2_000 },
      generation: 1,
      settings: { vertexPath: "baked-offsets", normals: "double", ridges: "off" },
    };
    const reply = await new Promise<HeightWorkerReply>((resolve, reject) => {
      const timer = setTimeout(() => {
        reject(new Error(`no answer within ${String(BAKE_TIMEOUT_MS)} ms`));
      }, BAKE_TIMEOUT_MS);
      worker.addEventListener("message", (event: MessageEvent<HeightWorkerReply>) => {
        clearTimeout(timer);
        resolve(event.data);
      });
      worker.addEventListener("error", (event: Event) => {
        clearTimeout(timer);
        // A module script that does not load fires a plain `Event`, an error inside it an
        // `ErrorEvent` with the message.
        const message =
          event instanceof ErrorEvent ? event.message : "the module script did not load";
        reject(new Error(`the worker failed: ${message}`));
      });
      worker.postMessage(request, []);
    });
    checks.check(
      "R05.T10.b a height worker bakes a patch in the built app",
      reply.kind === "baked",
      reply.kind === "baked" ? describeBake(reply.bake) : JSON.stringify(reply),
    );
    if (reply.kind === "baked") {
      const bake = reply.bake;
      const finite =
        bake.heights.every(Number.isFinite) &&
        (bake.offsets?.every(Number.isFinite) ?? false) &&
        bake.normals.every(Number.isFinite);
      checks.check(
        "R05.T10.b the bake's arrays have their layout and are finite",
        finite &&
          bake.heights.length === 65 * 65 * 2 &&
          bake.offsets?.length === 65 * 65 * 6 &&
          bake.normals.length === 129 * 129 * 2,
        describeBake(bake),
      );
    }
  } finally {
    worker.terminate();
  }
}

function describeBake(bake: BakedPatch): string {
  return (
    `heights ${String(bake.heights.length)}, offsets ${String(bake.offsets?.length ?? 0)}, ` +
    `normals ${String(bake.normals.length)}, range ${bake.heightRangeM.join("..")} m`
  );
}
