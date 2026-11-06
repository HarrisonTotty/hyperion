import { useId } from "react";

import { WHITE_BALANCES, type WhiteBalance, whiteBalanceName } from "../../view/post/whiteBalance";
import { readingParts } from "./ViewLabelBlock";
import { WHITE_BALANCE_LABEL, whiteBalanceReading } from "./viewRun";

/** Props of {@link WhiteBalanceControl}. */
export interface WhiteBalanceControlProps {
  /** The view's white balance as the operator chose it. */
  readonly balance: WhiteBalance;
  /**
   * The effective temperature of the star its last photorealistic frame adapted to, K
   * (`ViewFrameDrawn.balancedTeffK`), or `null` where it adapted nothing.
   */
  readonly balancedTeffK: number | null;
  /** Called with the operator's choice; a display control, so it acts at once. */
  readonly onBalance: (balance: WhiteBalance) => void;
}

/**
 * The photorealistic style's `WHITE BALANCE` in the `Exposure` panel (the owner's report,
 * 2026-11-05; the UX ruling, 2026-11-06): its reading, `D65` or `HOST · T EFF 3444 K`
 * (`whiteBalanceReading`), and under `SELECT` the choice of `D65` or `HOST`, each a
 * keyboard-operable button, the chosen one underlined, in the meter's grammar.
 *
 * @remarks
 * `D65` draws the light as it is, the display's white; `HOST` adapts it to the white of the host
 * star brightest at the `PRIMARY` view's camera, as a camera balanced to that light draws it
 * (`whiteBalance.ts`), every view of the display at once. The balance is the display's own, not the
 * ship's: a display control that acts at once, never held back. It stands while any view of the
 * display draws a photorealistic image.
 */
export function WhiteBalanceControl({
  balance,
  balancedTeffK,
  onBalance,
}: WhiteBalanceControlProps) {
  const readingId = useId();
  return (
    <div className="view-exposure__balance">
      <p className="field">
        <span className="field__label" id={readingId}>
          {WHITE_BALANCE_LABEL}
        </span>{" "}
        <output
          className="view-meter__value view-exposure__balance-reading"
          aria-labelledby={readingId}
        >
          {readingParts(whiteBalanceReading(balance, balancedTeffK))}
        </output>
      </p>
      <fieldset className="view-meter__modes">
        <legend className="field__label">SELECT</legend>
        {WHITE_BALANCES.map((each) => (
          <button
            key={each}
            type="button"
            className="control"
            aria-pressed={balance === each}
            onClick={() => {
              onBalance(each);
            }}
          >
            {whiteBalanceName(each)}
          </button>
        ))}
      </fieldset>
    </div>
  );
}
