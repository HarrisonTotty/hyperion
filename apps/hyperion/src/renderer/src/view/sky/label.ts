/**
 * The view label block's sky line (plan R06, Design note 23, T13.f), to the guide's `STARS` row as
 * signed off in R06.T15, with the stars-arriving note of R06.T11.f: the limit as a V magnitude in
 * `mag` with its kind, then the notes on the sky.
 *
 * @remarks
 * `STARS V 7.4 mag EYE` for the eye (its deepest limit, at the view's field factor) and
 * `STARS V 9.5 mag CAM` for a camera, never a magnitude alone. The notes follow, each after a
 * middle dot, in a fixed order: the sky's annunciations, `BEYOND 1000 ly: STREAMING` while the
 * reply held is not final and {@link SKY_DETAIL_LIMITED_NOTE} while it holds the sky at a synthetic
 * ceiling (plan R13, R13.T2.b), then what the sky leaves out, as one composed note,
 * `CLUSTERS AND WHITE DWARFS: NOT YET MODELLED`. Which of them a line holds depends on where it
 * stands ({@link SkyLinePlace}; decision-r06-t11f-stars-line), so that every reading fits its
 * block as measured (four lines at most at 1280 × 720). Before the sky's first reply the line reads
 * {@link SKY_PENDING}; where no sky is asked it stays R02's (`STAR_SOURCE` and
 * `STARS_WITHOUT_POSITION` in `displays/view/viewRun.ts`), which the label block chooses between.
 * While the link is down the limit is stale, `STARS V 10.1 mag S CAM` (R06.T13.j), as the edge is
 * in its note, each in a field of its own ({@link SKY_LIMIT_FIELD_CH}, {@link SKY_EDGE_FIELD_CH})
 * that the label block sets and marks; `PENDING` takes no mark.
 */

import { MAX_CUT_V, type SkyGapDto, type SkyResponse } from "@hyperion/protocol";

import { formatNumber } from "../../lib/format";
import { FOV_STEPS_DEG } from "../camera/projection";
import { MAN_EV100_MAX, programTriple, VIEW_CAMERA } from "../photometry/exposure";
import { cameraLimitV, DEFAULT_VIEW_CAMERA } from "./cameraLimit";

/** A view's role for its star limit: the eye's threshold or a camera's noise floor. */
export type SkyLimitKind = "eye" | "camera";

/** Each limit kind's word on the line, as the guide's `EYE` and `CAM` rows name them. */
const KIND_WORDS: Readonly<Record<SkyLimitKind, string>> = { eye: "EYE", camera: "CAM" };

/**
 * The `STARS` line's reading from the sky's request until its first reply, on every view's block,
 * after a jump too, as `LIGHTING: PENDING` reads (R06.T11.f).
 */
export const SKY_PENDING = "PENDING";

/**
 * The interim note (plan R13, R13.T2.b; decision-r13-t2b-note): while a reply holds the sky at a
 * synthetic ceiling (`synthetic_ceiling_v`) and carries no synthetic stars, the stars of layers C,
 * D and E beyond each direction's real boundary, at most `real_limit_ly` away, are not drawn
 * singly, and the band holds their light. It names no distance, since the boundary differs by
 * layer and direction and lies far within the limit where dust is thick; the guide's row names
 * the ceiling, the boundary's rule and the limit. R13.T8's synthetic note keeps its subject.
 */
export const SKY_DETAIL_LIMITED_NOTE = "DISTANT STARS: DETAIL LIMITED";

/**
 * A limit as the `STARS` line reads it before its kind, the run its field holds: `V 9.5 mag`, a V
 * magnitude to one decimal in `mag`.
 */
export function skyLimitReading(limitV: number): string {
  return `V ${formatNumber(limitV, 1)} mag`;
}

/**
 * The brightest limit a `STARS` line states at an exposure `MAN` takes, V: a camera's at the view
 * camera's darkest such triple, EV100 42 ({@link MAN_EV100_MAX}, its ND's densest at its shortest
 * shutter and base ISO), at its widest field, over a black sky: V -20.2.
 *
 * @remarks
 * A camera metering a star's disc reads below V -10, so a limit may have two digits and a sign.
 * `AUTO` meters past EV100 42, its ND pegged, only above the scene span, whose top, an O star's
 * disc, meters at 41.1 ({@link MAN_EV100_MAX}'s remarks); a limit of three digits would need about
 * EV100 148.
 */
const BRIGHTEST_LIMIT_V = cameraLimitV(
  DEFAULT_VIEW_CAMERA,
  programTriple(VIEW_CAMERA, MAN_EV100_MAX),
  Math.max(...FOV_STEPS_DEG),
  0,
);

/**
 * The width of the limit's field, ch: its widest reading with its stale mark, `V -20.2 mag S`
 * (wider than the deepest cut's `V 11.0 mag S`), so that the line keeps its breaks as the limit
 * changes and when it goes stale (the guide's "Numbers": "a field has a fixed width sized for its
 * longest possible value and its status marks"; R06.T13.j).
 */
export const SKY_LIMIT_FIELD_CH = Math.max(
  ...[BRIGHTEST_LIMIT_V, MAX_CUT_V].map((limitV) => `${skyLimitReading(limitV)} S`.length),
);

/**
 * The largest fixed shell edge a reply that is not final can state, ly: the last of the sim's
 * `SHELL_EDGES_LY` (`crates/hyperion-sim/src/sky/census/query.rs`), 500 ly then 1,000 × 2^k ly to
 * 128,000 ly, beyond which no cap reaches.
 */
const LARGEST_EDGE_LY = 128_000;

/**
 * An edge as the stars-arriving note reads it: `2000 ly`, `16,000 ly`, its digits grouped in
 * threes from five as every number's are (the guide's "Numbers").
 */
export function skyEdgeReading(edgeLy: number): string {
  return `${formatNumber(edgeLy, 0)} ly`;
}

/**
 * The width of the edge's field, ch: its longest reading with its stale mark, `128,000 ly S`, so
 * that the line keeps its breaks as the edge grows and when it goes stale (the guide's "Numbers":
 * "a field has a fixed width sized for its longest possible value and its status marks").
 */
export const SKY_EDGE_FIELD_CH = `${skyEdgeReading(LARGEST_EDGE_LY)} S`.length;

/**
 * The edge to which a reply's stars have arrived, ly, or `null` for a final reply: the least
 * `complete_to_ly` over the layers not yet final, which is a fixed shell edge (the sim's
 * `CompleteTo::least_edge`; R06.T11.d), never a ray's radius.
 *
 * @remarks
 * A reply that is not final but whose every layer is (which the server never sends) is complete to
 * every cap, so it has no edge to state either.
 */
export function streamingEdgeLy(response: SkyResponse): number | null {
  if (response.final) {
    return null;
  }
  const edges = response.census
    .filter((layer) => !layer.final)
    .map((layer) => layer.complete_to_ly);
  return edges.length === 0 ? null : Math.min(...edges);
}

/**
 * Whether a reply holds its sky at a synthetic ceiling, so that the interim note
 * {@link SKY_DETAIL_LIMITED_NOTE} stands (plan R13, R13.T2.b; decision-r13-t2b-note §2): the reply
 * states `synthetic_ceiling_v`, at any value, the cut's own included.
 *
 * @remarks
 * Neither `real_limit_ly` nor a
 * layer's `bright_beyond` is read. A reply that states none, from an older server or of a sky whose
 * caps already lie within the real limit, carries no note. R13.T8 adds "and carries no synthetic
 * stars".
 */
export function detailLimited(response: SkyResponse): boolean {
  return response.synthetic_ceiling_v !== undefined;
}

/**
 * Where a `STARS` line stands on its display, which says which of its sky's notes it holds
 * (decision-r06-t11f-stars-line, 1d). A display's views draw one sky, so its annunciations stand on
 * the `PRIMARY` view's line alone.
 *
 * - `alone`: the primary's, with no instrument open. Every note that holds.
 * - `beside`: the primary's beside an open instrument whose line shows the sky's reading, and so
 *   what the sky leaves out. One note, the first that holds: an annunciation, `STREAMING` first,
 *   else what the sky leaves out.
 * - `beside-unshown`: the primary's beside open instruments none of whose lines shows the sky's
 *   reading (not yet sized, or not yet drawn). One note, what the sky leaves out first, since no
 *   other line on the display says it.
 * - `instrument`: an instrument's. What the sky leaves out alone, never an annunciation.
 */
export type SkyLinePlace = "alone" | "beside" | "beside-unshown" | "instrument";

/** What a line's notes are made from, besides the sky's gaps. */
export interface SkyLineStanding {
  /** Where the line stands. */
  readonly place: SkyLinePlace;
  /** The edge the reply held has reached, ly ({@link streamingEdgeLy}), or `null` once final. */
  readonly streamingEdgeLy: number | null;
  /**
   * Whether the reply held keeps its sky at a synthetic ceiling ({@link detailLimited}), so that
   * {@link SKY_DETAIL_LIMITED_NOTE} stands after `STREAMING`.
   */
  readonly detailLimited: boolean;
}

/**
 * Each gap's subject in the composed `NOT YET MODELLED` note. The centre's members are a
 * feature's members too, so both read `CLUSTERS`, once.
 */
const GAP_PHRASES: Readonly<Record<SkyGapDto, string>> = {
  feature_members: "CLUSTERS",
  centre_members: "CLUSTERS",
  white_dwarfs: "WHITE DWARFS",
};

/** The subjects' fixed order in the note, whatever the gaps' order: clusters, then white dwarfs. */
const SUBJECT_ORDER: ReadonlyArray<string> = ["CLUSTERS", "WHITE DWARFS"];

/** `A`, `A AND B`, `A, B AND C`: the guide's composed list of what is not yet modelled. */
function joinSubjects(subjects: ReadonlyArray<string>): string {
  if (subjects.length <= 1) {
    return subjects.join("");
  }
  return `${subjects.slice(0, -1).join(", ")} AND ${subjects[subjects.length - 1]}`;
}

/**
 * One note on a `STARS` line: an annunciation of the sky (`STREAMING`, then the interim note,
 * {@link SKY_DETAIL_LIMITED_NOTE}), or what the sky leaves out, which always comes last.
 */
export type SkyNote =
  | { readonly kind: "streaming"; readonly edgeLy: number }
  | { readonly kind: "detail-limited" }
  | { readonly kind: "not-modelled"; readonly subjects: ReadonlyArray<string> };

/**
 * A note's text: `BEYOND 1000 ly: STREAMING`, `DISTANT STARS: DETAIL LIMITED`, or
 * `CLUSTERS AND WHITE DWARFS: NOT YET MODELLED`.
 */
function noteText(note: SkyNote): string {
  let text: string;
  switch (note.kind) {
    case "detail-limited":
      text = SKY_DETAIL_LIMITED_NOTE;
      break;
    case "streaming":
      text = `BEYOND ${skyEdgeReading(note.edgeLy)}: STREAMING`;
      break;
    case "not-modelled":
      text = `${joinSubjects(note.subjects)}: NOT YET MODELLED`;
      break;
  }
  return text;
}

/**
 * The notes a `STARS` line holds after its limit, in their order (decision-r06-t11f-stars-line,
 * 1d, and decision-r13-t2b-note §2): the sky's annunciations, `STREAMING` then the interim note,
 * then what it leaves out, as many as its place holds.
 *
 * @param gaps - The response's `not_modelled`.
 */
export function skyLineNotes(
  gaps: ReadonlyArray<SkyGapDto>,
  standing: SkyLineStanding,
): ReadonlyArray<SkyNote> {
  const present = new Set(gaps.map((gap) => GAP_PHRASES[gap]));
  const subjects = SUBJECT_ORDER.filter((subject) => present.has(subject));
  const streaming: ReadonlyArray<SkyNote> =
    standing.streamingEdgeLy === null
      ? []
      : [{ kind: "streaming", edgeLy: standing.streamingEdgeLy }];
  const interim: ReadonlyArray<SkyNote> = standing.detailLimited
    ? [{ kind: "detail-limited" }]
    : [];
  const annunciations = [...streaming, ...interim];
  const left: ReadonlyArray<SkyNote> =
    subjects.length === 0 ? [] : [{ kind: "not-modelled", subjects }];
  let held: ReadonlyArray<SkyNote>;
  switch (standing.place) {
    case "alone":
      held = [...annunciations, ...left];
      break;
    case "beside":
      held = [...annunciations, ...left].slice(0, 1);
      break;
    case "beside-unshown":
      held = left.length === 0 ? annunciations.slice(0, 1) : left;
      break;
    case "instrument":
      held = left;
      break;
  }
  return held;
}

/**
 * The `STARS` line's reading, after its label, once a reply of the sky is held: the limit, then
 * {@link skyLineNotes}, each after a middle dot.
 *
 * @remarks
 * The stale marks the limit and the edge take while the link is down are the label block's, which
 * sets each in its field ({@link skyLimitReading}, {@link skyEdgeReading}); the reading's text is
 * the same whether the sky is stale or not.
 *
 * @param limitV - The view's limit, quoted at its deepest over the sky: a camera's or the eye's.
 * @param gaps - The response's `not_modelled`.
 * @param standing - Where the line stands and the edge reached.
 */
export function skyLabelValue(
  limitV: number,
  limitKind: SkyLimitKind,
  gaps: ReadonlyArray<SkyGapDto>,
  standing: SkyLineStanding,
): string {
  const limit = `${skyLimitReading(limitV)} ${KIND_WORDS[limitKind]}`;
  return [limit, ...skyLineNotes(gaps, standing).map(noteText)].join(" · ");
}
