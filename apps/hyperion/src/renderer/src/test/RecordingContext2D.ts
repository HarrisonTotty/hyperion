import { vi } from "vitest";

/** A method called on a recorded context, with its arguments. */
export interface ContextCall {
  readonly type: "call";
  readonly name: string;
  readonly args: ReadonlyArray<unknown>;
  /** The canvas whose context it was, or `null` for a context made for none. */
  readonly canvas: HTMLCanvasElement | null;
}

/** A property set on a recorded context, with its value. */
export interface ContextSet {
  readonly type: "set";
  readonly name: string;
  readonly value: unknown;
  readonly canvas: HTMLCanvasElement | null;
}

/** One thing done to a recorded context. */
export type ContextRecord = ContextCall | ContextSet;

/**
 * The properties of `CanvasRenderingContext2D` and the value each reads before it is set, so that
 * reading one never returns a method.
 */
const PROPERTY_DEFAULTS: Readonly<Record<string, unknown>> = {
  direction: "inherit",
  fillStyle: "#000000",
  filter: "none",
  font: "10px sans-serif",
  fontKerning: "auto",
  fontStretch: "normal",
  fontVariantCaps: "normal",
  globalAlpha: 1,
  globalCompositeOperation: "source-over",
  imageSmoothingEnabled: true,
  imageSmoothingQuality: "low",
  letterSpacing: "0px",
  lineCap: "butt",
  lineDashOffset: 0,
  lineJoin: "miter",
  lineWidth: 1,
  miterLimit: 10,
  shadowBlur: 0,
  shadowColor: "rgba(0, 0, 0, 0)",
  shadowOffsetX: 0,
  shadowOffsetY: 0,
  strokeStyle: "#000000",
  textAlign: "start",
  textBaseline: "alphabetic",
  textRendering: "auto",
  wordSpacing: "0px",
};

/**
 * The context members a spatial display's canvas never uses: text, which stays in the DOM (plan
 * 05, D15); translucency, blending, blurs, shadows and gradients, which the guide bans from the
 * console; and pattern fills, of which hazard striping is the only one.
 *
 * @remarks
 * Kept here rather than in the tests of `spatial/`, whose sources are checked by `grep` to name
 * none of them.
 */
export const BANNED_SPATIAL_MEMBERS: ReadonlyArray<string> = [
  "fillText",
  "strokeText",
  "globalAlpha",
  "globalCompositeOperation",
  "filter",
  "shadowBlur",
  "shadowColor",
  "shadowOffsetX",
  "shadowOffsetY",
  "createLinearGradient",
  "createRadialGradient",
  "createConicGradient",
  "createPattern",
];

/** What `createImageData` and `getImageData` return: a blank picture of the size asked for. */
function blankImageData(args: ReadonlyArray<unknown>): {
  width: number;
  height: number;
  data: Uint8ClampedArray;
  colorSpace: "srgb";
} {
  const [first, second] = args;
  const size =
    typeof first === "object" && first !== null && "width" in first && "height" in first
      ? { width: Number(first.width), height: Number(first.height) }
      : { width: Number(first), height: Number(second) };
  return {
    width: size.width,
    height: size.height,
    data: new Uint8ClampedArray(size.width * size.height * 4),
    colorSpace: "srgb",
  };
}

/** What a recorded method returns: a value of the right shape where one is needed. */
function returnValue(name: string, args: ReadonlyArray<unknown>): unknown {
  switch (name) {
    case "createImageData":
    case "getImageData":
      return blankImageData(args);
    case "createLinearGradient":
    case "createRadialGradient":
    case "createConicGradient":
    case "createPattern":
      return { addColorStop: (): void => undefined, setTransform: (): void => undefined };
    case "measureText":
      return { width: 0 };
    case "getLineDash":
      return [];
    case "isPointInPath":
    case "isPointInStroke":
      return false;
    default:
      return undefined;
  }
}

/**
 * A stand-in for the 2D contexts of canvases, which jsdom lacks, recording every method called and
 * every property set on them, in order.
 *
 * @remarks
 * Each context it makes is a proxy: any method may be called and any property set, and each is
 * recorded, so a test can assert that something was never done (`fillText`, `globalAlpha`) as
 * well as what was. A property reads back the value last set on that context, or the browser's
 * default. The methods that return something return a value of the right shape:
 * `createImageData` a blank picture, a gradient an object that accepts colour stops. One recorder
 * may serve several canvases, one context each, as a browser gives; every record names its canvas.
 *
 * A context made for no canvas (`contextFor()`) has a `canvas` of `null`, so code that reads the
 * canvas's size, as `paint` does, is given a canvas's context instead.
 *
 * @example
 * const recorder = stubCanvas();
 * const canvas = document.createElement("canvas");
 * canvas.getContext("2d")?.fillRect(0, 0, 10, 10);
 * expect(recorder.names()).not.toContain("fillText");
 */
export class RecordingContext2D {
  /** Everything done to every context this recorder made, oldest first. */
  readonly records: ContextRecord[] = [];

  readonly #contexts = new Map<HTMLCanvasElement | null, CanvasRenderingContext2D>();

  /**
   * The recorded context of `canvas`, made on first use, or of no canvas.
   *
   * @remarks
   * The context is typed as the browser's, so that code under test takes it as it is: a documented
   * cast at the boundary this fake stands in for.
   */
  contextFor(canvas: HTMLCanvasElement | null = null): CanvasRenderingContext2D {
    const existing = this.#contexts.get(canvas);
    if (existing !== undefined) {
      return existing;
    }
    const values = new Map<string, unknown>();
    const records = this.records;
    const context = new Proxy(
      {},
      {
        get: (_target, property) => {
          // Not a thenable, so that an awaited or printed context is left alone.
          if (typeof property === "symbol" || property === "then") {
            return undefined;
          }
          if (property === "canvas") {
            return canvas;
          }
          if (values.has(property)) {
            return values.get(property);
          }
          if (Object.hasOwn(PROPERTY_DEFAULTS, property)) {
            return PROPERTY_DEFAULTS[property];
          }
          return (...args: unknown[]): unknown => {
            records.push({ type: "call", name: property, args, canvas });
            return returnValue(property, args);
          };
        },
        set: (_target, property, value: unknown) => {
          if (typeof property === "symbol") {
            return false;
          }
          values.set(property, value);
          records.push({ type: "set", name: property, value, canvas });
          return true;
        },
      },
    );
    // jsdom has no 2D context to be faithful to; the proxy answers every member the code under test
    // uses, and records it.
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion
    const typed = context as CanvasRenderingContext2D;
    this.#contexts.set(canvas, typed);
    return typed;
  }

  /** The calls to method `name`, oldest first. */
  calls(name: string): ContextCall[] {
    return this.records.filter(
      (record): record is ContextCall => record.type === "call" && record.name === name,
    );
  }

  /** The values property `name` was set to, oldest first. */
  sets(name: string): unknown[] {
    return this.records
      .filter((record): record is ContextSet => record.type === "set" && record.name === name)
      .map((record) => record.value);
  }

  /** The name of every method called and property set, each once. */
  names(): Set<string> {
    return new Set(this.records.map((record) => record.name));
  }

  /** Forgets what was recorded, keeping the contexts and the values set on them. */
  clear(): void {
    this.records.length = 0;
  }
}

/**
 * Gives every canvas a recorded 2D context, for the rest of the test.
 *
 * @remarks
 * Spies on `HTMLCanvasElement.prototype.getContext`, which jsdom does not implement: `"2d"` gets
 * the canvas's context from the returned recorder, and any other kind `null`, as a browser gives
 * for a kind it lacks. The spy is restored after the test by `restoreMocks`.
 *
 * @returns The recorder that holds what every canvas's context was asked to do.
 */
export function stubCanvas(): RecordingContext2D {
  const recorder = new RecordingContext2D();
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(function (
    this: HTMLCanvasElement,
    contextId: string,
  ) {
    return contextId === "2d" ? recorder.contextFor(this) : null;
  });
  return recorder;
}
