import { StaleMark } from "./StaleMark";

/** Props of {@link StaleSlot}. */
export interface StaleSlotProps {
  /** Whether the value the slot follows is stale: the slot then holds the guide's `S`. */
  readonly stale: boolean;
}

/**
 * Room after a value for the guide's trailing stale `S` ("Data states"), kept while the value is
 * live: {@link StaleMark} while `stale` holds, and nothing otherwise.
 *
 * @remarks
 * The slot is 2 ch wide whether it holds the mark or not (`.stale-slot`), so that the mark's coming
 * moves nothing after it: the guide's "Numbers" sizes a field for its longest value and its status
 * marks. It is the HR page's counts', the system list's rows' and the system readout's (plan 05,
 * P05.T14), after P12.T11.d's form for the spatial furniture, whose `Reading` keeps a slot of its
 * own (`.spatial-reading__stale-slot`). The mark keeps its word, `stale`, for assistive technology.
 */
export function StaleSlot({ stale }: StaleSlotProps) {
  return <span className="stale-slot">{stale ? <StaleMark /> : null}</span>;
}
