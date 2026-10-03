/**
 * GPU memory readings for the descent spike (plan R05, T14.c, Design note 18): the GPU process's
 * DRM fdinfo, and `nvidia-smi -q -x` where the driver is NVIDIA's.
 *
 * @remarks
 * DRM fdinfo is the kernel's per-client usage (Documentation/gpu/drm-usage-stats.rst): each open
 * DRM file of a process has an fdinfo with `drm-client-id`, and memory as `drm-total-<region>` and
 * `drm-resident-<region>` (amdgpu's older `drm-memory-<region>` is the resident amount). One client
 * can sit behind several file descriptors, and ANGLE and Dawn each open their own client, so the
 * reading sums over distinct client IDs on each device. NVIDIA's driver writes no `drm-*` keys at
 * all (checked on the RTX 3080 and driver 615.71.09, 2026-10-02: the fdinfo of `renderD128` holds
 * only `pos`, `flags`, `mnt_id` and `ino`), so there no DRM client is found, the reading is null
 * with that reason, and
 * `nvidia-smi` is the headline (decided 2026-09-30 by a delegated decision, hardware item 2).
 */

import { execFile } from "node:child_process";
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

/** One DRM client, from one fdinfo file. */
export interface DrmClient {
  readonly driver: string | null;
  /** The device's PCI address, as `drm-pdev` gives it, or `null` without one. */
  readonly pdev: string | null;
  readonly clientId: string;
  /** `drm-total-<region>`, bytes, by region. */
  readonly totalBytes: Readonly<Record<string, number>>;
  /** `drm-resident-<region>`, or `drm-memory-<region>` without it, bytes, by region. */
  readonly residentBytes: Readonly<Record<string, number>>;
}

/** A process's GPU memory from its DRM fdinfo, or why there is none. */
export type DrmMemoryReading =
  | {
      readonly kind: "drm";
      /** The sum over clients of every region's total, bytes; `null` unless every client gives one. */
      readonly totalBytes: number | null;
      /** The sum over clients of every region's resident amount, bytes. */
      readonly residentBytes: number;
      /** Distinct clients summed. */
      readonly clients: number;
      readonly drivers: ReadonlyArray<string>;
    }
  | { readonly kind: "unavailable"; readonly reason: string };

const UNIT_BYTES: Readonly<Record<string, number>> = {
  "": 1,
  KiB: 1024,
  MiB: 1024 ** 2,
  GiB: 1024 ** 3,
};

/** A memory value: an integer and an optional binary unit (bytes when absent). */
function parseBytes(text: string): number | undefined {
  const match = /^(\d+)\s*([KMG]iB)?$/.exec(text.trim());
  if (match === null) {
    return undefined;
  }
  const unit = UNIT_BYTES[match[2] ?? ""];
  return unit === undefined ? undefined : Number(match[1]) * unit;
}

/**
 * One fdinfo file's DRM client.
 *
 * @returns The client, or `null` when the file has no `drm-client-id` (not a DRM file, or a driver
 * that does not report usage).
 */
export function parseFdinfo(text: string): DrmClient | null {
  const keys = new Map<string, string>();
  for (const line of text.split("\n")) {
    const colon = line.indexOf(":");
    if (colon > 0) {
      keys.set(line.slice(0, colon).trim(), line.slice(colon + 1).trim());
    }
  }
  const clientId = keys.get("drm-client-id");
  if (clientId === undefined) {
    return null;
  }
  const totalBytes: Record<string, number> = {};
  const residentBytes: Record<string, number> = {};
  const legacy: Record<string, number> = {};
  for (const [key, value] of keys) {
    const memory = /^drm-(total|resident|memory)-(.+)$/.exec(key);
    const bytes = memory === null ? undefined : parseBytes(value);
    if (memory === null || bytes === undefined) {
      continue;
    }
    const [, kind, region = ""] = memory;
    const target = kind === "total" ? totalBytes : kind === "resident" ? residentBytes : legacy;
    target[region] = bytes;
  }
  for (const [region, bytes] of Object.entries(legacy)) {
    residentBytes[region] ??= bytes;
  }
  return {
    driver: keys.get("drm-driver") ?? null,
    pdev: keys.get("drm-pdev") ?? null,
    clientId,
    totalBytes,
    residentBytes,
  };
}

function sumValues(record: Readonly<Record<string, number>>): number {
  let sum = 0;
  for (const value of Object.values(record)) {
    sum += value;
  }
  return sum;
}

/**
 * The memory of a set of DRM clients, each distinct client (device and client ID) counted once.
 *
 * @param noClientReason - The reason given when there is no client with memory keys.
 */
export function sumDrmClients(
  clients: ReadonlyArray<DrmClient>,
  noClientReason: string,
): DrmMemoryReading {
  const distinct = new Map<string, DrmClient>();
  for (const client of clients) {
    distinct.set(`${client.pdev ?? ""}/${client.clientId}`, client);
  }
  const withMemory = [...distinct.values()].filter(
    (client) =>
      Object.keys(client.totalBytes).length > 0 || Object.keys(client.residentBytes).length > 0,
  );
  if (withMemory.length === 0) {
    return { kind: "unavailable", reason: noClientReason };
  }
  // A total from some clients only would understate the whole, so it is all or nothing.
  const total = withMemory.every((client) => Object.keys(client.totalBytes).length > 0)
    ? withMemory.reduce((sum, client) => sum + sumValues(client.totalBytes), 0)
    : null;
  let resident = 0;
  for (const client of withMemory) {
    resident += sumValues(client.residentBytes);
  }
  const drivers = [...new Set(withMemory.map(({ driver }) => driver ?? "unknown"))].toSorted();
  return {
    kind: "drm",
    totalBytes: total,
    residentBytes: resident,
    clients: withMemory.length,
    drivers,
  };
}

/** The file system calls {@link readDrmMemory} makes, for tests. */
export interface FdinfoFiles {
  readdir(path: string): Promise<ReadonlyArray<string>>;
  readFile(path: string): Promise<string>;
}

const NODE_FILES: FdinfoFiles = {
  readdir: (path) => readdir(path),
  readFile: (path) => readFile(path, "utf8"),
};

/**
 * The GPU memory a process holds, from `/proc/<pid>/fdinfo`.
 *
 * @returns The reading, or the reason there is none: not Linux, an unreadable directory, or no
 * DRM client with memory keys, as on NVIDIA.
 */
export async function readDrmMemory(
  pid: number,
  platform: NodeJS.Platform,
  files: FdinfoFiles = NODE_FILES,
): Promise<DrmMemoryReading> {
  if (platform !== "linux") {
    return { kind: "unavailable", reason: `DRM fdinfo is Linux's; this is ${platform}` };
  }
  const dir = `/proc/${pid}/fdinfo`;
  let entries: ReadonlyArray<string>;
  try {
    entries = await files.readdir(dir);
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    return { kind: "unavailable", reason: `${dir} could not be read: ${message}` };
  }
  const read = await Promise.all(
    entries.map((entry) =>
      files.readFile(join(dir, entry)).then(
        parseFdinfo,
        () =>
          // A descriptor closed between the listing and the read has nothing to report.
          null,
      ),
    ),
  );
  const clients = read.filter((client): client is DrmClient => client !== null);
  return sumDrmClients(
    clients,
    clients.length === 0
      ? "no DRM client in the process's fdinfo (NVIDIA's driver writes none)"
      : "the driver reports no drm-* memory keys",
  );
}

/** One process's memory on an NVIDIA GPU. */
export interface NvidiaProcess {
  readonly pid: number;
  /** `G` (graphics), `C` (compute) or `C+G`. */
  readonly type: string;
  readonly name: string;
  readonly usedBytes: number | null;
}

/** One NVIDIA GPU's memory. */
export interface NvidiaGpu {
  /** The PCI bus ID, as the `<gpu id=…>` attribute gives it. */
  readonly busId: string;
  readonly productName: string;
  readonly totalBytes: number | null;
  readonly reservedBytes: number | null;
  readonly usedBytes: number | null;
  readonly processes: ReadonlyArray<NvidiaProcess>;
}

/** What `nvidia-smi -q -x` reports, or why it reports nothing. */
export type NvidiaReading =
  | {
      readonly kind: "nvidia";
      readonly driverVersion: string;
      readonly gpus: ReadonlyArray<NvidiaGpu>;
    }
  | { readonly kind: "unavailable"; readonly reason: string };

/** The text of the first `<tag>` in `xml`, or `undefined`. */
function tagText(xml: string, tag: string): string | undefined {
  const match = new RegExp(`<${tag}>([^<]*)</${tag}>`).exec(xml);
  return match?.[1]?.trim();
}

/** The inner XML of the first `<tag>…</tag>` block, or `undefined`. */
function block(xml: string, tag: string): string | undefined {
  const start = xml.indexOf(`<${tag}>`);
  const end = xml.indexOf(`</${tag}>`, start);
  return start < 0 || end < 0 ? undefined : xml.slice(start + tag.length + 2, end);
}

/**
 * Parses `nvidia-smi -q -x`.
 *
 * @remarks
 * Only the elements read are parsed, by pattern: `driver_version`, each `<gpu id>` with its
 * `product_name`, its `fb_memory_usage` (`total`, `reserved`, `used`, in MiB) and its
 * `processes`' `process_info` (`pid`, `type`, `process_name`, `used_memory`). A value of `N/A`
 * reads as `null`.
 */
export function parseNvidiaSmi(xml: string): NvidiaReading {
  const driverVersion = tagText(xml, "driver_version") ?? tagText(xml, "kmd_version");
  if (driverVersion === undefined) {
    return { kind: "unavailable", reason: "nvidia-smi's output has no driver version" };
  }
  const gpus: NvidiaGpu[] = [];
  for (const match of xml.matchAll(/<gpu id="([^"]*)">([\s\S]*?)<\/gpu>/g)) {
    const [, busId = "", body = ""] = match;
    const fb = block(body, "fb_memory_usage") ?? "";
    const processes: NvidiaProcess[] = [];
    for (const info of (block(body, "processes") ?? "").matchAll(
      /<process_info>([\s\S]*?)<\/process_info>/g,
    )) {
      const process = info[1] ?? "";
      processes.push({
        pid: Number(tagText(process, "pid") ?? Number.NaN),
        type: tagText(process, "type") ?? "",
        name: tagText(process, "process_name") ?? "",
        usedBytes: parseBytes(tagText(process, "used_memory") ?? "") ?? null,
      });
    }
    gpus.push({
      busId,
      productName: tagText(body, "product_name") ?? "",
      totalBytes: parseBytes(tagText(fb, "total") ?? "") ?? null,
      reservedBytes: parseBytes(tagText(fb, "reserved") ?? "") ?? null,
      usedBytes: parseBytes(tagText(fb, "used") ?? "") ?? null,
      processes: processes.filter(({ pid }) => Number.isInteger(pid)),
    });
  }
  if (gpus.length === 0) {
    return { kind: "unavailable", reason: "nvidia-smi lists no GPU" };
  }
  return { kind: "nvidia", driverVersion, gpus };
}

/** Runs a command and resolves its standard output, for tests. */
export type RunCommand = (file: string, args: ReadonlyArray<string>) => Promise<string>;

/** How long `nvidia-smi` may take before its reading is given up, ms. */
const NVIDIA_SMI_TIMEOUT_MS = 5000;

const RUN_COMMAND: RunCommand = (file, args) =>
  new Promise((resolve, reject) => {
    execFile(file, [...args], { timeout: NVIDIA_SMI_TIMEOUT_MS }, (error, stdout) => {
      if (error === null) {
        resolve(stdout);
      } else {
        reject(error);
      }
    });
  });

/**
 * Reads `nvidia-smi -q -x`.
 *
 * @returns The reading, or why there is none: no `nvidia-smi` on the path (any non-NVIDIA
 * machine), or a run that failed.
 */
export async function readNvidiaSmi(run: RunCommand = RUN_COMMAND): Promise<NvidiaReading> {
  let xml: string;
  try {
    xml = await run("nvidia-smi", ["-q", "-x"]);
  } catch (error: unknown) {
    const code =
      error instanceof Error && "code" in error && typeof error.code === "string"
        ? error.code
        : undefined;
    const reason =
      code === "ENOENT"
        ? "no nvidia-smi on this machine"
        : `nvidia-smi failed: ${error instanceof Error ? error.message : String(error)}`;
    return { kind: "unavailable", reason };
  }
  return parseNvidiaSmi(xml);
}
