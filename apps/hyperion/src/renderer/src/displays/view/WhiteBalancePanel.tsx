import { useId } from "react";

import { WhiteBalanceControl, type WhiteBalanceControlProps } from "./WhiteBalanceControl";

/** Props of {@link WhiteBalancePanel}. */
export interface WhiteBalancePanelProps {
  /** The panel's ID, by which its disclosure button controls it. */
  readonly id: string;
  /** Whether the panel is folded behind its disclosure button. */
  readonly hidden: boolean;
  /** The view whose image the balance adapts first, its system designator on the title row. */
  readonly designator: string;
  /** The balance, its reading and its choice. */
  readonly balance: WhiteBalanceControlProps;
}

/**
 * The compact layout's `White balance` panel (plan R11, R11.T10.b's follow-up; the UX ruling,
 * 2026-11-18): the `Exposure` panel's `WHITE BALANCE` as a part of its own in the one-open fold,
 * since at 1280 × 720 the exposure with it does not fit the column beside the list.
 *
 * @remarks
 * Folded, it states nothing under the row: it has no fault and is never stale, and the `PRIMARY`
 * view's label block states the balance. The full layout keeps it in the `Exposure` panel.
 */
export function WhiteBalancePanel({ id, hidden, designator, balance }: WhiteBalancePanelProps) {
  const titleId = useId();
  return (
    <section className="panel view-balance" aria-labelledby={titleId} id={id} hidden={hidden}>
      <h2 className="panel__title" id={titleId}>
        White balance <span className="panel__designator">{designator}</span>
      </h2>
      <WhiteBalanceControl {...balance} />
    </section>
  );
}
