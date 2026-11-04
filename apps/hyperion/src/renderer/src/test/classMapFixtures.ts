/**
 * A worker's class-map answer the size of R10.T14.f's bench map, for the take's tests and its
 * bench (lane perf-main-thread): K classes over the faces' texels surveyed three quarters (faces 0
 * to 3 whole and half of face 4), a tenth of them in part, each texel's bytes over two or three
 * classes; optionally a maturity and a fine page; and the normal map of the same survey at a
 * field's level. Every value comes from one seeded generator, so that a map's bytes can be pinned.
 */
import { MAX_FIELD_LEVEL, fieldCellAt } from "../view/terrain/coverage";
import { MATURITY_NODES } from "../view/terrain/maturityNodes";
import { type ClassMapAnswer, PALETTE_NONE } from "../view/terrain/workers/messages";

/** What {@link surveyedAnswer} makes. */
export interface SurveyedAnswerOptions {
  /** K, 1 to 16. */
  readonly classes: number;
  /** Whether it carries a maturity: each texel's code and each class's nodes. */
  readonly maturity: boolean;
  /** Its fine page: every ninth texel refined, at its cells' corners or centres; none by default. */
  readonly fine?: "corners" | "centres";
  /** r, the cells along a refined texel's side, 2 or 4: 4 by default. */
  readonly fineSide?: 2 | 4;
  /** The normal map's level, 5 to 8: {@link MAX_FIELD_LEVEL} by default. */
  readonly normalLevel?: number;
}

/** A linear congruential generator's draws in [0, 1), from `seed`. */
function draws(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1_664_525) + 1_013_904_223) >>> 0;
    return state / 2 ** 32;
  };
}

/** Whether the survey holds cell `index` of `level`: faces 0 to 3, and face 4's lower half. */
function surveyed(level: number, index: number): boolean {
  const cell = fieldCellAt(level, index);
  return cell.face < 4 || (cell.face === 4 && cell.j < 2 ** level / 2);
}

/** A class-map answer at level 6 over three quarters of the faces (the module's comments). */
export function surveyedAnswer(options: SurveyedAnswerOptions): ClassMapAnswer {
  const k = options.classes;
  const next = draws(12_345 + 17 * k);
  const texels: number[] = [];
  const fractions: number[] = [];
  const weights: number[] = [];
  const codes: number[] = [];
  for (let index = 0; index < 6 * 4 ** 6; index += 1) {
    if (!surveyed(6, index)) {
      continue;
    }
    texels.push(index);
    fractions.push(next() < 0.1 ? Math.fround(0.25 + 0.5 * next()) : 1);
    const raw = Array.from({ length: k }, () => (next() < 0.35 ? next() : 0));
    raw[Math.floor(next() * k)] = 1;
    const sum = raw.reduce((a, b) => a + b, 0);
    const bytes = raw.map((w) => Math.floor((255 * w) / sum));
    bytes[0] = (bytes[0] ?? 0) + 255 - bytes.reduce((a, b) => a + b, 0);
    weights.push(...bytes);
    codes.push(Math.floor(next() * 65_536));
  }
  const classes: number[] = [];
  const photometry: number[] = [];
  for (let c = 0; c < k; c += 1) {
    const unnamed = c === k - 1 && k > 1;
    classes.push(c % 5 === 4 ? 2 : 1 + (c % 4), unnamed ? PALETTE_NONE : c);
    photometry.push(...(unnamed ? [0, 0, 0, 0, 0] : [0.1 + 0.05 * c, 0.11, 0.12, 1 + (c % 3), 1]));
  }
  const nodes = new Float32Array(3 * MATURITY_NODES * k);
  for (let at = 0; at < nodes.length; at += 1) {
    nodes[at] = 0.05 + 0.5 * next();
  }
  const normalLevel = options.normalLevel ?? MAX_FIELD_LEVEL;
  const normalCells: number[] = [];
  const pairs: number[] = [];
  for (let index = 0; index < 6 * 4 ** normalLevel; index += 1) {
    if (surveyed(normalLevel, index)) {
      normalCells.push(index);
      pairs.push(2 * next() - 1, 2 * next() - 1);
    }
  }
  const answer: ClassMapAnswer = {
    level: 6,
    fieldLevel: normalLevel,
    classes: Uint8Array.from(classes),
    photometry: Float32Array.from(photometry),
    texels: Uint32Array.from(texels),
    fractions: Float32Array.from(fractions),
    weights: Uint8Array.from(weights),
    complete: true,
    normalLevel,
    normalCells: Uint32Array.from(normalCells),
    normalPairs: Float16Array.from(pairs),
    normalsComplete: true,
    ...(options.maturity ? { maturity: { codes: Uint16Array.from(codes), nodes } } : {}),
  };
  if (options.fine === undefined) {
    return answer;
  }
  const side = options.fineSide ?? 4;
  const corners = options.fine === "corners";
  const readings = corners ? (side + 1) ** 2 : side * side;
  const refined: number[] = [];
  const fineSurveyed: number[] = [];
  const fineWeights: number[] = [];
  const fineCodes: number[] = [];
  for (let t = 0; t < texels.length; t += 9) {
    refined.push(t);
    for (let c = 0; c < side * side; c += 1) {
      fineSurveyed.push(next() < 0.9 ? 1 : 0);
    }
    for (let c = 0; c < readings; c += 1) {
      const major = Math.floor(next() * k);
      const none = next() < 0.05;
      for (let q = 0; q < k; q += 1) {
        fineWeights.push(none ? 0 : q === major ? 200 : q === (major + 1) % k ? 55 : 0);
      }
      if (options.maturity) {
        fineCodes.push(Math.floor(next() * 65_536));
      }
    }
  }
  return {
    ...answer,
    refinement: {
      side,
      corners,
      texels: Uint32Array.from(refined),
      surveyed: Uint8Array.from(fineSurveyed),
      weights: Uint8Array.from(fineWeights),
      codes: Uint16Array.from(fineCodes),
    },
    ...(corners ? { cellCorners: true as const } : {}),
  };
}
