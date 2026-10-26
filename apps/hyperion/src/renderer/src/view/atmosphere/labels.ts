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
 * (`displays/view/viewRun.ts`) beside the lit bodies' labels (`litLabelsOf`). The provisional
 * temperature profile is recorded in the plan and the code, not on the view (Design note 12).
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
 * - `atmosphereApproximate`: a thick world drawn with the analytic term because its regime's gate
 *   has not passed (Design note 9), or a body whose medium holds an NH₄SH mode with less than
 *   `CLOUD_DECK_SPLIT_OPTICAL_DEPTH` above it at 550 nm, drawn with the stated stand-in
 *   (decision-r08-licences, row 5), which clears only when measured constants replace it. The same
 *   holds for any other material drawn with a stated stand-in, a named analogue or, for a key this
 *   client does not know, the generic stand-in, under the same optical-depth rule
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
 * The fault a body's atmosphere raises when the console fails to compute it
 * (`decision-r08-t10a-leans.md`, item 6; the guide's `ATMOSPHERE NOT AVAILABLE` row, ruled under
 * the owner's delegation as the terrain's was).
 *
 * @remarks
 * A fault, not a ninth {@link AtmosphereLabel}: the body draws no atmosphere and carries none of
 * the labels, and R08.T10.b sets this on the label block's fault line in `--status-caution`, after
 * any graphics fault and the terrain's (`view/terrain/annunciation.ts`'s `TERRAIN_NOT_AVAILABLE`),
 * never among the steady statements and never debounced. It is raised by a throw from the medium's
 * assembly or the tables' factory other than the engine's loss, which is the graphics fault's. It
 * clears when the body's atmosphere is computed anew: at the body's next trigger, or when `VIEW` is
 * shown again and the cache forgets its failures.
 */
export const ATMOSPHERE_NOT_AVAILABLE =
  "ATMOSPHERE NOT AVAILABLE: computation failed, reopen VIEW to retry";

/**
 * What the operator can do about a body's refused or unreadable record, which asks it afresh
 * (`decision-r08-t10a-leans.md`, follow-up to item 6).
 */
const ATMOSPHERE_RETRY_REMEDY = "reopen VIEW to retry";

/**
 * The check an answer of another kind than the request's fails (`protocol_violation`), in
 * {@link atmosphereDataInvalid}'s place for `toBodyDetail`'s fault (`decision-r08-t10a-leans.md`,
 * follow-up to item 6).
 */
export const ATMOSPHERE_ANSWER_UNREADABLE = "answer unreadable";

/**
 * The note a body carries when the server refuses its `body_detail`, in place of its atmosphere's
 * notes: `ATMOSPHERE REJECTED: <the server's reason>, reopen VIEW to retry`
 * (`decision-r08-t10a-leans.md`, follow-up to item 6; the guide's `ATMOSPHERE REJECTED` row).
 *
 * @remarks
 * Steady, on the label block's plate after `ATMOSPHERE: NOT RESOLVED` and before the
 * annunciations, in `--text` for a refusal of what was asked (`lib/refusal.ts`'s `isRefusal`) and
 * in `--status-caution` for the server's own failure; R08.T10.b places and colours it. A server
 * too busy to answer refuses nothing, and raises none. As the survey line's
 * `SURVEY COVERAGE REJECTED` does, a reason of white space alone leaves the remedy alone.
 *
 * @param reason - The server's own reason, as its error's message carries it.
 */
export function atmosphereRejected(reason: string): string {
  const said = reason.trim();
  return `ATMOSPHERE REJECTED: ${said.length > 0 ? `${said}, ` : ""}${ATMOSPHERE_RETRY_REMEDY}`;
}

/**
 * The fault a body raises when the console cannot take its `body_detail`'s answer:
 * `ATMOSPHERE DATA INVALID: <the check that failed>, reopen VIEW to retry`
 * (`decision-r08-t10a-leans.md`, follow-up to item 6; the guide's `DATA INVALID` row).
 *
 * @remarks
 * On the label block's fault line in `--status-caution`, after {@link ATMOSPHERE_NOT_AVAILABLE};
 * R08.T10.b places it. No atmosphere is drawn for the body, which is never read as airless.
 *
 * @param check - `toBodyDetail`'s fault string, or {@link ATMOSPHERE_ANSWER_UNREADABLE} for an
 *   answer of another kind.
 */
export function atmosphereDataInvalid(check: string): string {
  return `ATMOSPHERE DATA INVALID: ${check}, ${ATMOSPHERE_RETRY_REMEDY}`;
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
