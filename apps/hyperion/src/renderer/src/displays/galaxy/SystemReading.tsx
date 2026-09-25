import type { ReactNode } from "react";

import { StaleMark } from "../../components/StaleMark";

/** Props of {@link SystemReading}. */
export interface SystemReadingProps {
  readonly label: ReactNode;
  /** The value's digits, or `null` for one that does not exist, shown as an em dash. */
  readonly value: string | null;
  /** The unit after the value: `ly`, `°`, or an element such as the drawn `M☉`. */
  readonly unit?: ReactNode;
  /** Whether the value takes the rest of the row, as a designation and an ID do. */
  readonly wide?: boolean;
  /** What the value is qualified by, such as the time an age is given at. */
  readonly beside?: ReactNode;
  /** Why a value that does not exist does not, beside its em dash: `NO LIGHT`. */
  readonly missingWhy?: string;
  /** Whether the label is a symbol that keeps its case, as `[Fe/H]` does. */
  readonly symbol?: boolean;
  /** Whether the value is a stale snapshot: muted, with the guide's trailing `S`. */
  readonly stale?: boolean;
}

/**
 * One labelled value of the `GALAXY` display's system readout, a `dt` and its `dd`: the value in
 * B612 Mono with its unit muted after it, or the guide's em dash in `--text-muted` for a value that
 * does not exist, with the reason where there is one (plan 05, P05.T10; plan 06, P06.T36). A
 * stale value is muted and trails the guide's `S`.
 */
export function SystemReading({
  label,
  value,
  unit,
  wide = false,
  beside,
  missingWhy,
  symbol = false,
  stale = false,
}: SystemReadingProps) {
  return (
    <>
      <dt className={symbol ? "system-readout__symbol" : undefined}>{label}</dt>
      <dd className={wide ? "system-readout__wide" : undefined}>
        {value === null ? (
          <>
            <span className="readout__missing">—</span>
            {missingWhy === undefined ? null : (
              <span className="system-readout__beside">{missingWhy}</span>
            )}
          </>
        ) : (
          <span className={stale ? "stale" : undefined}>
            <span className="system-readout__value">{value}</span>
            {unit === undefined ? null : (
              <>
                {/* The degree sign is written against the digits; other units after a space. */}
                {unit === "°" ? null : " "}
                <span className="system-readout__unit">{unit}</span>
              </>
            )}
            {beside === undefined ? null : <span className="system-readout__beside">{beside}</span>}
            {stale ? <StaleMark /> : null}
          </span>
        )}
      </dd>
    </>
  );
}
