import { useId } from "react";

import type { ViewId } from "../../view/camera/state";

/** One slot as the panel shows it. */
export interface InstrumentsPanelSlot {
  readonly id: ViewId;
  readonly name: string;
  readonly open: boolean;
  /** Whether the stage has room to open it: a closed slot without it has `OPEN` held back. */
  readonly room: boolean;
}

/** Props of {@link InstrumentsPanel}. */
export interface InstrumentsPanelProps {
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
  /** The ID of the reason it is held back with, or `null` where it acts. */
  readonly reasonId: string | null;
  readonly onPress: () => void;
}

/** A pressed-state button, held back and described by its reason where it cannot act. */
function PanelButton({ label, pressed, reasonId, onPress }: PanelButtonProps) {
  return (
    <button
      type="button"
      className="control preset-buttons__button"
      aria-pressed={pressed}
      // Held back rather than disabled, so that it keeps its focus and can say why.
      aria-disabled={reasonId === null ? undefined : "true"}
      aria-describedby={reasonId ?? undefined}
      onClick={() => {
        if (reasonId === null) {
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
 * or of any slot while no view can be drawn. Each reason is one line under the rows, however many
 * buttons it holds back, so that the side column keeps its height.
 */
export function InstrumentsPanel({
  primary,
  slots,
  operated,
  unavailable,
  onOpen,
  onClose,
  onOperate,
}: InstrumentsPanelProps) {
  const titleId = useId();
  const openReasonId = useId();
  const controlsReasonId = useId();
  const closed = slots.filter((slot) => !slot.open);
  const heldOpen = closed.filter((slot) => unavailable || !slot.room);
  const openReason = unavailable ? NO_VIEW : heldOpen.length > 0 ? noRoom(heldOpen) : null;
  return (
    <section className="panel view-instruments-panel" aria-labelledby={titleId}>
      <h2 className="panel__title" id={titleId}>
        Instruments
      </h2>
      {slots.map((slot) => (
        <fieldset key={slot.id} className="preset-buttons view-instruments-panel__row">
          <legend className="field__label view-instruments-panel__label">{slot.name}</legend>
          <PanelButton
            label="OPEN"
            pressed={slot.open}
            reasonId={heldOpen.includes(slot) ? openReasonId : null}
            onPress={() => {
              onOpen(slot.id);
            }}
          />
          <PanelButton
            label="CLOSE"
            pressed={!slot.open}
            reasonId={null}
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
          onPress={() => {
            onOperate(primary.id);
          }}
        />
        {slots.map((slot) => (
          <PanelButton
            key={slot.id}
            label={slot.name}
            pressed={operated === slot.id}
            reasonId={slot.open ? null : controlsReasonId}
            onPress={() => {
              onOperate(slot.id);
            }}
          />
        ))}
      </fieldset>
      {openReason === null || heldOpen.length === 0 ? null : (
        <p className="view-instruments-panel__reason" id={openReasonId}>
          {openReason}
        </p>
      )}
      {closed.length === 0 ? null : (
        <p className="view-instruments-panel__reason" id={controlsReasonId}>
          {notOpen(closed)}
        </p>
      )}
    </section>
  );
}
