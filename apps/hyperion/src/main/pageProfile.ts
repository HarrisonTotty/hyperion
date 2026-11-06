/**
 * The inspection profile's CPU profile of the page's main thread over the whole run (lane
 * perf-page-long-tasks): V8's sampling profiler over the window's own debugger, from the page's
 * load to the timing file's writing, by which each of the page's long tasks is told by what ran in
 * it, outside the windows' traces as well as in them.
 *
 * @remarks
 * Chromium attributes no script to a long animation frame of the app's `file:` page
 * (`longTasks.ts`), and the windows' traces cover only their own frames, so a task between them
 * (a bake's upload, a sky's reply) is named here: its samples' functions by self time, and the
 * outermost named function of the app they ran under (`onMessage`, `tick`), its entry. The
 * profile samples every millisecond; a run of two hours holds some 6 million samples, some 60 MB
 * of JSON, read once at the end. Its clock is the trace's, set against the page's by
 * `performance.now()` read just before the profile stops, within the round trip of one evaluation.
 * A profile is read at a trust boundary: one not shaped as V8's is none.
 */

/** One function's sampled time, on a thread over a window or the page's thread over a task. */
export interface ProfiledFunction {
  /** Its name, script file and line (`surveyEdges styles-….js:36206`), or V8's own (`(program)`). */
  readonly name: string;
  readonly ms: number;
}

/** The longest function name kept, characters: a Rust symbol of the surface module runs to 400. */
const FUNCTION_NAME_CHARS = 120;

/** The debugger's part the profile uses: Electron's `webContents.debugger`. */
export interface PageDebugger {
  attach(protocolVersion?: string): void;
  isAttached(): boolean;
  sendCommand(method: string, commandParams?: unknown): Promise<unknown>;
}

/** The profile's sampling interval, µs: V8's default, one sample a millisecond. */
export const PAGE_PROFILE_SAMPLING_US = 1000;

/** The functions a long task lists by self time. */
export const LONG_TASK_FUNCTIONS = 3;

/** One node of the profile: its caller's ID, its name, and whether it is the app's own code. */
interface Node {
  readonly parent: number | undefined;
  readonly name: string;
  /** Whether it is a named function of a script, which an entry may be. */
  readonly named: boolean;
}

/** A CPU profile of the page's main thread, its samples on the page's clock. */
export interface PageProfile {
  readonly nodes: ReadonlyMap<number, Node>;
  /** Each sample's node, in time order. */
  readonly sampleNodes: ReadonlyArray<number>;
  /** Each sample's time on the page's clock, ms, ascending. */
  readonly sampleAtMs: ReadonlyArray<number>;
  /** The profile's end on the page's clock, ms, which the last sample lasts until. */
  readonly endMs: number;
}

/** What ran in one long task, from the profile's samples within it. */
export interface LongTaskFunctions {
  /** The samples' time within it, ms. */
  readonly sampledMs: number;
  /**
   * The outermost named function of a script that most of its time ran under (`onMessage`,
   * `tick`), or `null` where none did.
   */
  readonly entry: string | null;
  /** Its hottest functions by self time, at most {@link LONG_TASK_FUNCTIONS}. */
  readonly self: ReadonlyArray<ProfiledFunction>;
}

/** V8's pseudo-functions that are no code's: the profile's root and its idle time. */
const NOT_CODE = new Set(["(root)", "(idle)"]);

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * A call frame's name as a CPU profile lists it, the windows' traces' and the page's alike:
 * function, script file and line.
 */
export function functionName(frame: Readonly<Record<string, unknown>>): string {
  const name = typeof frame["functionName"] === "string" ? frame["functionName"] : "";
  const url = typeof frame["url"] === "string" ? frame["url"] : "";
  const line = frame["lineNumber"];
  const file = url.split(/[?#]/u)[0]?.split("/").at(-1) ?? "";
  const lineText = typeof line === "number" && Number.isFinite(line) ? `:${String(line + 1)}` : "";
  const full =
    file.length === 0
      ? name.length === 0
        ? "(anonymous)"
        : name
      : `${name.length === 0 ? "(anonymous)" : name} ${file}${lineText}`;
  return full.length > FUNCTION_NAME_CHARS ? `${full.slice(0, FUNCTION_NAME_CHARS - 1)}…` : full;
}

function numbers(value: unknown): number[] | null {
  return Array.isArray(value) && value.every((each) => typeof each === "number")
    ? value.filter((each): each is number => typeof each === "number")
    : null;
}

/** `performance.now()` of an evaluation's result, or `null`. */
function pageNowOf(result: unknown): number | null {
  const value: unknown =
    isRecord(result) && isRecord(result["result"]) ? result["result"]["value"] : undefined;
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

/**
 * A V8 profile (`Profiler.stop`'s `profile`) as the page's clock reads it, the page's clock being
 * `pageNowMs` as the profile ended, or `null` where it is not one.
 */
export function readPageProfile(value: unknown, pageNowMs: number): PageProfile | null {
  const profile = isRecord(value) ? value["profile"] : undefined;
  if (!isRecord(profile) || !Array.isArray(profile["nodes"])) {
    return null;
  }
  const { startTime, endTime } = profile;
  const samples = numbers(profile["samples"]);
  const deltas = numbers(profile["timeDeltas"]);
  if (
    typeof startTime !== "number" ||
    typeof endTime !== "number" ||
    samples === null ||
    deltas === null ||
    samples.length !== deltas.length
  ) {
    return null;
  }
  const nodes = new Map<number, Node>();
  const parents = new Map<number, number>();
  for (const node of profile["nodes"]) {
    const id: unknown = isRecord(node) ? node["id"] : undefined;
    const frame: unknown = isRecord(node) ? node["callFrame"] : undefined;
    if (typeof id !== "number" || !isRecord(frame) || !isRecord(node)) {
      return null;
    }
    const name = functionName(frame);
    const own: unknown = frame["functionName"];
    const url: unknown = frame["url"];
    nodes.set(id, {
      parent: undefined,
      name,
      named: typeof own === "string" && own.length > 0 && typeof url === "string" && url.length > 0,
    });
    for (const child of numbers(node["children"]) ?? []) {
      parents.set(child, id);
    }
  }
  for (const [child, parent] of parents) {
    const node = nodes.get(child);
    if (node !== undefined) {
      nodes.set(child, { ...node, parent });
    }
  }
  // The trace's clock, µs, against the page's, ms, as the profile ended.
  const offsetMs = pageNowMs - endTime / 1000;
  const sampleAtMs: number[] = [];
  let atUs = startTime;
  for (const delta of deltas) {
    atUs += delta;
    sampleAtMs.push(atUs / 1000 + offsetMs);
  }
  return { nodes, sampleNodes: samples, sampleAtMs, endMs: endTime / 1000 + offsetMs };
}

/** The index of the first sample at or after `ms`. */
function firstAtOrAfter(profile: PageProfile, ms: number): number {
  let low = 0;
  let high = profile.sampleAtMs.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if ((profile.sampleAtMs[middle] ?? Number.POSITIVE_INFINITY) < ms) {
      low = middle + 1;
    } else {
      high = middle;
    }
  }
  return low;
}

/** The `count` largest of `totals`, ms, largest first, ties by name. */
function hottest(totals: ReadonlyMap<string, number>, count: number): ProfiledFunction[] {
  return [...totals]
    .filter(([name]) => !NOT_CODE.has(name))
    .toSorted(([an, a], [bn, b]) => b - a || (an < bn ? -1 : 1))
    .slice(0, count)
    .map(([name, ms]) => ({ name, ms }));
}

/**
 * What ran in the task [startMs, endMs) of the page's clock: each sample within it lasting until
 * the next, or the profile's end.
 */
export function longTaskFunctions(
  profile: PageProfile,
  startMs: number,
  endMs: number,
): LongTaskFunctions {
  const self = new Map<string, number>();
  const entries = new Map<string, number>();
  let sampledMs = 0;
  for (
    let i = firstAtOrAfter(profile, startMs);
    i < profile.sampleAtMs.length && (profile.sampleAtMs[i] ?? endMs) < endMs;
    i += 1
  ) {
    const at = profile.sampleAtMs[i] ?? endMs;
    const ms = Math.min(profile.sampleAtMs[i + 1] ?? profile.endMs, endMs) - at;
    const id = profile.sampleNodes[i];
    const node = id === undefined ? undefined : profile.nodes.get(id);
    if (node === undefined || node.name === "(idle)") {
      continue;
    }
    sampledMs += ms;
    self.set(node.name, (self.get(node.name) ?? 0) + ms);
    let entry: string | null = null;
    for (let each: Node | undefined = node; each !== undefined;) {
      if (each.named) {
        entry = each.name;
      }
      each = each.parent === undefined ? undefined : profile.nodes.get(each.parent);
    }
    if (entry !== null) {
      entries.set(entry, (entries.get(entry) ?? 0) + ms);
    }
  }
  return {
    sampledMs,
    entry: hottest(entries, 1)[0]?.name ?? null,
    self: hottest(self, LONG_TASK_FUNCTIONS),
  };
}

/** The whole run's profile of the page's main thread, over the window's debugger. */
export class PageProfiler {
  readonly #debugger: PageDebugger;
  readonly #log: (line: string) => void;
  #started: Promise<boolean> | null = null;

  constructor(pageDebugger: PageDebugger, log: (line: string) => void) {
    this.#debugger = pageDebugger;
    this.#log = log;
  }

  /** Starts the profile; a failure is logged, and the run then has none. */
  start(): Promise<void> {
    this.#started ??= (async () => {
      try {
        if (!this.#debugger.isAttached()) {
          this.#debugger.attach("1.3");
        }
        await this.#debugger.sendCommand("Profiler.enable");
        await this.#debugger.sendCommand("Profiler.setSamplingInterval", {
          interval: PAGE_PROFILE_SAMPLING_US,
        });
        await this.#debugger.sendCommand("Profiler.start");
        return true;
      } catch (error: unknown) {
        this.#log(`views check: the page's CPU profile did not start: ${String(error)}`);
        return false;
      }
    })();
    return this.#started.then(() => undefined);
  }

  /**
   * Stops the profile and reads it, or `null` where none was started, or it did not stop or read;
   * a failure is logged.
   */
  async stop(): Promise<PageProfile | null> {
    if (this.#started === null || !(await this.#started)) {
      return null;
    }
    this.#started = null;
    try {
      const pageNowMs = pageNowOf(
        await this.#debugger.sendCommand("Runtime.evaluate", {
          expression: "performance.now()",
          returnByValue: true,
        }),
      );
      const stopped = await this.#debugger.sendCommand("Profiler.stop");
      const profile = pageNowMs === null ? null : readPageProfile(stopped, pageNowMs);
      if (profile === null) {
        this.#log("views check: the page's CPU profile could not be read");
      }
      return profile;
    } catch (error: unknown) {
      this.#log(`views check: the page's CPU profile did not stop: ${String(error)}`);
      return null;
    }
  }
}
