/**
 * The view label block's sky line (plan R06, Design note 23, T13.f), to the guide's `STARS` row as
 * signed off in R06.T15, with the stars-arriving note of R06.T11.f: the limit as a V magnitude in
 * `mag` with its kind, then the notes on the sky.
 *
 * @remarks
 * `STARS V 7.4 mag EYE` for the eye (its deepest limit, at the view's field factor) and
 * `STARS V 9.5 mag CAM` for a camera, never a magnitude alone. The notes follow, each after a
 * middle dot, in a fixed order: the sky's annunciation, `BEYOND 2000 ly: STREAMING` while the reply
 * held is not final, then what the sky leaves out, as one composed note,
 * `CLUSTERS AND WHITE DWARFS: NOT YET MODELLED`. Which of them a line holds depends on where it
 * stands ({@link SkyLinePlace}; decision-r06-t11f-stars-line), so that no block's line ever takes
 * more lines than its final reading. Before the sky's first reply the line reads
 * {@link SKY_PENDING}; where no sky is asked it stays R02's (`STAR_SOURCE` and
 * `STARS_WITHOUT_POSITION` in `displays/view/viewRun.ts`), which the label block chooses between.
 */

import type { SkyGapDto, SkyResponse } from "@hyperion/protocol";

import { formatNumber } from "../../lib/format";

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
 * Where a `STARS` line stands on its display, which says which of its sky's notes it holds
 * (decision-r06-t11f-stars-line, 1d). A display's views draw one sky, so its annunciations stand on
 * the `PRIMARY` view's line alone.
 *
 * - `alone`: the primary's, with no instrument open. Every note that holds.
 * - `beside`: the primary's beside an open instrument whose line shows the sky's reading, and so
 *   what the sky leaves out. One note, the first that holds: the annunciation, else what the sky
 *   leaves out.
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
 * One note on a `STARS` line: an annunciation of the sky (`STREAMING`; R13.T2.b adds its interim
 * note after it), or what the sky leaves out, which always comes last.
 */
export type SkyNote =
  | { readonly kind: "streaming"; readonly edgeLy: number }
  | { readonly kind: "not-modelled"; readonly subjects: ReadonlyArray<string> };

/** A note's text: `BEYOND 2000 ly: STREAMING`, or `CLUSTERS AND WHITE DWARFS: NOT YET MODELLED`. */
function noteText(note: SkyNote): string {
  let text: string;
  switch (note.kind) {
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
 * 1d): the sky's annunciations, then what it leaves out, as many as its place holds.
 *
 * @param gaps - The response's `not_modelled`.
 */
export function skyLineNotes(
  gaps: ReadonlyArray<SkyGapDto>,
  standing: SkyLineStanding,
): ReadonlyArray<SkyNote> {
  const present = new Set(gaps.map((gap) => GAP_PHRASES[gap]));
  const subjects = SUBJECT_ORDER.filter((subject) => present.has(subject));
  const annunciations: ReadonlyArray<SkyNote> =
    standing.streamingEdgeLy === null
      ? []
      : [{ kind: "streaming", edgeLy: standing.streamingEdgeLy }];
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
 * @param limitV - The view's limit: a camera's, or the eye's deepest over the view.
 * @param gaps - The response's `not_modelled`.
 * @param standing - Where the line stands and the edge reached. The stale mark the edge takes while
 *   the link is down is the label block's, which sets the edge in its field.
 */
export function skyLabelValue(
  limitV: number,
  limitKind: SkyLimitKind,
  gaps: ReadonlyArray<SkyGapDto>,
  standing: SkyLineStanding,
): string {
  const limit = `V ${formatNumber(limitV, 1)} mag ${KIND_WORDS[limitKind]}`;
  return [limit, ...skyLineNotes(gaps, standing).map(noteText)].join(" · ");
}
