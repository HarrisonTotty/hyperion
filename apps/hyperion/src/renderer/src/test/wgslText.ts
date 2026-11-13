/**
 * WGSL source read blind to its whitespace, for the shaders' parity pins (plan R10, R10.T1.c's
 * client half): a module-scope function's body found by matching its braces, and text with each
 * run of whitespace as one space. A pin written against them holds whatever the source's
 * indentation, and on a checkout whose line ends are CRLF (`core.autocrlf` on Windows), where a
 * pin that embeds `"\n"` or looks for a function's end at `"\n}\n"` fails.
 *
 * @remarks
 * A test helper, not a parser: comments are kept, so a function's comments must not hold an
 * unmatched brace, and a function is found by its first `fn <name>(` in the source.
 */

/** `text` with each run of whitespace, line ends and indentation included, as one space. */
export function collapseWhitespace(text: string): string {
  return text.replaceAll(/\s+/gu, " ");
}

/**
 * Module-scope WGSL function `name`'s body in `source`, from its opening brace to the brace that
 * closes it, braces and all, its whitespace collapsed ({@link collapseWhitespace}).
 *
 * @example
 * `wgslFunction("fn one() -> f32 {\r\n  return 1.0;\r\n}\r\n", "one")` is
 * `"{ return 1.0; }"`.
 *
 * @throws Error where `source` has no function `name`, or its body is not closed.
 */
export function wgslFunction(source: string, name: string): string {
  const start = source.indexOf(`fn ${name}(`);
  const open = start < 0 ? -1 : source.indexOf("{", start);
  if (open < 0) {
    throw new Error(`the source has no function ${name}`);
  }
  let depth = 0;
  for (let end = open; end < source.length; end += 1) {
    const c = source[end];
    if (c === "{") {
      depth += 1;
    } else if (c === "}") {
      depth -= 1;
      if (depth === 0) {
        return collapseWhitespace(source.slice(open, end + 1));
      }
    }
  }
  throw new Error(`function ${name} is not closed`);
}
