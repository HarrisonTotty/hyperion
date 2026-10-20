import { useId } from "react";

import { DisclosureGlyph } from "../../components/DisclosureGlyph";
import { StatusText } from "../../components/StatusText";
import type { ViewId } from "../../view/camera/state";

/** One slot as the panel shows it. */
export interface InstrumentsPanelSlot {
  readonly id: ViewId;
  readonly name: string;
  readonly open: boolean;
  /** Whether the stage has room to open it: a closed slot without it has `OPEN` held back. */
  readonly room: boolean;
  /**
   * How an open slot is held where the stage cannot hold it whole (plan R10, R10.T12.b;
   * decision-r10-view-block-room §3): as its `stub` in its corner, or `whole`, off the stage; `null`
   * while it stands, or is closed.
   */
  readonly held: "stub" | "whole" | null;
}

/**
 * The panel's title row as a disclosure, in `VIEW`'s compact layout (R07.T19.b;
 * decision-r07-t19-layout, item 1b).
 */
export interface InstrumentsPanelFold {
  /** Whether the panel is folded to its title row. */
  readonly folded: boolean;
  /** Called when its disclosure button is pressed. */
  readonly onToggle: () => void;
}

/** Props of {@link InstrumentsPanel}. */
export interface InstrumentsPanelProps {
  /** The panel's ID, or none. */
  readonly id?: string | undefined;
  /** Its title row as a disclosure (the compact layout), or `undefined` where it never folds. */
  readonly fold?: InstrumentsPanelFold | undefined;
  /** Receives its disclosure button, to which the focus is given when the panel folds. */
  readonly toggleRef?: ((button: HTMLButtonElement | null) => void) | undefined;
  /** The primary view's identity and name (`PRIMARY`). */
  readonly primary: { readonly id: ViewId; readonly name: string };
  readonly slots: ReadonlyArray<InstrumentsPanelSlot>;
  /** The `CONTROLS` view: the one the side column's list and controls, and the keys, act on. */
  readonly operated: ViewId;
  /**
   * Whether no view can be drawn (the stage shows the graphics' annunciation in its place), which
   * holds every `OPEN` back.
   */
  readonly unavailable: boolean;
  readonly onOpen: (id: ViewId) => void;
  readonly onClose: (id: ViewId) => void;
  readonly onOperate: (id: ViewId) => void;
}

/** Why no slot can open while no view is drawn; the stage states the cause in the view's place. */
const NO_VIEW = "NOT AVAILABLE: no view can be drawn";

/** Names joined as a sentence lists them: `INSTRUMENT 1 and INSTRUMENT 2`. */
function namesOf(slots: ReadonlyArray<InstrumentsPanelSlot>): string {
  return slots.map((slot) => slot.name).join(" and ");
}

/** Why closed instruments cannot be the `CONTROLS` view (the guide's `NOT AVAILABLE` form). */
function notOpen(closed: ReadonlyArray<InstrumentsPanelSlot>): string {
  return `NOT AVAILABLE: ${namesOf(closed)} ${closed.length > 1 ? "are" : "is"} not open`;
}

/** Why slots cannot open: the stage has no room for them at this size and interface scale. */
function noRoom(slots: ReadonlyArray<InstrumentsPanelSlot>): string {
  return `NOT AVAILABLE: no room for ${namesOf(slots)} at this window size`;
}

interface PanelButtonProps {
  readonly label: string;
  readonly pressed: boolean;
  /** The ID of the reason that describes it, or `null` for none. */
  readonly reasonId: string | null;
  /**
   * Whether it acts: held back where it does not. An open slot's `OPEN` acts though its slot is
   * held, its reason saying where the slot is.
   */
  readonly acts: boolean;
  readonly onPress: () => void;
}

/** A pressed-state button, held back where it cannot act and described by its reason. */
function PanelButton({ label, pressed, reasonId, acts, onPress }: PanelButtonProps) {
  return (
    <button
      type="button"
      className="control preset-buttons__button"
      aria-pressed={pressed}
      // Held back rather than disabled, so that it keeps its focus and can say why.
      aria-disabled={acts ? undefined : "true"}
      aria-describedby={reasonId ?? undefined}
      onClick={() => {
        if (acts) {
          onPress();
        }
      }}
    >
      {label}
    </button>
  );
}

/**
 * The instrument views' panel (plan R07, T19; decision-r07-t19 item 2d), first in the side column:
 * each slot's congruent pair `OPEN` and `CLOSE`, the one in force pressed and underlined, and the
 * `CONTROLS` selector, which points the side column's `Targets`, `Camera` and `Style` panels, and
 * the view's single keys pressed off a canvas, at `PRIMARY`, `INSTRUMENT 1` or `INSTRUMENT 2`.
 *
 * @remarks
 * Display controls: they change what the console shows, never the ship, so each acts at once. The
 * selector always offers all three views, so that nothing moves as instruments open; a closed
 * instrument's is held back with its reason, as is the `OPEN` of a slot the stage has no room for,
 * or of any slot while no view can be drawn. An open slot the stage cannot hold whole is held
 * (plan R10, R10.T12.b; decision-r10-view-block-room §3): its row says so, in the same line as the
 * slots without room to open (`NOT AVAILABLE: no room for INSTRUMENT 2 at this window size`), which
 * describes its `OPEN`, and its `CONTROLS` button is held back by it, since a held view is not
 * drawn. Each reason is one line under the rows, however many buttons it holds back, so that the
 * side column keeps its height. In `VIEW`'s compact layout its title row is a disclosure button,
 * `INSTRUMENTS`, folded when the display mounts; while folded, the line naming the slots held
 * whole, off the stage, stands under it, as a folded panel's statuses stand under its row.
 */
export function InstrumentsPanel({
  id,
  fold,
  toggleRef,
  primary,
  slots,
  operated,
  unavailable,
  onOpen,
  onClose,
  onOperate,
}: InstrumentsPanelProps) {
  const titleId = useId();
  const bodyId = useId();
  const noViewId = useId();
  const noRoomId = useId();
  const controlsReasonId = useId();
  const closed = slots.filter((slot) => !slot.open);
  const heldOpen = closed.filter((slot) => unavailable || !slot.room);
  // Every slot the stage has no room for: a closed one's `OPEN` held back, and an open one held.
  const noRoomFor = slots.filter((slot) =>
    slot.open ? slot.held !== null : !unavailable && !slot.room,
  );
  const heldWhole = slots.filter((slot) => slot.open && slot.held === "whole");
  const reasonOf = (slot: InstrumentsPanelSlot): string | null => {
    if (unavailable && !slot.open) {
      return noViewId;
    }
    return noRoomFor.includes(slot) ? noRoomId : null;
  };
  return (
    <section
      className={
        fold?.folded === true
          ? "panel view-instruments-panel view-instruments-panel--folded"
          : "panel view-instruments-panel"
      }
      aria-labelledby={titleId}
      id={id}
    >
      <h2 className="panel__title view-instruments-panel__title" id={titleId}>
        {fold === undefined ? (
          "Instruments"
        ) : (
          <button
            ref={toggleRef}
            type="button"
            className="control disclosure"
            aria-expanded={!fold.folded}
            aria-controls={bodyId}
            onClick={fold.onToggle}
          >
            <DisclosureGlyph expanded={!fold.folded} />
            Instruments
          </button>
        )}
      </h2>
      {fold?.folded === true && heldWhole.length > 0 ? (
        <p className="view-instruments-panel__reason view-instruments-panel__standing">
          <StatusText text={noRoom(heldWhole)} />
        </p>
      ) : null}
      <div className="view-instruments-panel__body" id={bodyId} hidden={fold?.folded === true}>
        {slots.map((slot) => (
          <fieldset key={slot.id} className="preset-buttons view-instruments-panel__row">
            <legend className="field__label view-instruments-panel__label">{slot.name}</legend>
            <PanelButton
              label="OPEN"
              pressed={slot.open}
              reasonId={reasonOf(slot)}
              acts={!heldOpen.includes(slot)}
              onPress={() => {
                onOpen(slot.id);
              }}
            />
            <PanelButton
              label="CLOSE"
              pressed={!slot.open}
              reasonId={null}
              acts
              onPress={() => {
                onClose(slot.id);
              }}
            />
          </fieldset>
        ))}
        <fieldset className="preset-buttons view-instruments-panel__row">
          <legend className="field__label view-instruments-panel__label">CONTROLS</legend>
          <PanelButton
            label={primary.name}
            pressed={operated === primary.id}
            reasonId={null}
            acts
            onPress={() => {
              onOperate(primary.id);
            }}
          />
          {slots.map((slot) => (
            <PanelButton
              key={slot.id}
              label={slot.name}
              pressed={operated === slot.id}
              reasonId={!slot.open ? controlsReasonId : slot.held === null ? null : noRoomId}
              acts={slot.open && slot.held === null}
              onPress={() => {
                onOperate(slot.id);
              }}
            />
          ))}
        </fieldset>
        {unavailable && heldOpen.length > 0 ? (
          <p className="view-instruments-panel__reason" id={noViewId}>
            <StatusText text={NO_VIEW} />
          </p>
        ) : null}
        {noRoomFor.length === 0 ? null : (
          <p className="view-instruments-panel__reason" id={noRoomId}>
            <StatusText text={noRoom(noRoomFor)} />
          </p>
        )}
        {closed.length === 0 ? null : (
          <p className="view-instruments-panel__reason" id={controlsReasonId}>
            <StatusText text={notOpen(closed)} />
          </p>
        )}
      </div>
    </section>
  );
}
