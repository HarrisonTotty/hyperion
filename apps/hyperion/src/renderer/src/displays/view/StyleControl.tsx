import { useId } from "react";

import { StatusText } from "../../components/StatusText";
import { formatNumber } from "../../lib/format";
import type { RenderStyle } from "../../view/camera/state";
import { RENDER_STYLES, STYLE_TOGGLE_KEY, styleName } from "../../view/photoreal/style";
import { SLOPE_BASELINES_M } from "../../view/terrain/readouts";
import type { SlopeBaselineM } from "../../view/terrain/workers/messages";
import type { StyleRefusals } from "./styleRefusals";

/**
 * The `BASELINE` row's state and its callback (plan R10, R10.T12.a; decision-r10-t12a-scope §3.1):
 * the baseline the `CONTROLS` view's terrain readout reads `SLOPE` over.
 */
export interface StyleBaseline {
  /** The baseline chosen, m, whose button is pressed. */
  readonly chosen: SlopeBaselineM;
  /** Called with the baseline chosen, m; a display control, so it acts at once. */
  readonly onBaseline: (baselineM: SlopeBaselineM) => void;
}

/**
 * The `GRID` row's state and its callback (the owner's request, 2026-10-31): whether the view's
 * reference grid, each body's limb and graticule, is drawn over its photorealistic image.
 */
export interface StyleGrid {
  /** Whether the view draws its grid over the image, `ON`; `true` as a view starts. */
  readonly on: boolean;
  /** Called with the setting chosen; a display control, so it acts at once. */
  readonly onGrid: (on: boolean) => void;
}

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
  /**
   * The `BASELINE` row's state (R10.T12.a), given while the panel's view states a terrain readout,
   * the `PRIMARY` one; absent, the row is left out, as an instrument's slot states no readout.
   */
  readonly baseline?: StyleBaseline | undefined;
  /**
   * The `GRID` row's state (the owner's request, 2026-10-31): the panel's view's own, the row
   * standing while the view's style is `PHOTOREALISTIC`; absent, the row is left out.
   */
  readonly grid?: StyleGrid | undefined;
}

/** Props of {@link GridRow}. */
interface GridRowProps {
  readonly grid: StyleGrid;
}

/** The `GRID` row's settings, in order. */
const GRID_SETTINGS: ReadonlyArray<{ readonly on: boolean; readonly label: string }> = [
  { on: true, label: "ON" },
  { on: false, label: "OFF" },
];

/**
 * The `GRID` row (the owner's request, 2026-10-31): its legend on the left of `ON` and `OFF` in
 * the panel's row form, the chosen one pressed and underlined as a preset is; a display control,
 * acting at once, with no key.
 */
function GridRow({ grid }: GridRowProps) {
  return (
    <fieldset className="preset-buttons view-style__baseline view-style__grid">
      <legend className="field__label view-style__baseline-label">GRID</legend>
      {GRID_SETTINGS.map((setting) => (
        <button
          key={setting.label}
          type="button"
          className="control preset-buttons__button view-style__baseline-button"
          aria-pressed={setting.on === grid.on}
          onClick={() => {
            grid.onGrid(setting.on);
          }}
        >
          {setting.label}
        </button>
      ))}
    </fieldset>
  );
}

/** Props of {@link BaselineRow}. */
interface BaselineRowProps {
  readonly baseline: StyleBaseline;
}

/**
 * The `BASELINE` row (plan R10, R10.T12.a; decision-r10-t12a-scope §3.1): its legend on the left
 * of `2 m`, `5 m` and `100 m` in the `INSTRUMENTS` panel's row form, the chosen one pressed and
 * underlined as a preset is; a display control, acting at once, with no key.
 */
function BaselineRow({ baseline }: BaselineRowProps) {
  return (
    <fieldset className="preset-buttons view-style__baseline">
      <legend className="field__label view-style__baseline-label">BASELINE</legend>
      {SLOPE_BASELINES_M.map((baselineM) => (
        <button
          key={baselineM}
          type="button"
          className="control preset-buttons__button view-style__baseline-button"
          aria-pressed={baselineM === baseline.chosen}
          onClick={() => {
            baseline.onBaseline(baselineM);
          }}
        >
          {formatNumber(baselineM, 0)} m
        </button>
      ))}
    </fieldset>
  );
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
 *
 * Under them, while the view's style is `PHOTOREALISTIC` and offered and `grid` is given, stands
 * the `GRID` row, whose `OFF` leaves each body's limb and graticule out of the image's overlay (the
 * owner's request, 2026-10-31). A style asked for and held back draws the wireframe, which has no
 * image to clear, so the row never stands beside the reason, and the panel's tallest state, the
 * reason and `BASELINE`, keeps the full layout's least height (plan R10, R10.T12.d; lean,
 * 2026-10-31).
 *
 * After the style's buttons and their reason stands the `BASELINE` row while `baseline` is given
 * (R10.T12.a): the baseline `SLOPE` is read over on the view's terrain readout, which it sets
 * whatever the readout's state and whether or not a view is drawn, since it is the readout's
 * setting, not the drawing's.
 */
export function StyleControl({
  renderStyle,
  refusals: byStyle,
  faulted,
  onStyle,
  designator,
  id,
  hidden,
  baseline,
  grid,
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
          <StatusText text={reason} />
        </p>
      )}
      {grid === undefined ||
      renderStyle !== "photorealistic" ||
      byStyle.photorealistic !== null ? null : (
        <GridRow grid={grid} />
      )}
      {baseline === undefined ? null : <BaselineRow baseline={baseline} />}
    </section>
  );
}
