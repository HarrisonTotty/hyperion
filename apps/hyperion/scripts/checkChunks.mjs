#!/usr/bin/env node
// Checks that the renderer build loads the engine lazily (R01 Design note 14): the entry chunk,
// and every chunk it imports statically, hold no Babylon code, and a chunk named `babylon` exists.
// Run after `pnpm build`: `node apps/hyperion/scripts/checkChunks.mjs`. Exits 0 when both hold,
// 1 when either fails, 2 when the build is missing.
//
// Babylon code is recognised by strings its modules carry through minification: `babylonjs`
// (its CDN and documentation URLs) and `Babylon.js`.

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const RENDERER_OUT = resolve(dirname(fileURLToPath(import.meta.url)), "../out/renderer");
const ASSETS = join(RENDERER_OUT, "assets");
const MARKERS = ["babylonjs", "Babylon.js"];

function fail(message, code = 1) {
  console.error(`checkChunks: ${message}`);
  process.exit(code);
}

if (!existsSync(join(RENDERER_OUT, "index.html")) || !existsSync(ASSETS)) {
  fail(`no renderer build at ${RENDERER_OUT}; run \`pnpm build\` first`, 2);
}

const html = readFileSync(join(RENDERER_OUT, "index.html"), "utf8");
const entries = [...html.matchAll(/<script[^>]*\bsrc="\.\/assets\/([^"]+\.js)"/g)].map(
  (match) => match[1],
);
if (entries.length === 0) {
  fail("index.html names no entry chunk", 2);
}

/** The entry chunks and every chunk they import statically, not through `import()`. */
function staticClosure(roots) {
  const seen = new Set();
  const pending = [...roots];
  while (pending.length > 0) {
    const name = pending.pop();
    if (seen.has(name)) {
      continue;
    }
    seen.add(name);
    const code = readFileSync(join(ASSETS, name), "utf8");
    for (const match of code.matchAll(/(?:from|import)\s*["']\.\/([^"']+\.js)["']/g)) {
      pending.push(match[1]);
    }
  }
  return [...seen];
}

const holdsBabylon = (name) => {
  const code = readFileSync(join(ASSETS, name), "utf8");
  return MARKERS.some((marker) => code.includes(marker));
};

const eager = staticClosure(entries);
const tainted = eager.filter(holdsBabylon);
if (tainted.length > 0) {
  fail(`the entry loads Babylon code eagerly, in ${tainted.join(", ")}`);
}

const babylonChunks = readdirSync(ASSETS).filter(
  (name) => /^babylon-[\w-]+\.js$/.test(name) && holdsBabylon(name),
);
if (babylonChunks.length === 0) {
  fail("no `babylon` chunk holds Babylon code: is anything importing view/engine/loadEngine.ts?");
}

console.warn(
  `checkChunks: entry ${eager.join(", ")} holds no Babylon code; lazy chunk ${babylonChunks.join(", ")}`,
);
