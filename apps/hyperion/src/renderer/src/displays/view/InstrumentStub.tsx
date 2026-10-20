import { type Ref, useId } from "react";

import { StatusText } from "../../components/StatusText";
import { STUB_REASON } from "./viewRoom";

/** Props of {@link InstrumentStub}. */
export interface InstrumentStubProps {
  /** The slot's name: `INSTRUMENT 2`. */
  readonly name: string;
  /** Closes the slot, as its row's `CLOSE` on the `INSTRUMENTS` panel does. */
  readonly onClose: () => void;
  /** Receives the stub's panel, which the display measures for the stage's room. */
  readonly ref?: Ref<HTMLElement> | undefined;
}

/**
 * A held `INSTRUMENT 2` in its corner while `INSTRUMENT 1` stands (plan R10, R10.T12.b;
 * decision-r10-view-block-room §3): its title row with its `CLOSE`, and
 * `NOT AVAILABLE: no room at this window size`, so that the operator finds where the instrument
 * was and why it is not drawn.
 *
 * @remarks
 * The slot draws nothing and its view is not drawn while it is held; its camera, style and target
 * are kept, and it returns as it was, as a cut, once the stage would hold it whole with a line to
 * spare. The stub stands where the slot stands, at the stage's foot, never past it.
 */
export function InstrumentStub({ name, onClose, ref }: InstrumentStubProps) {
  const titleId = useId();
  const reasonId = useId();
  return (
    <section
      ref={ref}
      className="panel view-instrument view-instrument--slot-2 view-instrument--stub"
      aria-labelledby={titleId}
      aria-describedby={reasonId}
    >
      <div className="view-instrument__stub-row">
        <h2 className="panel__title view-instrument__title" id={titleId}>
          {name}
        </h2>
        <button type="button" className="control" onClick={onClose}>
          CLOSE
        </button>
      </div>
      <p className="view-instrument__reason" id={reasonId}>
        <StatusText text={STUB_REASON} />
      </p>
    </section>
  );
}
