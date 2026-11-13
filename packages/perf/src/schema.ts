/**
 * The record's validators: small combinators that read an `unknown` into a typed value, naming the
 * first field that is missing or mistyped.
 *
 * @remarks
 * Each validator is typed against the interface it reads ({@link object}'s field map has every key
 * of its type), so that a field added to an interface without its validator is a compile error.
 */

/**
 * A record that failed validation: the line of `runs.v1.jsonl` (1-based, `null` outside a file),
 * the field's path (`runs[0].frames.p50Ms`, empty for the record itself) and why.
 */
export class PerfRecordError extends Error {
  /** The 1-based line of the store, or `null` for a record not read from one. */
  readonly line: number | null;
  /** The field's path from the record's root, such as `harness.warmUp.seconds`. */
  readonly field: string;
  /** What is wrong with it. */
  readonly reason: string;

  constructor(field: string, reason: string, line: number | null = null) {
    const where = line === null ? "" : `line ${String(line)}: `;
    super(`${where}${field.length > 0 ? field : "the record"}: ${reason}`);
    this.name = "PerfRecordError";
    this.line = line;
    this.field = field;
    this.reason = reason;
  }
}

/** A validator: reads `value`, found at `path`, into a `T`, or throws a {@link PerfRecordError}. */
export type Schema<T> = (value: unknown, path: string) => T;

/** A JSON value, as a setting's value is stored. */
export type JsonValue = string | number | boolean | null | ReadonlyArray<JsonValue> | JsonObject;

/** A JSON object of {@link JsonValue}s. */
export interface JsonObject {
  readonly [key: string]: JsonValue;
}

function describe(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "an array";
  if (typeof value === "number" && !Number.isFinite(value)) return String(value);
  return typeof value;
}

function child(path: string, key: string): string {
  return path.length > 0 ? `${path}.${key}` : key;
}

function isPlainObject(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Any string. */
export const str: Schema<string> = (value, path) => {
  if (typeof value !== "string") {
    throw new PerfRecordError(path, `expected a string, got ${describe(value)}`);
  }
  return value;
};

/** A string that is not empty. */
export const nonEmpty: Schema<string> = (value, path) => {
  const text = str(value, path);
  if (text.length === 0) throw new PerfRecordError(path, "expected a string that is not empty");
  return text;
};

/** A finite number. */
export const num: Schema<number> = (value, path) => {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new PerfRecordError(path, `expected a finite number, got ${describe(value)}`);
  }
  return value;
};

/** A finite number at or above zero. */
export const nonNegative: Schema<number> = (value, path) => {
  const n = num(value, path);
  if (n < 0) throw new PerfRecordError(path, `expected a number at or above 0, got ${String(n)}`);
  return n;
};

/** A number from 0 to 1. */
export const fraction: Schema<number> = (value, path) => {
  const n = num(value, path);
  if (n < 0 || n > 1)
    throw new PerfRecordError(path, `expected a fraction in [0, 1], got ${String(n)}`);
  return n;
};

/** An integer at or above zero. */
export const count: Schema<number> = (value, path) => {
  const n = num(value, path);
  if (!Number.isInteger(n) || n < 0) {
    throw new PerfRecordError(path, `expected a whole number at or above 0, got ${String(n)}`);
  }
  return n;
};

/** An integer at or above one. */
export const positiveInt: Schema<number> = (value, path) => {
  const n = count(value, path);
  if (n < 1)
    throw new PerfRecordError(path, `expected a whole number at or above 1, got ${String(n)}`);
  return n;
};

/** A boolean. */
export const bool: Schema<boolean> = (value, path) => {
  if (typeof value !== "boolean") {
    throw new PerfRecordError(path, `expected a boolean, got ${describe(value)}`);
  }
  return value;
};

/** One of a set of strings. */
export function oneOf<T extends string>(values: ReadonlyArray<T>): Schema<T> {
  return (value, path) => {
    const text = str(value, path);
    const found = values.find((known) => known === text);
    if (found === undefined) {
      throw new PerfRecordError(path, `expected one of ${values.join(", ")}, got "${text}"`);
    }
    return found;
  };
}

/** A string that a type predicate accepts, such as a scene ID. */
export function narrowed<T extends string>(
  accepts: (text: string) => text is T,
  expected: string,
): Schema<T> {
  return (value, path) => {
    const text = str(value, path);
    if (!accepts(text)) throw new PerfRecordError(path, `expected ${expected}, got "${text}"`);
    return text;
  };
}

/** A value or `null`. */
export function nullable<T>(schema: Schema<T>): Schema<T | null> {
  return (value, path) => (value === null ? null : schema(value, path));
}

/** An array, each element read by `schema`. */
export function array<T>(schema: Schema<T>): Schema<ReadonlyArray<T>> {
  return (value, path) => {
    if (!Array.isArray(value)) {
      throw new PerfRecordError(path, `expected an array, got ${describe(value)}`);
    }
    const items: unknown[] = value;
    return items.map((item, index) => schema(item, `${path}[${String(index)}]`));
  };
}

/** The validators of every field of `T`, by name. */
export type Fields<T> = { readonly [K in keyof T]-?: Schema<T[K]> };

/**
 * An object with exactly the fields named: each missing field and each field not named is an
 * error.
 */
export function object<T>(fields: Fields<T>): Schema<T> {
  const names = Object.keys(fields);
  return (value, path) => {
    if (!isPlainObject(value)) {
      throw new PerfRecordError(path, `expected an object, got ${describe(value)}`);
    }
    const out: Record<string, unknown> = {};
    for (const name of names) {
      const fieldPath = child(path, name);
      if (!Object.hasOwn(value, name)) throw new PerfRecordError(fieldPath, "missing");
      const schema: unknown = Reflect.get(fields, name);
      if (typeof schema !== "function") {
        throw new Error(`the validator of ${fieldPath} is not a function`);
      }
      out[name] = Reflect.apply(schema, undefined, [value[name], fieldPath]);
    }
    for (const name of Object.keys(value)) {
      if (!Object.hasOwn(fields, name)) {
        throw new PerfRecordError(child(path, name), "not a field of the record's version");
      }
    }
    // A trust boundary: every field of `T` was read above by its own validator, and no other.
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion
    return out as T;
  };
}

/** An object whose keys a predicate accepts and whose values `schema` reads; any key may be absent. */
export function keyed<K extends string, V>(
  accepts: (key: string) => key is K,
  expected: string,
  schema: Schema<V>,
): Schema<Readonly<Partial<Record<K, V>>>> {
  return (value, path) => {
    if (!isPlainObject(value)) {
      throw new PerfRecordError(path, `expected an object, got ${describe(value)}`);
    }
    const out: Partial<Record<K, V>> = {};
    for (const [key, item] of Object.entries(value)) {
      if (!accepts(key))
        throw new PerfRecordError(child(path, key), `expected ${expected} as a key`);
      out[key] = schema(item, child(path, key));
    }
    return out;
  };
}

/** Any JSON value. */
export const json: Schema<JsonValue> = (value, path) => {
  if (value === null || typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number") return num(value, path);
  if (Array.isArray(value)) return array(json)(value, path);
  if (isPlainObject(value)) {
    const out: Record<string, JsonValue> = {};
    for (const [key, item] of Object.entries(value)) out[key] = json(item, child(path, key));
    return out;
  }
  throw new PerfRecordError(path, `expected a JSON value, got ${describe(value)}`);
};

/** A JSON object of any {@link JsonValue}s. */
export const jsonObject: Schema<JsonObject> = (value, path) => {
  if (!isPlainObject(value)) {
    throw new PerfRecordError(path, `expected an object, got ${describe(value)}`);
  }
  const out: Record<string, JsonValue> = {};
  for (const [key, item] of Object.entries(value)) out[key] = json(item, child(path, key));
  return out;
};
