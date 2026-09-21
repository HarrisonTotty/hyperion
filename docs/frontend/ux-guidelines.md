# HYPERION UX Guidelines

The visual and interaction standard for every station console in the `hyperion` frontend.

HYPERION's consoles should look like flight hardware that an agency certified, not like a
game menu. The reference points are NASA crew displays and glass cockpits for how
information behaves, and _The Expanse_ and _Starfield_ for how a believable near-future
ship feels. When those pull in different directions, the operational standard wins.

Rules written with "must" and "never" are requirements. Rules written with "should" are
defaults that a console may break with a stated reason.

## Principles

1. **Function before atmosphere.** Every element on a console answers a question the
   operator has or accepts a command they can give. If it does neither, remove it. No fake
   scrolling hex, no spinning decorations, no unlabelled greebles.
2. **Dark and quiet.** A nominal ship is a calm screen: neutral text on a dark surface and
   no saturated colour. Colour, motion and sound are spent only on things that need the
   operator's attention, so that they still work when something goes wrong.
3. **The console is in the world.** There is no game UI layered over a ship UI. Menus,
   settings, connection state and errors are presented as ship systems, in the same
   language and the same components as everything else.
4. **One ship, one system.** Stations differ in content, never in grammar. A value, a
   caution, a button and a timer look and behave the same at Helm as at Engineering.
5. **Honest data.** Show the real simulated quantity with its unit, precision and age.
   Never show a number the simulation does not have, and never hide that a value is stale,
   estimated or missing.
6. **Learnable depth.** Flight-simulator detail is the goal, but depth comes from layered
   displays and consistent conventions, not from crowding a single screen.

## What to take from the references

| Reference                    | Take                                                                                                                                                                                                    | Leave                                                                                                 |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| NASA crew displays, cockpits | Reserved colour meanings, alert classes, number and time formats, stale-data marking, arm-then-execute commanding, fixed display frame                                                                  | Light backgrounds, dense 1990s widget chrome                                                          |
| _The Expanse_ (Rocinante)    | "Everything is there for a purpose", restrained palette with sparse accents, faint reference grids and orbit lines on spatial displays, an under-the-hood engineering tone, no giant flashing alert box | Holograms and gesture input, faction palettes that reuse red as decoration, legibility-for-camera art |
| _Starfield_ ("NASA-punk")    | Technology extrapolated from present-day hardware, thin rules and tick marks, stencilled labels with designators, flat matte surfaces, segmented power and level bars, off-white rather than pure white | Controller radial menus, large empty margins, decorative multi-colour stripes inside consoles         |

## Colour

Colour carries meaning first and style second. The status colours below are **reserved**:
never use them for decoration, branding, chart series or hover effects.

| Token               | Value     | Meaning                                                                         |
| ------------------- | --------- | ------------------------------------------------------------------------------- |
| `--surface-0`       | `#05080d` | Application background                                                          |
| `--surface-1`       | `#0b121c` | Panel                                                                           |
| `--surface-2`       | `#131e2d` | Inset field, raised or hovered control                                          |
| `--line`            | `#1c2a3a` | Decorative hairlines, grids, panel borders                                      |
| `--line-strong`     | `#557190` | Outlines of controls and input fields                                           |
| `--text`            | `#c8d6e5` | Values and primary text                                                         |
| `--text-muted`      | `#8a9db3` | Labels, units, secondary text                                                   |
| `--accent`          | `#5cc8e6` | Interactive, selected or available: focus rings, active tab, armed-ready states |
| `--status-nominal`  | `#4ade80` | Confirmed good state, only where the operator needs confirmation                |
| `--status-advisory` | `#60a5fa` | Advisory alert: awareness, no action required                                   |
| `--status-caution`  | `#fbbf24` | Caution alert: action required soon; limit exceeded; data overflow              |
| `--status-warning`  | `#f87171` | Warning and emergency alerts: act now                                           |
| `--target`          | `#e879f9` | Commanded or target values: setpoints, plotted course, autopilot targets        |

- Always use the tokens. Never write a literal colour in a component or stylesheet.
- Text and the parts of a symbol that carry meaning must reach a contrast of 6:1 against
  their surface, which is NASA's minimum; its preferred 10:1 is met by `--text`. Control
  outlines must reach 3:1. Every pairing in the table meets this on all three surfaces;
  check any new pairing.
- Never signal state by colour alone. Pair it with text, a symbol, a position or a shape.
- Do not paint nominal values green. Green is for a state the operator is waiting to
  confirm, such as `LINK NOMINAL` or `DOCKED`. A normal reading is plain `--text`.
- Red and yellow never appear unless an alert, a limit violation or a failed system is
  being reported.
- Solid status fills use `--surface-0` text on the status colour, never white.
- No gradients, glows, drop shadows, blurs or transparency effects on console chrome.
  Depth comes from the three surfaces and hairlines. The ban covers chrome: panels,
  controls, frames and backgrounds. A colour ramp that encodes data, such as a raster
  field under "Graphs, schematics and spatial displays", is not a gradient in this sense.
- The interface is dark only. Do not add a light theme.

## Typography

- Two families: **B612** for labels and prose, and **B612 Mono** for every number and
  table. They were designed for aircraft cockpit displays. Both are bundled through
  `@fontsource` and exposed as `--font-sans` and `--font-mono`. Never fetch a font at
  runtime.
- B612 covers Latin text and a small symbol set, including `°`, `µ`, `×`, `−`, `—`, `↑` and
  `↓`. Check that a new symbol exists in the font before using it, because a fallback glyph
  from the system font will not match. Draw anything else as an SVG icon.
- `☉` (U+2609) is in neither B612 nor B612 Mono. It is drawn as an inline SVG sized to the
  text, a circle with a centre dot in `currentColor`, and the accessible name "solar
  masses" goes on the unit as a whole. `⊕` (U+2295) is reserved, to be drawn the same way
  when planets need it. Neither character may be typed into a string that reaches the
  screen.
- The typeface must tell `0` from `O` and `1` from `l` and `I`.
- Numbers are always monospaced with tabular figures, so that a changing value never
  shifts its neighbours.
- **Upper case** for display titles, panel titles, field labels, button labels and status
  annunciations of three words or fewer, with `0.1em` to `0.2em` of letter spacing.
  **Mixed case** for everything that is a sentence: alert descriptions, procedures, logs,
  generated descriptions. This departs from NASA's all-mixed-case rule in favour of the
  annunciator convention the references share.
- No text smaller than `0.875rem`. Primary readouts are `1.125rem` or larger. These keep
  character height above NASA's 0.25° minimum at a normal desktop viewing distance.
- Use at most four sizes on one console. Weight is regular, with bold for display titles only. No italics, and no
  bold for emphasis in running text: emphasis on a console means an alert.
- Left-align text. Right-align numbers in columns so that decimal points line up.

## Numbers, units and time

- Every numeric value shows its unit, either beside the value or once for a labelled
  group. Units are SI, written with correct symbols and a space: `12.4 km/s`, `310 K`.
- Beside SI, stellar and system masses are in solar masses, `M☉` (drawn, see
  "Typography"), and ages are in `Myr` and `Gyr`. Universe time is in years, `yr`. Rates
  and densities compose allowed units: `°/Myr`, `/ly³`, `SYSTEMS/ly²`.
- A value outside its unit's ladder is written in E notation with three significant
  figures: `5.20E10 M☉`. A legend tick at an exact power of ten is written `1E-4`. B612 has
  no superscript digits beyond `¹`, `²` and `³` and no superscript minus, so `10⁻⁴` cannot be
  set, and long digit strings are not allowed.
- The same quantity uses the same unit and precision everywhere on the ship. Show the
  precision the operator can act on, not the precision the simulation has.
- Scale units rather than printing long numbers: `m`, `km`, `Mm`, `Gm`, then `AU` and `ly`.
  A value switches unit with hysteresis so that it does not flicker at a boundary.
- Write `0.25`, never `.25`. Suppress other leading zeros. Always show `-`. Show `+` only
  where direction matters, such as closure rate or delta-v remaining.
- Group digits in threes once a value has five or more digits: `12,480 km`.
- A field has a fixed width sized for its longest possible value and its status marks.
- Angles are degrees with a degree sign, `000°` to `359°` for bearings, zero-padded.
- Times use a 24-hour clock and always carry a label naming the time system:
  `MET 57/14:08:33` (days/hours:minutes:seconds), `UTC 14:08:33`. Countdown timers are
  negative before the event and positive after it: `T-00:04:12`, `T+00:00:30`.
- Universe time, the galaxy's own clock counted from the generator's epoch, is labelled `UT`
  and shown as signed years: `UT +12.50 yr`. `UT` never means Universal Time; the wall
  clock in the header strip is `UTC`.
- Present information in directly usable form. Never make the operator do arithmetic:
  show time to closest approach, not just range and closing speed.

## Layout

Every console is built on the same fixed frame:

1. **Header strip**, top. Ship name, station title, and the always-visible status area:
   labelled ship time, server link state, and the alert counts. It never scrolls away and
   looks identical at every station.
2. **Work area**, centre. Panels on a grid.
3. **Navigation bar**, bottom. The station's displays as tabs, in a fixed order.

- Each display has a unique title in the same place. Recurring fields keep the same
  relative position on every display that shows them.
- Arrange panels to follow the task: left to right, top to bottom, in procedure order.
  Put things that are used together next to each other.
- A display should contain what its task needs without visiting another display. Nothing
  is more than four actions deep.
- Consoles fill the window and do not scroll. Vertical scrolling is allowed only inside
  lists, logs and procedures, and must show position and total (`12-24 of 87`). Never
  scroll horizontally.
- Lay out on a `0.25rem` base unit. Panels are rectangles with a `1px` `--line` border and
  square or `2px` corners. A panel has an upper-case title and may carry a system
  designator (`EPS-2`, `RCS FWD`).
- Separate groups with space first and a hairline second. Do not box every value.
- Labels sit to the left of their value, or above it in columns. Labels are horizontal.
- Static labels, live values, editable fields, selectable controls and navigation must
  each be recognisable on sight. An operator should never have to click to find out what
  is clickable.
- A simulation, training or replay mode must look unmistakably different from live
  operation, through a persistent labelled banner in the header strip.
- Design for 1920×1080 first. Consoles must remain usable at 1280×720.

## Data states

A live value is always in exactly one of these states, and each looks the same everywhere.

| State            | Presentation                                                                                           |
| ---------------- | ------------------------------------------------------------------------------------------------------ |
| Nominal          | `--text`                                                                                               |
| Caution limit    | Value in `--status-caution`, followed by `↑` or `↓` for the limit that was crossed                     |
| Warning limit    | Value in `--status-warning`, followed by `↑` or `↓`                                                    |
| Off scale        | Pegged indicator plus `↑`/`↓`; gauges never wrap or clip silently                                      |
| Stale            | Last value in `--text-muted` with a trailing `S` mark; the age is available on request                 |
| Missing          | An em dash `—` in `--text-muted`. Never `0`, `NaN`, `null` or an empty field                           |
| Overflow         | Every digit replaced by `*` in `--status-caution`, keeping the sign and decimal point                  |
| Estimated        | Prefixed with `~`. Used for derived or sensor-limited values, such as the mass of an unscanned contact |
| Commanded/target | `--target`, shown beside the actual value rather than replacing it                                     |

- A value becomes stale when its source has not updated within twice its expected period.
  Loss of the server link makes every live value on the console stale at once.
- Elements with states or modes always show the current one. Automation always shows its
  level (`AUTO`, `MAN`, `INHIBITED`) and who is in control of a system.
- Historical data on a graph is labelled as such and is distinguishable from live data.

## Alerts

Four classes, shared by the whole ship:

| Class     | Colour              | Meaning                             | Presentation                                                  |
| --------- | ------------------- | ----------------------------------- | ------------------------------------------------------------- |
| Emergency | `--status-warning`  | Threat to the ship or crew: act now | Reverse video, fast flash until acknowledged, continuous tone |
| Warning   | `--status-warning`  | Imminent loss of a system: act now  | Reverse video, slow flash until acknowledged, repeating tone  |
| Caution   | `--status-caution`  | Degraded or out of limits: act soon | Coloured text and symbol, single tone                         |
| Advisory  | `--status-advisory` | Awareness only                      | Coloured text in the alert list, no sound                     |

- Alerts are raised by the server from simulation state. A console never invents one.
- The header strip shows the count of active emergency, warning and caution alerts and the
  number that are unacknowledged. The newest unacknowledged alert is shown in full.
- An alert names the system, says what is wrong and, where known, the cause and the
  action: `EPS-2 BUS B UNDERVOLT: shed load or start APU`. Each has a stable identifier,
  a timestamp and a link to its procedure when one exists.
- Every station has an alert list that can be sorted by priority, time and system, and
  from which an alert can be acknowledged. Acknowledging stops the flash and the tone. It
  never hides the alert, which stays until the condition clears.
- One action silences all audible annunciation.
- Reverse video and flashing are reserved for emergency and warning alerts. Nothing else
  may use them.
- Flash text slowly (0.8 Hz, 70% on) between full and reduced intensity so that it stays
  readable. Only small non-text indicators may flash fast (3 Hz, 50% on). Never flash a
  panel, a border around the screen or the whole display. All flashing on a console is
  synchronised, and stops after 10 seconds unless the alert is an unacknowledged
  emergency.
- No modal alert boxes and no screen-wide red overlays. The ship reports a hull breach in
  the same composed typographic voice as a filter change. The urgency comes from the
  class, the tone and the procedure.
- Route cautions to the station that owns the system. Emergencies appear at every station.

## Controls and commanding

- Controls that change the state of the ship must be visually distinct from controls that
  only change what the console displays, such as tabs, filters and zoom.
- Every button has a text label that is visible in every state. Icons are additions to a
  label, never a replacement, and come from one shared set.
- A control shows each of these states distinctly: enabled, hovered, focused, pressed,
  selected, disabled, and pending. A disabled control says why on hover or focus.
- Opposites come in congruent pairs in a consistent order: `ON`/`OFF`, `OPEN`/`CLOSE`,
  `ENABLE`/`INHIBIT`, `AUTO`/`MAN`. Command names must be hard to confuse with each other.
- Commanding is closed-loop. After a command the control shows `PENDING`, and then the
  result as reported by the server: accepted, rejected with a reason, or timed out. Never
  update the display optimistically as if the ship had already obeyed.
- Hazardous or irreversible commands (jettison, reactor scram, weapons release, jump)
  require two separate actions: `ARM`, then `EXECUTE`. A `SAFE` control that disarms sits
  to the right of or beneath `ARM`. An armed control is outlined with black-and-yellow
  hazard striping and disarms itself after a timeout. This striping is the only pattern
  fill in the interface.
- Before the operator overrides or shuts down an automated system, state the consequence.
- Data entry fields show their expected format and unit, reject invalid input with a
  message that says what is valid, right-align numbers and left-align text.
- Every console is fully operable from the keyboard. Frequent actions have single-key
  bindings shown on the control. Focus is always visible as a `2px` `--accent` outline.
- Pointer targets are at least `2rem` square. Consoles may run on touch screens, so no
  action may depend on hover or on a right click alone.

## Graphs, schematics and spatial displays

- A graph has a title above it, a label and unit on each axis, values at major ticks, and
  a label or legend for each series. Limit lines are drawn in the alert colour they
  trigger. Series are told apart by line style and label first and by colour second.
  Series colours are tints of `--text` and `--accent`, never the status colours.
- A schematic labels every component. Flow lines are solid for a single medium, and use
  labelled line styles when several media share a diagram. Crossing lines that connect
  have a dot. Crossing lines without a dot do not connect. Arrows show flow direction
  where the layout does not. System boundaries look different from flow lines.
- Spatial displays (tactical plot, orbit map, star chart) use thin vector lines on
  `--surface-0`, with a faint `--line` reference grid or range rings for scale. They always
  show scale, orientation and the reference frame. Predicted paths are dashed and
  commanded paths use `--target`. Contacts use a small fixed symbol set in which shape
  encodes the type and the label carries identity, so that colour stays free for status.
- Spatial displays are true to scale by default. Any exaggeration of size or distance
  made for visibility must be labelled on the display (`BODIES NOT TO SCALE`).
- A continuous field, such as column density or dust, may be drawn as a raster rather than
  in vector lines. It uses a single hue, from `--surface-0` to `--text`. The ramp is
  logarithmic when the data span more than two orders of magnitude, and the legend says
  which (`LOG SCALE`). The floor is stated, and values at or below it are exactly
  `--surface-0`. A legend with tick values and the unit is mandatory. Each pixel shows the
  value computed for it: no smoothing or interpolation that invents values.
- A three-dimensional spatial display (star chart, tactical plot, orbit map) follows one set
  of conventions, so that every such display reads alike:
  - The projection is orthographic only, so the whole picture has one scale.
  - The camera orbits the view centre by azimuth and elevation, with no roll. Presets
    `TOP`, `SIDE` and `FRONT` give axis-aligned views, and the default view is oblique.
  - A reference plane through the centre carries the `--line` grid and rings, and a stalk
    joins each mark to its foot on the plane.
  - A mark is filled above the plane and open below it, with the same outline. Fill means
    nothing else on a spatial display.
  - Size may encode a class, never depth, and a legend says so (`SYMBOLS NOT TO SCALE`).
    Nothing is dimmed by depth.
  - `--accent` marks what is available, such as a reachable system. A bracket reticle marks
    the selection, and the reticle in `--target` marks a commanded destination.
  - A range sphere's outline is a circle drawn at 6:1 contrast, and is labelled apart from
    the ring on the plane. The edge of the fetched data is drawn, so that unfetched space
    never looks empty.
  - The view always shows an axis triad, the azimuth and elevation, a 1-2-5 scale bar, the
    frame name, the centre and the time.
  - It redraws on demand only, never on a loop.
  - The canvas is paired with a DOM list of its marks, from which they are selected with the
    keyboard.
- Render fast-changing instruments on a canvas. Text that must be read stays in the DOM.

## Motion and sound

- Motion communicates a change of state and nothing else. No idle animation, ambient
  shimmer, boot sequences on navigation, typewriter text or animated backgrounds.
- State transitions take 80 to 150 ms with a simple ease-out. Displays switch instantly.
- Live values update in place without tweening the digits. Limit readouts to about 4 Hz
  so that they can be read, however fast the simulation runs. Gauges and plots may move
  at frame rate.
- Honour `prefers-reduced-motion`: drop transitions and replace flashing with steady
  reverse video.
- Sounds are short, dry and functional: control feedback, command results and the alert
  tones. Each alert class has one distinct tone. There is no interface music.

## Voice and nomenclature

- Write like a flight data file: terse, literal and impersonal. `NO CARRIER`, not
  "Oops, we lost the connection!". No exclamation marks, humour or second person.
- The ship's systems never speak as a person. No "I", no "please", no "thinking…".
- An error says what is wrong, what caused it if known, and what the operator can do.
- One name per thing. Systems, displays, commands and abbreviations come from a single
  ship-wide nomenclature list, and an abbreviation is used only if it is on that list.
- Real engineering and astronautical terms are preferred to invented ones: `DELTA-V`,
  `PERIAPSIS`, `RCS`, `EPS`. Invented technology is named in the same register.
- Directions in the galaxy are named for the galaxy itself: `COREWARD` and `RIMWARD`
  (towards and away from the galactic axis), `SPINWARD` and `ANTISPINWARD` (with and
  against the direction of rotation), `NORTH` and `SOUTH` (+z, from which the galaxy
  rotates counter-clockwise, and −z). They are local to a point, and undefined on the axis.
- The galaxy-wide reference frame is named `GALACTIC`. Its coordinates are given as
  `RADIUS` (distance from the axis), `ANGLE` (from +x, counter-clockwise seen from the
  north) and `HEIGHT` (along +z).

### Nomenclature list

| Name        | Kind         | Meaning                                                  |
| ----------- | ------------ | -------------------------------------------------------- |
| `LINK`      | Display      | The server link: endpoint, versions, latency             |
| `GALAXY`    | Display      | The universe, its galaxy parameters, map and local chart |
| `AZM`       | Abbreviation | Azimuth of a spatial display's camera                    |
| `ELV`       | Abbreviation | Elevation of a spatial display's camera                  |
| `DESIG`     | Abbreviation | Designation                                              |
| `DIST`      | Abbreviation | Distance                                                 |
| `ID`        | Abbreviation | Identifier: a system's or universe's 16-digit hex ID     |
| `INIT MASS` | Abbreviation | Initial mass                                             |
| `MIN MASS`  | Abbreviation | Minimum mass: the lowest initial mass a query includes   |
| `GEN VER`   | Abbreviation | Generator version                                        |
| `EXP`       | Abbreviation | Expected count                                           |
| `RET`       | Abbreviation | Returned count                                           |
| `UT`        | Time system  | Universe time, from the generator's epoch                |

## Accessibility

- Follow the markup rules in `.claude/rules/typescript-dev.md`: native elements first,
  accessible names on every control, live values in `output` or an ARIA live region.
  Alerts use `role="alert"` for emergency and warning and `role="status"` for the rest.
- The contrast, colour-independence, keyboard, target-size and reduced-motion rules above
  are requirements, not enhancements.
- No content flashes more than three times a second, and no flashing area is large.
- Interface scale is adjustable from 80% to 150% without breaking a console's layout, so
  size everything in `rem`.

## Never

- Decorative data: random numbers, fake code, waveforms that measure nothing.
- Holograms, glassmorphism, neon glow, scanlines, CRT curvature, chromatic aberration.
- A status colour used as an accent, or an accent used as a status.
- A number without a unit, a time without a time system, a button without a label.
- Optimistic command feedback, or a stale value shown as live.
- Modal dialogs that block the console for anything short of an `ARM`/`EXECUTE` decision.
- Web or game conventions that break the fiction: toasts, hamburger menus, spinners,
  skeleton loaders, emoji, achievement popups, "Loading…".

## Sources

- NASA-STD-3001 Volume 2, [Appendix F: Display Standard][nasa-f], for colour meanings,
  alert classes, number and time formats, data states, flashing, commanding and layout.
- [WCAG 2.2][wcag] for non-text contrast, flash thresholds and target size.
- Airbus and Intactile, [B612][b612], the open cockpit display typeface.
- Rhys Yorke on _The Expanse_ screen graphics: [Pushing Pixels interview][yorke],
  [HUDS+GUIS][hudsguis].
- Bethesda on _Starfield_'s "NASA-punk" direction: [Istvan Pely interview][pely].
- The dark-cockpit philosophy and magenta-for-commanded-values are glass-cockpit
  conventions rather than a NASA requirement.

[nasa-f]: https://www.nasa.gov/reference/appendix-f-vol-2/
[wcag]: https://www.w3.org/TR/WCAG22/
[b612]: https://b612-font.com/
[yorke]: https://www.pushing-pixels.org/2021/09/04/the-art-and-craft-of-screen-graphics-interview-with-rhys-yorke.html
[hudsguis]: https://www.hudsandguis.com/home/2021/theexpanse
[pely]: https://hypebeast.com/2023/8/starfield-art-launch-interview-istvan-pely-bethesda
