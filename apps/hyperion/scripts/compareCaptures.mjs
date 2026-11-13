// Compares two sets of captures of a bounded lever (plan R14, Design note 10; R14.T1.d): for each
// pair of 8-bit sRGB PNGs, how many of its terrain pixels differ, the share within a third of a stop
// in luminance (the bounded levers' unit), and the largest difference, stops. By hand, as
// `levelCounts.mjs` runs; not part of `ci`.
//
//   node scripts/compareCaptures.mjs <before> <after> [--mask <png>] [--min-share <0–1>]
//
// <before> and <after> are two PNGs, or two directories whose PNGs of the same name are paired
// (`just test-render --captures=DIR`'s `<name>-<variant>.png`). --mask is a PNG of the frames' size,
// white where the terrain is (test-render's `r14-t1d-terrain-mask-<variant>.png`); without it every
// pixel counts. The exposure must be the same in both, as test-render's captures hold it: auto
// exposure's histogram would move every pixel when the terrain's move. A pixel is within where its
// luminance is within a third of a stop of the other's, or where no channel differs by more than one
// 8-bit code, which near black is more than a third of a stop. --min-share (default 0.995) makes
// the exit code 1 where a pair's share falls below it.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join } from "node:path";
import { inflateSync } from "node:zlib";

const THIRD_STOP = 1 / 3;

function option(args, name, fallback) {
  const at = args.indexOf(`--${name}`);
  return at >= 0 && at + 1 < args.length ? args[at + 1] : fallback;
}

/** The arguments that are neither an option nor an option's value. */
function positional(args) {
  return args.filter(
    (arg, at) =>
      !String(arg).startsWith("--") && (at === 0 || !String(args[at - 1]).startsWith("--")),
  );
}

/** PNG's Paeth predictor of a byte from its left, upper and upper-left neighbours. */
function paeth(a, b, c) {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  return pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
}

/** An 8-bit, non-interlaced RGB or RGBA PNG's pixels as RGBA, row by row from the top. */
function readPng(path) {
  const bytes = readFileSync(path);
  const signature = [137, 80, 78, 71, 13, 10, 26, 10];
  if (!signature.every((byte, at) => bytes[at] === byte)) {
    throw new Error(`${path} is not a PNG`);
  }
  let width = 0;
  let height = 0;
  let channels = 0;
  const data = [];
  for (let at = 8; at < bytes.length;) {
    const length = bytes.readUInt32BE(at);
    const type = bytes.toString("latin1", at + 4, at + 8);
    const body = bytes.subarray(at + 8, at + 8 + length);
    if (type === "IHDR") {
      width = body.readUInt32BE(0);
      height = body.readUInt32BE(4);
      const [depth, colour, , , interlace] = [body[8], body[9], body[10], body[11], body[12]];
      if (depth !== 8 || (colour !== 2 && colour !== 6) || interlace !== 0) {
        throw new Error(`${path}: only 8-bit RGB or RGBA without interlace is read`);
      }
      channels = colour === 6 ? 4 : 3;
    } else if (type === "IDAT") {
      data.push(body);
    } else if (type === "IEND") {
      break;
    }
    at += 12 + length;
  }
  const raw = inflateSync(Buffer.concat(data));
  const stride = width * channels;
  const pixels = new Uint8Array(width * height * 4);
  const row = new Uint8Array(stride);
  const previous = new Uint8Array(stride);
  for (let y = 0; y < height; y += 1) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let x = 0; x < stride; x += 1) {
      const a = x >= channels ? row[x - channels] : 0;
      const b = previous[x];
      const c = x >= channels ? previous[x - channels] : 0;
      const predictor = [0, a, b, (a + b) >> 1, paeth(a, b, c)][filter];
      if (predictor === undefined) {
        throw new Error(`${path}: row ${y} has filter ${filter}`);
      }
      row[x] = (line[x] + predictor) & 0xff;
    }
    for (let x = 0; x < width; x += 1) {
      for (let k = 0; k < 4; k += 1) {
        pixels[4 * (y * width + x) + k] = k < channels ? row[x * channels + k] : 255;
      }
    }
    previous.set(row);
  }
  return { width, height, pixels };
}

/** An 8-bit sRGB code's linear value (IEC 61966-2-1). */
function linear(code) {
  const v = code / 255;
  return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
}

/** Rec. 709's luminance of pixel `p`. */
function luminance(pixels, p) {
  return (
    0.2126 * linear(pixels[4 * p]) +
    0.7152 * linear(pixels[4 * p + 1]) +
    0.0722 * linear(pixels[4 * p + 2])
  );
}

/** One pair's comparison over the mask's pixels. */
function compare(before, after, mask) {
  if (before.width !== after.width || before.height !== after.height) {
    throw new Error(
      `the frames are ${before.width} × ${before.height} and ${after.width} × ${after.height}`,
    );
  }
  // Half the first 8-bit code's linear value: a black pixel's floor.
  const floor = linear(0.5);
  let terrain = 0;
  let differing = 0;
  let within = 0;
  let largestStops = 0;
  for (let p = 0; p < before.width * before.height; p += 1) {
    if (mask !== null && mask.pixels[4 * p] < 128) {
      continue;
    }
    terrain += 1;
    const codes = [0, 1, 2].map((k) =>
      Math.abs(before.pixels[4 * p + k] - after.pixels[4 * p + k]),
    );
    if (codes.every((d) => d === 0)) {
      within += 1;
      continue;
    }
    differing += 1;
    const stops = Math.abs(
      Math.log2(
        Math.max(luminance(after.pixels, p), floor) / Math.max(luminance(before.pixels, p), floor),
      ),
    );
    largestStops = Math.max(largestStops, stops);
    within += stops <= THIRD_STOP || codes.every((d) => d <= 1) ? 1 : 0;
  }
  return { terrain, differing, within, largestStops };
}

/** The pairs to compare: two PNGs, or two directories' PNGs of the same name. */
function pairs(before, after) {
  if (String(before).endsWith(".png")) {
    return [[before, after]];
  }
  return readdirSync(before)
    .filter((name) => name.endsWith(".png"))
    .filter((name) => {
      try {
        return statSync(join(after, name)).isFile();
      } catch {
        return false;
      }
    })
    .toSorted()
    .map((name) => [join(before, name), join(after, name)]);
}

function main(args) {
  const [before, after] = positional(args);
  if (before === undefined || after === undefined) {
    process.stderr.write(
      "usage: node scripts/compareCaptures.mjs <before> <after> [--mask <png>] [--min-share <0–1>]\n",
    );
    return 2;
  }
  const maskPath = option(args, "mask", null);
  const mask = maskPath === null ? null : readPng(maskPath);
  const minShare = Number(option(args, "min-share", "0.995"));
  let failed = false;
  for (const [a, b] of pairs(before, after)) {
    const result = compare(readPng(a), readPng(b), mask);
    const share = result.within / Math.max(1, result.terrain);
    failed ||= share < minShare;
    process.stdout.write(
      `${basename(a)} → ${basename(b)}: ${result.terrain} terrain pixels, ${result.differing} differing, ` +
        `${(100 * share).toFixed(2)}% within a third of a stop, the rest ${result.terrain - result.within}, ` +
        `the largest ${result.largestStops.toFixed(3)} stops\n`,
    );
  }
  return failed ? 1 : 0;
}

process.exitCode = main(process.argv.slice(2));
