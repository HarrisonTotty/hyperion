// The Node floor's preflight: exits 1 with one line when the running Node is older than the floor
// that the root package.json's `engines.node` declares (`>=X.Y.Z`), 2 with one line when it cannot
// read that floor, and 0, silently, otherwise. The packages' `test` and `typecheck` scripts run it
// before vitest and tsc, and `just smart-ci` before a plan that runs either, so that a Node too old
// for the tests stops at once rather than failing them by the dozen.
//
// Why the floor is 26.0.0: the tests construct `Float16Array`, global since Node 24.0.0 (V8 13.6;
// nodejs.org/en/blog/release/v24.0.0), and the logic tests, which run in Node rather than jsdom,
// `ErrorEvent`, global since Node 25.0.0 (nodejs.org/api/globals.html#errorevent). vitest 5.0 and
// jsdom 30.1 accept Node 22, 24 and 26 onwards but not 25 (their `engines`), which is past its end
// of life (2026-06-01) in any case; 26 is an LTS line from 2026-10-28.
//
// Usage: `node tools/node-floor/check.cjs [package.json]`, the repository's own by default. Plain
// CommonJS that parses on Node 8 and later, so that an old Node still prints the line; it runs
// wherever Node does, Linux, macOS and Windows. Only the numeric X.Y.Z of the running version is
// compared: a prerelease of the floor's own version (26.0.0-rc.1) passes, having its features.
"use strict";

const fs = require("fs");
const path = require("path");

const manifest =
  process.argv.length > 2 ? process.argv[2] : path.join(__dirname, "..", "..", "package.json");
let range;
try {
  const engines = JSON.parse(fs.readFileSync(manifest, "utf8")).engines;
  range = typeof engines === "object" && engines !== null ? String(engines.node) : "undefined";
} catch (error) {
  console.error(`error: the Node floor's preflight cannot read ${manifest}: ${String(error)}`);
  process.exit(2);
}
const floor = /^>=\s*(\d+)\.(\d+)\.(\d+)$/.exec(range);
if (floor === null) {
  console.error(
    `error: ${manifest}'s engines.node is "${range}", not the ">=X.Y.Z" the preflight reads`,
  );
  process.exit(2);
}
const wanted = floor.slice(1).map(Number);
const running = process.versions.node.split(".").map((part) => Number.parseInt(part, 10));
for (let i = 0; i < 3; i += 1) {
  if (running[i] !== wanted[i]) {
    if (running[i] < wanted[i]) {
      console.error(
        `error: Node ${process.versions.node} is older than HYPERION's floor ${wanted.join(".")} ` +
          `(engines.node in package.json); install Node ${wanted[0]} or newer`,
      );
      process.exit(1);
    }
    break;
  }
}
