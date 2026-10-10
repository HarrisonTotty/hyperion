/**
 * The thick regime's thresholds (plan R08, Design note 9; Provides, "Thick atmospheres").
 *
 * @remarks
 * R08.T5.b created this file for the deck split's optical depth, which its stand-in label reads
 * (Design note 12). R08.T13 adds `classifyRegime` and `THICK_MS_BOUNDARY` beside it.
 */

/**
 * The vertical optical depth at 550 nm above which a body-wide cloud deck splits the atmosphere in
 * two, Hillaire's tables above it and the deck's bake below (Design note 9).
 *
 * @remarks
 * The rendering brainstorm's figure ("a cloud deck of optical depth above about 10 splits the
 * atmosphere in two", its "Atmosphere" section and open question 3). The stand-in label takes the
 * same threshold: a stand-in material lying deeper than it carries no label
 * (`decision-r08-licences.md` row 5; `materials/materials.ts`). Light from beneath is not nil
 * there: a conservative deck of τ 10 transmits 31–47% diffusely for g 0.7–0.85, by the two-stream
 * T = 1 ÷ (1 + ¾(1 − g)τ). The deck's bake (R08.T15) carries that light.
 */
export const CLOUD_DECK_SPLIT_OPTICAL_DEPTH = 10;
