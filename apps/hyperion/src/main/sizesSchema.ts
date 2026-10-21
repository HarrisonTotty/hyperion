/**
 * The strict check of the descent spike's measured sizes (plan R10, R10.T14.b;
 * decision-r10-t14b-measure §6(g)): the renderer's `sizes` block, which the IPC handler treats as
 * `unknown` until it passes, and the results file's, which `validateResults` checks.
 *
 * @remarks
 * A block's shape is written once as a schema whose type the compiler derives from the block's own
 * type ({@link Schema}), so that every field of the type has its check, of its kind, and a field
 * added to the type without one is a compile error. The check walks the schema: every object holds
 * exactly its schema's keys, but an optional key that a block written before it lacks (a binding
 * cache's layout, R10.T14.b's clamp), every number is finite (whole and at least 0 for a count),
 * every choice is one of its choices, and every figure is a value with no reason or `null` with
 * one. A list's first malformed item alone is named. A pass that describes no view must be marked
 * not sized, with no rule and no samples, and a sized pass must have both. The columns of each
 * sized pass's samples are then checked as the memory series' are: each as long as the samples, -1
 * at its gaps' samples and nowhere else. The memory's cascades must be the engine's `shadows` peak,
 * with a reason exactly where none was allocated, and the engine's whole peak must lie between its
 * largest category's and their sum. A binding cache that states its layout holds the requested
 * slots with no fallback, and at most them on one.
 */

import type {
  SpikeSizesBakeTimes,
  SpikeSizesBakeWindow,
  SpikeSizesBinding,
  SpikeSizesColumn,
  SpikeSizesCountColumns,
  SpikeSizesCountPeaks,
  SpikeSizesEnginePeak,
  SpikeSizesForcedMark,
  SpikeSizesLoggedSegment,
  SpikeSizesMeanMax,
  SpikeSizesMemory,
  SpikeSizesMoment,
  SpikeSizesPass,
  SpikeSizesPeak,
  SpikeSizesPriceTerm,
  SpikeSizesReport,
  SpikeSizesRule,
  SpikeSizesRuleFigure,
  SpikeSizesSeries,
  SpikeSizesShare,
  SpikeSizesSpread,
} from "../preload/api";

/**
 * How a number is checked: finite (`number`), finite and at least 0 (`nonNegative`), whole and at
 * least 0 (`count`), or whole and at least -1 (`index`, a segment's index or -1 for none).
 */
export type NumberSchema = "number" | "nonNegative" | "count" | "index";

/**
 * The schema of a value of type `T`: a check for each of its kinds, derived from the type so that
 * the compiler holds the schema to it. `null` in a type is `nullable`; a string-literal union lists
 * its choices; a list its items' schema; a figure (`{ value, reason }`) its value's; an object every
 * one of its keys' ({@link FieldSchema}).
 */
export type Schema<T> = null extends T
  ? { readonly nullable: Schema<Exclude<T, null>> }
  : [T] extends [boolean]
    ? "boolean"
    : [T] extends [number]
      ? NumberSchema
      : [T] extends [string]
        ? string extends T
          ? "string"
          : { readonly oneOf: ReadonlyArray<T> }
        : [T] extends [ReadonlyArray<infer U>]
          ? { readonly list: Schema<U> }
          : [T] extends [{ readonly value: unknown; readonly reason: string | null }]
            ? { readonly figure: Schema<Exclude<T["value"], null>> }
            : [T] extends [object]
              ? { readonly fields: { readonly [K in keyof T]-?: FieldSchema<T, K> } }
              : never;

/**
 * The schema of an object's key `K` of `T`: its value's, or for an optional key `optional` with its
 * value's, which passes where the key is absent, a field older blocks were written without, and
 * where present checks its value, `undefined` refused.
 */
export type FieldSchema<T, K extends keyof T> =
  Partial<Pick<T, K>> extends Pick<T, K>
    ? { readonly optional: Schema<Exclude<T[K], undefined>> }
    : Schema<T[K]>;

/** Any schema, as the check walks it. */
export type AnySchema =
  | NumberSchema
  | "string"
  | "boolean"
  | { readonly nullable: AnySchema }
  | { readonly optional: AnySchema }
  | { readonly oneOf: ReadonlyArray<string> }
  | { readonly list: AnySchema }
  | { readonly figure: AnySchema }
  | { readonly fields: Readonly<Record<string, AnySchema>> };

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Whether `value` passes the number check `schema`. */
function numberOk(value: unknown, schema: NumberSchema): boolean {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return false;
  }
  let ok: boolean;
  switch (schema) {
    case "number":
      ok = true;
      break;
    case "nonNegative":
      ok = value >= 0;
      break;
    case "count":
      ok = Number.isInteger(value) && value >= 0;
      break;
    case "index":
      ok = Number.isInteger(value) && value >= -1;
      break;
  }
  return ok;
}

/** What a number check asks, in a problem's words. */
const NUMBER_WORDS: Readonly<Record<NumberSchema, string>> = {
  number: "a finite number",
  nonNegative: "a finite number at least 0",
  count: "a whole count",
  index: "a whole index or -1",
};

/**
 * The problems of `value` against `schema` at `path`, added to `problems`.
 *
 * @returns Whether it passed.
 */
export function checkSchema(
  value: unknown,
  schema: AnySchema,
  path: string,
  problems: string[],
): boolean {
  const before = problems.length;
  if (typeof schema === "string") {
    switch (schema) {
      case "string":
        if (typeof value !== "string") {
          problems.push(`${path} is not a string`);
        }
        break;
      case "boolean":
        if (typeof value !== "boolean") {
          problems.push(`${path} is not a boolean`);
        }
        break;
      case "number":
      case "nonNegative":
      case "count":
      case "index":
        if (!numberOk(value, schema)) {
          problems.push(`${path} is not ${NUMBER_WORDS[schema]}`);
        }
        break;
    }
    return problems.length === before;
  }
  if ("nullable" in schema) {
    return value === null || checkSchema(value, schema.nullable, path, problems);
  }
  if ("optional" in schema) {
    // The fields loop passes an absent key; a present one holds its value, so `undefined` fails.
    return checkSchema(value, schema.optional, path, problems);
  }
  if ("oneOf" in schema) {
    if (!schema.oneOf.some((choice) => choice === value)) {
      problems.push(`${path} is not one of ${schema.oneOf.join(", ")}`);
    }
    return problems.length === before;
  }
  if ("list" in schema) {
    if (!Array.isArray(value)) {
      problems.push(`${path} is not a list`);
      return false;
    }
    // The first malformed item alone: a column of a thousand samples names one.
    const bad = value.findIndex(
      (item: unknown, i) => !checkSchema(item, schema.list, `${path}[${i}]`, problems),
    );
    return bad === -1;
  }
  if (!isRecord(value)) {
    problems.push(`${path} is not an object`);
    return false;
  }
  if ("figure" in schema) {
    const keys = Object.keys(value);
    const figure = value["value"];
    const reason = value["reason"];
    if (keys.length !== 2 || !("value" in value) || !("reason" in value)) {
      problems.push(`${path} is not a value with its reason`);
    } else if (figure === null) {
      if (typeof reason !== "string" || reason.length === 0) {
        problems.push(`${path} is null without a reason`);
      }
    } else if (reason !== null) {
      problems.push(`${path} has both a value and a reason`);
    } else {
      checkSchema(figure, schema.figure, `${path}.value`, problems);
    }
    return problems.length === before;
  }
  const expected = Object.keys(schema.fields);
  for (const key of Object.keys(value)) {
    if (!expected.includes(key)) {
      problems.push(`${path}.${key} is not a field of the block`);
    }
  }
  for (const key of expected) {
    const field = schema.fields[key];
    if (field === undefined) {
      continue;
    }
    if (!(key in value)) {
      if (typeof field === "string" || !("optional" in field)) {
        problems.push(`${path}.${key} is missing`);
      }
      continue;
    }
    checkSchema(value[key], field, `${path}.${key}`, problems);
  }
  return problems.length === before;
}

const MOMENT: Schema<SpikeSizesMoment> = {
  fields: {
    scriptTimeS: "number",
    segment: { nullable: "string" },
    heightAboveGroundM: { nullable: "number" },
  },
};

const PEAK: Schema<SpikeSizesPeak> = {
  fields: { patches: "count", at: { nullable: MOMENT } },
};

const COUNT_PEAKS: Schema<SpikeSizesCountPeaks> = {
  fields: { views: { list: PEAK }, union: PEAK },
};

const SHARE: Schema<SpikeSizesShare> = {
  fields: { share: { nullable: "nonNegative" }, at: { nullable: MOMENT } },
};

const RULE_FIGURE: Schema<SpikeSizesRuleFigure> = {
  fields: { slots: "nonNegative", bytes: "count" },
};

const RULE: Schema<SpikeSizesRule> = {
  fields: { ruled: RULE_FIGURE, sameSample: RULE_FIGURE, union: RULE_FIGURE, frustum: RULE_FIGURE },
};

const PRICE_TERM: Schema<SpikeSizesPriceTerm> = {
  fields: {
    allRoundPatches: "number",
    frustumPatches: "number",
    allRoundSlots: "number",
    frustumSlots: "number",
    allRoundShare: { nullable: "number" },
    frustumShare: { nullable: "number" },
  },
};

const FORCED_MARK: Schema<SpikeSizesForcedMark> = {
  fields: { atS: "number", forced: "count", forcedResident: "count" },
};

const SPREAD: Schema<SpikeSizesSpread> = {
  fields: {
    p50: { nullable: "nonNegative" },
    p95: { nullable: "nonNegative" },
    max: { nullable: "nonNegative" },
  },
};

const MEAN_MAX: Schema<SpikeSizesMeanMax> = {
  fields: { mean: "number", max: "number" },
};

const COLUMN: Schema<SpikeSizesColumn> = {
  fields: {
    samples: { list: "index" },
    gaps: { list: { fields: { from: "count", to: "count", reason: "string" } } },
  },
};

const COUNT_COLUMNS: Schema<SpikeSizesCountColumns> = {
  fields: {
    frustum: COLUMN,
    frustumNoMargin: COLUMN,
    allRound: COLUMN,
    frustumDiagnostic: COLUMN,
    allRoundDiagnostic: COLUMN,
  },
};

const ENGINE_PEAK_FIELDS = {
  bytes: "count",
  atS: { nullable: "number" },
} as const satisfies Schema<SpikeSizesEnginePeak>["fields"];

const STYLE = { oneOf: ["photorealistic", "wireframe"] } as const;
const CACHE = { oneOf: ["lit", "wireframe"] } as const;

const BAKE_TIMES_FIELDS = {
  posted: "count",
  baked: "count",
  failed: "count",
  unsurveyed: "count",
  postToReplyMs: SPREAD,
  computeMs: SPREAD,
} as const satisfies Schema<SpikeSizesBakeTimes>["fields"];

const BAKE_WINDOW: Schema<SpikeSizesBakeWindow> = {
  fields: {
    ...BAKE_TIMES_FIELDS,
    residents: "count",
    bakedNeverResident: "number",
    spanMs: "nonNegative",
    bakesPerSecond: { nullable: "nonNegative" },
  },
};

const SERIES: Schema<SpikeSizesSeries> = {
  fields: {
    seconds: { fields: { first: "count", skipped: { list: "count" } } },
    segmentRuns: { list: { fields: { segment: "index", from: "count" } } },
    heightAboveGroundM: { list: { nullable: "number" } },
    views: { list: COUNT_COLUMNS },
    union: { nullable: COUNT_COLUMNS },
    unionDiffers: "count",
    forced: { list: "count" },
    forcedResident: COLUMN,
    allRoundResident: COLUMN,
    allRoundSwept: { list: "count" },
    usedSlots: COLUMN,
  },
};

const LOGGED_SEGMENT: Schema<SpikeSizesLoggedSegment> = {
  fields: {
    segment: { nullable: "string" },
    samples: "count",
    selectedMax: "count",
    limitedSamples: "count",
    selectedResidentMax: { nullable: "count" },
    usedSlotsMax: { nullable: "count" },
    pinnedForcedMax: { nullable: "count" },
    pinnedDrawnMax: { nullable: "count" },
  },
};

const PASS: Schema<SpikeSizesPass> = {
  fields: {
    cache: CACHE,
    views: { list: { fields: { id: "string", style: STYLE, weight: "nonNegative" } } },
    notSized: { nullable: "string" },
    bytesPerSlot: "count",
    slots: { nullable: "count" },
    attachments: "count",
    samples: "count",
    failedSamples: "count",
    firstFailure: { nullable: "string" },
    counts: {
      fields: {
        frustum: COUNT_PEAKS,
        frustumNoMargin: COUNT_PEAKS,
        allRound: COUNT_PEAKS,
        frustumDiagnostic: COUNT_PEAKS,
        allRoundDiagnostic: COUNT_PEAKS,
      },
    },
    forced: { fields: { patches: "count", at: { nullable: MOMENT }, seen: "count" } },
    residentShare: SHARE,
    sweptShare: SHARE,
    allRoundEverResident: "boolean",
    capAtAllRoundPeak: { nullable: "nonNegative" },
    deepestOccluderM: { figure: "number" },
    asBuiltOccluderM: "number",
    boundRatios: {
      list: {
        fields: {
          version: "count",
          firstSeenS: "number",
          levels: {
            list: {
              fields: {
                level: "count",
                boundM: "nonNegative",
                chargedM: "nonNegative",
                edgeM: "nonNegative",
                ratio: "nonNegative",
                chargedRatio: "nonNegative",
              },
            },
          },
        },
      },
    },
    boxHeights: {
      list: {
        fields: {
          level: "count",
          maxM: "nonNegative",
          medianAtFrustumPeakM: { nullable: "nonNegative" },
        },
      },
    },
    countingMs: SPREAD,
    rule: { nullable: RULE },
    ruleDiagnostic: { nullable: RULE },
    occluderPrice: {
      nullable: {
        fields: {
          views: { list: PRICE_TERM },
          union: PRICE_TERM,
          sizedAllRoundSlots: "nonNegative",
          ruledSlots: "number",
          ruledShare: { nullable: "number" },
        },
      },
    },
    contact: {
      fields: {
        firstForcedS: { nullable: "number" },
        allResidentS: { nullable: "number" },
        allResidentHeld: { nullable: "boolean" },
        touchdown: { nullable: FORCED_MARK },
        leadS: { nullable: "number" },
        sessionContact: { nullable: FORCED_MARK },
      },
    },
    streaming: {
      list: {
        fields: {
          segment: { nullable: "string" },
          frames: "count",
          drawn: MEAN_MAX,
          standingIn: MEAN_MAX,
          missing: MEAN_MAX,
          standInShare: MEAN_MAX,
          limitedFrames: "count",
          residents: "count",
          residentsPerS: { nullable: "nonNegative" },
        },
      },
    },
    logged: { list: LOGGED_SEGMENT },
    series: { nullable: SERIES },
  },
};

/** The renderer's `sizes` block's schema (`SpikeSizesReport`). */
export const SIZES_REPORT_SCHEMA: Schema<SpikeSizesReport> = {
  fields: {
    segments: { list: "string" },
    secondView: {
      fields: {
        id: "string",
        engineName: "string",
        style: STYLE,
        weight: "nonNegative",
        viewports: {
          list: { fields: { widthPx: "count", heightPx: "count", fromS: "number" } },
        },
      },
    },
    sampledLowestM: { figure: "number" },
    passes: { list: PASS },
    forcedSets: {
      figure: {
        fields: {
          comparedFrames: "count",
          differingFrames: "count",
          firstDifferingS: { nullable: "number" },
          largestDifference: "count",
        },
      },
    },
    segmentFigures: {
      list: {
        fields: {
          segment: { nullable: "string" },
          bakes: BAKE_WINDOW,
          groundSpeedPerHeight: { nullable: MEAN_MAX },
        },
      },
    },
    bakes: {
      fields: {
        workersMade: "count",
        total: BAKE_WINDOW,
        byWorker: { list: { fields: { ...BAKE_TIMES_FIELDS, worker: "count" } } },
        requests: { list: { fields: { kind: "string", count: "count" } } },
        replies: { list: { fields: { kind: "string", count: "count" } } },
        inFlight: "count",
        abandoned: "count",
      },
    },
    memory: {
      fields: {
        samples: "count",
        workerFieldBytes: { fields: { bytes: "count", atS: "number" } },
        keptFieldBytes: { fields: { bytes: "count", atS: "number" } },
        sessionGpu: { list: { fields: { category: "string", bytes: "count", atS: "number" } } },
        caches: {
          list: {
            fields: {
              cache: CACHE,
              slotCount: "count",
              bytesPerSlot: "count",
              allocationBytes: "count",
            },
          },
        },
        cascades: {
          fields: {
            bytes: "count",
            source: { oneOf: ["engine", "none"] },
            reason: { nullable: "string" },
            specBytes: "count",
            differsFromSpec: "boolean",
          },
        },
        engine: {
          fields: {
            categories: { list: { fields: { ...ENGINE_PEAK_FIELDS, category: "string" } } },
            peak: { fields: ENGINE_PEAK_FIELDS },
            unseenDestroyed: "count",
          },
        },
        itemizedGpuBytes: "count",
        itemizedTotalBytes: "count",
        workersStarted: "count",
      },
    },
    binding: {
      fields: {
        adapterBytes: { figure: "count" },
        requestedBytes: { figure: "count" },
        deviceBytes: { figure: "count" },
        caches: {
          list: {
            fields: {
              cache: CACHE,
              slotCount: "count",
              layout: {
                optional: {
                  fields: {
                    requestedSlotCount: "count",
                    fallback: { oneOf: ["none", "binding-limit"] },
                  },
                },
              },
              ceilingAtRequestedSlots: { nullable: "count" },
              ceilingAtDefaultSlots: { nullable: "count" },
            },
          },
        },
      },
    },
    readouts: {
      fields: {
        intervalMs: "nonNegative",
        publishes: "count",
        answeredPublishes: "count",
        asked: "count",
        answered: "count",
        failed: "count",
        failComputationCalls: "count",
      },
    },
  },
};

/** The five counts' names, in their order (§2). */
const COUNT_NAMES = [
  "frustum",
  "frustumNoMargin",
  "allRound",
  "frustumDiagnostic",
  "allRoundDiagnostic",
] as const satisfies ReadonlyArray<keyof SpikeSizesCountColumns>;

/** One column's problems: as long as the samples, its gaps in order, -1 at their samples alone. */
function columnProblems(
  column: SpikeSizesColumn,
  length: number,
  path: string,
  problems: string[],
): void {
  if (column.samples.length !== length) {
    problems.push(`${path} has ${column.samples.length} samples, not the pass's ${length}`);
    return;
  }
  const inGap = new Set<number>();
  let previousTo = -1;
  for (const [i, gap] of column.gaps.entries()) {
    if (
      gap.from <= previousTo ||
      gap.to < gap.from ||
      gap.to >= length ||
      gap.reason.length === 0
    ) {
      problems.push(`${path}.gaps[${i}] is not a stretch of samples with a reason, after the last`);
      return;
    }
    for (let sample = gap.from; sample <= gap.to; sample += 1) {
      inGap.add(sample);
    }
    previousTo = gap.to;
  }
  const wrong = column.samples.findIndex((value, i) => (value === -1) !== inGap.has(i));
  if (wrong !== -1) {
    problems.push(`${path}.samples[${wrong}] is -1 outside a gap, or not -1 within one`);
  }
}

/**
 * A pass's problems beyond its shape: a pass with no described view is not sized, with no rule and
 * no samples, and a sized one has both; then its samples' ({@link seriesProblems}).
 */
function passProblems(
  pass: SpikeSizesPass,
  segments: number,
  path: string,
  problems: string[],
): void {
  const standIn = pass.views.length === 0;
  if ((pass.notSized !== null) !== standIn) {
    problems.push(`${path}.notSized is given exactly where the pass describes no view`);
  }
  if ((pass.rule === null) !== standIn) {
    problems.push(`${path}.rule is null exactly where the pass describes no view`);
  }
  for (const name of COUNT_NAMES) {
    const peaks = pass.counts[name];
    if (peaks.views.length !== pass.views.length) {
      problems.push(
        `${path}.counts.${name} has ${peaks.views.length} views' peaks, not ${pass.views.length}`,
      );
    }
  }
  if (pass.series === null) {
    if (!standIn) {
      problems.push(`${path}.series is null for a pass that describes a view`);
    }
    return;
  }
  if (standIn) {
    problems.push(`${path}.series is given for a pass that describes no view`);
    return;
  }
  seriesProblems(pass, pass.series, segments, path, problems);
}

/** A sized pass's samples' problems: every column as long as its samples, one view column a view. */
function seriesProblems(
  pass: SpikeSizesPass,
  series: SpikeSizesSeries,
  segments: number,
  path: string,
  problems: string[],
): void {
  const length = pass.samples;
  for (const [name, list] of [
    ["heightAboveGroundM", series.heightAboveGroundM],
    ["forced", series.forced],
    ["allRoundSwept", series.allRoundSwept],
  ] as const) {
    if (list.length !== length) {
      problems.push(`${path}.series.${name} has ${list.length} samples, not the pass's ${length}`);
    }
  }
  const { first, skipped } = series.seconds;
  if (skipped.some((second, i) => second <= (i === 0 ? first : (skipped[i - 1] ?? first)))) {
    problems.push(`${path}.series.seconds.skipped is not ascending after the first second`);
  }
  const runs = series.segmentRuns;
  if (
    runs.some(
      (run, i) =>
        run.from >= length ||
        (i === 0 ? run.from !== 0 : run.from <= (runs[i - 1]?.from ?? -1)) ||
        run.segment === runs[i - 1]?.segment,
    ) ||
    (length > 0 && runs.length === 0)
  ) {
    problems.push(`${path}.series.segmentRuns is not runs of the samples from the first, in order`);
  }
  if (runs.some((run) => run.segment >= segments)) {
    problems.push(`${path}.series.segmentRuns indexes past the block's ${segments} segments`);
  }
  if (series.views.length !== pass.views.length) {
    problems.push(
      `${path}.series.views has ${series.views.length} views, not ${pass.views.length}`,
    );
  }
  if ((series.union === null) !== (pass.views.length === 1)) {
    problems.push(
      `${path}.series.union is given exactly where the pass describes other than one view`,
    );
  }
  if (series.union !== null && series.unionDiffers !== 0) {
    problems.push(`${path}.series.unionDiffers is not 0 with the unions given`);
  }
  const counts: Array<readonly [string, SpikeSizesCountColumns]> = series.views.map(
    (view, k) => [`views[${k}]`, view] as const,
  );
  if (series.union !== null) {
    counts.push(["union", series.union]);
  }
  for (const [where, columns] of counts) {
    for (const name of COUNT_NAMES) {
      columnProblems(columns[name], length, `${path}.series.${where}.${name}`, problems);
    }
  }
  for (const name of ["forcedResident", "allRoundResident", "usedSlots"] as const) {
    columnProblems(series[name], length, `${path}.series.${name}`, problems);
  }
}

/**
 * The engine's memory category the cascades are allocated under: the renderer's
 * `CASCADE_MEMORY_CATEGORY` (`cascades.ts`), which the main process does not import.
 */
export const CASCADE_CATEGORY = "shadows";

/**
 * The memory's problems beyond its shape: the cascades are the engine's `shadows` peak, with a
 * reason exactly where none was allocated and flagged exactly where they differ from the spec, and
 * the engine's whole peak lies between its largest category's and their sum.
 */
function memoryProblems(memory: SpikeSizesMemory, path: string, problems: string[]): void {
  const { cascades, engine } = memory;
  const none = cascades.source === "none";
  if ((cascades.reason !== null) !== none) {
    problems.push(`${path}.cascades.reason is given exactly where none was allocated`);
  }
  const shadows =
    engine.categories.find(({ category }) => category === CASCADE_CATEGORY)?.bytes ?? 0;
  if (cascades.bytes !== shadows || (cascades.bytes === 0) !== none) {
    problems.push(
      `${path}.cascades.bytes is the engine's ${CASCADE_CATEGORY} peak, above 0 exactly where they were allocated`,
    );
  }
  if (cascades.differsFromSpec !== (!none && cascades.bytes !== cascades.specBytes)) {
    problems.push(
      `${path}.cascades.differsFromSpec is set exactly where the allocated bytes differ from the spec's`,
    );
  }
  let largest = 0;
  let sum = 0;
  for (const { bytes } of engine.categories) {
    largest = Math.max(largest, bytes);
    sum += bytes;
  }
  if (engine.peak.bytes < largest || engine.peak.bytes > sum) {
    problems.push(
      `${path}.engine.peak.bytes lies between its largest category's peak and their sum`,
    );
  }
}

/**
 * The binding's problems beyond its shape: a cache that states its layout (R10.T14.b's clamp,
 * decision-r10-t14b-constants §4) holds the requested slots on its own path, and at most them on a
 * fallback; one written before the clamp, without it, has nothing more to check.
 */
function bindingProblems(binding: SpikeSizesBinding, path: string, problems: string[]): void {
  for (const [k, { slotCount, layout }] of binding.caches.entries()) {
    if (layout === undefined) {
      continue;
    }
    let held: boolean;
    switch (layout.fallback) {
      case "none":
        held = slotCount === layout.requestedSlotCount;
        break;
      case "binding-limit":
        held = slotCount <= layout.requestedSlotCount;
        break;
    }
    if (!held) {
      problems.push(
        `${path}.caches[${k}].slotCount is its layout's requestedSlotCount with no fallback, and at most it on one`,
      );
    }
  }
}

/**
 * Whether `value` is a well-formed `sizes` block: its {@link SIZES_REPORT_SCHEMA} shape, the
 * structural check whose problems `problems` takes.
 */
function conforms(value: unknown, problems: string[]): value is SpikeSizesReport {
  return checkSchema(value, SIZES_REPORT_SCHEMA, "sizes", problems);
}

/**
 * The problems of `value` as the renderer's `sizes` block, each named from `sizes`: its shape, then
 * each pass's samples' columns, then the memory's cascades and engine tally, then the binding's
 * caches; empty for a well-formed block.
 */
export function sizesReportProblems(value: unknown): string[] {
  const problems: string[] = [];
  if (!conforms(value, problems)) {
    return problems;
  }
  for (const [k, pass] of value.passes.entries()) {
    passProblems(pass, value.segments.length, `sizes.passes[${k}]`, problems);
  }
  memoryProblems(value.memory, "sizes.memory", problems);
  bindingProblems(value.binding, "sizes.binding", problems);
  return problems;
}

/** Whether `value` is a well-formed `sizes` block (no {@link sizesReportProblems}). */
export function isSpikeSizesReport(value: unknown): value is SpikeSizesReport {
  return sizesReportProblems(value).length === 0;
}
