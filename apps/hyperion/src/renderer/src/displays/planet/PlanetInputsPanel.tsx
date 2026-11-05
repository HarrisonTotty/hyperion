import { type KeyboardEvent, type ReactNode, useId, useState } from "react";

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
import { SeedField } from "./SeedField";
import type { PlanetViewer, ViewerCommand } from "./usePlanetViewer";

/** The panel's groups of inputs, in procedure order: the star, the orbit, the planet, its volatiles. */
const GROUPS: ReadonlyArray<{
  readonly title: string;
  readonly inputs: ReadonlyArray<NumericInputName>;
}> = [
  { title: "STAR", inputs: ["starMassMsun", "feHDex", "ageGyr"] },
  { title: "ORBIT", inputs: ["smaAu", "eccentricity", "formedAtAu"] },
  { title: "PLANET", inputs: ["massMearth", "dayHours", "tiltDeg"] },
];

/** The volatile ranks, folded behind their disclosure as the panel opens, each drawn by default. */
const VOLATILES: ReadonlyArray<NumericInputName> = ["waterRank", "carbonRank", "nitrogenRank"];

/** What the folded volatiles' disclosure says of them: `ALL DRAWN`, or how many are given. */
function volatilesSummary(inputs: PlanetInputs): string {
  const given = VOLATILES.filter((name) => inputs.values[name] !== null).length;
  return given === 0
    ? "ALL DRAWN"
    : `${String(given)} GIVEN, ${String(VOLATILES.length - given)} DRAWN`;
}

/** The key that generates the panel's inputs from anywhere on the display but a text field. */
export const GENERATE_KEY = "g";

/** Props of {@link PlanetInputsPanel}. */
export interface PlanetInputsPanelProps {
  readonly viewer: PlanetViewer;
  /** Why the commands are held back (no universe open, the link down), or `null`. */
  readonly heldBack: string | null;
  /** The ID the words saying why take, which the bar's `RANDOMIZE` is described by too. */
  readonly heldBackId: string;
  /** What stands after the inputs and before the seed: the viewer's `SUN` (`SunControl`). */
  readonly sun: ReactNode;
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
 *   `Escape`, or `Enter` in the emptied field, drops what was typed. A refusal the server gives,
 *   which names its input, stands under that input's field.
 * - An input the generator may draw reads `DRAWN` while it is left to the draw: its field shows the
 *   value drawn for the planet on show while the panel holds that planet's inputs, and the Missing
 *   dash otherwise, since another planet draws another value. An entry gives it; `DRAW` beside a
 *   given one leaves it to the draw again.
 * - The volatile ranks, drawn unless given, are folded behind their disclosure as the panel opens,
 *   which says how many are given, so that the panel stands whole at 1280×720; a refusal of one
 *   opens it.
 * - `GENERATE` (`G`), beside `SEED`, shows the panel's inputs as a planet, its `PENDING`, then its
 *   outcome, under it. It and the bar's `RANDOMIZE` are held back with the reason while no universe
 *   is open or the link is down.
 * - `BODY` heads the panel ({@link BodyField}): `OPEN BODY` shows a planet of the galaxy by its ID
 *   and puts its inputs, read back, on the panel. While the panel holds the inputs of a planet
 *   whose system has more stars than one, a note says that they hold its host alone.
 */
export function PlanetInputsPanel({ viewer, heldBack, heldBackId, sun }: PlanetInputsPanelProps) {
  const titleId = useId();
  const statusId = useId();
  const volatilesId = useId();
  const [volatilesOpen, setVolatilesOpen] = useState(false);
  const { inputs, command, shown } = viewer;
  const refused = command?.state.kind === "rejected" ? refusedInput(command.field) : null;
  const drawnShown = shown !== null && samePlanet(shown.inputs, inputs) ? shown.drawn : null;
  const refusalOf = (name: NumericInputName): string | null =>
    refused === name && command !== null && command.state.kind === "rejected"
      ? command.state.reason
      : null;
  const row = (name: NumericInputName) => (
    <InputRow
      key={name}
      spec={inputSpec(name)}
      wire={inputs.values[name]}
      drawn={drawnShown?.[name] ?? null}
      serverRefusal={refusalOf(name)}
      onEnter={(wire) => {
        viewer.setInput(name, wire);
      }}
    />
  );
  // A refusal of a folded rank opens its group, so that it stands under its field.
  const volatilesShown = volatilesOpen || VOLATILES.some((name) => refusalOf(name) !== null);
  const statusOf = (kind: ViewerCommand["kind"]) =>
    command !== null && command.kind === kind && command.state.kind !== "shown"
      ? command.state
      : null;
  const generateStatus = statusOf("generate");
  // The galaxy's planet on show, and whether the panel still holds its inputs.
  const bodyShown =
    shown !== null && shown.source.kind === "body"
      ? {
          body: shown.body,
          multipleStars: shown.source.multipleStars,
          held: samePlanet(shown.inputs, inputs),
        }
      : null;
  return (
    <section className="panel planet-inputs" aria-labelledby={titleId}>
      <h2 className="panel__title" id={titleId}>
        INPUTS
      </h2>
      <BodyField
        shownBody={bodyShown?.body ?? null}
        status={statusOf("open")}
        heldBack={heldBack}
        heldBackId={heldBackId}
        onOpen={viewer.openBody}
      />
      {bodyShown !== null && bodyShown.multipleStars && bodyShown.held ? (
        <p className="planet-inputs__note">
          <StatusText text="MULTIPLE STARS: the inputs hold its host alone" />
        </p>
      ) : null}
      <div className="planet-inputs__groups">
        {GROUPS.map((group) => (
          <fieldset key={group.title} className="planet-inputs__group">
            <legend className="planet-inputs__legend">{group.title}</legend>
            {group.inputs.map(row)}
          </fieldset>
        ))}
        <fieldset className="planet-inputs__group">
          <legend className="planet-inputs__legend">
            <button
              type="button"
              className="control disclosure"
              aria-expanded={volatilesShown}
              aria-controls={volatilesId}
              onClick={() => {
                setVolatilesOpen(!volatilesShown);
              }}
            >
              <DisclosureGlyph expanded={volatilesShown} />
              VOLATILES · RANK
            </button>
            {volatilesShown ? null : (
              <span className="planet-inputs__summary">{volatilesSummary(inputs)}</span>
            )}
          </legend>
          <div id={volatilesId} hidden={!volatilesShown}>
            {VOLATILES.map(row)}
          </div>
        </fieldset>
      </div>
      {sun}
      <div className="planet-inputs__generate">
        <SeedField
          label="SEED"
          seed={inputs.seed}
          serverRefusal={
            refused === "seed" && command?.state.kind === "rejected" ? command.state.reason : null
          }
          onEnter={viewer.setSeed}
        />
        <button
          type="button"
          className="command"
          aria-disabled={heldBack === null ? undefined : "true"}
          aria-describedby={describedBy([
            heldBack === null ? null : heldBackId,
            generateStatus === null ? null : statusId,
          ])}
          aria-keyshortcuts={GENERATE_KEY.toUpperCase()}
          onClick={() => {
            if (heldBack === null) {
              viewer.generate();
            }
          }}
        >
          <span className="control__key">{GENERATE_KEY.toUpperCase()}</span> GENERATE
        </button>
      </div>
      {heldBack === null ? null : (
        <p className="planet-inputs__held" id={heldBackId}>
          <StatusText text={`NOT AVAILABLE: ${heldBack}`} />
        </p>
      )}
      {generateStatus === null ? null : (
        <RequestStatus state={generateStatus} id={statusId} subject="GENERATE" />
      )}
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
  /** Enters a wire value, or `null` to leave it to its draw. */
  readonly onEnter: (wire: number | null) => void;
}

/** One input's row: its label, field, unit, and its provenance where it may be drawn. */
function InputRow({ spec, wire, drawn, serverRefusal, onEnter }: InputRowProps) {
  const fieldId = useId();
  const hintId = useId();
  const errorId = useId();
  const [draft, setDraft] = useState<string | null>(null);
  const [refusal, setRefusal] = useState<string | null>(null);
  const shownValue = wire ?? drawn;
  const text = shownValue === null ? "" : formatInput(spec, shownValue);
  const enter = (input: HTMLInputElement): void => {
    if (draft === null) {
      return;
    }
    if (isEmptyEntry(draft)) {
      setDraft(null);
      setRefusal(null);
      showSelected(input, text);
      return;
    }
    const parsed = parseEntry(spec, draft);
    if (parsed.ok) {
      setDraft(null);
      setRefusal(null);
      onEnter(parsed.wire);
    } else {
      setRefusal(parsed.reason);
    }
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
    if (event.key === "Enter") {
      event.preventDefault();
      enter(event.currentTarget);
    } else if (event.key === "Escape") {
      event.preventDefault();
      setDraft(null);
      setRefusal(null);
      showSelected(event.currentTarget, text);
    }
  };
  const shownRefusal = refusal ?? serverRefusal;
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
          setDraft(event.target.value);
          setRefusal(null);
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
              setDraft(null);
              setRefusal(null);
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
