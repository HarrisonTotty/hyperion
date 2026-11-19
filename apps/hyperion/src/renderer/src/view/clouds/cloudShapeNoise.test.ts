import { describe, expect, it } from "vitest";

import {
  BASE_NOISE_SIZE,
  CloudShapeNoiseBuilder,
  cloudShapeNoises,
  CloudShapeNoiseSource,
  CURL_NOISE_SIZE,
  DETAIL_NOISE_SIZE,
  type ShapeNoiseReply,
  type ShapeNoiseRequest,
  serveShapeNoises,
} from "./cloudShapeNoise";

const SEED = 0x2c5f_9a13;
const NOISES = cloudShapeNoises(SEED);

/** The mean and variance of a tile's values on [0, 1]. */
function moments(bytes: Uint8Array): { readonly mean: number; readonly variance: number } {
  let sum = 0;
  let squares = 0;
  for (const b of bytes) {
    sum += b / 255;
    squares += (b / 255) ** 2;
  }
  const mean = sum / bytes.length;
  return { mean, variance: squares / bytes.length - mean * mean };
}

/** The correlation of a 3D tile's values with its neighbours `lag` texels along x. */
function lagCorrelation(bytes: Uint8Array, size: number, lag: number): number {
  const { mean, variance } = moments(bytes);
  let sum = 0;
  for (let t = 0; t < bytes.length; t += 1) {
    const x = t % size;
    const neighbour = t - x + ((x + lag) % size);
    sum += ((bytes[t] ?? 0) / 255 - mean) * ((bytes[neighbour] ?? 0) / 255 - mean);
  }
  return sum / bytes.length / variance;
}

describe("the cloud volume's shape noises", () => {
  it("are 128³, 32³ and 128², one byte a texel: about 2 MiB in all", () => {
    expect(NOISES.base.length).toBe(BASE_NOISE_SIZE ** 3);
    expect(NOISES.detail.length).toBe(DETAIL_NOISE_SIZE ** 3);
    expect(NOISES.curl.length).toBe(CURL_NOISE_SIZE ** 2);
    const bytes = NOISES.base.length + NOISES.detail.length + NOISES.curl.length;
    expect(bytes / 2 ** 20).toBeCloseTo(2.05, 2);
  });

  it("are uniform on [0, 1] once equalised", () => {
    for (const bytes of [NOISES.base, NOISES.detail, NOISES.curl]) {
      const { mean, variance } = moments(bytes);
      expect(Math.abs(mean - 0.5)).toBeLessThan(0.005);
      // A uniform variable's variance is 1/12.
      expect(Math.abs(variance - 1 / 12)).toBeLessThan(0.002);
    }
  });

  it("are smooth at a texel and decorrelated over a tile's quarter", () => {
    expect(lagCorrelation(NOISES.base, BASE_NOISE_SIZE, 1)).toBeGreaterThan(0.9);
    expect(Math.abs(lagCorrelation(NOISES.base, BASE_NOISE_SIZE, 32))).toBeLessThan(0.25);
    expect(lagCorrelation(NOISES.detail, DETAIL_NOISE_SIZE, 1)).toBeGreaterThan(0.8);
  });

  it("tile: the last texel along x is as near the first as its other neighbour", () => {
    const size = BASE_NOISE_SIZE;
    let wrapped = 0;
    let inner = 0;
    for (let row = 0; row < size * size; row += 1) {
      const at = row * size;
      wrapped += Math.abs((NOISES.base[at + size - 1] ?? 0) - (NOISES.base[at] ?? 0));
      inner += Math.abs((NOISES.base[at + size - 1] ?? 0) - (NOISES.base[at + size - 2] ?? 0));
    }
    expect(wrapped / inner).toBeGreaterThan(0.8);
    expect(wrapped / inner).toBeLessThan(1.25);
  });

  it("differ between worlds", () => {
    const other = cloudShapeNoises(SEED + 1);
    let same = 0;
    for (let t = 0; t < other.base.length; t += 1) {
      same += other.base[t] === NOISES.base[t] ? 1 : 0;
    }
    expect(same / other.base.length).toBeLessThan(0.05);
  });

  it("are the same however their making is spread over steps", () => {
    const builder = new CloudShapeNoiseBuilder(SEED);
    let steps = 1;
    let made = builder.step();
    while (made === null) {
      made = builder.step();
      steps += 1;
    }
    expect(steps).toBe(builder.stepCount);
    expect(made.base).toEqual(NOISES.base);
    expect(made.detail).toEqual(NOISES.detail);
    expect(made.curl).toEqual(NOISES.curl);
  });
});

/** A worker that answers each request at once through `serveShapeNoises`, as the real one does. */
class FakeShapeNoiseWorker {
  readonly #replies: Array<(event: MessageEvent<ShapeNoiseReply>) => void> = [];
  readonly #requests: Array<(event: MessageEvent<ShapeNoiseRequest>) => void> = [];
  readonly posted: number[] = [];
  terminated = false;

  constructor() {
    serveShapeNoises({
      addEventListener: (_type, listener) => {
        this.#requests.push(listener);
      },
      postMessage: (reply) => {
        for (const listener of this.#replies) {
          listener(new MessageEvent("message", { data: reply }));
        }
      },
    });
  }

  addEventListener(
    _type: "message",
    listener: (event: MessageEvent<ShapeNoiseReply>) => void,
  ): void {
    this.#replies.push(listener);
  }

  postMessage(message: ShapeNoiseRequest): void {
    this.posted.push(message.seed);
    for (const listener of this.#requests) {
      listener(new MessageEvent("message", { data: message }));
    }
  }

  terminate(): void {
    this.terminated = true;
  }
}

describe("a world's shape noises as frames ask for them", () => {
  it("are made by the worker, asked once a world", () => {
    const worker = new FakeShapeNoiseWorker();
    const source = new CloudShapeNoiseSource(worker);
    const made = source.noisesOf(SEED, () => 0);
    expect(made?.base).toEqual(NOISES.base);
    expect(source.noisesOf(SEED, () => 0)).toBe(made);
    expect(worker.posted).toEqual([SEED]);
    source.dispose();
    expect(worker.terminated).toBe(true);
  });

  it("are made a step a call within the budget where no worker makes them", () => {
    const source = new CloudShapeNoiseSource(null);
    let clock = 0;
    let calls = 0;
    // Each reading of the clock moves it 1 ms: a budget of 2 ms takes two steps a call.
    const now = (): number => {
      clock += 1;
      return clock;
    };
    let made = source.noisesOf(SEED, now);
    while (made === null) {
      made = source.noisesOf(SEED, now);
      calls += 1;
    }
    expect(calls).toBeGreaterThan(100);
    expect(made.detail).toEqual(NOISES.detail);
    expect(source.noisesNow(SEED)).toBe(made);
  });

  it("lets the last world's go when another is asked for", () => {
    const source = new CloudShapeNoiseSource(new FakeShapeNoiseWorker());
    expect(source.noisesOf(SEED, () => 0)).not.toBeNull();
    const other = source.noisesOf(SEED + 1, () => 0);
    expect(other?.base).not.toEqual(NOISES.base);
  });
});
