import {
  type FocusEvent,
  Fragment,
  type KeyboardEvent,
  type ReactNode,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { DisclosureGlyph } from "../../components/DisclosureGlyph";
import { EarthMassUnit } from "../../components/EarthMassUnit";
import { RequestStatus } from "../../components/RequestStatus";
import { SolarMassUnit } from "../../components/SolarMassUnit";
import { StatusText } from "../../components/StatusText";
import { isEmptyEntry, showSelected } from "../../lib/textEntry";
import {
  formatInput,
  formatInputsString,
  inputSpec,
  type NumericInputName,
  type NumericInputSpec,
  parseEntry,
  type PlanetInputs,
  rangeWords,
  refusedInput,
  type InputUnit,
  unitWords,
} from "./planetInputs";
import { BodyField } from "./BodyField";
import { describedBy } from "./describedBy";
import {
  DEFAULT_INPUTS_FOLD,
  type InputsFold,
  type InputsLayout,
  toggledInputsFold,
} from "./inputsLayout";
import { SeedField } from "./SeedField";
import { EntryRefusalContext, type EntryRefusals, useEntryDraft } from "./useEntryDraft";
import type { CommandState, PlanetViewer, ViewerCommand } from "./usePlanetViewer";

/** One group of the panel's inputs: the part it folds as, its title and its inputs, in order. */
interface InputGroup {
  readonly part: Exclude<InputsFold, "shown">;
  readonly title: string;
  readonly inputs: ReadonlyArray<NumericInputName>;
}

/**
 * The panel's groups of inputs, in procedure order: the star, the orbit, the planet, and its
 * volatile ranks, each drawn by default, which fold as the panel opens in either layout.
 */
const GROUPS: ReadonlyArray<InputGroup> = [
  { part: "star", title: "STAR", inputs: ["starMassMsun", "feHDex", "ageGyr"] },
  { part: "orbit", title: "ORBIT", inputs: ["smaAu", "eccentricity", "formedAtAu"] },
  { part: "planet", title: "PLANET", inputs: ["massMearth", "dayHours", "tiltDeg"] },
  {
    part: "volatiles",
    title: "VOLATILES · RANK",
    inputs: ["waterRank", "carbonRank", "nitrogenRank"],
  },
];

/** The group an input stands in. */
function partOf(name: NumericInputName): InputGroup["part"] | null {
  return GROUPS.find((group) => group.inputs.includes(name))?.part ?? null;
}

/** The part of the panel `target` stands in, by its `data-fold`, or `null`. */
function partAt(target: EventTarget | null): InputsFold | null {
  const part =
    target instanceof Element ? target.closest("[data-fold]")?.getAttribute("data-fold") : null;
  return part === "shown" ||
    part === "star" ||
    part === "orbit" ||
    part === "planet" ||
    part === "volatiles"
    ? part
    : null;
}

/** What the generator drew for the planet on show (`ShownPlanet`'s). */
type ShownDrawn = NonNullable<PlanetViewer["shown"]>["drawn"];

/** Props of {@link GroupSummary}. */
interface GroupSummaryProps {
  readonly group: InputGroup;
  readonly inputs: PlanetInputs;
  /** The values drawn for the planet on show, where the panel holds that planet's inputs. */
  readonly drawn: ShownDrawn | null;
}

/**
 * What a folded group says of its inputs, beside its disclosure: what each of its rows shows, in
 * order, joined by `·`, each value in its unit, the Missing dash where none stands, `DRAWN` after
 * one left to the draw, and a value with no unit, or whose unit an earlier row's shares, after its
 * label (`1 AU · ECC 0.0167 · FORMED AT — AU DRAWN`), since a drawn value is a reading, which no
 * fold may hide (R15.T6's review).
 */
function GroupSummary({ group, inputs, drawn }: GroupSummaryProps) {
  return (
    <span className="planet-inputs__summary">
      {group.inputs.map((name, at) => {
        const spec = inputSpec(name);
        const wire = inputs.values[name];
        const value = wire ?? drawn?.[name] ?? null;
        const isDrawn = spec.drawable && wire === null;
        // A value with no unit, or whose unit an earlier row's shares, after its label.
        const labelled =
          spec.unit === "" ||
          group.inputs.slice(0, at).some((earlier) => inputSpec(earlier).unit === spec.unit);
        return (
          <Fragment key={name}>
            {at === 0 ? null : " · "}
            <span className="planet-inputs__summary-input">
              {labelled ? <span className="field__label">{spec.label} </span> : null}
              {value === null ? (
                <span className="readout__missing">—</span>
              ) : (
                <span className="planet-inputs__value">{formatInput(spec, value)}</span>
              )}
              {spec.unit === "" ? null : (
                <span className="planet-inputs__unit">
                  {spec.unit === "°" ? null : " "}
                  <UnitMark unit={spec.unit} />
                </span>
              )}
              {isDrawn ? <span className="field__label"> DRAWN</span> : null}
            </span>
          </Fragment>
        );
      })}
    </span>
  );
}

/**
 * What the folded `SHOWN` says of the planet on show: `GENERATED` from the inputs, `BODY` and the
 * galaxy's planet's or moon's ID, set in the monospaced face as the field sets it, or `NONE` before
 * the first.
 */
function ShownSummary({ shown }: { readonly shown: PlanetViewer["shown"] }) {
  let summary: ReactNode;
  if (shown === null) {
    summary = "NONE";
  } else if (shown.source.kind === "generated") {
    summary = "GENERATED";
  } else {
    summary = (
      <>
        BODY <span className="planet-inputs__id">{shown.body.toUpperCase()}</span>
      </>
    );
  }
  return <span className="planet-inputs__summary">{summary}</span>;
}

/** Where a command stands until its planet is shown. */
type CommandOutcome = Exclude<CommandState, { readonly kind: "shown" }>;

/**
 * A command's outcome as it stands under its button: a refusal of `named`, a field of the panel
 * whose reason stands on show under it, names the field instead of stating the reason again
 * (`GENERATE REJECTED: see SMA`), so that the column keeps its room at 1280×720 (R15.T6's fold);
 * one of a field folded away states its reason.
 */
function outcomeUnder(
  state: CommandOutcome | null,
  named: NumericInputName | "seed" | null,
): CommandOutcome | null {
  if (state === null || named === null || state.kind !== "rejected") {
    return state;
  }
  return { ...state, reason: `see ${named === "seed" ? "SEED" : inputSpec(named).label}` };
}

/** The key that generates the panel's inputs from anywhere on the display but a text field. */
export const GENERATE_KEY = "g";

/**
 * Why `GENERATE` and `SHARE` are held back while a moon is shown: it has none of the viewer's
 * inputs, and the panel's are another planet's (plan R15, T5's follow-up).
 */
export const NO_INPUTS_REASON = "NO INPUTS: A MOON";

/** Props of {@link PlanetInputsPanel}. */
export interface PlanetInputsPanelProps {
  readonly viewer: PlanetViewer;
  /** Why the commands are held back (no universe open, the link down), or `null`. */
  readonly heldBack: string | null;
  /** The ID of the bar's words saying why, by which `GENERATE` and `OPEN BODY` are described. */
  readonly heldBackId: string;
  /**
   * The ID of the panel's {@link NO_INPUTS_REASON}, standing while the viewer holds no inputs,
   * by which `GENERATE` and the bar's `SHARE` are described.
   */
  readonly noInputsId: string;
  /** What stands after the inputs and before the seed: the viewer's `SUN` (`SunControl`). */
  readonly sun: ReactNode;
  /** The column's layout, by its height (`inputsLayout.ts`'s `inputsLayout`). */
  readonly layout: InputsLayout;
}

/** Whether two sets of inputs are the same planet's. */
function samePlanet(a: PlanetInputs, b: PlanetInputs): boolean {
  return formatInputsString(a) === formatInputsString(b);
}

/**
 * The `PLANET VIEWER`'s `INPUTS` panel (plan R15): the generator's own inputs of one planet and
 * its star, its seed, `GENERATE`, and where the last command stands.
 *
 * @remarks
 * - Each input is a field in its unit, right-aligned, that enters its value with `Enter` or when it
 *   is left, refusing a value out of range with what is valid (`MASS INVALID: enter 0.08 to 150`);
 *   `Escape`, or `Enter` in the emptied field, drops what was typed. One such refusal stands at a
 *   time on the panel, a newer one dropping the older and what was typed with it, and a fold drops
 *   those of the fields it hides ({@link useEntryDraft}). A refusal the server gives, which names its
 *   input, stands under that input's field.
 * - An input the generator may draw reads `DRAWN` while it is left to the draw: its field shows the
 *   value drawn for the planet on show while the panel holds that planet's inputs, and the Missing
 *   dash otherwise, since another planet draws another value. An entry gives it; `DRAW` beside a
 *   given one leaves it to the draw again.
 * - `GENERATE` (`G`), beside `SEED`, shows the panel's inputs as a planet, its `PENDING`, then its
 *   outcome, under it. It and the bar's `RANDOMIZE` are held back with the reason, which the bar
 *   states, while no universe is open or the link is down.
 * - `SHOWN` stands beside the panel's title, folded as it opens, saying what the planet on show
 *   is: `GENERATED`, `BODY` and its ID, or `NONE`. Opened, its `BODY` ({@link BodyField}) and
 *   `OPEN BODY` show a planet of the galaxy by its ID and put its inputs, read back, on the panel.
 *   While the panel holds the inputs of a planet whose system has more stars than one, a note says
 *   that they hold its host alone.
 * - A moon on show (`SHOWN` `BODY` and its ID) has none of the inputs: the groups keep their last
 *   values, muted, under {@link NO_INPUTS_REASON}, which holds `GENERATE` and the bar's `SHARE` back
 *   while it stands; an input entered gives the panel its inputs again, and `RANDOMIZE` and
 *   `OPEN BODY` stay offered.
 * - In the `full` layout `STAR`, `ORBIT` and `PLANET` stand open, and the volatile ranks and
 *   `SHOWN` fold on their own, folded as the panel opens. In the `compact` one, a column too short
 *   for that, as at 1280×720 (`inputsLayout.ts`), exactly one of `SHOWN` and the four groups is open,
 *   `STAR` as the panel opens: opening another folds it, and folding it opens `STAR` again, as
 *   `VIEW`'s compact column does (the guide's Layout: a page too short for its layout). A folded
 *   group says what its rows show ({@link GroupSummary}). On a switch to it, the part that holds
 *   the focus is the one open; the focus in a part that folds goes to its disclosure.
 * - A new refusal of an input in a folded group opens that group, and a new `OPEN BODY` (a
 *   hand-over from `OPEN IN PLANET VIEWER` among them) opens `SHOWN`, once each, so that its state
 *   stands under its field; the operator may fold either again.
 */
export function PlanetInputsPanel({
  viewer,
  heldBack,
  heldBackId,
  noInputsId,
  sun,
  layout,
}: PlanetInputsPanelProps) {
  const titleId = useId();
  const statusId = useId();
  const partsId = useId();
  const [volatilesOpen, setVolatilesOpen] = useState(false);
  const [shownOpen, setShownOpen] = useState(false);
  // In the compact layout exactly one part is open, STAR by default.
  const [compactOpen, setCompactOpen] = useState<InputsFold>(DEFAULT_INPUTS_FOLD);
  // The part that holds the focus, as the panel's focus events tell it.
  const [focusedPart, setFocusedPart] = useState<InputsFold | null>(null);
  const [foldLayout, setFoldLayout] = useState(layout);
  // The field whose entry refusal stands, the panel's one (useEntryDraft).
  const [refusalHolder, setRefusalHolder] = useState<string | null>(null);
  const refusals = useMemo<EntryRefusals>(
    () => ({ holder: refusalHolder, hold: setRefusalHolder }),
    [refusalHolder],
  );
  const { inputs, command, shown, noInputs } = viewer;
  const generateHeld = heldBack !== null || noInputs;
  const refused = command?.state.kind === "rejected" ? refusedInput(command.field) : null;
  const drawnShown =
    shown !== null && shown.inputs !== null && samePlanet(shown.inputs, inputs)
      ? shown.drawn
      : null;
  const refusalOf = (name: NumericInputName): string | null =>
    refused === name && command !== null && command.state.kind === "rejected"
      ? command.state.reason
      : null;
  const compact = layout === "compact";
  // On a switch to the compact layout, the part holding the focus is the one open; on a switch to
  // the full one, the part open in the compact one stays open.
  const switched = foldLayout !== layout;
  const focusOpens = switched && compact && focusedPart !== null ? focusedPart : null;
  const opened = focusOpens ?? compactOpen;
  if (opened !== compactOpen) {
    setCompactOpen(opened);
  }
  if (switched) {
    setFoldLayout(layout);
    if (!compact && compactOpen === "volatiles") {
      setVolatilesOpen(true);
    }
    if (!compact && compactOpen === "shown") {
      setShownOpen(true);
    }
  }
  const isOpen = (part: InputsFold): boolean => {
    if (compact) {
      return opened === part;
    }
    return part === "shown" ? shownOpen : part === "volatiles" ? volatilesOpen : true;
  };
  const toggle = (part: InputsFold): void => {
    if (compact) {
      setCompactOpen(toggledInputsFold(opened, part));
    } else if (part === "shown") {
      setShownOpen(!shownOpen);
    } else if (part === "volatiles") {
      setVolatilesOpen(!volatilesOpen);
    }
  };
  const row = (name: NumericInputName, folded: boolean) => (
    <InputRow
      key={name}
      spec={inputSpec(name)}
      wire={inputs.values[name]}
      drawn={drawnShown?.[name] ?? null}
      serverRefusal={refusalOf(name)}
      folded={folded}
      onEnter={(wire) => {
        viewer.setInput(name, wire);
      }}
    />
  );
  const statusOf = (kind: ViewerCommand["kind"]) =>
    command !== null && command.kind === kind && command.state.kind !== "shown"
      ? command.state
      : null;
  // The refused input's part, and whether its reason stands on show under its field.
  const refusedPart = refused === null || refused === "seed" ? null : partOf(refused);
  const reasonShown = refused === "seed" || (refusedPart !== null && isOpen(refusedPart));
  const generateStatus = outcomeUnder(statusOf("generate"), reasonShown ? refused : null);
  // The galaxy's planet on show, and whether the panel still holds its inputs.
  const bodyShown =
    shown !== null && shown.source.kind !== "generated"
      ? {
          body: shown.body,
          multipleStars: shown.source.kind === "body" && shown.source.multipleStars,
          held: shown.inputs !== null && samePlanet(shown.inputs, inputs),
        }
      : null;
  const openStatus = statusOf("open");
  // A new command state opens the part its state stands in, once, adjusting state as it renders
  // (React's pattern for state that follows a prop), so that the operator may fold it again.
  const commandState = command?.state ?? null;
  const [seenState, setSeenState] = useState(commandState);
  if (commandState !== seenState) {
    setSeenState(commandState);
    if (openStatus !== null) {
      setShownOpen(true);
      setCompactOpen("shown");
    } else if (refusedPart !== null) {
      if (refusedPart === "volatiles") {
        setVolatilesOpen(true);
      }
      setCompactOpen(refusedPart);
    }
  }
  const onFocus = (event: FocusEvent<HTMLDivElement>): void => {
    setFocusedPart(partAt(event.target));
  };
  const onBlur = (event: FocusEvent<HTMLDivElement>): void => {
    // The focus left the panel for another control, or for nothing by the operator's own hand,
    // not because its part was folded, as `VIEW`'s compact column tells it.
    const { relatedTarget, target, currentTarget } = event;
    const leftForControl = relatedTarget instanceof Node && !currentTarget.contains(relatedTarget);
    const leftForNothing =
      relatedTarget === null && target.isConnected && target.closest("[hidden]") === null;
    if (leftForControl || leftForNothing) {
      setFocusedPart(null);
    }
  };
  // The disclosures, by their part, to which the focus in a part that folds goes.
  const disclosuresRef = useRef(new Map<InputsFold, HTMLButtonElement>());
  const disclosureRef =
    (part: InputsFold) =>
    (button: HTMLButtonElement | null): void => {
      if (button === null) {
        disclosuresRef.current.delete(part);
      } else {
        disclosuresRef.current.set(part, button);
      }
    };
  // Where the focus goes once a render takes it from the part that held it: from a part that
  // folds to its disclosure, and from a disclosure the full layout does without (STAR's, ORBIT's,
  // PLANET's) to its part's first field, never to the page.
  const focusedOpen = focusedPart !== null && isOpen(focusedPart);
  // Whether the part that holds the focus has a disclosure in this layout: every part in the
  // compact one, and SHOWN's and the volatile ranks' alone in the full one.
  const focusedHasDisclosure =
    focusedPart !== null && (compact || focusedPart === "shown" || focusedPart === "volatiles");
  useLayoutEffect(() => {
    if (focusedPart === null) {
      return;
    }
    const active = document.activeElement;
    const lost =
      active === null ||
      active === document.body ||
      !active.isConnected ||
      active.closest("[hidden]") !== null;
    if (!lost) {
      return;
    }
    if (focusedOpen || !focusedHasDisclosure) {
      document.getElementById(`${partsId}-${focusedPart}`)?.querySelector("input")?.focus();
    } else {
      disclosuresRef.current.get(focusedPart)?.focus();
    }
  }, [focusedPart, focusedOpen, focusedHasDisclosure, partsId, disclosuresRef]);
  const disclosure = (part: InputsFold, title: string): ReactNode => (
    <button
      ref={disclosureRef(part)}
      type="button"
      className="control disclosure"
      aria-expanded={isOpen(part)}
      aria-controls={`${partsId}-${part}`}
      onClick={() => {
        toggle(part);
      }}
    >
      <DisclosureGlyph expanded={isOpen(part)} />
      {title}
    </button>
  );
  return (
    <section className="panel planet-inputs" aria-labelledby={titleId}>
      <EntryRefusalContext value={refusals}>
        <div className="planet-inputs__parts" onFocus={onFocus} onBlur={onBlur}>
          <div className="planet-inputs__head" data-fold="shown">
            <h2 className="panel__title" id={titleId}>
              INPUTS
            </h2>
            {disclosure("shown", "SHOWN")}
            {isOpen("shown") ? null : <ShownSummary shown={shown} />}
          </div>
          <div
            className="planet-inputs__shown"
            id={`${partsId}-shown`}
            data-fold="shown"
            hidden={!isOpen("shown")}
          >
            <BodyField
              shownBody={bodyShown?.body ?? null}
              status={openStatus}
              heldBack={heldBack}
              heldBackId={heldBackId}
              folded={!isOpen("shown")}
              onOpen={viewer.openBody}
            />
          </div>
          <div className={`planet-inputs__groups${noInputs ? " planet-inputs__groups--none" : ""}`}>
            {noInputs ? (
              <p className="planet-inputs__note" id={noInputsId}>
                <StatusText text={NO_INPUTS_REASON} />
              </p>
            ) : null}
            {bodyShown !== null && bodyShown.multipleStars && bodyShown.held ? (
              <p className="planet-inputs__note">
                <StatusText text="MULTIPLE STARS: the inputs hold its host alone" />
              </p>
            ) : null}
            {GROUPS.map((group) => {
              const open = isOpen(group.part);
              // The full layout's STAR, ORBIT and PLANET stand open, titled with no disclosure.
              const folds = compact || group.part === "volatiles";
              return (
                <fieldset key={group.part} className="planet-inputs__group" data-fold={group.part}>
                  <legend className="planet-inputs__legend">
                    {folds ? disclosure(group.part, group.title) : group.title}
                    {open ? null : (
                      <GroupSummary group={group} inputs={inputs} drawn={drawnShown} />
                    )}
                  </legend>
                  <div id={`${partsId}-${group.part}`} hidden={!open}>
                    {group.inputs.map((name) => row(name, !open))}
                  </div>
                </fieldset>
              );
            })}
          </div>
          {sun}
          <div className="planet-inputs__generate">
            <SeedField
              label="SEED"
              seed={inputs.seed}
              serverRefusal={
                refused === "seed" && command?.state.kind === "rejected"
                  ? command.state.reason
                  : null
              }
              onEnter={viewer.setSeed}
            />
            <button
              type="button"
              className="command"
              aria-disabled={generateHeld ? "true" : undefined}
              aria-describedby={describedBy([
                heldBack === null ? null : heldBackId,
                noInputs ? noInputsId : null,
                generateStatus === null ? null : statusId,
              ])}
              aria-keyshortcuts={GENERATE_KEY.toUpperCase()}
              onClick={() => {
                if (!generateHeld) {
                  viewer.generate();
                }
              }}
            >
              <span className="control__key">{GENERATE_KEY.toUpperCase()}</span> GENERATE
            </button>
          </div>
          {generateStatus === null ? null : (
            <RequestStatus state={generateStatus} id={statusId} subject="GENERATE" />
          )}
        </div>
      </EntryRefusalContext>
    </section>
  );
}

/** An input's unit: the drawn mass units, or the symbol. */
function UnitMark({ unit }: { readonly unit: InputUnit }) {
  let mark: ReactNode;
  switch (unit) {
    case "msun":
      mark = <SolarMassUnit />;
      break;
    case "mearth":
      mark = <EarthMassUnit />;
      break;
    case "dex":
    case "Gyr":
    case "AU":
    case "d":
    case "°":
    case "":
      mark = unit;
      break;
  }
  return mark;
}

/** Props of {@link InputRow}. */
interface InputRowProps {
  readonly spec: NumericInputSpec;
  /** The input's wire value, or `null` while it is left to its draw. */
  readonly wire: number | null;
  /** The value drawn for it on the planet on show, where the panel holds that planet's inputs. */
  readonly drawn: number | null;
  /** The server's refusal of this input, or `null`. */
  readonly serverRefusal: string | null;
  /** Whether its group is folded, which drops what was typed and its refusal. */
  readonly folded: boolean;
  /** Enters a wire value, or `null` to leave it to its draw. */
  readonly onEnter: (wire: number | null) => void;
}

/** One input's row: its label, field, unit, and its provenance where it may be drawn. */
function InputRow({ spec, wire, drawn, serverRefusal, folded, onEnter }: InputRowProps) {
  const fieldId = useId();
  const hintId = useId();
  const errorId = useId();
  const { draft, refusal, type, drop, refuse } = useEntryDraft(folded);
  const shownValue = wire ?? drawn;
  const text = shownValue === null ? "" : formatInput(spec, shownValue);
  const enter = (input: HTMLInputElement): void => {
    if (draft === null) {
      return;
    }
    if (isEmptyEntry(draft)) {
      drop();
      showSelected(input, text);
      return;
    }
    const parsed = parseEntry(spec, draft);
    if (parsed.ok) {
      drop();
      onEnter(parsed.wire);
    } else {
      refuse(parsed.reason);
    }
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
    if (event.key === "Enter") {
      event.preventDefault();
      enter(event.currentTarget);
    } else if (event.key === "Escape") {
      event.preventDefault();
      drop();
      showSelected(event.currentTarget, text);
    }
  };
  const shownRefusal =
    refusal ??
    (serverRefusal === null ? null : `${spec.label.toUpperCase()} REJECTED: ${serverRefusal}`);
  const isDrawn = spec.drawable && wire === null;
  return (
    <div className="planet-inputs__row">
      <label className="planet-inputs__label" htmlFor={fieldId}>
        {spec.label}
      </label>
      <input
        id={fieldId}
        className="form-field__input form-field__input--number"
        type="text"
        inputMode="decimal"
        autoComplete="off"
        spellCheck={false}
        value={draft ?? (isDrawn && drawn === null ? "—" : text)}
        aria-invalid={shownRefusal === null ? undefined : "true"}
        aria-describedby={shownRefusal === null ? hintId : `${hintId} ${errorId}`}
        onChange={(event) => {
          type(event.target.value);
        }}
        onFocus={(event) => {
          event.currentTarget.select();
        }}
        onKeyDown={onKeyDown}
        onBlur={(event) => {
          enter(event.currentTarget);
        }}
      />
      <span className="planet-inputs__unit">
        <UnitMark unit={spec.unit} />
      </span>
      <span className="planet-inputs__provenance">
        {!spec.drawable ? null : isDrawn ? (
          <span className="field__label">DRAWN</span>
        ) : (
          <button
            type="button"
            className="command planet-inputs__draw-one"
            aria-label={`DRAW ${spec.label}`}
            onClick={() => {
              drop();
              onEnter(null);
            }}
          >
            DRAW
          </button>
        )}
      </span>
      <span className="visually-hidden" id={hintId}>
        {`${spec.unit === "" ? "" : `${unitWords(spec.unit)}, `}${rangeWords(spec)}${spec.drawable ? ", or drawn" : ""}`}
      </span>
      {shownRefusal === null ? null : (
        <p className="form-field__error planet-inputs__error" id={errorId}>
          <StatusText text={shownRefusal} />
        </p>
      )}
    </div>
  );
}
