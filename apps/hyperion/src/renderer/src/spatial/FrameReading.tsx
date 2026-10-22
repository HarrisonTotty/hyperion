/** Props of {@link FrameReading}. */
export interface FrameReadingProps {
  /** The reference frame's name: `GALACTIC`, `SYSTEM BARYCENTRIC`. */
  readonly frameName: string;
}

/**
 * The reference frame a spatial view's places are given in, `FRAME GALACTIC`: the first of its
 * furniture's readings, and of a graph's line that reads the same answer (plan 12, P12.T11.d).
 *
 * @remarks
 * A term and its definition, for a `dl` of readings. The frame's name is a word, so it takes no
 * fixed slot and never the stale `S`: a frame is not a value read from the answer.
 */
export function FrameReading({ frameName }: FrameReadingProps) {
  return (
    <div className="field spatial-reading">
      <dt className="field__label">FRAME</dt> <dd>{frameName}</dd>
    </div>
  );
}
