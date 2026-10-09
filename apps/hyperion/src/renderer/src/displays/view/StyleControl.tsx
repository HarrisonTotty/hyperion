import { useId } from "react";

import type { RenderStyle } from "../../view/camera/state";
import { RENDER_STYLES, STYLE_TOGGLE_KEY, styleName } from "../../view/photoreal/style";
import type { StyleRefusals } from "./styleRefusals";

/** Props of {@link StyleControl}. */
export interface StyleControlProps {
  /** The panel's ID, by which a disclosure button controls it (R07.T19.b), or none. */
  readonly id?: string | undefined;
  /** Whether the panel is folded behind its disclosure button in the compact layout (R07.T19.b). */
  readonly hidden?: boolean | undefined;
  /**
   * The view the panel acts on, its system designator on the title row (`PRIMARY`,
   * `INSTRUMENT 1`; R07.T19), or none.
   */
  readonly designator?: string | undefined;
  /** The view's style. */
  readonly renderStyle: RenderStyle;
  /** Why each style is held back, `null` where it is offered (`styleRefusals`). */
  readonly refusals: StyleRefusals;
  /** Whether the reason is a fault of the graphics, shown in `--status-caution` while it lasts. */
  readonly faulted: boolean;
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
 * words: the graphics' condition, or the photorealistic view's failure to make its pipelines
 * (`styleRefusals`). Mounted by R07.T8.a beside the camera controls.
 */
export function StyleControl({
  renderStyle,
  refusals: byStyle,
  faulted,
  onStyle,
  designator,
  id,
  hidden,
}: StyleControlProps) {
  const titleId = useId();
  const reasonId = useId();
  const refusals = RENDER_STYLES.map((each) => byStyle[each]);
  const reason = refusals.find((refusal) => refusal !== null) ?? null;
  return (
    <section className="panel view-style" aria-labelledby={titleId} id={id} hidden={hidden}>
      <h2 className="panel__title" id={titleId}>
        Style
        {designator === undefined ? null : (
          <>
            {" "}
            <span className="panel__designator">{designator}</span>
          </>
        )}
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
        <p
          className={
            faulted ? "view-style__reason view-style__reason--fault" : "view-style__reason"
          }
          id={reasonId}
        >
          {reason}
        </p>
      )}
    </section>
  );
}
