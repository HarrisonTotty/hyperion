import type { ReactNode } from "react";

/** What a reading shows: a value with its unit, a value in words, or the em dash with why. */
export type Shown =
  | { readonly kind: "value"; readonly value: string; readonly unit?: ReactNode }
  | { readonly kind: "missing"; readonly why?: string };

/** The guide's Missing state: the em dash in `--text-muted`, never a blank or a zero. */
export const MISSING: Shown = { kind: "missing" };

/** A value, with its unit where it has one. */
export function shown(text: string, unit?: ReactNode): Shown {
  return unit === undefined ? { kind: "value", value: text } : { kind: "value", value: text, unit };
}

/** Props of {@link ReadoutRow}. */
export interface ReadoutRowProps {
  readonly label: ReactNode;
  readonly shown: Shown;
  /** Whether the value takes the rest of the row, as a designation does. */
  readonly wide?: boolean;
}

/**
 * One labelled value of the `SYSTEM` display's readout, a `dt` and its `dd`: the value in B612 Mono
 * with its unit muted after it, or the em dash with the reason it is missing (plan 14, P14.T43.b).
 */
export function ReadoutRow({ label, shown: reading, wide = false }: ReadoutRowProps) {
  return (
    <>
      <dt>{label}</dt>
      <dd className={wide ? "body-readout__wide" : undefined}>
        {reading.kind === "missing" ? (
          <>
            <span className="readout__missing">—</span>
            {reading.why === undefined ? null : (
              <span className="body-readout__why">{reading.why}</span>
            )}
          </>
        ) : (
          <>
            <span className="body-readout__value">{reading.value}</span>
            {reading.unit === undefined ? null : (
              <>
                {" "}
                <span className="body-readout__unit">{reading.unit}</span>
              </>
            )}
          </>
        )}
      </dd>
    </>
  );
}
