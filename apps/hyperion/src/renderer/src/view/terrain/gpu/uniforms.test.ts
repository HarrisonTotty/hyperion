import { describe, expect, it } from "vitest";

import vertexGolden from "../../../../../../../../crates/hyperion-surface/tests/golden/vertex_f32.golden?raw";
import { vec3 } from "../../../geometry/vec3";
import type { Face, PatchKey } from "../patchKey";
import {
  CONTACT_RECORD_BYTES,
  CONTACTS_HEADER_BYTES,
  ContactRecords,
  INSTANCE_RECORD_BYTES,
  InstanceRecords,
  MAX_CONTACTS,
  patchTerms,
  SLOT_RECORD_BYTES,
  writeSlotRecord,
} from "./uniforms";

/** WGS 84, the test planet's figure (NIMA TR8350.2): a and c = a (1 − 1 ÷ 298.257223563). */
const WGS84 = { equatorialRadiusM: 6_378_137, polarRadiusM: 6_378_137 * (1 - 1 / 298.257223563) };

function bitsOf(x: number): number {
  const f = new Float32Array([x]);
  return new Uint32Array(f.buffer)[0] ?? Number.NaN;
}

function valueOf(hex: string): number {
  const u = new Uint32Array([Number.parseInt(hex, 16)]);
  return new Float32Array(u.buffer)[0] ?? Number.NaN;
}

interface GoldenPatch {
  readonly key: PatchKey;
  /** The `terms` line's 24 values as `f32` bits, then `straddles`. */
  readonly terms: ReadonlyArray<number>;
  readonly straddles: boolean;
}

function isFace(n: number): n is Face {
  return Number.isInteger(n) && n >= 0 && n <= 5;
}

/** The patches and their `terms` lines in T4.b's golden (its format in `tests/vertex_golden.rs`). */
function goldenPatches(): GoldenPatch[] {
  const patches: GoldenPatch[] = [];
  let key: PatchKey | null = null;
  for (const line of vertexGolden.split("\n")) {
    const words = line.trim().split(/\s+/);
    if (words[0] === "patch") {
      const [face, level, i, j] = words.slice(2).map(Number);
      if (
        face === undefined ||
        !isFace(face) ||
        level === undefined ||
        i === undefined ||
        j === undefined
      ) {
        throw new Error(`a malformed patch line: ${line}`);
      }
      key = { face, level, i, j };
    } else if (words[0] === "terms") {
      if (key === null) {
        throw new Error("a terms line before any patch");
      }
      const values = words.slice(1, 25).map((hex) => Number.parseInt(hex, 16));
      patches.push({ key, terms: values, straddles: words[25] === "1" });
      key = null;
    }
  }
  return patches;
}

describe("a slot record", () => {
  const patches = goldenPatches();

  it("reads twelve patches from T4.b's golden", () => {
    expect(patches).toHaveLength(12);
  });

  it("holds the Rust f32 terms of every golden patch bit for bit", () => {
    for (const { key, terms, straddles } of patches) {
      const h0 = valueOf((terms[23] ?? 0).toString(16));
      const out = new ArrayBuffer(2 * SLOT_RECORD_BYTES);
      writeSlotRecord(out, 1, patchTerms(key, WGS84, h0), 12.5);
      const f = new Float32Array(out, SLOT_RECORD_BYTES);
      const u = new Uint32Array(out, SLOT_RECORD_BYTES);
      const bits = (word: number): number => bitsOf(f[word] ?? Number.NaN);
      // The golden's order: a, e₁, e₂, s₀, t₀, u₀, v₀, step, scale, m₀, ν₀, h₀.
      const recordWords = [
        0, 1, 2, 4, 5, 6, 8, 9, 10, 3, 7, 11, 15, 19, 12, 13, 14, 16, 17, 18, 20, 21, 22, 23,
      ];
      expect(recordWords.map(bits), `patch ${JSON.stringify(key)}`).toEqual(terms);
      expect(f[24]).toBe(12.5);
      expect(u[25]).toBe(straddles ? 1 : 0);
    }
  });

  it("refuses a slot outside its buffer", () => {
    const terms = patchTerms({ face: 0, level: 3, i: 1, j: 2 }, WGS84, 0);
    expect(() => {
      writeSlotRecord(new ArrayBuffer(SLOT_RECORD_BYTES), 1, terms, 0);
    }).toThrow(RangeError);
  });
});

describe("an instance record", () => {
  it("carries the f64 camera-relative origin narrowed once, the slot and the morph range", () => {
    const origin = vec3(6_378_137.3, -12.25, 4.0);
    const camera = vec3(6_378_000.1, 0, 0);
    const relative = vec3(origin.x - camera.x, origin.y - camera.y, origin.z - camera.z);
    const records = new InstanceRecords(4);
    records.push(7, relative, 100, 200);
    const f = new Float32Array(records.buffer);
    const u = new Uint32Array(records.buffer);
    expect(f[0]).toBe(Math.fround(origin.x - camera.x));
    // Narrowing each end first would lose the difference to f32's 0.5 m step at an Earth's radius.
    expect(Math.fround(origin.x) - Math.fround(camera.x)).not.toBe(f[0]);
    expect([f[1], f[2], u[3], f[4], f[5]]).toEqual([-12.25, 4, 7, 100, 200]);
    expect(records.bytes().byteLength).toBe(INSTANCE_RECORD_BYTES);
  });

  it("refuses more records than its capacity", () => {
    const records = new InstanceRecords(1);
    records.push(0, vec3(0, 0, 0), 0, 1);
    expect(() => {
      records.push(1, vec3(0, 0, 0), 0, 1);
    }).toThrow(RangeError);
  });

  it("starts each frame empty", () => {
    const records = new InstanceRecords(1);
    records.push(0, vec3(0, 0, 0), 0, 1);
    records.clear();
    expect(records.count).toBe(0);
    expect(records.bytes().byteLength).toBe(0);
  });
});

describe("the contacts buffer", () => {
  it("counts its contacts in the header and carries each one's centre, radius and ramp", () => {
    const contacts = new ContactRecords();
    contacts.push(vec3(1, 2, 3), 30, 17.7);
    contacts.push(vec3(-1, 0, 0.5), 10, 17.7);
    const u = new Uint32Array(contacts.buffer);
    const f = new Float32Array(contacts.buffer);
    expect(u[0]).toBe(2);
    const second = (CONTACTS_HEADER_BYTES + CONTACT_RECORD_BYTES) / 4;
    expect([f[second], f[second + 1], f[second + 2], f[second + 3]]).toEqual([-1, 0, 0.5, 10]);
    expect(f[second + 4]).toBe(Math.fround(17.7));
    expect(contacts.bytes().byteLength).toBe(CONTACTS_HEADER_BYTES + 2 * CONTACT_RECORD_BYTES);
    contacts.clear();
    expect(u[0]).toBe(0);
  });

  it("refuses a negative radius", () => {
    expect(() => {
      new ContactRecords().push(vec3(0, 0, 0), -1, 1);
    }).toThrow(RangeError);
  });

  it("refuses more contacts than its capacity", () => {
    const contacts = new ContactRecords();
    for (let n = 0; n < MAX_CONTACTS; n += 1) {
      contacts.push(vec3(n, 0, 0), 1, 1);
    }
    expect(() => {
      contacts.push(vec3(0, 0, 0), 1, 1);
    }).toThrow(RangeError);
  });
});
