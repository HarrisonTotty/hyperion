import type { ReactNode } from "react";

import { StaleSlot } from "../../components/StaleSlot";

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
  /**
   * Whether the value is a stale snapshot: muted, with the guide's trailing `S` in the slot the
   * reading keeps for it. Given, true or false, for a value read from a source that can go stale;
   * absent for a reading with no slot at all.
   */
  readonly stale?: boolean | undefined;
  /**
   * Whether the value is a status in a value's place (`NOT YET FORMED`, `NOT YET MODELLED`), which
   * keeps its slot and takes no mark, a status not being a value, but is muted with its source
   * while `stale` holds, so that a stale answer's words never read as live.
   */
  readonly status?: boolean;
  /**
   * The width of the value's slot, in characters of its monospaced figures, the value
   * right-aligned in it, so that a reading whose digits vary keeps its width (§ Numbers); as wide
   * as its value when absent.
   */
  readonly widthCh?: number | undefined;
}

/**
 * One labelled value of the `GALAXY` display's system readout, a `dt` and its `dd`: the value in
 * B612 Mono with its unit muted after it, or the guide's em dash in `--text-muted` for a value that
 * does not exist, with the reason where there is one (plan 05, P05.T10; plan 06, P06.T36).
 *
 * @remarks
 * A reading given `stale`, true or false, keeps a slot for the guide's trailing `S`
 * ({@link StaleSlot}): after the value's unit and before what it is qualified by, as the guide's
 * `STARS V 10.1 mag S CAM` has it, and after the em dash and before its reason. The slot holds the
 * mark while `stale` holds and the value exists, the em dash having no last value, so that the
 * mark's coming moves nothing after it (plan 05, P05.T14). A stale value is muted as well. A
 * status in a value's place keeps its slot empty, and is muted while its source is stale. A
 * reading given no `stale` keeps no slot and takes no mark.
 */
export function SystemReading({
  label,
  value,
  unit,
  wide = false,
  beside,
  missingWhy,
  symbol = false,
  stale,
  status = false,
  widthCh,
}: SystemReadingProps) {
  const muted = stale === true;
  const marked = muted && !status && value !== null;
  const slot = stale === undefined ? null : <StaleSlot stale={marked} />;
  return (
    <>
      <dt className={symbol ? "system-readout__symbol" : undefined}>{label}</dt>
      <dd className={wide ? "system-readout__wide" : undefined}>
        {value === null ? (
          <>
            <span className="readout__missing">—</span>
            {slot}
            {missingWhy === undefined ? null : (
              <span className="system-readout__beside">{missingWhy}</span>
            )}
          </>
        ) : (
          <span className={muted ? "stale" : undefined}>
            {widthCh === undefined ? (
              <span className="system-readout__value">{value}</span>
            ) : (
              <span
                className="system-readout__value system-readout__value--fixed"
                style={{ minWidth: `${widthCh}ch` }}
              >
                {value}
              </span>
            )}
            {unit === undefined ? null : (
              <>
                {/* The degree sign is written against the digits; other units after a space. */}
                {unit === "°" ? null : " "}
                <span className="system-readout__unit">{unit}</span>
              </>
            )}
            {slot}
            {beside === undefined ? null : <span className="system-readout__beside">{beside}</span>}
          </span>
        )}
      </dd>
    </>
  );
}
