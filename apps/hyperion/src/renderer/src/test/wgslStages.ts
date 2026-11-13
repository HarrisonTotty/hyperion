/**
 * Which shader stages read each `@group(2)` binding of a composed WGSL source, by a static walk of
 * its entry points' call graphs (R10's decision-r10-t10b-r09-t2e §5): the check that a material's
 * declared storage-buffer stages are the stages that use them.
 *
 * @remarks
 * A test helper, not a parser: comments are stripped, each module-scope `fn` body is found by
 * matching braces, and a function is taken to call every function, and read every binding, whose
 * name it mentions as a word not after a `.` (a member access). WGSL has no function pointers or
 * overloads, so a mention is a call; a local or parameter that shadows a binding's name would be
 * read as a use, which the terrain's sources do not have. The adapter's own validation, on
 * SwiftShader in the smoke harness, checks the layout against the shaders as well.
 */

import type { ShaderStageName } from "../view/engine/types";

/** `source` without its `//` and block comments. */
function stripComments(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//gu, " ").replace(/\/\/[^\n]*/gu, " ");
}

/** The words of `text` that are not member accesses. */
function wordsOf(text: string): Set<string> {
  return new Set(text.match(/(?<![.\w])[A-Za-z_]\w*/gu) ?? []);
}

/** Each module-scope function's name and body. */
function functionsOf(source: string): Map<string, string> {
  const functions = new Map<string, string>();
  const head = /\bfn\s+([A-Za-z_]\w*)\s*\(/gu;
  for (let match = head.exec(source); match !== null; match = head.exec(source)) {
    const name = match[1];
    const open = source.indexOf("{", head.lastIndex);
    if (name === undefined || open < 0) {
      continue;
    }
    let depth = 0;
    let end = open;
    for (; end < source.length; end += 1) {
      const c = source[end];
      if (c === "{") {
        depth += 1;
      } else if (c === "}") {
        depth -= 1;
        if (depth === 0) {
          break;
        }
      }
    }
    functions.set(name, source.slice(open + 1, end));
    head.lastIndex = end;
  }
  return functions;
}

/**
 * The stages whose entry points (`@vertex fn vertexMain`, `@fragment fn fragmentMain`) reach a
 * read of each `@group(2)` binding of `source`, by binding number; a binding no entry point reads
 * is absent.
 */
export function bindingStages(source: string): Map<number, Set<ShaderStageName>> {
  const text = stripComments(source);
  const bindings = new Map<string, number>();
  const declaration = /@group\(2\)\s*@binding\((\d+)\)\s*var(?:<[^>]*>)?\s+([A-Za-z_]\w*)/gu;
  for (const match of text.matchAll(declaration)) {
    const [, binding, name] = match;
    if (binding !== undefined && name !== undefined) {
      bindings.set(name, Number(binding));
    }
  }
  const functions = functionsOf(text);
  const words = new Map([...functions].map(([name, body]) => [name, wordsOf(body)]));
  const stages = new Map<number, Set<ShaderStageName>>();
  const entries: ReadonlyArray<readonly [ShaderStageName, RegExp]> = [
    ["vertex", /@vertex\s+fn\s+([A-Za-z_]\w*)/u],
    ["fragment", /@fragment\s+fn\s+([A-Za-z_]\w*)/u],
  ];
  for (const [stage, pattern] of entries) {
    const entry = pattern.exec(text)?.[1];
    if (entry === undefined) {
      continue;
    }
    const seen = new Set<string>();
    const pending = [entry];
    for (let name = pending.pop(); name !== undefined; name = pending.pop()) {
      if (seen.has(name)) {
        continue;
      }
      seen.add(name);
      for (const word of words.get(name) ?? []) {
        const binding = bindings.get(word);
        if (binding !== undefined) {
          const set = stages.get(binding) ?? new Set<ShaderStageName>();
          set.add(stage);
          stages.set(binding, set);
        }
        if (functions.has(word) && !seen.has(word)) {
          pending.push(word);
        }
      }
    }
  }
  return stages;
}
