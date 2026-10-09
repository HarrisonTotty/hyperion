/**
 * What a photorealistic view says about the atmospheres it draws (plan R08, Design note 12): six
 * statements for its label block, drafted for the owner (R08.T2) as rows of the guide's
 * nomenclature list (`docs/frontend/ux-guidelines.md`, each marked
 * `_Draft (plan R08, R08.T2): the owner signs off._`).
 *
 * @remarks
 * The three labels follow the guide's grammar for a withheld or unmodelled section, as
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
 *   gas envelope's `envelope` section. No atmosphere is drawn, since drawing another world's would
 *   be invention.
 * - `atmosphereNotYetModelled`: the surface section is `not_modelled`, every generated body's until
 *   P14.T24.a's figures are on the wire (R08.T1's P14.T35.e); or the body's atmosphere is a gas
 *   envelope (a gas or ice giant, whose surface section is `not_applicable`, or a sub-Neptune) and
 *   its envelope (P14.T24.d) is absent from the wire, which reads `not_modelled` as every absent
 *   section does (`lib/system/bodiesWire.ts`); or a kept scene sets the body none. No atmosphere is
 *   drawn (decision-r08-giant-label).
 * - `aerosolsNotYetModelled`: plan 14 publishes no aerosol or absorber inventory, so the air is
 *   drawn with its gases' scattering alone. It covers the absorbers too: ozone and methane are
 *   drawn only from the inventory.
 * - `atmospherePending`: the body's surface or envelope section has been asked of the server
 *   (`body_detail`) and its reply is not yet drawn. No atmosphere is drawn meanwhile, and the note
 *   clears by itself (decision-r08-giant-label).
 * - `atmosphereComputing`: a gated thick bake is running, and the analytic term is drawn meanwhile
 *   (Design note 9).
 * - `atmosphereApproximate`: a thick world drawn with the analytic term because its regime's gate
 *   has not passed (Design note 9), or a strongly oblate body drawn with one slice before R08.T6.f
 *   (Design note 17). It also stands in `atmosphereComputing`'s place if a bake fails, until the
 *   body's atmosphere is next computed.
 *
 * @remarks
 * Per body, as R08.T10.a's `assembleMedium` and R08.T14.c's bake give them: at most one of
 * `atmosphereNotResolved` and `atmosphereNotYetModelled`, the withheld section first, since the
 * client cannot see past it; no `aerosolsNotYetModelled` under either, since none of the body's air
 * is drawn; and at most one of `atmosphereComputing` and `atmosphereApproximate`, the first while a
 * gated bake runs (decision-r08-giant-label; the guide's draft rows).
 */
export type AtmosphereLabel =
  | "atmosphereNotResolved"
  | "atmosphereNotYetModelled"
  | "aerosolsNotYetModelled"
  | "atmospherePending"
  | "atmosphereComputing"
  | "atmosphereApproximate";

/** Each label's statement on the label block, as the guide's draft rows spell them. */
export const ATMOSPHERE_STATEMENTS: Readonly<Record<AtmosphereLabel, string>> = {
  atmosphereNotResolved: "ATMOSPHERE: NOT RESOLVED",
  atmosphereNotYetModelled: "ATMOSPHERE: NOT YET MODELLED",
  aerosolsNotYetModelled: "AEROSOLS: NOT YET MODELLED",
  atmospherePending: "ATMOSPHERE: PENDING",
  atmosphereComputing: "ATMOSPHERE: COMPUTING",
  atmosphereApproximate: "ATMOSPHERE: APPROXIMATE",
};

/**
 * Every label, in the order the six stand among themselves on the label block: the two
 * `NOT YET MODELLED` notes, then `ATMOSPHERE: NOT RESOLVED` after them as the guide's "Data states"
 * places a `NOT RESOLVED` note, then the three annunciations (decision-r08-giant-label).
 *
 * @remarks
 * Where they stand among the block's other notes is R08.T10.b's: after `photorealStatements`'
 * others, `litLabelsOf`'s among them, so that `ATMOSPHERE: NOT RESOLVED` stands after every
 * `NOT YET MODELLED` note on the block, `ROTATION: NOT YET MODELLED` included.
 */
export const ATMOSPHERE_LABELS: ReadonlyArray<AtmosphereLabel> = [
  "atmosphereNotYetModelled",
  "aerosolsNotYetModelled",
  "atmosphereNotResolved",
  "atmospherePending",
  "atmosphereComputing",
  "atmosphereApproximate",
];

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
