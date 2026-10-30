/**
 * The pinned ports against the sim's `libm` (plan R08, R08.T20): `hyperion-base`'s
 * `tests/client_math_golden.rs` writes `libm` 0.2.16's values at each function's special values,
 * thresholds, hard cases, near-midpoint and random inputs, and the digests of its sweeps, to
 * `golden/math/client_pairs.golden`; each port gives every one of them bit for bit.
 *
 * @remarks
 * The sweeps' generator and domains are the Rust test's, and their digest the testkit's
 * `golden::f64_word_digest`, line for line: Mulberry32, a binary exponent uniform over the
 * domain's range with a uniform significand, and MurmurHash3's 32-bit body over each result's
 * words, high first, a NaN as `0x7ff80000` and 0.
 */
import { describe, expect, it } from "vitest";

import golden from "../../../../../../crates/hyperion-base/tests/golden/math/client_pairs.golden?raw";
import {
  acos,
  asin,
  atan,
  atan2,
  cbrt,
  cos,
  exp,
  expm1,
  log,
  log1p,
  log2,
  pow,
  sin,
  tan,
} from "./pinnedMath";

/** The one-argument ports, by the golden's names. */
const UNARY: Readonly<Record<string, (x: number) => number>> = {
  exp,
  log,
  log2,
  log1p,
  expm1,
  sin,
  cos,
  tan,
  asin,
  acos,
  atan,
  cbrt,
};

/** The two-argument ports, their arguments in the golden's order. */
const BINARY: Readonly<Record<string, (a: number, b: number) => number>> = { atan2, pow };

/** The bits of a double as 16 hexadecimal digits, or `nan`, as the golden writes them. */
function word(x: number): string {
  if (Number.isNaN(x)) {
    return "nan";
  }
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, x);
  return view.getBigUint64(0).toString(16).padStart(16, "0");
}

/** The double a golden token names. */
function valueOf(token: string | undefined): number {
  if (token === "nan") {
    return Number.NaN;
  }
  if (token === undefined || !/^[0-9a-f]{16}$/u.test(token)) {
    throw new Error(`${String(token)} is not a double's bits`);
  }
  const view = new DataView(new ArrayBuffer(8));
  view.setBigUint64(0, BigInt(`0x${token}`));
  return view.getFloat64(0);
}

/** One pinned value: the function, its arguments' tokens and `libm`'s result's. */
interface Pair {
  readonly name: string;
  readonly args: ReadonlyArray<string>;
  readonly result: string;
}

/** One sweep: the function, its arguments' domains, the count, the seed and `libm`'s digest. */
interface Sweep {
  readonly name: string;
  readonly domains: ReadonlyArray<string>;
  readonly count: number;
  readonly seed: number;
  readonly digest: string;
}

/** The golden's pairs and sweeps. */
function parse(text: string): { pairs: Pair[]; sweeps: Sweep[] } {
  const pairs: Pair[] = [];
  const sweeps: Sweep[] = [];
  for (const line of text.split("\n")) {
    if (line.length === 0 || line.startsWith("#")) {
      continue;
    }
    const tokens = line.split(" ");
    const [head, name] = tokens;
    if (head === "sweep" && name !== undefined) {
      const tail = tokens.slice(-3);
      sweeps.push({
        name,
        domains: tokens.slice(2, -3),
        count: Number(tail[0]),
        seed: Number(tail[1]),
        digest: tail[2] ?? "",
      });
    } else if (head !== undefined) {
      pairs.push({ name: head, args: tokens.slice(1, -1), result: tokens.at(-1) ?? "" });
    }
  }
  return { pairs, sweeps };
}

const { pairs: PAIRS, sweeps: SWEEPS } = parse(golden);

/** The port a pair or sweep names, applied to its arguments. */
function evaluate(name: string, args: ReadonlyArray<number>): number {
  const unary = UNARY[name];
  const binary = BINARY[name];
  const [a, b] = args;
  if (unary !== undefined && a !== undefined && args.length === 1) {
    return unary(a);
  }
  if (binary !== undefined && a !== undefined && b !== undefined && args.length === 2) {
    return binary(a, b);
  }
  throw new Error(`the golden names ${name} with ${String(args.length)} arguments`);
}

/** Mulberry32, the Rust test's `Mulberry32`. */
function mulberry32(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return (t ^ (t >>> 14)) >>> 0;
  };
}

/** A domain's drawer, the Rust test's `Domain::draw`, from its text `<sign><low>..<high>`. */
function drawer(text: string): (next: () => number) => number {
  const match = /^(?<sign>[+~-])(?<low>-?\d+)\.\.(?<high>-?\d+)$/u.exec(text);
  const low = Number(match?.groups?.["low"]);
  const high = Number(match?.groups?.["high"]);
  const sign = match?.groups?.["sign"];
  if (sign === undefined || !Number.isInteger(low) || !Number.isInteger(high)) {
    throw new Error(`${text} is not a sweep's domain`);
  }
  const view = new DataView(new ArrayBuffer(8));
  const span = high - low + 1;
  return (next) => {
    const w0 = next();
    const w1 = next();
    const w2 = next();
    const biased = Math.max(low + (w0 % span) + 1023, 0);
    const negative = sign === "-" ? 1 : sign === "~" ? w1 >>> 31 : 0;
    view.setUint32(0, negative * 0x80000000 + biased * 0x100000 + (w1 & 0xfffff));
    view.setUint32(4, w2);
    return view.getFloat64(0);
  };
}

/** The testkit's `golden::f64_word_digest`: MurmurHash3's 32-bit body and finaliser. */
class Digest {
  private hash = 0;
  private words = 0;
  private readonly view = new DataView(new ArrayBuffer(8));

  value(x: number): void {
    if (Number.isNaN(x)) {
      this.word(0x7ff80000);
      this.word(0);
    } else {
      this.view.setFloat64(0, x);
      this.word(this.view.getUint32(0));
      this.word(this.view.getUint32(4));
    }
  }

  finish(): string {
    let h = (this.hash ^ this.words) >>> 0;
    h ^= h >>> 16;
    h = Math.imul(h, 0x85ebca6b);
    h ^= h >>> 13;
    h = Math.imul(h, 0xc2b2ae35);
    h ^= h >>> 16;
    return (h >>> 0).toString(16).padStart(8, "0");
  }

  private word(w: number): void {
    let k = Math.imul(w, 0xcc9e2d51);
    k = (k << 15) | (k >>> 17);
    k = Math.imul(k, 0x1b873593);
    let h = this.hash ^ k;
    h = (h << 13) | (h >>> 19);
    this.hash = (Math.imul(h, 5) + 0xe6546b64) >>> 0;
    this.words += 1;
  }
}

/** Every port, by name. */
const NAMES = [...Object.keys(UNARY), ...Object.keys(BINARY)];

describe("the pinned ports against libm 0.2.16", () => {
  it("pins every port, each at a hundred inputs or more and in one sweep or more", () => {
    const counts = NAMES.map((name) => [
      name,
      PAIRS.filter((p) => p.name === name).length >= 100,
      SWEEPS.some((s) => s.name === name),
    ]);
    expect(counts).toEqual(NAMES.map((name) => [name, true, true]));
  });

  it("names no function the client does not port", () => {
    expect(new Set([...PAIRS, ...SWEEPS].map((p) => p.name))).toEqual(new Set(NAMES));
  });

  it.each(NAMES)("gives libm's bits for %s at every pinned input", (name) => {
    const mismatches = PAIRS.filter((p) => p.name === name).flatMap((p) => {
      const got = word(evaluate(name, p.args.map(valueOf)));
      return got === p.result ? [] : [`${name}(${p.args.join(", ")}) = ${got}, libm ${p.result}`];
    });
    expect(mismatches).toEqual([]);
  });

  it.each(SWEEPS.map((s) => [`${s.name} ${s.domains.join(" ")}`, s] as const))(
    "gives libm's digest over the sweep %s",
    (_, sweep) => {
      const next = mulberry32(sweep.seed);
      const draws = sweep.domains.map(drawer);
      const digest = new Digest();
      for (let i = 0; i < sweep.count; i += 1) {
        digest.value(
          evaluate(
            sweep.name,
            draws.map((draw) => draw(next)),
          ),
        );
      }
      expect(digest.finish()).toBe(sweep.digest);
    },
  );
});
