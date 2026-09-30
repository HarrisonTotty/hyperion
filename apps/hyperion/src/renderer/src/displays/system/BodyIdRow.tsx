/** Props of {@link BodyIdRow}. */
export interface BodyIdRowProps {
  /** The ID as the screen writes it (`formatBodyIdHex`): the system's digits, `.`, the index. */
  readonly text: string;
}

/**
 * A body's `ID` row: the value takes the rest of the row, and may break after the full stop between
 * the system's 16 digits and the body's index, so that a narrow readout, as the bodies column is at
 * 1280 x 720, sets the index on a second line rather than scrolling sideways (the guide's § Layout:
 * never scroll horizontally). The digits themselves never break.
 */
export function BodyIdRow({ text }: BodyIdRowProps) {
  const stop = text.indexOf(".");
  return (
    <>
      <dt>ID</dt>
      <dd className="body-readout__wide">
        <span className="body-readout__value">
          {stop < 0 ? (
            text
          ) : (
            <>
              {text.slice(0, stop + 1)}
              <wbr />
              {text.slice(stop + 1)}
            </>
          )}
        </span>
      </dd>
    </>
  );
}
