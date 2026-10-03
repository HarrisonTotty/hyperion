import { useId } from "react";

import type { RenderStyle } from "../../view/camera/state";
import type { StyleAvailability } from "../../view/engine/platform";
import {
  RENDER_STYLES,
  STYLE_TOGGLE_KEY,
  styleName,
  styleRefusal,
} from "../../view/photoreal/style";

/** Props of {@link StyleControl}. */
export interface StyleControlProps {
  /** The view's style. */
  readonly renderStyle: RenderStyle;
  /** The styles the adapter offers (R01's `styleAvailability`). */
  readonly availability: StyleAvailability;
  /** Called with the style chosen; a display control, so it acts at once. */
  readonly onStyle: (style: RenderStyle) => void;
}

/**
 * A view's style switch (plan R07, T7; Design note 8): `WIREFRAME` or `PHOTOREALISTIC`, each a
 * keyboard-operable button, the chosen one underlined, in a group whose legend shows the view's
 * single key that toggles them (`4 STYLE`, as GALAXY's `K STARS` shows its cycling key).
 *
 * @remarks
 * A display control: it changes what the view draws, never the ship. A style the adapter refuses
 * (the photorealistic style on a software adapter) is held back and says why, in the guide's
 * words. Unmounted until R07.T8.a draws lit bodies, so that no view switches to an empty image
 * (the orchestrator's ruling, 2026-10-03).
 */
export function StyleControl({ renderStyle, availability, onStyle }: StyleControlProps) {
  const titleId = useId();
  const reasonId = useId();
  const refusals = RENDER_STYLES.map((each) => styleRefusal(each, availability));
  const reason = refusals.find((refusal) => refusal !== null) ?? null;
  return (
    <section className="panel view-style" aria-labelledby={titleId}>
      <h2 className="panel__title" id={titleId}>
        Style
      </h2>
      <fieldset className="preset-buttons" aria-keyshortcuts={STYLE_TOGGLE_KEY}>
        <legend className="field__label">
          <span className="control__key">{STYLE_TOGGLE_KEY}</span> STYLE
        </legend>
        {RENDER_STYLES.map((each, index) => {
          const heldBack = refusals[index] !== null;
          return (
            <button
              key={each}
              type="button"
              className="control preset-buttons__button"
              aria-pressed={each === renderStyle}
              // Held back rather than disabled, so that it keeps its focus and can say why.
              aria-disabled={heldBack ? "true" : undefined}
              aria-describedby={heldBack ? reasonId : undefined}
              onClick={() => {
                if (!heldBack) {
                  onStyle(each);
                }
              }}
            >
              {styleName(each)}
            </button>
          );
        })}
      </fieldset>
      {reason === null ? null : (
        <p className="view-style__reason" id={reasonId}>
          {reason}
        </p>
      )}
    </section>
  );
}
