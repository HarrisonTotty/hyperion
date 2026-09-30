/**
 * The guide's stale mark: a trailing `S` after a value whose source has stopped backing it, named
 * for assistive technology ("Data states").
 *
 * @remarks
 * A space before it, so that the mark reads apart from the value it follows; the `S` itself is
 * hidden from assistive technology, which is given the word instead. The value it follows is muted
 * with the `.stale` class, since the guide asks for both the colour and the mark.
 */
export function StaleMark() {
  return (
    <>
      {" "}
      <span className="stale-mark" aria-hidden="true">
        S
      </span>
      <span className="visually-hidden">stale</span>
    </>
  );
}
