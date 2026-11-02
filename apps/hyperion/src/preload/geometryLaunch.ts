/**
 * How the main process hands a launch's geometry override (`--geometry`, plan R14, Design note 17)
 * to the preload, and how both sides read and write it.
 *
 * @remarks
 * The override is `<field>=<value>[,<field>=<value>…]` over the renderer's `GeometrySettings`:
 * `clusterPath`, `casterDepthCull` and `litClusters` `true` or `false`, `casterStrideCap` 1, 2, 4 or
 * 8, `overlayDepth` `redraw`, `scene-depth` or `cluster`, and `farCascadeRate` 1, 2 or 4. As
 * `qualityLaunch.ts` does for the setting, it rides in the renderer's own `argv` as an extra
 * switch, `--hyperion-geometry=<override>`, present only when the launch gave one, in the canonical
 * form {@link geometryText} writes, and the preload gives it to `HyperionApi.geometry`. Both sides
 * of the hand-off use this module; it imports types only, so it is safe in either bundle.
 */

import type { GeometryOverride } from "./api";

export type { GeometryOverride } from "./api";

/** The switch carrying the override. */
const GEOMETRY_SWITCH = "--hyperion-geometry=";

/** The override's fields in the order {@link geometryText} writes them. */
const FIELDS = [
  "clusterPath",
  "casterDepthCull",
  "casterStrideCap",
  "overlayDepth",
  "litClusters",
  "farCascadeRate",
] as const;

/** A field's values, as a refusal names them. */
const VALUES: Readonly<Record<(typeof FIELDS)[number], string>> = {
  clusterPath: "true or false",
  casterDepthCull: "true or false",
  casterStrideCap: "1, 2, 4 or 8",
  overlayDepth: "redraw, scene-depth or cluster",
  litClusters: "true or false",
  farCascadeRate: "1, 2 or 4",
};

/** The refusal of `text` as `field`'s value. */
function refused(field: (typeof FIELDS)[number], text: string): Error {
  return new Error(`${field} is ${VALUES[field]}, not "${text}"`);
}

/** `text` as a switch's value, `true` or `false`. */
function booleanOf(
  field: "clusterPath" | "casterDepthCull" | "litClusters",
  text: string,
): boolean {
  if (text !== "true" && text !== "false") {
    throw refused(field, text);
  }
  return text === "true";
}

/** An override being read, its fields written one by one. */
type OverrideBeingRead = { -readonly [K in keyof GeometryOverride]: GeometryOverride[K] };

/** Writes `field` at `text` into `out`. */
function readField(out: OverrideBeingRead, field: (typeof FIELDS)[number], text: string): void {
  switch (field) {
    case "clusterPath":
    case "casterDepthCull":
    case "litClusters":
      out[field] = booleanOf(field, text);
      break;
    case "casterStrideCap": {
      const cap = text === "1" ? 1 : text === "2" ? 2 : text === "4" ? 4 : text === "8" ? 8 : null;
      if (cap === null) {
        throw refused(field, text);
      }
      out.casterStrideCap = cap;
      break;
    }
    case "overlayDepth":
      if (text !== "redraw" && text !== "scene-depth" && text !== "cluster") {
        throw refused(field, text);
      }
      out.overlayDepth = text;
      break;
    case "farCascadeRate": {
      const rate = text === "1" ? 1 : text === "2" ? 2 : text === "4" ? 4 : null;
      if (rate === null) {
        throw refused(field, text);
      }
      out.farCascadeRate = rate;
      break;
    }
  }
}

/**
 * Reads an override, `<field>=<value>[,…]`, each field named at most once.
 *
 * @throws Error naming the pair that is not `<field>=<value>`, the field given twice, or the value
 *   that is not one of its field's.
 */
export function parseGeometryOverride(text: string): GeometryOverride {
  const out: OverrideBeingRead = {};
  for (const pair of text.split(",")) {
    const [name = "", value, ...rest] = pair.split("=");
    const field = FIELDS.find((each) => each === name);
    if (field === undefined || value === undefined || rest.length > 0) {
      throw new Error(`"${pair}" is not <field>=<value>, a field one of ${FIELDS.join(", ")}`);
    }
    if (out[field] !== undefined) {
      throw new Error(`${field} is given twice`);
    }
    readField(out, field, value);
  }
  return out;
}

/** The override in its canonical form, `<field>=<value>` in the fields' order, comma-separated. */
export function geometryText(override: GeometryOverride): string {
  return FIELDS.flatMap((field) => {
    const value = override[field];
    return value === undefined ? [] : [`${field}=${String(value)}`];
  }).join(",");
}

/** The extra renderer argument that carries `override`, for `webPreferences.additionalArguments`. */
export function geometrySwitch(override: GeometryOverride): string {
  return `${GEOMETRY_SWITCH}${geometryText(override)}`;
}

/**
 * The override carried by `argv`, the renderer's arguments, or `null` on a launch that gave none.
 *
 * @throws Error if the switch is there but does not hold an override, which means the window was
 *   created with something other than {@link geometrySwitch}'s output.
 */
export function geometryFromArgv(argv: readonly string[]): GeometryOverride | null {
  const carrier = argv.find((argument) => argument.startsWith(GEOMETRY_SWITCH));
  if (carrier === undefined) {
    return null;
  }
  try {
    return parseGeometryOverride(carrier.slice(GEOMETRY_SWITCH.length));
  } catch (error: unknown) {
    throw new Error(`the renderer's ${GEOMETRY_SWITCH} does not hold an override`, {
      cause: error,
    });
  }
}
