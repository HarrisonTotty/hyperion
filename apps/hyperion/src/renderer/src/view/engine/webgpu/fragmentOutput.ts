/**
 * A material's colour output read from its WGSL and checked against its target's format (plan
 * R14, R14.T2.b's finding), so that a mismatch is refused by name before the device refuses the
 * pipeline as a device error.
 *
 * @remarks
 * The rules are WebGPU's for a colour target's output (W3C WebGPU, §10.3.4, "validating
 * GPUFragmentState"): the output at the target's location has a floating-point scalar for a
 * `float` format, a signed one for `sint` and an unsigned one for `uint`; a component for each of
 * the format's; an alpha (a `vec4`) when it blends with the source's alpha, as every blend mode of
 * `BLEND_STATES` does; and is present when the target's write mask is not 0.
 *
 * A reading of the source, not a parser: comments are stripped, the entry point's return is read
 * as an attributed type or a struct's members, and an `alias` is followed. A source whose entry
 * point, or whose output's type, it cannot read is left to the device's own validation.
 */

import type { WgslMaterialSpec } from "../types";

/** A scalar type a fragment output's components may have. */
export type OutputScalar = "f32" | "f16" | "i32" | "u32";

/** The type of a fragment output: its scalar and its number of components, 1 to 4. */
export interface FragmentOutputType {
  readonly scalar: OutputScalar;
  readonly components: number;
}

/**
 * What an entry point writes at a location: a type, nothing, or a source not read here (no such
 * entry point, or a type that is no scalar or vector).
 */
export type FragmentOutput =
  | { readonly kind: "written"; readonly type: FragmentOutputType }
  | { readonly kind: "none" }
  | { readonly kind: "unread" };

/** How a format's texels are sampled, as WebGPU names its formats' sample types (§26.1). */
export type TexelSampleType = "float" | "uint" | "sint";

/** What a colour format holds of a fragment's output: its sample type and its components. */
export interface FormatTexels {
  readonly sampleType: TexelSampleType;
  readonly components: number;
}

const UNREAD: FragmentOutput = { kind: "unread" };
const NONE: FragmentOutput = { kind: "none" };

/** `source` without its `//` and block comments. */
function stripComments(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//gu, " ").replace(/\/\/[^\n]*/gu, " ");
}

/** `text` split at its commas outside parentheses and angle brackets. */
function splitTopLevel(text: string): ReadonlyArray<string> {
  const parts: string[] = [];
  let depth = 0;
  let start = 0;
  for (let at = 0; at < text.length; at += 1) {
    const c = text[at];
    if (c === "(" || c === "<") {
      depth += 1;
    } else if (c === ")" || c === ">") {
      depth -= 1;
    } else if (c === "," && depth === 0) {
      parts.push(text.slice(start, at));
      start = at + 1;
    }
  }
  parts.push(text.slice(start));
  return parts.map((part) => part.trim()).filter((part) => part.length > 0);
}

/**
 * A declaration's `@location`, if any, and what follows its attributes: `unread` for a location
 * that is no decimal literal (`0`, `0u` or `0i`), such as a constant's name.
 */
function attributed(declaration: string): {
  readonly location: number | "unread" | undefined;
  readonly rest: string;
} {
  const attribute = /^@\s*(\w+)\s*(?:\(([^)]*)\))?\s*/u;
  let rest = declaration.trim();
  let location: number | "unread" | undefined;
  for (let match = attribute.exec(rest); match !== null; match = attribute.exec(rest)) {
    if (match[1] === "location") {
      const literal = /^\s*(\d+)[iu]?\s*$/u.exec(match[2] ?? "")?.[1];
      location = literal === undefined ? "unread" : Number(literal);
    }
    rest = rest.slice(match[0].length);
  }
  return { location, rest };
}

/** Whether `name` is a WGSL identifier, as a struct's or an alias's name is. */
function isIdentifier(name: string): boolean {
  return /^[A-Za-z_]\w*$/u.test(name);
}

/** The type `name` stands for in `text`, its aliases followed. */
function resolvedType(text: string, name: string): string {
  let type = name.replaceAll(/\s+/gu, "");
  // A bounded walk, so that a cycle of aliases, which WGSL refuses anyway, ends.
  for (let step = 0; step < 8 && isIdentifier(type); step += 1) {
    const alias = new RegExp(String.raw`\balias\s+${type}\s*=\s*([^;]+);`, "u").exec(text)?.[1];
    if (alias === undefined) {
      break;
    }
    type = alias.replaceAll(/\s+/gu, "");
  }
  return type;
}

/** The scalar types a fragment output may have. */
const SCALARS: ReadonlyArray<OutputScalar> = ["f32", "f16", "i32", "u32"];

/** The scalar and component count of a WGSL scalar or vector type, or `undefined` for any other. */
export function outputTypeOf(type: string): FragmentOutputType | undefined {
  const match = /^(?:vec([234])(?:([fhiu])|<(\w+)>)|(\w+))$/u.exec(type);
  if (match === null) {
    return undefined;
  }
  const [, size, suffix, generic, bare] = match;
  // A shorthand's suffix: `h` is `f16`, every other its letter and 32 bits.
  const named = suffix === undefined ? (generic ?? bare) : suffix === "h" ? "f16" : `${suffix}32`;
  const scalar = SCALARS.find((each) => each === named);
  return scalar === undefined
    ? undefined
    : { scalar, components: size === undefined ? 1 : Number(size) };
}

/** Each scalar's suffix in a vector's shorthand: `vec4f`, `vec4h`. */
const SUFFIXES: Readonly<Record<OutputScalar, string>> = { f32: "f", f16: "h", i32: "i", u32: "u" };

/** `type` as WGSL writes it: `u32`, `vec4f`. */
export function outputTypeName(type: FragmentOutputType): string {
  return type.components === 1 ? type.scalar : `vec${type.components}${SUFFIXES[type.scalar]}`;
}

/** `type` resolved in `text` as an output, or unread. */
function writtenOf(text: string, type: string): FragmentOutput {
  const read = outputTypeOf(resolvedType(text, type));
  return read === undefined ? UNREAD : { kind: "written", type: read };
}

/**
 * What `entryPoint` of `wgsl` writes at `@location(location)`, by its return: an attributed type,
 * or the member of its return struct at that location.
 */
export function fragmentOutputAt(
  wgsl: string,
  entryPoint: string,
  location: number,
): FragmentOutput {
  if (!isIdentifier(entryPoint)) {
    return UNREAD;
  }
  const text = stripComments(wgsl);
  const head = new RegExp(String.raw`\bfn\s+${entryPoint}\s*\(`, "u").exec(text);
  if (head === null) {
    return UNREAD;
  }
  let depth = 1;
  let at = head.index + head[0].length;
  for (; at < text.length && depth > 0; at += 1) {
    const c = text[at];
    depth += c === "(" ? 1 : c === ")" ? -1 : 0;
  }
  const rest = text.slice(at);
  if (/^\s*\{/u.test(rest)) {
    return NONE;
  }
  const returned = /^\s*->([^{]*)\{/u.exec(rest)?.[1];
  if (returned === undefined) {
    return UNREAD;
  }
  const declared = attributed(returned);
  if (returned.trim().startsWith("@")) {
    if (declared.location === "unread") {
      return UNREAD;
    }
    return declared.location === location ? writtenOf(text, declared.rest) : NONE;
  }
  const struct = resolvedType(text, declared.rest);
  if (!isIdentifier(struct)) {
    return UNREAD;
  }
  const body = new RegExp(String.raw`\bstruct\s+${struct}\s*\{([^}]*)\}`, "u").exec(text)?.[1];
  if (body === undefined) {
    return UNREAD;
  }
  // A member at a location not read may be the one asked for.
  let unread = false;
  for (const member of splitTopLevel(body)) {
    const { location: memberLocation, rest: named } = attributed(member);
    const type = /^\w+\s*:\s*(.+)$/u.exec(named)?.[1];
    if (memberLocation === location) {
      return type === undefined ? UNREAD : writtenOf(text, type);
    }
    unread ||= memberLocation === "unread";
  }
  return unread ? UNREAD : NONE;
}

/**
 * The sample type and components of a colour format, by its name as WebGPU names its formats
 * (§26.1): `r32uint` holds one `uint`, `rg11b10ufloat` three floats, `bgra8unorm-srgb` four.
 */
export function formatTexels(format: GPUTextureFormat): FormatTexels {
  const name = format.replace(/-srgb$/u, "");
  const sampleType: TexelSampleType = name.endsWith("uint")
    ? "uint"
    : name.endsWith("sint")
      ? "sint"
      : "float";
  const channels = name.replace(/(?:unorm|snorm|uint|sint|ufloat|float)$/u, "");
  return { sampleType, components: channels.replaceAll(/[^rgba]/gu, "").length };
}

/** The scalars a format of `sampleType` takes. */
const SCALARS_OF: Readonly<Record<TexelSampleType, ReadonlyArray<OutputScalar>>> = {
  float: ["f32", "f16"],
  uint: ["u32"],
  sint: ["i32"],
};

/** The narrowest output a format of `texels` takes: `u32` for `r32uint`, `vec4f` for `rgba16float`. */
function narrowestOutput(texels: FormatTexels): string {
  const scalar = SCALARS_OF[texels.sampleType][0] ?? "f32";
  return outputTypeName({ scalar, components: texels.components });
}

/**
 * Checks that `spec`'s fragment entry point writes what a colour target of `format` takes at
 * `@location(0)`.
 *
 * @throws Error naming the material, the format and the two types, for an output whose scalar is
 *   not of the format's sample type, that has fewer components than the format, or that has no
 *   alpha for a blend; or naming the material and the format, for a material that writes colour
 *   with no output there.
 */
export function assertFragmentOutputFits(
  spec: Pick<WgslMaterialSpec, "name" | "fragmentWgsl" | "blend" | "colourWrites">,
  entryPoint: string,
  format: GPUTextureFormat,
): void {
  const output = fragmentOutputAt(spec.fragmentWgsl, entryPoint, 0);
  switch (output.kind) {
    case "unread":
      return;
    case "none":
      if (spec.colourWrites) {
        throw new Error(
          `material ${spec.name} writes colour into ${format}, but its ${entryPoint} has no @location(0) output`,
        );
      }
      return;
    case "written":
      break;
  }
  const texels = formatTexels(format);
  const has = outputTypeName(output.type);
  if (
    !SCALARS_OF[texels.sampleType].includes(output.type.scalar) ||
    output.type.components < texels.components
  ) {
    throw new Error(
      `material ${spec.name} writes ${has} at @location(0), but ${format} takes ${narrowestOutput(texels)}`,
    );
  }
  if (spec.blend !== "none" && output.type.components < 4) {
    throw new Error(
      `material ${spec.name} blends ${spec.blend} into ${format} from ${has}, which has no alpha`,
    );
  }
}
