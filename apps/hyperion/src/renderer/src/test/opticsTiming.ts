import {
  type KDistributionTable,
  loadKDistributions,
  type PartReader,
} from "../view/atmosphere/absorbers/kDistribution";
import type { AbsorberCurveSums } from "../view/atmosphere/absorbers";
import { lineSpeciesOf } from "../view/atmosphere/assemble";
import type { BodyAtmosphereInput } from "../view/atmosphere/atmosphereInput";
import { OpticsJobs, type OpticsJobsDeps } from "../view/atmosphere/opticsJobs";
import { type OpticsReply, transferablesOf } from "../view/atmosphere/opticsMessages";

/** One body's optics times in process, ms, and what the render thread would hold of it, B. */
export interface OpticsTimes {
  /** Reading and decoding the input's line tables, once a worker. */
  readonly loadMs: number;
  /** The assembly on its worker, its tables loaded. */
  readonly assembleMs: number;
  /**
   * Each light set's shares (the brightest light alone, then the others), each share on its own
   * worker: share 0 on the worker that assembled, the others forming the body's specs themselves.
   */
  readonly shareMs: ReadonlyArray<ReadonlyArray<number>>;
  /** Each light set's join. */
  readonly joinMs: ReadonlyArray<number>;
  /**
   * The typed arrays of the assembly the render thread holds for the body's entry: its medium,
   * with its absorbers' specs and their line tables at the body's temperature, and its body.
   */
  readonly mediumBytes: number;
  /** The typed arrays of its light slots' channel curves. */
  readonly curvesBytes: number;
}

/** Whether a reply is of `kind`. */
function isReply<K extends OpticsReply["kind"]>(
  reply: OpticsReply,
  kind: K,
): reply is Extract<OpticsReply, { readonly kind: K }> {
  return reply.kind === kind;
}

/** A reply of `kind`, or an error naming what came. */
function replyOf<K extends OpticsReply["kind"]>(
  reply: OpticsReply,
  kind: K,
): Extract<OpticsReply, { readonly kind: K }> {
  if (!isReply(reply, kind)) {
    throw new Error(
      `the optics job was answered ${reply.kind}${reply.kind === "failed" ? `: ${reply.message}` : ""}`,
    );
  }
  return reply;
}

/** The bytes of every buffer reachable from `value`. */
function bytesOf(value: unknown): number {
  return transferablesOf(value).reduce((sum, buffer) => sum + buffer.byteLength, 0);
}

/** The time `run` takes, ms, and its value. */
async function timed<T>(
  run: () => Promise<T>,
): Promise<{ readonly ms: number; readonly value: T }> {
  const start = performance.now();
  const value = await run();
  return { ms: performance.now() - start, value };
}

/**
 * Times one body's optics as the pool runs them, in process and one job after another
 * (plan R08, R08.T10.a, for R08.T11's arrival line): the line tables' load, the assembly, each
 * light set's shares and join, with `shares` shares a set, the pool's size; and the bytes the
 * render thread would hold for the body's entry. On a quiet machine the slowest share of a set is
 * the pool's time for it.
 *
 * @param readPart - How the committed parts are read: `kDistributionFixtures.ts`'s `inlinePart`.
 * @throws Error where a job fails or a table cannot be read.
 */
export async function timeOpticsBuild(
  input: BodyAtmosphereInput,
  readPart: PartReader,
  shares: number,
): Promise<OpticsTimes> {
  const load = await timed(async () => loadKDistributions(lineSpeciesOf(input), readPart));
  const tables: ReadonlyArray<KDistributionTable> = load.value;
  const deps: OpticsJobsDeps = { loadKDistributions: async () => Promise.resolve(tables) };
  if (!Number.isInteger(shares) || shares < 1) {
    throw new RangeError(`a light set takes at least one share, not ${shares}`);
  }
  const assembler = new OpticsJobs(deps);
  let id = 0;
  const nextId = (): number => {
    id += 1;
    return id;
  };
  const assembly = await timed(async () =>
    assembler.answer({ kind: "assemble", id: nextId(), input }),
  );
  const { assembled } = replyOf(assembly.value.reply, "assembled");
  const lights = input.kind === "record" ? input.lights.length : 0;
  const others = Array.from({ length: lights - 1 }, (_, k) => k + 1);
  const sets = lights === 0 ? [] : others.length === 0 ? [[0]] : [[0], others];
  const workers = [assembler, ...Array.from({ length: shares - 1 }, () => new OpticsJobs(deps))];
  const shareMs: number[][] = [];
  const joinMs: number[] = [];
  const curves: unknown[] = [];
  for (const slots of sets) {
    const times: number[] = [];
    const sums: Array<ReadonlyArray<AbsorberCurveSums>> = [];
    for (const [index, jobs] of workers.entries()) {
      // One share after another, each timed alone, as each would run on its own worker.
      // oxlint-disable-next-line no-await-in-loop
      const share = await timed(async () =>
        jobs.answer({
          kind: "curve-sums",
          id: nextId(),
          input,
          slots,
          share: { index, count: shares },
        }),
      );
      times.push(share.ms);
      sums.push(replyOf(share.value.reply, "curve-sums").sums);
    }
    // oxlint-disable-next-line no-await-in-loop
    const join = await timed(async () =>
      assembler.answer({ kind: "curve-join", id: nextId(), input, slots, sums }),
    );
    shareMs.push(times);
    joinMs.push(join.ms);
    curves.push(replyOf(join.value.reply, "curves").curves);
  }
  return {
    loadMs: load.ms,
    assembleMs: assembly.ms,
    shareMs,
    joinMs,
    mediumBytes: bytesOf(assembled),
    curvesBytes: bytesOf(curves),
  };
}
