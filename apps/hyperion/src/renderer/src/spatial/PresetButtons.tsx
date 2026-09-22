import { type CameraAngles, matchesPreset, PRESETS, type PresetName } from "./camera";

/** A preset view's control: the preset, its label, and its single key. */
export interface PresetControl {
  readonly name: PresetName;
  readonly label: string;
  readonly key: string;
}

/** Each preset view's label and single key, in the order they are offered (plan 05, D10). */
export const PRESET_CONTROLS: ReadonlyArray<PresetControl> = [
  { name: "top", label: "TOP", key: "T" },
  { name: "side", label: "SIDE", key: "S" },
  { name: "front", label: "FRONT", key: "F" },
  { name: "oblique", label: "OBLIQUE", key: "O" },
];

interface PresetButtonsProps {
  /** Where the camera looks now, which decides the preset shown as pressed. */
  readonly angles: CameraAngles;
  readonly onChoose: (name: PresetName) => void;
}

/**
 * The preset views of a spatial view as buttons, `TOP`, `SIDE`, `FRONT` and `OBLIQUE`, each showing
 * its single key.
 *
 * @remarks
 * Display controls, which change only what the view shows (`.control`). A button is pressed
 * (`aria-pressed`, and a rule under it) while the camera looks exactly in its direction, whatever
 * the zoom; turning away releases it.
 */
export function PresetButtons({ angles, onChoose }: PresetButtonsProps) {
  return (
    <fieldset className="preset-buttons" aria-label="Views">
      {PRESET_CONTROLS.map(({ name, label, key }) => (
        <button
          key={name}
          type="button"
          className="control preset-buttons__button"
          aria-pressed={matchesPreset(angles, PRESETS[name])}
          aria-keyshortcuts={key}
          onClick={() => {
            onChoose(name);
          }}
        >
          <span className="control__key">{key}</span> {label}
        </button>
      ))}
    </fieldset>
  );
}
