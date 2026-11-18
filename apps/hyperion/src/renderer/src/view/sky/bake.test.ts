import { describe, expect, it } from "vitest";

import { FakeAdapter, INTEL_UHD_620_INFO } from "../../test/fakeGpu";
import { FakeRenderEngine } from "../../test/fakeRenderEngine";
import type { BufferSpec, MemoryCategory } from "../engine/memory";
import type { BufferHandle, RenderEngine, TextureHandle } from "../engine/types";
import {
  bakeSkyCube,
  hasTexelSolidAngles,
  paddedRowTexels,
  releaseBakedCube,
  type SolidAnglesWorker,
  warmTexelSolidAngles,
} from "./bake";
import {
  cubeLevels,
  divideBySolidAngle,
  faceMipChain,
  peakScaleExponent,
  scaleByPowerOfTwo,
} from "./mips";
import { texelSolidAnglesLowerF32, texelSolidAnglesSr } from "./cube";
import { packRgb9e5Texels } from "./pack";
import { solidAnglesReply } from "./solidAnglesJob";
import { splatCpu, splatPoints } from "./splatCpu";

/** What the recording engine was asked to do. */
interface Recorded {
  readonly cubes: Array<{ name: string; sizePx: number; mips: number; category: MemoryCategory }>;
  readonly levels: Array<{ level: number; packed: Uint32Array }>;
  readonly writes: Array<{ name: string; data: Uint8Array }>;
  readonly released: string[];
}

/** An engine on a device without float32-blendable that records the CPU bake's calls. */
async function recordingEngine(): Promise<{ engine: RenderEngine; recorded: Recorded }> {
  const adapter = new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] });
  await adapter.requestDevice();
  const device = adapter.devices.at(0);
  if (device === undefined) {
    throw new Error("the fake adapter made no device");
  }
  const recorded: Recorded = { cubes: [], levels: [], writes: [], released: [] };
  const engine = Object.assign(new FakeRenderEngine(device), {
    createPackedCube: (
      sizePx: number,
      mips: number,
      category: MemoryCategory,
      name = "packed star cube",
    ): TextureHandle => {
      recorded.cubes.push({ name, sizePx, mips, category });
      return { kind: "texture", name };
    },
    writePackedCubeLevel: (_cube: TextureHandle, level: number, packed: Uint32Array): void => {
      recorded.levels.push({ level, packed });
    },
    createBuffer: (spec: BufferSpec): BufferHandle => ({
      kind: "buffer",
      name: spec.name,
      bytes: spec.bytes,
    }),
    writeBuffer: (buffer: BufferHandle, _offset: number, data: ArrayBufferView): void => {
      recorded.writes.push({
        name: buffer.name,
        data: new Uint8Array(data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength)),
      });
    },
    releaseBuffer: (buffer: BufferHandle): void => {
      recorded.released.push(buffer.name);
    },
    releaseTexture: (texture: TextureHandle): void => {
      recorded.released.push(texture.name);
    },
  });
  return { engine, recorded };
}

const DIRECTIONS = Float32Array.from([1, 0.1, -0.2, -0.3, 1, 0.4, 0.2, -0.5, -1, 0.9, 0.9, 0.9]);
const LIGHT = Float32Array.from([
  1e-6, 2e-6, 3e-6, 5e-7, 5e-7, 5e-7, 4e-8, 1e-8, 2e-8, 1e-5, 1e-5, 1e-5,
]);

describe("bakeSkyCube on the CPU", () => {
  it("writes every level of the TypeScript reference into a named sky-cube cube", async () => {
    const { engine, recorded } = await recordingEngine();
    const baked = bakeSkyCube(engine, {
      directions: DIRECTIONS,
      illuminanceLx: LIGHT,
      faceSizePx: 8,
      name: "sky cube: test",
    });
    // The reference: the six faces splatted, divided by solid angle, scaled, their chains packed.
    const faces = splatCpu(splatPoints(DIRECTIONS, LIGHT), 8);
    for (const face of faces) {
      divideBySolidAngle(face, 8, texelSolidAnglesSr(8));
    }
    const exponent = peakScaleExponent(faces);
    for (const face of faces) {
      scaleByPowerOfTwo(face, exponent);
    }
    const want = cubeLevels(faces.map((face) => faceMipChain(face, 8))).map((level) =>
      packRgb9e5Texels(level),
    );
    expect(baked.path).toBe("cpu");
    expect(recorded.cubes).toEqual([
      { name: "sky cube: test", sizePx: 8, mips: 4, category: "sky-cube" },
    ]);
    expect(recorded.levels.map((l) => l.level)).toEqual([0, 1, 2, 3]);
    recorded.levels.forEach(({ packed }, level) => {
      expect([...packed]).toEqual([...(want[level] ?? [])]);
    });
    // The peak's bits give the exponent applied: 2^(15 − k).
    const peak = recorded.writes.find((write) => write.name === "sky cube: test peak");
    expect(new Float32Array(peak?.data.buffer ?? new ArrayBuffer(4))[0]).toBe(2 ** (15 - exponent));
  });

  it("releases the cube and its peak", async () => {
    const { engine, recorded } = await recordingEngine();
    const baked = bakeSkyCube(engine, {
      directions: DIRECTIONS,
      illuminanceLx: LIGHT,
      faceSizePx: 4,
      name: "sky cube: test",
    });
    releaseBakedCube(engine, baked);
    expect(recorded.released).toEqual(["sky cube: test", "sky cube: test peak"]);
  });

  it("releases the cube it made when it fails partway (lane perf-long-tasks-3)", async () => {
    const { engine, recorded } = await recordingEngine();
    const failing = Object.assign(engine, {
      writePackedCubeLevel: (): void => {
        throw new Error("the device was lost");
      },
    });
    expect(() =>
      bakeSkyCube(failing, {
        directions: DIRECTIONS,
        illuminanceLx: LIGHT,
        faceSizePx: 4,
        name: "sky cube: test",
      }),
    ).toThrow(/lost/);
    expect([recorded.cubes.map((cube) => cube.name), recorded.released]).toEqual([
      ["sky cube: test"],
      ["sky cube: test"],
    ]);
  });

  it("refuses directions and light of different counts", async () => {
    const { engine } = await recordingEngine();
    expect(() =>
      bakeSkyCube(engine, {
        directions: DIRECTIONS,
        illuminanceLx: LIGHT.subarray(3),
        faceSizePx: 4,
        name: "x",
      }),
    ).toThrow(/same whole stars/);
  });
});

describe("paddedRowTexels", () => {
  it("pads a face's row of 4-byte texels to 256 bytes", () => {
    expect([
      paddedRowTexels(1),
      paddedRowTexels(64),
      paddedRowTexels(65),
      paddedRowTexels(3_072),
    ]).toEqual([64, 64, 128, 3_072]);
  });
});

/** A worker that answers each face size with `answer`, after a microtask, and counts its starts. */
function fakeWorkers(answer: (sizePx: number) => unknown): {
  start: () => SolidAnglesWorker;
  readonly started: () => number;
  readonly terminated: () => number;
} {
  let started = 0;
  let terminated = 0;
  return {
    start: () => {
      started += 1;
      const listeners = new Map<string, Array<(event: MessageEvent<unknown>) => void>>();
      return {
        addEventListener: (type: string, listener: (event: MessageEvent<unknown>) => void) => {
          listeners.set(type, [...(listeners.get(type) ?? []), listener]);
        },
        postMessage: (sizePx: number) => {
          queueMicrotask(() => {
            const data = answer(sizePx);
            const type = data instanceof Error ? "error" : "message";
            for (const listener of listeners.get(type) ?? []) {
              listener(new MessageEvent("message", { data }));
            }
          });
        },
        terminate: () => {
          terminated += 1;
        },
      };
    },
    started: () => started,
    terminated: () => terminated,
  };
}

describe("warmTexelSolidAngles (perf-long-tasks-2)", () => {
  it("keeps the worker's table for the face size, starting one worker a size", async () => {
    const workers = fakeWorkers((sizePx) => texelSolidAnglesLowerF32(sizePx));
    expect(hasTexelSolidAngles(8)).toBe(false);
    warmTexelSolidAngles(8, workers.start);
    warmTexelSolidAngles(8, workers.start);
    expect(workers.started()).toBe(1);
    await Promise.resolve();
    expect(hasTexelSolidAngles(8)).toBe(true);
    expect(workers.terminated()).toBe(1);
    warmTexelSolidAngles(8, workers.start);
    expect(workers.started()).toBe(1);
  });

  it("drops a reply of the wrong length or a failed worker, and tries again on the next call", async () => {
    const short = fakeWorkers(() => new Float32Array(3));
    warmTexelSolidAngles(16, short.start);
    await Promise.resolve();
    expect(hasTexelSolidAngles(16)).toBe(false);
    const failing = fakeWorkers(() => new Error("no worker"));
    warmTexelSolidAngles(16, failing.start);
    await Promise.resolve();
    expect(hasTexelSolidAngles(16)).toBe(false);
    expect(failing.terminated()).toBe(1);
    const good = fakeWorkers((sizePx) => texelSolidAnglesLowerF32(sizePx));
    warmTexelSolidAngles(16, good.start);
    await Promise.resolve();
    expect(hasTexelSolidAngles(16)).toBe(true);
  });

  it("does nothing where no worker can start", () => {
    warmTexelSolidAngles(24, () => null);
    expect(hasTexelSolidAngles(24)).toBe(false);
  });

  it("has the worker's job answer a face size with cube.ts's table and refuse anything else", () => {
    expect(solidAnglesReply(12)).toStrictEqual(texelSolidAnglesLowerF32(12));
    for (const message of ["12", 0, 1.5, null]) {
      expect(solidAnglesReply(message)).toBeNull();
    }
  });
});
