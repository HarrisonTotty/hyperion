import type { ReactNode } from "react";

/** What ends a status's state: its colon, and the space before the sentence after it. */
const STATE_END = ": ";

/** A lower-case letter, which makes what follows the state a sentence rather than an annunciation. */
const LOWER_CASE = /[a-z]/u;

interface StatusTextProps {
  /**
   * The line as one string: `NOT AVAILABLE: no room for DUST OVERLAY at this window size`,
   * `SCENE STALE: NO CARRIER`, `PENDING`.
   */
  readonly text: string;
  /**
   * How a part's words are set, where they hold runs a line must not break inside
   * (`exposureNote`) or a sign that is drawn; the words as they stand where absent. Called with
   * each part's text, the whole line's where it is one part.
   */
  readonly renderPart?: ((part: string) => ReactNode) | undefined;
}

/**
 * A status line's words in the guide's one form for a state and its cause (Typography;
 * decision-gg-port-a2 §7, signoff-11 item 24): the state and its colon tracked as an annunciation,
 * and the sentence after them, which says the cause, the consequence or what the operator can do,
 * untracked, a name inside it included.
 *
 * @remarks
 * The line splits at its first `": "`, so that a server's reason keeps its own colons: the text
 * through that colon goes in `.status-text__state`, and the space after it and the rest in
 * `.status-text__sentence`. The rest is a sentence only where it holds a lower-case letter: the
 * server's reasons are its errors' `Display` text, lower case by the Rust rules, and every state is
 * upper case by construction. A line with no `": "` (`PENDING`, `NO CARRIER`), or whose rest is
 * upper case (`SCENE STALE: NO CARRIER`, `AUTO NOT AVAILABLE: NO IMAGE TO METER`), is one
 * annunciation and one `.status-text__state`, tracked whole. A unit symbol is lower case too, so
 * an upper-case rest holding one (`BEYOND 2000 ly`) would read as a sentence; no caller passes such
 * a line, and the sky's `BEYOND 2000 ly: STREAMING`, a reading's note, is not passed.
 *
 * Phrasing content only, so that it goes inside an `output`, a `p` or a `span`. Both parts inherit
 * the line's size, face and colour, which carry its standing, and set only their letter spacing, so
 * that a container's own spacing may stay. The text content is the line's, unchanged where
 * `renderPart` keeps each part's words as `exposureNote` does, so a live region holding it
 * announces the same words. Pass only a status: a sentence with no state, such as `INHIBIT`'s
 * consequence, is not one, and would be tracked whole here.
 */
export function StatusText({ text, renderPart }: StatusTextProps) {
  const set = renderPart ?? ((part: string) => part);
  const end = text.indexOf(STATE_END);
  const rest = end < 0 ? "" : text.slice(end + 1);
  if (!LOWER_CASE.test(rest)) {
    return <span className="status-text__state">{set(text)}</span>;
  }
  return (
    <>
      <span className="status-text__state">{set(text.slice(0, end + 1))}</span>
      <span className="status-text__sentence">{set(rest)}</span>
    </>
  );
}
