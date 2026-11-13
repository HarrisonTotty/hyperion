import { mkdir, mkdtemp, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { pngChunk, rgbaPng } from "../renderer/src/test/pngFixtures";
import { testStillLabel } from "../renderer/src/test/stillFixtures";
import { withStillLabel } from "../renderer/src/view/stills/pngLabel";
import { STILL_MAX_SIDE_PX, stillLabelText } from "../renderer/src/view/stills/stillLabel";
import {
  checkStill,
  MAX_STILL_PNG_BYTES,
  registerStillHandlers,
  STILL_ID,
  STILL_LIST_MAX,
  StillCallRefused,
  StillFolder,
  type StillFolderDeps,
  stillIdAt,
  stillPngLimitBytes,
  STILLS_CHANNELS,
} from "./stills";

/** A clock that steps a millisecond a call from 2026-11-13, and random bytes that count up. */
function stepping(): StillFolderDeps {
  let ms = Date.UTC(2026, 10, 13, 12, 0, 0, 0);
  let n = 0;
  return {
    now: () => new Date((ms += 1)),
    randomBytes: (count) => Uint8Array.from({ length: count }, () => (n += 1) & 0xff),
  };
}

/** A labelled still's PNG and its label, as the renderer hands them over. */
function still(setting: "high" | "low" = "high") {
  const label = testStillLabel({ setting });
  return { png: withStillLabel(rgbaPng(16, 8), label), label };
}

/**
 * The most bytes an encoder that cannot compress takes for a still: stored deflate blocks in IDAT
 * chunks of 8 KiB, libpng's, with the label's chunk at its largest.
 */
function worstBytes(widthPx: number, heightPx: number): number {
  const scanlines = heightPx * (1 + 4 * widthPx);
  // zlib's 2-byte header and 4-byte check, 5 bytes a stored block of at most 65,535.
  const zlib = scanlines + 6 + 5 * Math.ceil(scanlines / 65_535);
  // Signature, IHDR and IEND; the IDAT chunks' framing; the label's chunk at its largest.
  return 8 + 25 + 12 + zlib + 12 * Math.ceil(zlib / 8192) + 12 + 20 + 65_536;
}

let dir = "";

beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), "hyperion-stills-"));
});

afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

describe("a still's arguments, checked (R11.T10.c)", () => {
  it("accepts a labelled PNG of its label's size, whose chunk is its sidecar's label", () => {
    const { png, label } = still();
    expect(checkStill(png, label)).toEqual({ kind: "accepted", png, label });
  });

  it("refuses an image over the largest still's bound before it reads the label", () => {
    // The bound is checked on the length alone: no byte of this array is read.
    const huge = new Uint8Array(new ArrayBuffer(MAX_STILL_PNG_BYTES + 1));
    expect(checkStill(huge, { not: "a label" })).toEqual({ kind: "refused", reason: "oversized" });
  });

  it("refuses an image over its label's size's bound", () => {
    const { label } = still();
    const png = new Uint8Array(stillPngLimitBytes(label.size) + 1);
    expect(checkStill(png, label)).toEqual({ kind: "refused", reason: "oversized" });
  });

  it("holds an incompressible still within its bound: stored deflate in IDAT chunks of 8 KiB", () => {
    expect([
      worstBytes(7680, 4320) <= stillPngLimitBytes({ widthPx: 7680, heightPx: 4320 }),
      worstBytes(1, 1) <= stillPngLimitBytes({ widthPx: 1, heightPx: 1 }),
      worstBytes(STILL_MAX_SIDE_PX, STILL_MAX_SIDE_PX) <= MAX_STILL_PNG_BYTES,
    ]).toEqual([true, true, true]);
  });

  it.each([
    ["a malformed label", { schema: "hyperion-still/1" }],
    ["no label", undefined],
    ["a label of another schema", { ...testStillLabel(), schema: "other" }],
  ])("refuses %s", (_, label) => {
    expect(checkStill(still().png, label)).toEqual({
      kind: "refused",
      reason: "malformed-label",
    });
  });

  it("refuses an image that is not bytes, or not a PNG", () => {
    const { label } = still();
    expect([checkStill("png", label), checkStill(new Uint8Array(32), label)]).toEqual([
      { kind: "refused", reason: "not-png" },
      { kind: "refused", reason: "not-png" },
    ]);
  });

  it("refuses a PNG of another size than its label's", () => {
    const label = testStillLabel();
    const png = withStillLabel(rgbaPng(16, 8), label);
    expect(checkStill(png, { ...label, size: { widthPx: 8, heightPx: 16 } })).toEqual({
      kind: "refused",
      reason: "size-mismatch",
    });
  });

  it("refuses a PNG whose chunk is not its sidecar's label, or that has none", () => {
    const label = testStillLabel();
    const other = withStillLabel(rgbaPng(16, 8), testStillLabel({ setting: "low" }));
    expect([checkStill(other, label), checkStill(rgbaPng(16, 8), label)]).toEqual([
      { kind: "refused", reason: "label-mismatch" },
      { kind: "refused", reason: "label-mismatch" },
    ]);
  });
});

describe("the stills' folder (R11.T10.c)", () => {
  it("names a still by when it was saved and its random bytes", () => {
    const id = stillIdAt(
      new Date(Date.UTC(2026, 10, 13, 1, 2, 3, 45)),
      Uint8Array.of(1, 2, 254, 255),
    );
    expect([id, STILL_ID.test(id)]).toEqual(["20261113T010203045Z-0102feff", true]);
  });

  it("writes a still's PNG and its sidecar, and nothing else", async () => {
    const folder = new StillFolder(join(dir, "stills"), stepping());
    const { png, label } = still();
    const answer = await folder.save(png, label);
    const id = answer.kind === "saved" ? answer.still.id : "";
    const names = (await readdir(join(dir, "stills"))).toSorted();
    expect([
      names,
      new Uint8Array(await readFile(join(dir, "stills", `${id}.png`))),
      await readFile(join(dir, "stills", `${id}.json`), "utf8"),
    ]).toEqual([[`${id}.json`, `${id}.png`], png, stillLabelText(label)]);
  });

  it("writes nothing for a refused still", async () => {
    const folder = new StillFolder(join(dir, "stills"), stepping());
    const answer = await folder.save(rgbaPng(16, 8), testStillLabel());
    expect([answer, await folder.list()]).toEqual([
      { kind: "refused", reason: "label-mismatch" },
      [],
    ]);
  });

  it("answers a failed write with the system's code, leaving no file behind", async () => {
    // A file where the directory should be: the directory cannot be made.
    const blocked = join(dir, "stills");
    await writeFile(blocked, "");
    const { png, label } = still();
    const answer = await new StillFolder(blocked, stepping()).save(png, label);
    expect([answer.kind, await readdir(dir)]).toEqual(["failed", ["stills"]]);
  });

  it("lists the stills newest first, passing over other files and broken sidecars", async () => {
    const folder = new StillFolder(dir, stepping());
    const first = await folder.save(still("low").png, still("low").label);
    const second = await folder.save(still().png, still().label);
    await writeFile(join(dir, "notes.json"), "{}");
    await writeFile(join(dir, "20261113T120000000Z-ffffffff.json"), "{ not json");
    await writeFile(join(dir, "20261113T120000001Z-fffffffe.json"), '{"schema":"other"}');
    const ids = (await folder.list()).map(({ id }) => id);
    expect(ids).toEqual(
      [second, first].map((answer) => (answer.kind === "saved" ? answer.still.id : "")),
    );
  });

  it("lists at most its bound of stills", async () => {
    const text = stillLabelText(testStillLabel());
    await Promise.all(
      Array.from({ length: STILL_LIST_MAX + 3 }, (_, i) =>
        writeFile(join(dir, `20261113T${String(i).padStart(9, "0")}Z-00000000.json`), text),
      ),
    );
    expect((await new StillFolder(dir, stepping()).list()).length).toBe(STILL_LIST_MAX);
  });

  it("lists nothing from a folder not yet made", async () => {
    expect(await new StillFolder(join(dir, "stills"), stepping()).list()).toEqual([]);
  });

  it("reads a still back, its PNG and its sidecar's label", async () => {
    const folder = new StillFolder(dir, stepping());
    const { png, label } = still();
    const answer = await folder.save(png, label);
    const id = answer.kind === "saved" ? answer.still.id : "";
    expect(await folder.read(id)).toEqual({ id, png, label });
  });

  it.each([
    ["a path out of the folder", "../stills/20261113T120000000Z-00000000"],
    ["a name that is not an ID", "notes"],
    ["no string", 7],
    ["an ID of no still", "20261113T120000000Z-00000000"],
  ])("reads nothing for %s", async (_, id) => {
    expect(await new StillFolder(dir, stepping()).read(id)).toBeNull();
  });

  it("reads nothing where the PNG has gone", async () => {
    const folder = new StillFolder(dir, stepping());
    const answer = await folder.save(still().png, still().label);
    const id = answer.kind === "saved" ? answer.still.id : "";
    await rm(join(dir, `${id}.png`));
    expect(await folder.read(id)).toBeNull();
  });
});

/** The handlers registered over a folder in `dir`, by channel, and a sender that is or is not own. */
function handlers(folder: StillFolder = new StillFolder(dir, stepping())) {
  const registered = new Map<
    string,
    (event: { own: boolean }, ...args: unknown[]) => Promise<unknown>
  >();
  const logError = vi.fn<(message: string, error: unknown) => void>();
  registerStillHandlers<{ own: boolean }>({
    handle: (channel, listener) => {
      registered.set(channel, listener);
    },
    isSender: (event) => event.own,
    folder,
    logError,
  });
  const call = (channel: string, own: boolean, ...args: unknown[]): Promise<unknown> => {
    const listener = registered.get(channel);
    if (listener === undefined) {
      throw new Error(`no handler on ${channel}`);
    }
    return listener({ own }, ...args);
  };
  return { registered, call, logError };
}

describe("the stills' handlers (R11.T10.c)", () => {
  it("registers one handler an operation", () => {
    expect([...handlers().registered.keys()]).toEqual([
      STILLS_CHANNELS.save,
      STILLS_CHANNELS.list,
      STILLS_CHANNELS.read,
    ]);
  });

  it("refuses a foreign sender on every channel, writing and reading nothing", async () => {
    const { call } = handlers();
    const { png, label } = still();
    const refused = await Promise.allSettled([
      call(STILLS_CHANNELS.save, false, png, label),
      call(STILLS_CHANNELS.list, false),
      call(STILLS_CHANNELS.read, false, "20261113T120000000Z-00000000"),
    ]);
    expect([
      refused.map((each) => each.status === "rejected" && each.reason instanceof StillCallRefused),
      await readdir(dir),
    ]).toEqual([[true, true, true], []]);
  });

  it("saves, lists and reads back for the window's own page", async () => {
    const { call } = handlers();
    const { png, label } = still();
    const saved = await call(STILLS_CHANNELS.save, true, png, label);
    const id = (await readdir(dir)).find((name) => name.endsWith(".png"))?.slice(0, -".png".length);
    expect([
      saved,
      await call(STILLS_CHANNELS.list, true),
      await call(STILLS_CHANNELS.read, true, id),
    ]).toEqual([{ kind: "saved", still: { id, label } }, [{ id, label }], { id, png, label }]);
  });

  it("refuses an oversized image and a malformed label for the own page too", async () => {
    const { call } = handlers();
    const { png, label } = still();
    expect([
      await call(STILLS_CHANNELS.save, true, new Uint8Array(MAX_STILL_PNG_BYTES + 1), label),
      await call(STILLS_CHANNELS.save, true, png, { ...label, decoration: "yes" }),
    ]).toEqual([
      { kind: "refused", reason: "oversized" },
      { kind: "refused", reason: "malformed-label" },
    ]);
  });

  it("refuses a PNG with a chunk past its end", () => {
    const { png, label } = still();
    const longer = Uint8Array.from([...png, ...pngChunk("tEXt", new Uint8Array(4))]);
    expect(checkStill(longer, label)).toEqual({ kind: "refused", reason: "not-png" });
  });

  it("rejects a failure the folder does not answer with the channel's name alone, and logs it", async () => {
    class Broken extends StillFolder {
      override list(): Promise<never> {
        return Promise.reject(new Error("EACCES: /home/someone/.config/hyperion/stills"));
      }
    }
    const { call, logError } = handlers(new Broken(dir, stepping()));
    await expect(call(STILLS_CHANNELS.list, true)).rejects.toThrow(
      /^hyperion:stills:list failed$/u,
    );
    expect(logError).toHaveBeenCalledTimes(1);
  });
});

describe("the stills' folder, at its edges (R11.T10.c's security review)", () => {
  it("refuses a second still while one is being written, and takes the next after it", async () => {
    const folder = new StillFolder(dir, stepping());
    const { png, label } = still();
    const first = folder.save(png, label);
    const second = await folder.save(png, label);
    const answers = [(await first).kind, second, (await folder.save(png, label)).kind];
    expect(answers).toEqual(["saved", { kind: "refused", reason: "busy" }, "saved"]);
  });

  it("takes the PNG away again where its sidecar cannot be written", async () => {
    const folder = new StillFolder(dir, stepping());
    // The first save's ID, from the stepping clock and bytes: a directory in its sidecar's place.
    const id = stillIdAt(new Date(Date.UTC(2026, 10, 13, 12, 0, 0, 1)), Uint8Array.of(1, 2, 3, 4));
    await mkdir(join(dir, `${id}.json`));
    const { png, label } = still();
    const answer = await folder.save(png, label);
    expect([answer.kind, await readdir(dir)]).toEqual(["failed", [`${id}.json`]]);
  });

  it("passes over a sidecar that cannot be read as a file, listing the rest", async () => {
    const folder = new StillFolder(dir, stepping());
    const saved = await folder.save(still().png, still().label);
    await mkdir(join(dir, "20261113T235959999Z-ffffffff.json"));
    const ids = (await folder.list()).map(({ id }) => id);
    expect(ids).toEqual([saved.kind === "saved" ? saved.still.id : ""]);
  });

  it("refuses a label whose fields are a typed array, before it walks one", () => {
    const { png, label } = still();
    expect(checkStill(png, { ...label, camera: new Uint8Array(1_000_000) })).toEqual({
      kind: "refused",
      reason: "malformed-label",
    });
  });

  it("reads a PNG of many tiny chunks in one pass, keeping none of them", () => {
    const { png, label } = still();
    // 100,000 empty ancillary chunks after IHDR: 1.2 MB of frames.
    const empty = pngChunk("tEXt", new Uint8Array(0));
    const filler = new Uint8Array(empty.byteLength * 100_000);
    for (let i = 0; i < 100_000; i += 1) {
      filler.set(empty, i * empty.byteLength);
    }
    const at = 8 + 25;
    const padded = new Uint8Array(png.byteLength + filler.byteLength);
    padded.set(png.subarray(0, at), 0);
    padded.set(filler, at);
    padded.set(png.subarray(at), at + filler.byteLength);
    // Over the bound of a 16 × 8 still; within a 512 × 512 one's, whose size it is not.
    expect([
      checkStill(padded, label),
      checkStill(padded, { ...label, size: { widthPx: 512, heightPx: 512 } }),
    ]).toEqual([
      { kind: "refused", reason: "oversized" },
      { kind: "refused", reason: "size-mismatch" },
    ]);
  });
});
