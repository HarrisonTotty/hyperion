/**
 * What a photorealistic view says about the atmospheres it draws (plan R08, Design note 12): eight
 * statements for its label block, rows of the guide's nomenclature list
 * (`docs/frontend/ux-guidelines.md`; drafted by R08.T2, signed off 2026-10-09; the exosphere's
 * signed off 2026-10-10, `signoff-2.md` item 6; the silicate vapour's from decision-backlog-2 §2.5,
 * signed off with P14.T43.e's console row, `signoff-12.md` item 27).
 *
 * @remarks
 * The five labels follow the guide's grammar for a withheld or unmodelled section, as
 * `BODY PHOTOMETRY: NOT YET MODELLED` does. The three annunciations concern the view's own drawing
 * (the rendering brainstorm's "What the guide must gain", item 7), as `TERRAIN: STREAMING` and
 * `LIGHTING: PENDING` do: steady, in `--text`, on the label block's plate, neither a data state nor
 * an alert. None takes a status colour or the word "degraded". Each note follows the section the
 * body's atmosphere comes from: its surface section, or a gas envelope's own, never a giant's
 * `not_applicable` surface section (decision-r08-giant-label). R08.T10.a's `assembleMedium` gives
 * each body its keys, and R08.T10.b joins {@link atmosphereStatements} to `photorealStatements`
 * (`displays/view/viewRun.ts`) beside the lit bodies' labels (`litLabelsOf`). The labels are
 * scene-level; since R08.T10.c `ATMOSPHERE: APPROXIMATE` is followed by each of its reasons on a
 * line of its own, naming the bodies it holds for ({@link atmosphereApproximateLines};
 * `decision-r08-approximate-plate.md`, item 1b), while the `NOT YET MODELLED` notes, the
 * generator's, stay scene-level. The provisional temperature profile is recorded in the plan and
 * the code, not on the view (Design note 12).
 */

/**
 * Why a body's atmosphere is not drawn, or not drawn as computed physics (Design note 12).
 *
 * - `atmosphereNotResolved`: the section the body's atmosphere comes from is withheld by the
 *   detail level granted: its surface section, a giant's included, which the server withholds below
 *   the `surface` level although it is `not_applicable`; or, once P14.T35.e puts it on the wire, a
 *   gas envelope's `envelope` section. No atmosphere is drawn, since any atmosphere drawn in its
 *   place would be invented.
 * - `atmosphereNotYetModelled`: the surface section is `not_modelled`, as an older server's is,
 *   or a stand-in spin's (P14.T35.e put P14.T24.a's figures on the wire); or the body's atmosphere
 *   is a gas envelope (a gas or ice giant, whose surface section is `not_applicable`, or a
 *   sub-Neptune) and its envelope (P14.T24.d) is absent from the wire, which reads `not_modelled`
 *   as every absent section does (`lib/system/bodiesWire.ts`), or is computed and not yet drawn by
 *   this console, until R08.T16.a draws an envelope (`decision-r08-t10a-leans.md`, item 5); or a
 *   kept scene leaves the body's atmosphere unset. No atmosphere is drawn
 *   (decision-r08-giant-label). It clears when the generator computes the body's atmosphere and
 *   the console draws it, or the scene sets one.
 * - `aerosolsNotYetModelled`: plan 14 publishes no aerosol or absorber inventory, so the air is
 *   drawn with its gases' scattering alone. Its statement names the absorbers too: ozone and
 *   methane, like the aerosols, are drawn only from the inventory.
 * - `exosphereNotYetModelled`: a generated body is airless, its atmosphere computed with no air in
 *   it, and the generator does not yet compute its exosphere, the unbound gas an airless body may
 *   still hold (P14.T55.c). No atmosphere is drawn: an exosphere's scattering of sunlight is far
 *   too faint to see, but its resonance light, a Mercury-class body's sodium above its limb, is
 *   not drawn either. The note is never read as "none", and a kept scene's body set airless
 *   carries none (`signoff-2.md` item 6).
 * - `silicateVapourNotYetModelled`: a generated body's surface holds melt (its state
 *   `magma_ocean`, or a crust whose `melt_area_fraction` is above 0) and its `silicate_vapour`
 *   member is absent (P14.T35.f; every server until P14.T55.b): the melt's own vapour, chiefly Na,
 *   O₂, O and SiO over a melt with no volatiles (Schaefer and Fegley 2009), is not drawn, nor its
 *   sodium's resonance light. Any air the surface section computes is drawn as computed beside it.
 *   It stands on its own line, beside `exosphereNotYetModelled` where an airless body holds a pool
 *   of melt (decision-backlog-2 §2.5; `signoff-12.md` item 27).
 * - `atmospherePending`: the body's surface or envelope section has been asked of the server
 *   (`body_detail`) and its reply is not yet drawn. No atmosphere is drawn meanwhile, and the note
 *   clears by itself (decision-r08-giant-label).
 * - `atmosphereComputing`: a gated thick bake is running, and the analytic term is drawn meanwhile
 *   (Design note 9).
 * - `atmosphereApproximate`: a body routed past `thin`, drawn with the analytic term because its
 *   regime's gate has not passed (Design note 9; R08.T13.d), or a body whose medium holds an NH₄SH
 *   mode with less than `CLOUD_DECK_SPLIT_OPTICAL_DEPTH` above it at 550 nm, drawn with the stated
 *   stand-in (decision-r08-licences, row 5), which clears only when measured constants replace it.
 *   The same holds for any other material drawn with a stated stand-in, a named analogue or, for a
 *   key this client does not know, the generic stand-in, under the same optical-depth rule
 *   (decision-composition §1.9; `materials/materials.ts`'s `materialLabels`), and for an aerosol
 *   mode drawn with a named analogue's phase function or the Henyey–Greenstein fallback (R08.T5.c;
 *   `aerosol.ts`'s `phaseLabels`), which clears when a published model for it replaces it. It
 *   also stands in `atmosphereComputing`'s place if a bake fails, until the body's atmosphere is
 *   next computed. A strongly oblate thin body no longer takes it: since R08.T6.f its tables are
 *   built in its κ slices and latitude bands (Design note 17), which every body's kernels read, and
 *   it took it only while one slice stood for them.
 *
 * @remarks
 * Per body, as R08.T10.a's `assembleMedium` and R08.T14.c's bake give them: at most one of
 * `atmosphereNotResolved` and `atmosphereNotYetModelled`, the withheld section first, since the
 * client cannot see past it; no `aerosolsNotYetModelled` under either, since none of the body's air
 * is drawn; at most one of `atmosphereComputing` and `atmosphereApproximate`, the first while a
 * gated bake runs (decision-r08-giant-label; the guide's rows); and for a generated airless body
 * `exosphereNotYetModelled` alone (a kept scene's, none), never beside the other two
 * `NOT YET MODELLED` notes for the same body (`signoff-2.md` item 6), but for
 * `silicateVapourNotYetModelled` where it holds a pool of melt.
 */
export type AtmosphereLabel =
  | "atmosphereNotResolved"
  | "atmosphereNotYetModelled"
  | "aerosolsNotYetModelled"
  | "exosphereNotYetModelled"
  | "silicateVapourNotYetModelled"
  | "atmospherePending"
  | "atmosphereComputing"
  | "atmosphereApproximate";

/** Each label's statement on the label block, as the guide's rows spell them. */
export const ATMOSPHERE_STATEMENTS: Readonly<Record<AtmosphereLabel, string>> = {
  atmosphereNotResolved: "ATMOSPHERE: NOT RESOLVED",
  atmosphereNotYetModelled: "ATMOSPHERE: NOT YET MODELLED",
  aerosolsNotYetModelled: "AEROSOLS AND ABSORBERS: NOT YET MODELLED",
  exosphereNotYetModelled: "EXOSPHERE: NOT YET MODELLED",
  silicateVapourNotYetModelled: "SILICATE VAPOUR: NOT YET MODELLED",
  atmospherePending: "ATMOSPHERE: PENDING",
  atmosphereComputing: "ATMOSPHERE: COMPUTING",
  atmosphereApproximate: "ATMOSPHERE: APPROXIMATE",
};

/**
 * Every label, in the order the eight stand among themselves on the label block: the four
 * `NOT YET MODELLED` notes, then `ATMOSPHERE: NOT RESOLVED` after them as the guide's "Data states"
 * places a `NOT RESOLVED` note, then the three annunciations (decision-r08-giant-label;
 * `signoff-2.md`, "Client strings").
 *
 * @remarks
 * Where they stand among the block's other notes is R08.T10.b's: after `photorealStatements`'
 * others, `litLabelsOf`'s among them, so that `ATMOSPHERE: NOT RESOLVED` stands after every
 * `NOT YET MODELLED` note on the block, `ROTATION: NOT YET MODELLED` included.
 */
export const ATMOSPHERE_LABELS: ReadonlyArray<AtmosphereLabel> = [
  "atmosphereNotYetModelled",
  "aerosolsNotYetModelled",
  "exosphereNotYetModelled",
  "silicateVapourNotYetModelled",
  "atmosphereNotResolved",
  "atmospherePending",
  "atmosphereComputing",
  "atmosphereApproximate",
];

/**
 * Why a body has no atmosphere drawn and none of its {@link AtmosphereLabel}s, though the view
 * draws it larger than a point (`decision-r08-t10a-leans.md`, item 6 and its follow-up): its
 * medium or its tables failed to build (`failed`), a kept scene's media, loaded once a page,
 * failed to load (`loadFailed`, R08.T10.b; `decision-r08-t10b-ux.md`, item 4), the server refused
 * its `body_detail`
 * (`rejected`, with the server's code and reason, and whether it refused what was asked,
 * `lib/refusal.ts`'s `isRefusal`), or the console could not take the answer (`invalid`, with the
 * check that failed). {@link atmosphereProblemLines} words them.
 */
export type AtmosphereProblem =
  | { readonly kind: "failed" }
  | { readonly kind: "loadFailed" }
  | {
      readonly kind: "rejected";
      readonly code: string;
      readonly message: string;
      readonly refusal: boolean;
    }
  | { readonly kind: "invalid"; readonly check: string };

/**
 * What the operator can do about a body's failed, refused or unreadable atmosphere, which asks for
 * it afresh (`decision-r08-t10a-leans.md`, item 6 and its follow-up).
 */
const ATMOSPHERE_RETRY_REMEDY = "reopen VIEW to retry";

/**
 * What the operator can do about a kept scene's atmospheres whose load failed, which a page loads
 * once and cannot retry, as the descent spike's terrain is made once a run (`decision-r08-t10b-ux.md`,
 * item 4).
 */
const ATMOSPHERE_RELAUNCH_REMEDY = "relaunch to retry";

/**
 * Designations as one line names several things: `A`, `A and B`, `A, B and C`, in the order given
 * (the guide's `NOT AVAILABLE: no room for SURVEY COVERAGE, DATUM or GROUND AT LINE OF SIGHT …`;
 * `decision-r08-t10a-leans.md`, UX review, item 2).
 *
 * @throws RangeError for an empty list: a line names at least one body.
 */
function namesOf(designations: ReadonlyArray<string>): string {
  const last = designations.at(-1);
  if (last === undefined) {
    throw new RangeError("an atmosphere's fault, refusal or reason names at least one body");
  }
  return designations.length === 1 ? last : `${designations.slice(0, -1).join(", ")} and ${last}`;
}

/**
 * The fault bodies' atmospheres raise when the console fails to compute them:
 * `ATMOSPHERE NOT AVAILABLE: computation failed for <designations>, reopen VIEW to retry`
 * (`decision-r08-t10a-leans.md`, item 6 and its UX review; the guide's `ATMOSPHERE NOT AVAILABLE`
 * row, ruled under the owner's delegation as the terrain's was).
 *
 * @remarks
 * A fault, not a ninth {@link AtmosphereLabel}: each body draws no atmosphere and carries none of
 * the labels, and R08.T10.b sets this on the label block's fault line in `--status-caution`, after
 * any graphics fault and the terrain's (`view/terrain/annunciation.ts`'s `TERRAIN_NOT_AVAILABLE`),
 * never among the steady statements and never debounced. It is raised by a throw from the medium's
 * assembly or the tables' factory other than the engine's loss, which is the graphics fault's;
 * never by a computation that `ATMOSPHERE: COMPUTING` announces, whose failure is
 * `ATMOSPHERE: APPROXIMATE`'s (R08.T14.c). It clears when the body's atmosphere is computed anew:
 * at the body's next trigger, or when `VIEW` is shown again and the cache forgets its failures.
 *
 * On `ATMOSPHERE TEST`, whose atmospheres load once a run, a failed load is this fault for every
 * body with air, `relaunch to retry` in place of `reopen VIEW to retry` (`decision-r08-t10b-ux.md`,
 * item 4), and it does not clear when `VIEW` is shown again.
 *
 * @param designations - Every body it holds for, in designation order; at least one.
 * @param remedy - `reopen` for a computation the console tries anew, `relaunch` for a kept
 *   scene's load, which a page cannot retry.
 */
export function atmosphereNotAvailable(
  designations: ReadonlyArray<string>,
  remedy: "reopen" | "relaunch" = "reopen",
): string {
  const words = remedy === "reopen" ? ATMOSPHERE_RETRY_REMEDY : ATMOSPHERE_RELAUNCH_REMEDY;
  return `ATMOSPHERE NOT AVAILABLE: computation failed for ${namesOf(designations)}, ${words}`;
}

/**
 * The check an answer of another kind than the request's fails (`protocol_violation`), in
 * {@link atmosphereDataInvalid}'s place for `toBodyDetail`'s fault (`decision-r08-t10a-leans.md`,
 * follow-up to item 6).
 */
export const ATMOSPHERE_ANSWER_UNREADABLE = "answer unreadable";

/**
 * The note bodies carry when the server refuses their `body_detail`, in place of their
 * atmospheres' notes: `ATMOSPHERE REJECTED: for <designations>, <the server's reason>, reopen VIEW
 * to retry` (`decision-r08-t10a-leans.md`, follow-up to item 6 and its UX review; the guide's
 * `ATMOSPHERE REJECTED` row).
 *
 * @remarks
 * Steady, on the label block's plate after `ATMOSPHERE: NOT RESOLVED` and before the
 * annunciations, in `--text` for a refusal of what was asked (`lib/refusal.ts`'s `isRefusal`) and
 * in `--status-caution` for the server's own failure; R08.T10.b places and colours it. A server
 * too busy to answer refuses nothing, and raises none. The designations lead the server's reason,
 * free text the console does not extend. As the survey line's `SURVEY COVERAGE REJECTED` does, a
 * reason of white space alone leaves the remedy alone.
 *
 * @param designations - Every body the reason holds for, in designation order; at least one.
 * @param reason - The server's own reason, as its error's message carries it.
 */
export function atmosphereRejected(designations: ReadonlyArray<string>, reason: string): string {
  const said = reason.trim();
  const words = said.length > 0 ? `${said}, ${ATMOSPHERE_RETRY_REMEDY}` : ATMOSPHERE_RETRY_REMEDY;
  return `ATMOSPHERE REJECTED: for ${namesOf(designations)}, ${words}`;
}

/**
 * The fault bodies raise when the console cannot take their `body_detail`'s answers:
 * `ATMOSPHERE DATA INVALID: <the check that failed> for <designations>, reopen VIEW to retry`
 * (`decision-r08-t10a-leans.md`, follow-up to item 6 and its UX review; the guide's `DATA INVALID`
 * row).
 *
 * @remarks
 * On the label block's fault line in `--status-caution`, after {@link atmosphereNotAvailable}'s
 * lines; R08.T10.b places it. No atmosphere is drawn for the body, which is never read as airless.
 *
 * @param designations - Every body the check failed for, in designation order; at least one.
 * @param check - `toBodyDetail`'s fault string, or {@link ATMOSPHERE_ANSWER_UNREADABLE} for an
 *   answer of another kind.
 */
export function atmosphereDataInvalid(designations: ReadonlyArray<string>, check: string): string {
  return `ATMOSPHERE DATA INVALID: ${check} for ${namesOf(designations)}, ${ATMOSPHERE_RETRY_REMEDY}`;
}

/** An `ATMOSPHERE REJECTED` line, and whether the server refused what was asked (`--text`). */
export interface AtmosphereRejectedLine {
  readonly statement: string;
  /** `--text` where true, `--status-caution` where the server failed on its side. */
  readonly refusal: boolean;
}

/** The view's atmosphere problems, worded: the plate's lines and the fault line's. */
export interface AtmosphereProblemLines {
  /** `ATMOSPHERE REJECTED` lines, one for each reason. */
  readonly plate: ReadonlyArray<AtmosphereRejectedLine>;
  /** `ATMOSPHERE NOT AVAILABLE`'s line, then `ATMOSPHERE DATA INVALID`'s, one for each check. */
  readonly fault: ReadonlyArray<string>;
}

/** A body drawn larger than a point that has an {@link AtmosphereProblem}, by its designation. */
export interface BodyAtmosphereProblem {
  readonly designation: string;
  readonly problem: AtmosphereProblem;
}

/**
 * Designation order: a stable order that never moves as the camera does, numbers by their value
 * (`… /2` before `… /10`), fixed to one locale so that it never varies between machines.
 */
const DESIGNATION_ORDER = new Intl.Collator("en", { numeric: true });

/**
 * A view's atmosphere problems as its label block states them (`decision-r08-t10a-leans.md`, UX
 * review, item 2): one line for each distinct phrase or reason, naming every body it holds for in
 * designation order, the lines of each kind in the order of their first bodies.
 *
 * @remarks
 * A body listed twice is named once. `ATMOSPHERE REJECTED`'s lines are told apart by the server's
 * reason, trimmed, and by whether it refused what was asked, which sets the line's colour.
 */
export function atmosphereProblemLines(
  bodies: ReadonlyArray<BodyAtmosphereProblem>,
): AtmosphereProblemLines {
  const failed = new Set<string>();
  const loadFailed = new Set<string>();
  const rejected = new Map<string, { reason: string; refusal: boolean; names: Set<string> }>();
  const invalid = new Map<string, Set<string>>();
  for (const { designation, problem } of bodies) {
    switch (problem.kind) {
      case "failed":
        failed.add(designation);
        break;
      case "loadFailed":
        loadFailed.add(designation);
        break;
      case "rejected": {
        const reason = problem.message.trim();
        const key = JSON.stringify([problem.refusal, reason]);
        const held = rejected.get(key) ?? { reason, refusal: problem.refusal, names: new Set() };
        held.names.add(designation);
        rejected.set(key, held);
        break;
      }
      case "invalid": {
        const names = invalid.get(problem.check) ?? new Set();
        names.add(designation);
        invalid.set(problem.check, names);
        break;
      }
    }
  }
  const ordered = (names: Iterable<string>): ReadonlyArray<string> =>
    [...names].toSorted((a, b) => DESIGNATION_ORDER.compare(a, b));
  const byFirst = <T extends { readonly names: ReadonlyArray<string> }>(
    lines: ReadonlyArray<T>,
  ): ReadonlyArray<T> =>
    lines.toSorted((a, b) => DESIGNATION_ORDER.compare(a.names[0] ?? "", b.names[0] ?? ""));
  const plate = byFirst(
    [...rejected.values()].map((held) => ({
      reason: held.reason,
      refusal: held.refusal,
      names: ordered(held.names),
    })),
  ).map((held) => ({
    statement: atmosphereRejected(held.names, held.reason),
    refusal: held.refusal,
  }));
  const checks = byFirst(
    [...invalid.entries()].map(([check, names]) => ({ check, names: ordered(names) })),
  ).map((held) => atmosphereDataInvalid(held.names, held.check));
  const notAvailable = [
    ...(failed.size > 0 ? [atmosphereNotAvailable(ordered(failed))] : []),
    ...(loadFailed.size > 0 ? [atmosphereNotAvailable(ordered(loadFailed), "relaunch")] : []),
  ];
  return { plate, fault: [...notAvailable, ...checks] };
}

/** A body drawn with `atmosphereApproximate`, by its designation, with its reasons in its order. */
export interface BodyApproximateReasons {
  readonly designation: string;
  /** Why it carries the label, as player text (`assemble.ts`'s reasons, `thick/regime.ts`'s). */
  readonly reasons: ReadonlyArray<string>;
}

/**
 * The first token of a reason, up to a space, colon or comma: the word or the formula it begins
 * with.
 */
const FIRST_TOKEN = /^[^ :,]+/u;

/**
 * A token of lower-case letters alone, or of such words joined by hyphens (`ice-giant`,
 * `ultraviolet-absorbing`, of `lib/substances.ts`'s names): a word, never a formula (`H2O`) or a
 * symbol (`e−`, whose sign is U+2212, not a hyphen).
 */
const LOWER_CASE_WORD = /^[a-z]+(?:-[a-z]+)*$/u;

/**
 * A reason as a line begins it: a capital first letter where its first token is a word in lower
 * case (`multiple scattering: …` → `Multiple scattering: …`, `liquid H2SO4 particles: …` →
 * `Liquid H2SO4 particles: …`, `ice-giant haze: …` → `Ice-giant haze: …`), its case as given where it begins with a formula or a
 * symbol, whose case is its meaning (`H2O: …`, `H2O–air pairs: …`, `e− scattering: …`).
 */
function capitalised(reason: string): string {
  const token = FIRST_TOKEN.exec(reason)?.[0] ?? "";
  return LOWER_CASE_WORD.test(token)
    ? `${token.charAt(0).toUpperCase()}${reason.slice(1)}`
    : reason;
}

/**
 * The lines that follow `ATMOSPHERE: APPROXIMATE` on a view's plate (R08.T10.c;
 * `decision-r08-approximate-plate.md`, item 1b): one for each distinct reason, compared after its
 * capital, `<reason>, for <designations>`, naming each body it holds for once, in designation order,
 * joined as the fault lines join them (`A and B`, `A, B and C`).
 *
 * @remarks
 * The lines stand in the order of their first bodies, and lines with the same first body in that
 * body's own order of its reasons, as {@link atmosphereProblemLines} orders its own. A body with no
 * reasons gives no line.
 */
export function atmosphereApproximateLines(
  bodies: ReadonlyArray<BodyApproximateReasons>,
): ReadonlyArray<string> {
  // Taken in designation order, each line is first met at its first body, in that body's order.
  const byReason = new Map<string, Set<string>>();
  for (const { designation, reasons } of bodies.toSorted((a, b) =>
    DESIGNATION_ORDER.compare(a.designation, b.designation),
  )) {
    for (const reason of reasons) {
      const line = capitalised(reason);
      const names = byReason.get(line) ?? new Set<string>();
      names.add(designation);
      byReason.set(line, names);
    }
  }
  return [...byReason].map(([reason, names]) => `${reason}, for ${namesOf([...names])}`);
}

/**
 * One of a view's atmosphere notes on its label block's plate: a steady `note`; one of
 * `ATMOSPHERE: APPROXIMATE`'s reasons, a steady `reason`, a description set as a sentence, never
 * tracked (R08.T10.d); or an `ATMOSPHERE REJECTED` line with its standing as every request's
 * `REJECTED` takes one, `refused` (in `--text`) where the server refused what was asked and `fault`
 * (in `--status-caution`) for its own failure.
 */
export interface AtmosphereNote {
  readonly statement: string;
  readonly standing: "note" | "reason" | "refused" | "fault";
}

/**
 * A view's atmosphere notes in their place on the plate (plan R08, R08.T10.b): the statements of
 * its bodies' labels in {@link ATMOSPHERE_LABELS}' order, each a `note`, with its `ATMOSPHERE
 * REJECTED` lines after `ATMOSPHERE: NOT RESOLVED` and before the annunciations
 * (`decision-r08-t10a-leans.md`, follow-up to item 6), each standing `refused` or `fault` as the
 * server refused what was asked or failed, and `ATMOSPHERE: APPROXIMATE`'s reasons straight after
 * it (R08.T10.c; `decision-r08-approximate-plate.md`, item 1b), each a `reason` (R08.T10.d; the
 * same decision's §6, 2026-10-27).
 *
 * @param rejected - {@link atmosphereProblemLines}' `plate`.
 * @param approximate - {@link atmosphereApproximateLines}' lines; none stand without
 *   `atmosphereApproximate` among the labels.
 */
export function atmosphereNotes(
  labels: Iterable<AtmosphereLabel>,
  rejected: ReadonlyArray<AtmosphereRejectedLine>,
  approximate: ReadonlyArray<string>,
): ReadonlyArray<AtmosphereNote> {
  const held = new Set(labels);
  const notes: AtmosphereNote[] = [];
  const refusals = rejected.map((line): AtmosphereNote => ({
    statement: line.statement,
    standing: line.refusal ? "refused" : "fault",
  }));
  for (const label of ATMOSPHERE_LABELS) {
    if (label === "atmospherePending") {
      notes.push(...refusals);
    }
    if (held.has(label)) {
      notes.push({ statement: ATMOSPHERE_STATEMENTS[label], standing: "note" });
      if (label === "atmosphereApproximate") {
        notes.push(
          ...approximate.map((statement): AtmosphereNote => ({ statement, standing: "reason" })),
        );
      }
    }
  }
  return notes;
}

/**
 * The statements of the labels the view's bodies carry, each once, in {@link ATMOSPHERE_LABELS}'
 * order.
 *
 * @remarks
 * Scene-level, as the lit bodies' labels are: two bodies with the same label give one statement,
 * and no statement names its body.
 */
export function atmosphereStatements(labels: Iterable<AtmosphereLabel>): ReadonlyArray<string> {
  const held = new Set(labels);
  return ATMOSPHERE_LABELS.filter((label) => held.has(label)).map(
    (label) => ATMOSPHERE_STATEMENTS[label],
  );
}
