/**
 * The static checks on the atmosphere's assembled WGSL, shared by `view/atmosphere/hillaire.test.ts`
 * and `view/atmosphere/tables.test.ts`: no module calls WGSL's builtin trigonometry, keeps one of
 * R05's radius forms, or keeps sebh's texel remap (plan R08, Design notes 18 and 19, R08.T6.g);
 * and no helper a per-frame march's step reaches takes or returns an array by value
 * ({@link arraysByValueFrom}, R08.T8.d).
 */

/**
 * What no atmosphere module may call or keep, each with the pattern that finds it in WGSL whose
 * comments are stripped ({@link wgslCode}).
 *
 * @remarks
 * - WGSL's `sin`, `cos`, `tan`, `asin`, `acos`, `atan` and `atan2`, which WGSL bounds only loosely
 *   (`view/shaders/preciseTrig.wgsl` serves instead).
 * - R05's radius forms, which formed heights, discriminants and table coordinates as differences
 *   of radii: `mediumAt(r)`, `distanceToTop`, `distanceToBottom`, `intersectsGround`, `maxDistance`,
 *   `shellIntersectsGround`, `shellRMuToUv`, `transmittanceRMuToUv`, `transmittanceUvToRMu`,
 *   `shellMultiScatteringRMuToUv`, `multiScatteringUvToRMu`, `multiScatteringRMuToUv`, the ray
 *   march's `heightAbove`, and the `Medium` uniform's `topRadiusM`.
 * - sebh's sub-texel remap, `subUvsToUnit` and `unitToSubUvs`, whose read did not invert its write.
 */
export const FORBIDDEN_IN_ATMOSPHERE_WGSL: ReadonlyArray<readonly [string, RegExp]> = [
  ["builtin trigonometry", /\b(sin|cos|tan|asin|acos|atan|atan2)\s*\(/u],
  ["mediumAt(r)", /\bmediumAt\s*\(/u],
  ["distanceToTop", /\bdistanceToTop\b/u],
  ["distanceToBottom", /\bdistanceToBottom\b/u],
  ["intersectsGround", /\bintersectsGround\b/u],
  ["maxDistance", /\bmaxDistance\s*\(/u],
  ["shellIntersectsGround", /\bshellIntersectsGround\b/u],
  ["shellRMuToUv", /\bshellRMuToUv\b/u],
  ["transmittanceRMuToUv", /\btransmittanceRMuToUv\b/u],
  ["transmittanceUvToRMu", /\btransmittanceUvToRMu\b/u],
  ["shellMultiScatteringRMuToUv", /\bshellMultiScatteringRMuToUv\b/u],
  ["multiScatteringUvToRMu", /\bmultiScatteringUvToRMu\b/u],
  ["multiScatteringRMuToUv", /\bmultiScatteringRMuToUv\b/u],
  ["heightAbove", /\bheightAbove\b/u],
  ["topRadiusM", /\btopRadiusM\b/u],
  ["subUvsToUnit", /\bsubUvsToUnit\b/u],
  ["unitToSubUvs", /\bunitToSubUvs\b/u],
];

/**
 * `wgsl` without its line and block comments, as the compiler reads it: both kinds in one pass from
 * the left, so that a `/*` inside a line comment opens nothing (block comments are not nested in
 * the atmosphere's modules, which hold none).
 */
export function wgslCode(wgsl: string): string {
  return wgsl.replaceAll(/\/\/[^\n]*|\/\*[\s\S]*?\*\//gu, " ");
}

/** The names of the forbidden forms `wgsl` holds, comments aside. */
export function forbiddenIn(wgsl: string): string[] {
  const code = wgslCode(wgsl);
  return FORBIDDEN_IN_ATMOSPHERE_WGSL.filter(([, pattern]) => pattern.test(code)).map(
    ([name]) => name,
  );
}

/** A `name : type` declaration in WGSL: a parameter, or a struct's member. */
export interface WgslDeclaration {
  readonly name: string;
  /** The declared type as written, `""` where none is. */
  readonly type: string;
}

/** A WGSL function as {@link wgslFunctions} reads it: its parameters, its return type and its body. */
export interface WgslFunction {
  readonly params: readonly WgslDeclaration[];
  /** The return type, `""` for none. */
  readonly returns: string;
  readonly body: string;
}

/** The index of the bracket that closes the one at `open` in `code`, or -1. */
function closingBracket(code: string, open: number): number {
  const opener = code[open];
  const closer = opener === "(" ? ")" : opener === "{" ? "}" : "";
  let depth = 0;
  for (let i = open; i < code.length; i += 1) {
    if (code[i] === opener) {
      depth += 1;
    } else if (code[i] === closer) {
      depth -= 1;
      if (depth === 0) {
        return i;
      }
    }
  }
  return -1;
}

/** `text` split at its commas outside any `<>`, `()` or `[]`, each part trimmed, empty ones left out. */
function topLevelParts(text: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let from = 0;
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i];
    if (c === "<" || c === "(" || c === "[") {
      depth += 1;
    } else if (c === ">" || c === ")" || c === "]") {
      depth -= 1;
    } else if (c === "," && depth === 0) {
      parts.push(text.slice(from, i).trim());
      from = i + 1;
    }
  }
  parts.push(text.slice(from).trim());
  return parts.filter((part) => part.length > 0);
}

/** A `name : type` declaration's name and type. */
function declarationOf(part: string): WgslDeclaration {
  const colon = part.indexOf(":");
  return colon < 0
    ? { name: part, type: "" }
    : { name: part.slice(0, colon).trim(), type: part.slice(colon + 1).trim() };
}

/** The functions `wgsl` declares, by name, comments aside ({@link wgslCode}). */
export function wgslFunctions(wgsl: string): ReadonlyMap<string, WgslFunction> {
  const code = wgslCode(wgsl);
  const found = new Map<string, WgslFunction>();
  for (const match of code.matchAll(/\bfn\s+(\w+)\s*\(/gu)) {
    const name = match[1];
    const open = match.index + match[0].length - 1;
    const close = closingBracket(code, open);
    const bodyOpen = code.indexOf("{", close);
    const bodyClose = bodyOpen < 0 ? -1 : closingBracket(code, bodyOpen);
    if (name === undefined || close < 0 || bodyClose < 0) {
      continue;
    }
    const arrow = code.slice(close + 1, bodyOpen).match(/->\s*([\s\S]*)$/u);
    found.set(name, {
      params: topLevelParts(code.slice(open + 1, close)).map(declarationOf),
      returns: arrow?.[1]?.trim() ?? "",
      body: code.slice(bodyOpen + 1, bodyClose),
    });
  }
  return found;
}

/** The structs `wgsl` declares, by name, with their members' types, comments aside. */
export function wgslStructs(wgsl: string): ReadonlyMap<string, readonly string[]> {
  const code = wgslCode(wgsl);
  const found = new Map<string, readonly string[]>();
  for (const match of code.matchAll(/\bstruct\s+(\w+)\s*\{/gu)) {
    const name = match[1];
    const open = match.index + match[0].length - 1;
    const close = closingBracket(code, open);
    if (name !== undefined && close >= 0) {
      found.set(
        name,
        topLevelParts(code.slice(open + 1, close)).map((part) => declarationOf(part).type),
      );
    }
  }
  return found;
}

/**
 * The functions `wgsl` declares that `roots` reach through their calls, the roots among them: what
 * an entry point's compiled code holds (plan R08, R08.T8.e).
 */
export function wgslReached(wgsl: string, roots: readonly string[]): ReadonlySet<string> {
  const functions = wgslFunctions(wgsl);
  const reached = new Set<string>();
  const pending = [...roots];
  for (let name = pending.pop(); name !== undefined; name = pending.pop()) {
    const fn = functions.get(name);
    if (reached.has(name) || fn === undefined) {
      continue;
    }
    reached.add(name);
    for (const call of fn.body.matchAll(/\b(\w+)\s*\(/gu)) {
      const callee = call[1];
      if (callee !== undefined && functions.has(callee) && !reached.has(callee)) {
        pending.push(callee);
      }
    }
  }
  return reached;
}

/**
 * Each parameter or return type, of the functions `roots` and of every helper they reach through
 * their calls, that holds an array by value (plan R08, R08.T8.d; `decision-r08-t8-leans.md` §8), as
 * `"<fn> <param> : <type>"` or `"<fn> -> <type>"`, sorted by function name, parameters before the
 * return.
 *
 * @remarks
 * A type holds an array by value where it is an `array<…>`, or a struct with such a member at any
 * depth; a `ptr<…>` holds none. Tint keeps an array value indexed by a loop variable in a
 * function-scope variable of its own, stored whole at each call, and a function-scope array indexed
 * by a variable lives in the invocation's local memory: in the per-frame marches' steps that cost
 * the ray march about 7 times R05's step on the RTX 3080, as R05.T14's `Medium` by value had cost
 * its own. A helper's local variables are not counted.
 */
export function arraysByValueFrom(wgsl: string, roots: readonly string[]): string[] {
  const functions = wgslFunctions(wgsl);
  const structs = wgslStructs(wgsl);
  const holdsArray = (type: string, seen: ReadonlySet<string> = new Set()): boolean => {
    if (type.startsWith("ptr<")) {
      return false;
    }
    if (type.startsWith("array<")) {
      return true;
    }
    const members = structs.get(type);
    if (members === undefined || seen.has(type)) {
      return false;
    }
    return members.some((member) => holdsArray(member, new Set([...seen, type])));
  };
  const reached = wgslReached(wgsl, roots);
  const offences: string[] = [];
  for (const name of [...reached].toSorted()) {
    const fn = functions.get(name);
    for (const param of fn?.params ?? []) {
      if (holdsArray(param.type)) {
        offences.push(`${name} ${param.name} : ${param.type}`);
      }
    }
    if (fn !== undefined && holdsArray(fn.returns)) {
      offences.push(`${name} -> ${fn.returns}`);
    }
  }
  return offences;
}
