/**
 * The still images' store in the main process (plan R11, R11.T10.c, Design note 17): each still a
 * PNG and its JSON sidecar under `userData/stills/`, written atomically, after the sender, the size
 * and the label's schema are checked.
 *
 * @remarks
 * A still is a file rather than IndexedDB, because the renderer's origin differs between
 * development and production and a still is an artefact a player keeps; it is not ship state
 * (Design note 17). Each call is refused unless it comes from a window's own page (`ipcSender.ts`),
 * and its arguments are `unknown` until they are checked, the size first, before anything is
 * parsed: the PNG within the bound for the largest still, then the label of the schema
 * (`view/stills/stillLabel.ts`), then the PNG within the bound for the label's size, one pass over
 * the PNG (`view/stills/pngLabel.ts`), of the label's size, whose `hyperion-still` chunk is the
 * sidecar's label, text for text. One still is written at a time. Each file is written to a
 * temporary name beside it, synced and renamed over its own, the PNG before the sidecar, so that a
 * still is listed, by its sidecar, only once both are whole; a sidecar that fails takes its PNG
 * with it. A file is read through one handle, which must be a regular file within its bound, and a
 * file that cannot be read is passed over, so that an answer never carries a path.
 */

import { randomBytes } from "node:crypto";
import { constants } from "node:fs";
import { mkdir, open, readdir, rename, rm } from "node:fs/promises";
import { join } from "node:path";

import type {
  SavedStill,
  StillFile,
  StillLabel,
  StillRefusal,
  StillSaveAnswer,
  StillSize,
} from "../preload/api";
import { labelOfScan, scanPng } from "../renderer/src/view/stills/pngLabel";
import {
  parseStillLabel,
  STILL_LABEL_MAX_BYTES,
  STILL_MAX_SIDE_PX,
  stillLabelText,
} from "../renderer/src/view/stills/stillLabel";

/**
 * The stills' channels, one an operation; the preload repeats them (`preload/stillsApi.ts`'s
 * `STILLS_CHANNEL_NAMES`, which `stillsApi.test.ts` holds equal), and so does the smoke's
 * (`smoke/result.ts`'s `SMOKE_STILLS_CHANNELS`, which `result.test.ts` holds equal).
 */
export const STILLS_CHANNELS = {
  save: "hyperion:stills:save",
  list: "hyperion:stills:list",
  read: "hyperion:stills:read",
} as const;

/** The directory under `userData` that holds the stills (Design note 17). */
export const STILLS_DIR_NAME = "stills";

/** The most stills a listing gives, newest first: more than a session takes. */
export const STILL_LIST_MAX = 256;

/** The bytes beyond its scanlines' bound a still's PNG may take: its label, its other chunks. */
const PNG_ALLOWANCE_BYTES = 1 << 20;

/**
 * The most bytes a still of `size` may take as a PNG.
 *
 * @remarks
 * Its 8-bit RGBA scanlines, a filter byte and 4 bytes a pixel each (the PNG specification, Third
 * Edition, §7.2 and §7.3), are H × (1 + 4W) bytes; an encoder that cannot compress them stores them
 * in deflate blocks of at most 65,535 bytes with 5 bytes of framing each (RFC 1951 §3.2.4) in
 * chunks of 12 bytes' framing, so a sixty-fourth more than the scanlines bounds both for chunks of
 * 768 bytes or more; {@link PNG_ALLOWANCE_BYTES} more holds the label's chunk, which is at most
 * 64 KiB, and every other ancillary chunk. Plan R11's lean (2026-11-13).
 */
export function stillPngLimitBytes(size: StillSize): number {
  const scanlineBytes = size.heightPx * (1 + 4 * size.widthPx);
  return scanlineBytes + Math.ceil(scanlineBytes / 64) + PNG_ALLOWANCE_BYTES;
}

/** The most bytes any still's PNG may take: the largest still's bound, checked before anything else. */
export const MAX_STILL_PNG_BYTES = stillPngLimitBytes({
  widthPx: STILL_MAX_SIDE_PX,
  heightPx: STILL_MAX_SIDE_PX,
});

/** A still's ID: when it was saved, to the millisecond in UTC, and eight random hex digits. */
export const STILL_ID = /^\d{8}T\d{9}Z-[0-9a-f]{8}$/u;

/** `bytes` as hex digits, two a byte. */
function hexOf(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

/** The ID of a still saved at `now`, with `random`'s first four bytes as its hex digits. */
export function stillIdAt(now: Date, random: Uint8Array): string {
  const stamp = now.toISOString().replaceAll(/[-:.]/gu, "");
  return `${stamp}-${hexOf(random.subarray(0, 4))}`;
}

/** What checking a still's arguments gave: the still to write, or why it is refused. */
export type StillCheck =
  | { readonly kind: "accepted"; readonly png: Uint8Array; readonly label: StillLabel }
  | { readonly kind: "refused"; readonly reason: StillRefusal };

/** A refusal for `reason`. */
function refused(reason: StillRefusal): StillCheck {
  return { kind: "refused", reason };
}

/**
 * Checks a still's arguments as they arrived, `unknown` until checked: the size first, then the
 * label's schema, then the PNG against the label, in one pass over it.
 */
export function checkStill(png: unknown, label: unknown): StillCheck {
  if (!(png instanceof Uint8Array)) {
    return refused("not-png");
  }
  if (png.byteLength > MAX_STILL_PNG_BYTES) {
    return refused("oversized");
  }
  const checked = parseStillLabel(label);
  if (checked === null) {
    return refused("malformed-label");
  }
  if (png.byteLength > stillPngLimitBytes(checked.size)) {
    return refused("oversized");
  }
  const scan = scanPng(png);
  if (scan.kind === "not-png") {
    return refused("not-png");
  }
  if (scan.size.widthPx !== checked.size.widthPx || scan.size.heightPx !== checked.size.heightPx) {
    return refused("size-mismatch");
  }
  const embedded = labelOfScan(scan);
  if (embedded.kind !== "label" || embedded.text !== stillLabelText(checked)) {
    return refused("label-mismatch");
  }
  return { kind: "accepted", png, label: checked };
}

/** The system's code for a failed file operation, such as `ENOSPC`, or `write failed`. */
function failureReason(error: unknown): string {
  const code = error instanceof Error ? Reflect.get(error, "code") : undefined;
  return typeof code === "string" && /^E[A-Z]+$/u.test(code) ? code : "write failed";
}

/** Removes `path` if it is there, and says nothing of a failure: it never hides the first error. */
async function removeQuietly(path: string): Promise<void> {
  try {
    await rm(path, { force: true });
  } catch {
    // The first error is the one the caller answers with; a leftover temporary file is harmless.
  }
}

/**
 * The flags a still's file is read with: read only, never through a symbolic link where the
 * platform has the flag, and never waiting on a FIFO. Windows' `fs.constants` has neither
 * `O_NOFOLLOW` nor `O_NONBLOCK`, and a bitwise or reads the missing member as 0.
 */
const READ_FLAGS = constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK;

/**
 * The bytes of the file at `path`, read through one handle, where it is a regular file of at most
 * `maxBytes`; `null` for any other file, or one that cannot be read.
 */
async function readBounded(path: string, maxBytes: number): Promise<Uint8Array | null> {
  try {
    const file = await open(path, READ_FLAGS);
    try {
      const info = await file.stat();
      if (!info.isFile() || info.size > maxBytes) {
        return null;
      }
      const bytes = await file.readFile();
      return bytes.byteLength > maxBytes
        ? null
        : new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    } finally {
      await file.close();
    }
  } catch {
    return null;
  }
}

/** What a {@link StillFolder} needs besides its directory: a clock and random bytes. */
export interface StillFolderDeps {
  readonly now: () => Date;
  /** `count` random bytes. */
  readonly randomBytes: (count: number) => Uint8Array;
}

/** The clock and `node:crypto`'s random bytes. */
export const NODE_STILL_FOLDER_DEPS: StillFolderDeps = {
  now: () => new Date(),
  randomBytes: (count) => randomBytes(count),
};

/** The stills' directory: their PNGs and sidecars, `<id>.png` and `<id>.json`. */
export class StillFolder {
  readonly #dir: string;
  readonly #deps: StillFolderDeps;
  /** Whether a still is being written: one is written at a time. */
  #saving = false;

  /** @param dir - The directory, `userData/stills/`; made at the first save. */
  constructor(dir: string, deps: StillFolderDeps = NODE_STILL_FOLDER_DEPS) {
    this.#dir = dir;
    this.#deps = deps;
  }

  /**
   * Checks a still's arguments ({@link checkStill}) and, accepted, writes its two files; refuses a
   * still while another is being written.
   */
  async save(png: unknown, label: unknown): Promise<StillSaveAnswer> {
    if (this.#saving) {
      return { kind: "refused", reason: "busy" };
    }
    const checked = checkStill(png, label);
    if (checked.kind === "refused") {
      return checked;
    }
    this.#saving = true;
    const id = stillIdAt(this.#deps.now(), this.#deps.randomBytes(4));
    const pngPath = join(this.#dir, `${id}.png`);
    try {
      await mkdir(this.#dir, { recursive: true });
      await this.#writeAtomic(pngPath, checked.png);
      try {
        await this.#writeAtomic(
          join(this.#dir, `${id}.json`),
          new TextEncoder().encode(stillLabelText(checked.label)),
        );
      } catch (error: unknown) {
        // A PNG without its sidecar is never listed: it goes with the sidecar that failed.
        await removeQuietly(pngPath);
        throw error;
      }
    } catch (error: unknown) {
      return { kind: "failed", reason: failureReason(error) };
    } finally {
      this.#saving = false;
    }
    return { kind: "saved", still: { id, label: checked.label } };
  }

  /**
   * The saved stills, newest first, at most {@link STILL_LIST_MAX}: each sidecar of a still's ID
   * whose label is of the schema; a file of any other name or label, or that cannot be read, is
   * passed over, and a folder that cannot be read lists none.
   */
  async list(): Promise<ReadonlyArray<SavedStill>> {
    let names: string[];
    try {
      names = await readdir(this.#dir);
    } catch {
      return [];
    }
    const ids = names
      .filter((name) => name.endsWith(".json"))
      .map((name) => name.slice(0, -".json".length))
      .filter((id) => STILL_ID.test(id))
      .toSorted()
      .toReversed()
      .slice(0, STILL_LIST_MAX);
    const labels = await Promise.all(ids.map((id) => this.#readLabel(id)));
    return ids.flatMap((id, i) => {
      const label = labels[i] ?? null;
      return label === null ? [] : [{ id, label }];
    });
  }

  /**
   * The still `id` names, its PNG and its sidecar's label, or `null` where `id` is not a still's
   * ID, either file is missing, not a regular file, over its bound or cannot be read, or its
   * sidecar is not a label of the schema.
   */
  async read(id: unknown): Promise<StillFile | null> {
    if (typeof id !== "string" || !STILL_ID.test(id)) {
      return null;
    }
    const label = await this.#readLabel(id);
    if (label === null) {
      return null;
    }
    const png = await readBounded(join(this.#dir, `${id}.png`), stillPngLimitBytes(label.size));
    return png === null ? null : { id, png, label };
  }

  /** The label in `id`'s sidecar, or `null` where it cannot be read or is not of the schema. */
  async #readLabel(id: string): Promise<StillLabel | null> {
    const bytes = await readBounded(join(this.#dir, `${id}.json`), STILL_LABEL_MAX_BYTES);
    if (bytes === null) {
      return null;
    }
    try {
      return parseStillLabel(JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)));
    } catch {
      return null;
    }
  }

  /**
   * Writes `bytes` at `path` atomically: to a temporary file beside it, synced, then renamed over
   * it, so that `path` is either absent or whole; the temporary file is removed on a failure.
   */
  async #writeAtomic(path: string, bytes: Uint8Array): Promise<void> {
    const temporary = `${path}.${hexOf(this.#deps.randomBytes(4))}.tmp`;
    try {
      const file = await open(temporary, "wx");
      try {
        await file.writeFile(bytes);
        await file.sync();
      } finally {
        await file.close();
      }
      await rename(temporary, path);
    } catch (error: unknown) {
      await removeQuietly(temporary);
      throw error;
    }
  }
}

/** A still call from a sender that is not a window's own page. */
export class StillCallRefused extends Error {
  override readonly name = "StillCallRefused";
}

/** What the stills' handlers need: `ipcMain.handle`, the sender check, the folder and a log. */
export interface StillHandlerDeps<E> {
  readonly handle: (
    channel: string,
    listener: (event: E, ...args: unknown[]) => Promise<unknown>,
  ) => void;
  /** Whether `event` comes from a window's own page, its main frame (`ipcSender.ts`). */
  readonly isSender: (event: E) => boolean;
  readonly folder: StillFolder;
  /** Logs an unexpected failure in the main process, where its details stay. */
  readonly logError: (message: string, error: unknown) => void;
}

/**
 * Registers the stills' three handlers: each refuses a call that is not from a window's own page,
 * and hands its arguments, `unknown`, to the folder, which checks them.
 *
 * @remarks
 * Once a process: `ipcMain.handle` takes a channel once, and every window checks against its own
 * main frame through `isSender`. A failure the folder does not answer itself is logged here and
 * rejected with the channel's name alone, so that no path reaches the renderer.
 */
export function registerStillHandlers<E>(deps: StillHandlerDeps<E>): void {
  const guarded = (channel: string, run: (args: ReadonlyArray<unknown>) => Promise<unknown>) => {
    deps.handle(channel, async (event, ...args) => {
      if (!deps.isSender(event)) {
        throw new StillCallRefused(`${channel} refused: not a window's own page`);
      }
      try {
        return await run(args);
      } catch (error: unknown) {
        deps.logError(`${channel} failed:`, error);
        // Electron hands the renderer only the message: the cause, with any path in it, stays here.
        throw new Error(`${channel} failed`, { cause: error });
      }
    });
  };
  guarded(STILLS_CHANNELS.save, ([png, label]) => deps.folder.save(png, label));
  guarded(STILLS_CHANNELS.list, () => deps.folder.list());
  guarded(STILLS_CHANNELS.read, ([id]) => deps.folder.read(id));
}
