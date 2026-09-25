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

| Reference                    | Take                                                                                                                                                                                                         | Leave                                                                                                 |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------- |
| NASA crew displays, cockpits | Reserved colour meanings, alert classes, number and time formats, stale-data marking, arm-then-execute commanding, fixed display frame                                                                       | Light backgrounds, dense 1990s widget chrome                                                          |
| _The Expanse_ (Rocinante)    | "Everything is there for a purpose", restrained palette with sparse accents, faint reference grids and thin orbit lines on spatial displays, an under-the-hood engineering tone, no giant flashing alert box | Holograms and gesture input, faction palettes that reuse red as decoration, legibility-for-camera art |
| _Starfield_ ("NASA-punk")    | Technology extrapolated from present-day hardware, thin rules and tick marks, stencilled labels with designators, flat matte surfaces, segmented power and level bars, off-white rather than pure white      | Controller radial menus, large empty margins, decorative multi-colour stripes inside consoles         |

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
- B612 covers Latin text and a small symbol set, including `°`, `µ`, `×`, `·`, `−`, `—`, `↑`
  and `↓`. Check that a new symbol exists in the font before using it, because a fallback glyph
  from the system font will not match. Draw anything else as an SVG icon.
- `☉` (U+2609) is in neither B612 nor B612 Mono. It is drawn as an inline SVG sized to the
  text, a circle with a centre dot in `currentColor`, and the accessible name "solar
  masses" goes on the unit as a whole. `⊕` (U+2295) is drawn the same way, a circle with a
  cross in `currentColor`, and the accessible name "Earth masses" goes on the unit as a whole.
  Neither character may be typed into a string that reaches the screen.
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
  "Typography"), and ages are in `kyr`, `Myr` and `Gyr`. Universe time is in years, `yr`. Rates
  and densities compose allowed units: `°/Myr`, `/ly³`, `SYSTEMS/ly²`.
- Gas and dust take the astronomers' units: extinction and other magnitudes in `mag`, column
  density in `/cm²`, number density in `/cm³`, and pressure over Boltzmann's constant in `K/cm³`.
  They are composed as the rates are, because `cm⁻²` cannot be set.
- Stars also take the astronomers' units of luminosity and radius, `L☉` and `R☉`, with the `☉`
  drawn (see "Typography"); their accessible names are "solar luminosities" and "solar radii". A
  neutron star's or black hole's radius is in `km`. Metallicity, `[Fe/H]`, is in `dex`: the
  base-10 logarithm of a star's iron-to-hydrogen ratio over the Sun's. As a label, `[Fe/H]` keeps
  its case, as unit symbols do. Magnetic fields are in gauss, `G`, `kG` and `MG`.
- Every planetary mass, from a moon to a giant of 13 Jupiter masses (`4131 M⊕`), is in Earth
  masses, `M⊕`, with the `⊕` drawn (see "Typography"). A brown dwarf's mass is in `M☉`, the unit
  of the stellar sequence it continues. There is no Jupiter-mass unit. Planetary radii are in `km`.
- Orbital, rotation and pulsation periods are in days, `d`, up to 1000 d, and in `yr` above.
  Distances within a system run `km`, `Mm` and `Gm`, then `AU` from 0.1 AU. Both switch with
  hysteresis, as any scaled unit does.
- A value outside its unit's ladder is written in E notation with three significant
  figures: `5.20E10 M☉`. A legend tick at an exact power of ten is written `1E-4`. B612 has
  no superscript digits beyond `¹`, `²` and `³` and no superscript minus, so `10⁻⁴` cannot be
  set, and long digit strings are not allowed. `<sup>` and `<sub>` are not used either: they
  set text smaller than the smallest size allowed. A symbol's subscript is written in
  parentheses: `A(V)`, `E(B-V)`, `N(H)`, `M(V)`.
- The same quantity uses the same unit and precision everywhere on the ship. Show the
  precision the operator can act on, not the precision the simulation has.
- Scale units rather than printing long numbers: `m`, `km`, `Mm`, `Gm`, then `AU` and `ly`.
  A value switches unit with hysteresis so that it does not flicker at a boundary.
- Write `0.25`, never `.25`. Suppress other leading zeros. Always show `-`. Show `+` only
  where direction matters, such as closure rate or delta-v remaining.
- Group digits in threes once a value has five or more digits: `12,480 km`.
- A field has a fixed width sized for its longest possible value and its status marks.
- Angles are degrees with a degree sign, `000°` to `359°` for bearings, zero-padded.
- Fluxes are in `W/m²`, and small fluxes in E notation: `3.20E-12 W/m²`.
- A direction from the ship is an azimuth and a signed elevation, `047° +12°`. The azimuth follows
  the bearing rule and runs from `COREWARD` through `SPINWARD`. The elevation runs from `-90°` to
  `+90°`, positive `NORTH`, zero-padded to two digits and always signed. Within a light-year of
  the galactic axis, where `COREWARD` is undefined, the azimuth runs from +x and the direction
  says so (`FROM +X`).
- Times use a 24-hour clock and always carry a label naming the time system:
  `MET 57/14:08:33` (days/hours:minutes:seconds), `UTC 14:08:33`. Countdown timers are
  negative before the event and positive after it: `T-00:04:12`, `T+00:00:30`.
- Universe time, the galaxy's own clock counted from the generator's epoch, is labelled `UT`
  and shown as signed years: `UT +12.50 yr`. `UT` never means Universal Time; the wall
  clock in the header strip is `UTC`.
- A display that can be stepped away from the chart's time shows its own time, labelled
  `DISPLAY TIME`. It is universe time and not the ship's clock. It is shown as signed whole years,
  then days/hours:minutes:seconds in the `MET` form: `DISPLAY TIME UT +12 yr 183/14:08:33`. The
  year is the Julian year of 365.25 days, so the day runs from `000` to `365`. The sign applies to
  the whole time: `UT -0 yr 000/00:00:01` is one second before the epoch. The field is sized for
  `UT -1000 yr 000/00:00:00`, the edge of the clock window. A display time never moves unless the
  operator steps it or runs it. At the clock window's edge it stops and says `CLOCK WINDOW LIMIT`.
  This long form is used only on a display that steps finer than 0.01 yr; every other display
  shows universe time as `UT +12.50 yr`.
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
  lists, logs, procedures and readouts, which are lists of readings, and must show position and
  total (`12-24 of 87`). Never scroll horizontally.
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

| State            | Presentation                                                                                                                                     |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Nominal          | `--text`                                                                                                                                         |
| Caution limit    | Value in `--status-caution`, followed by `↑` or `↓` for the limit that was crossed                                                               |
| Warning limit    | Value in `--status-warning`, followed by `↑` or `↓`                                                                                              |
| Off scale        | Pegged indicator plus `↑`/`↓`; gauges never wrap or clip silently                                                                                |
| Stale            | Last value in `--text-muted` with a trailing `S` mark; the age is available on request                                                           |
| Missing          | An em dash `—` in `--text-muted`. Never `0`, `NaN`, `null` or an empty field                                                                     |
| Overflow         | Every digit replaced by `*` in `--status-caution`, keeping the sign and decimal point                                                            |
| Estimated        | Prefixed with `~`. Used for derived or sensor-limited values, such as the mass of an unscanned contact                                           |
| Observed         | The value as its light shows it, with its light age available beside it (`LIGHT AGE 4210 yr`); a present value extrapolated from it is Estimated |
| Commanded/target | `--target`, shown beside the actual value rather than replacing it                                                                               |

- A value becomes stale when its source has not updated within twice its expected period.
  Loss of the server link makes every live value on the console stale at once.
- Elements with states or modes always show the current one. Automation always shows its
  level (`AUTO`, `MAN`, `INHIBITED`) and who is in control of a system.
- Historical data on a graph is labelled as such and is distinguishable from live data.
- A readout's sections carry one of four states, which the server sets and the console never
  infers, and each looks the same everywhere:

  | State            | Presentation                                                                                        |
  | ---------------- | --------------------------------------------------------------------------------------------------- |
  | Shown            | The section's values, each in its own data state                                                    |
  | Not resolved     | `NOT RESOLVED`, once, in place of the section: the detail level granted to the console withholds it |
  | Not yet modelled | `NOT YET MODELLED`, once, in place of the section: this generator version does not compute it       |
  | Not applicable   | Nothing: the section and its rows are left out, as a gas giant has no surface section               |
  - `NOT YET MODELLED` must never be read as "none", so a display whose empty space could be read
    as absence also says what is not modelled, composed from the same states:
    `MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED`. That note is a label followed by
    a three-word annunciation, so it is in upper case.
  - "None" is a value, not a state: an airless world's atmosphere is shown, with no gas in it.
  - A single value the generator does not compute, inside a section that is shown, is Missing: the
    em dash. So is an empty cell in a table column.

## Alerts

Four classes, shared by the whole ship:

| Class     | Colour              | Meaning                             | Presentation                                                  |
| --------- | ------------------- | ----------------------------------- | ------------------------------------------------------------- |
| Emergency | `--status-warning`  | Threat to the ship or crew: act now | Reverse video, fast flash until acknowledged, continuous tone |
| Warning   | `--status-warning`  | Imminent loss of a system: act now  | Reverse video, slow flash until acknowledged, repeating tone  |
| Caution   | `--status-caution`  | Degraded or out of limits: act soon | Coloured text and symbol, single tone                         |
| Advisory  | `--status-advisory` | Awareness only                      | Coloured text in the alert list, no sound                     |

- Alerts are raised by the server from simulation state. A console never invents one.
- A sensor detection is an Advisory alert raised by the server. It names the sensor system, the
  kind of event, its direction and whether its source is resolved:
  `SENSORS TRANSIENT 047° +12°: unresolved`. It clears when the event's flux at the ship falls
  below the sensor's detection limit.
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
- A scatter plot may run an axis in reverse where its science does: the HR diagram plots
  temperature falling from left to right. A logarithmic axis says `LOG SCALE`, and its ticks at
  exact powers of ten follow the E-notation rule (`1E-4 L☉`). A point beyond an axis is pegged at
  its edge with a drawn arrowhead, and the caption counts the pegged points and those not plotted.
- A schematic labels every component. Flow lines are solid for a single medium, and use
  labelled line styles when several media share a diagram. Crossing lines that connect
  have a dot. Crossing lines without a dot do not connect. Arrows show flow direction
  where the layout does not. System boundaries look different from flow lines.
- Spatial displays (tactical plot, orbit map, star chart) use thin vector lines on
  `--surface-0`, with a faint `--line` reference grid or range rings for scale. They always
  show scale, orientation and the reference frame. Predicted paths are dashed and
  commanded paths use `--target`. Contacts use a small fixed symbol set in which shape
  encodes the type and the label carries identity, so that colour stays free for status. A
  feature is the exception: its shape tells it from a system or body, and its kind is named
  beside the mark (see below).
- Spatial displays are true to scale by default. Any exaggeration of size or distance
  made for visibility must be labelled on the display (`BODIES NOT TO SCALE`).
- A bearing without a range is a solid ray from the observer, ending at the display's edge with
  its label. An apparent position, where the light shows a system, is a tick joined to its present
  position by a dashed line, since dashes mean prediction. The display's mode, `NOW` or
  `OBSERVED FROM`, is shown with its frame and time.
- A continuous field, such as column density or dust, may be drawn as a raster rather than
  in vector lines. It uses a single hue, from `--surface-0` to `--text`. The ramp is
  logarithmic when the data span more than two orders of magnitude, and the legend says
  which (`LOG SCALE`). The floor is stated, and values at or below it are exactly
  `--surface-0`. A legend with tick values and the unit is mandatory. Each pixel shows the
  value computed for it: no smoothing or interpolation that invents values.
- A display may offer a second raster quantity in place of the first, such as extinction in
  place of column density. Each has its own legend with its title, unit, scale and floor. One
  raster may modify another only as a named overlay. Both legends are then shown, and the display
  names the overlay and its quantity: `DUST OVERLAY: A(V), WHOLE LINE OF SIGHT`. A mark or line
  drawn over a raster has a `1px` `--surface-0` casing on each side, so that it reads over every
  ramp value. A casing is a solid outline, never a blur or a glow.
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
  - The legend names every symbol the display can draw, in words. A filter that hides marks
    hides them from the list as well, and the count line gives what is shown, the total and the
    filter: `412 OF 1630 SHOWN: LIVING`.
  - `--accent` marks what is available, such as a reachable system. A bracket reticle marks
    the selection, and the reticle in `--target` marks a commanded destination.
  - A range sphere's outline is a circle drawn at 6:1 contrast, and is labelled apart from
    the ring on the plane. The edge of the fetched data is drawn, so that unfetched space
    never looks empty.
  - The view always shows an axis triad, the azimuth and elevation, a 1-2-5 scale bar, the
    frame name, the centre and the time.
  - It redraws only when something it shows changes, never on an idle loop. While the operator
    runs its time, it redraws at frame rate and stops when the time is held; its time readout
    still updates at about 4 Hz, as any live value does.
  - The canvas is paired with a DOM list of its marks, from which they are selected with the
    keyboard.
- The orbit map, the three-dimensional spatial display of one system, keeps those conventions and
  adds these:
  - Its reference plane is the system's own, labelled `SYSTEM PLANE`: the orbital plane of the
    primary host's planets, or for a close binary the binary's. Until a system has planets, the
    plane is `GALACTIC PLANE`. The grid, rings, stalks, fill rule and the `TOP`, `SIDE` and
    `FRONT` presets follow that plane, and the legend names it (`FILLED ABOVE SYSTEM PLANE`). The
    axis triad and the core arrow still point galactic `NORTH`, `COREWARD` and `SPINWARD`. The
    frame is `SYSTEM BARYCENTRIC`, or `BODY <designation>` while a body is focused with
    `FOCUS BODY`, in which its moons and rings are drawn and distances are in `km` and `Mm`.
  - Orbits are solid `--text-muted` ellipses. They say where a body lies, so they meet the 6:1 that
    the parts of a symbol carrying meaning need; they are reference marks and not predictions, so
    they are never dashed. The selected body's orbit is a solid `--text` line `2px` wide, because
    colour alone must not carry the selection. An orbit in `--text-muted` is not stale:
    staleness is carried by the `S` and the view's stale marking, never by a line's colour.
  - Stable zones, the snow line and the habitable zone are labelled annuli, drawn as their two
    edges in solid `--text-muted`, and each can be switched off. The optimistic habitable zone's
    edges carry short ticks every 10° pointing into the band, so that it differs from the
    conservative zone by shape.
  - A belt, and a planet's rings in the `BODY` frame, are annuli whose two edges are joined by
    short radial ticks every 10°. The cometary halo is one labelled circle at its outer radius,
    drawn only when it is inside the view. No annulus is filled, hatched or dotted.
  - `--line` is for the reference plane's grid and scale rings only.
  - Distances are true to scale, with a 1-2-5 scale bar. Bodies are not, and the display says
    `BODIES NOT TO SCALE` once, in place of the legend's `SYMBOLS NOT TO SCALE`.
  - Bodies move only when the display time changes, never on their own, so a held map is still.
  - The zoom presets `INNER`, `ALL` and `BELTS` fit the outer limit of the habitable zone or the
    fifth body, the outermost planet, and the outermost belt. Zooming by hand releases a preset.
- The ship-wide symbol set. Every outline is closed, so that it can be filled above the reference
  plane and drawn open below it. The list and readout name every kind in words, so shape is never
  the only signal.

  | Symbol            | Outline                                                      | Meaning                                                                                                             |
  | ----------------- | ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- |
  | Circle            | A circle                                                     | A protostar, pre-main-sequence star, dwarf, subgiant or hot subdwarf, or a brown dwarf                              |
  | Ringed circle     | A circle inside an outer ring; only the inner disc is filled | A giant, supergiant or Wolf-Rayet star                                                                              |
  | Diamond           | A square on its corner                                       | A white dwarf                                                                                                       |
  | Triangle          | Point up                                                     | A neutron star                                                                                                      |
  | Square            | A square                                                     | A black hole                                                                                                        |
  | Inverted triangle | Point down                                                   | A planet, bound or free-floating; its size tells a giant from a planet from a dwarf planet                          |
  | Pentagon          | A regular pentagon, point up                                 | A moon                                                                                                              |
  | Hexagon           | A regular hexagon, flat at top and bottom                    | An unresolved contact: a body whose kind the sensors have not resolved                                              |
  | Four-point star   | A closed four-pointed star                                   | A transient: the source of a sensor detection                                                                       |
  | Greek cross       | A cross of four equal arms                                   | A feature: a group of systems or a cloud of gas, its kind named beside it; shells and superbubbles are circles only |

  A system whose star left no remnant is listed and not drawn. A host keeps its symbol and its
  mass layer's size on every display.

- A feature, a group of systems or a cloud of gas, is marked by one outline that no system or
  body uses, a Greek cross, and its kind is named by an abbreviation beside the mark and in words
  in the list and readout. A feature wider than its mark is also drawn at its true radius as a
  solid `--text-muted` circle. So are a remnant shell and a superbubble, which are labelled at the
  circle and have no mark of their own. On a display without a reference plane, such as the
  galaxy map, feature marks are drawn open. A feature layer still being computed says
  `FEATURES PENDING`, so that unfetched features never look absent.
- A stellar stream's track is a solid `1px` `--text-muted` line, labelled at one end, cased as
  any line over a raster is. A dwarf core takes the feature mark.
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
- A reading made of parts, such as a system's origin, joins them with a middle dot `·`.
- Real engineering and astronautical terms are preferred to invented ones: `DELTA-V`,
  `PERIAPSIS`, `RCS`, `EPS`. Invented technology is named in the same register.
- Directions in the galaxy are named for the galaxy itself: `COREWARD` and `RIMWARD`
  (towards and away from the galactic axis), `SPINWARD` and `ANTISPINWARD` (with and
  against the direction of rotation), `NORTH` and `SOUTH` (+z, from which the galaxy
  rotates counter-clockwise, and −z). They are local to a point, and undefined on the axis.
- The galaxy-wide reference frame is named `GALACTIC`. Its coordinates are given as
  `RADIUS` (distance from the axis), `ANGLE` (from +x, counter-clockwise seen from the
  north) and `HEIGHT` (along +z). A readout groups the three under the heading `GALACTIC`.

### Nomenclature list

| Name                                                                   | Kind                   | Meaning                                                                                                                                                                                                                                  |
| ---------------------------------------------------------------------- | ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `LINK`                                                                 | Display                | The server link: endpoint, versions, latency                                                                                                                                                                                             |
| `GALAXY`                                                               | Display                | The universe, its galaxy parameters, map and local chart                                                                                                                                                                                 |
| `SYSTEM`                                                               | Display                | One system: its hosts and bodies, the orbit map, the body list and readout, and the display time                                                                                                                                         |
| `AZM`                                                                  | Abbreviation           | Azimuth of a spatial display's camera                                                                                                                                                                                                    |
| `ELV`                                                                  | Abbreviation           | Elevation of a spatial display's camera                                                                                                                                                                                                  |
| `DESIG`                                                                | Abbreviation           | Designation                                                                                                                                                                                                                              |
| `DIST`                                                                 | Abbreviation           | Distance                                                                                                                                                                                                                                 |
| `ID`                                                                   | Abbreviation           | Identifier: a system's or universe's 16-digit hex ID                                                                                                                                                                                     |
| `INIT MASS`                                                            | Abbreviation           | Initial mass                                                                                                                                                                                                                             |
| `MIN MASS`                                                             | Abbreviation           | Minimum mass: the lowest initial mass a query includes                                                                                                                                                                                   |
| `GEN VER`                                                              | Abbreviation           | Generator version                                                                                                                                                                                                                        |
| `EXP`                                                                  | Abbreviation           | Expected count                                                                                                                                                                                                                           |
| `RET`                                                                  | Abbreviation           | Returned count                                                                                                                                                                                                                           |
| `UT`                                                                   | Time system            | Universe time, from the generator's epoch                                                                                                                                                                                                |
| `DISPLAY TIME`                                                         | Time system            | A display's own universe time, stepped by the operator; not the ship's clock                                                                                                                                                             |
| `CLOCK WINDOW LIMIT`                                                   | Status                 | The display time has reached the edge of the clock window, 1000 yr either side of the epoch                                                                                                                                              |
| `DRIVE RANGE`                                                          | Setting                | The range a chart counts systems within. `RANGE` on the chart's curve labels, `PLANE` for its trace in the galactic plane, `SET` after every drawn value; `IN RANGE` / `OUT OF RANGE` (`OUT` in the system list) for a system's standing |
| `STARS`                                                                | Filter                 | Which systems the chart shows and lists: `ALL`, `LIVING` or `REMNANTS`                                                                                                                                                                   |
| `LIVING`                                                               | Filter                 | Systems whose brightest star has not yet died                                                                                                                                                                                            |
| `REMNANTS`                                                             | Filter, map population | Systems whose brightest object is a white dwarf, neutron star or black hole; as the map's third population, the remnants of stars born above 8 M☉, which its legend states (`INIT MASS ABOVE 8 M☉`)                                      |
| `OPEN SYSTEM`                                                          | Command                | Opens the `SYSTEM` display on the system selected on the `GALAXY` display, at the chart's time                                                                                                                                           |
| `FOCUS BODY`                                                           | Command                | Centres the orbit map on the selected planet, in its `BODY` frame                                                                                                                                                                        |
| `SYSTEM PLANE`                                                         | Reference plane        | The orbit map's reference plane: the primary host's planetary plane, or a close binary's orbit                                                                                                                                           |
| `GALACTIC PLANE`                                                       | Reference plane        | The orbit map's plane before a system has planets                                                                                                                                                                                        |
| `SYSTEM BARYCENTRIC`                                                   | Frame                  | A system's frame, centred on its barycentre, with galactic axes                                                                                                                                                                          |
| `BODY`                                                                 | Frame                  | `BODY <designation>`: a focused body's frame, in which its moons and rings are drawn                                                                                                                                                     |
| `INNER`                                                                | Preset                 | Orbit map zoom to the habitable zone's outer limit or the fifth body                                                                                                                                                                     |
| `ALL`                                                                  | Preset                 | Orbit map zoom to the outermost planet                                                                                                                                                                                                   |
| `BELTS`                                                                | Preset                 | Orbit map zoom to the outermost belt                                                                                                                                                                                                     |
| `BODIES NOT TO SCALE`                                                  | Label                  | Body symbols are drawn larger than the bodies; distances are to scale                                                                                                                                                                    |
| `RINGS`                                                                | Label                  | A planet's ring system                                                                                                                                                                                                                   |
| `COMETARY HALO`                                                        | Label                  | A system's outer cloud of comets, drawn as a circle at its outer radius                                                                                                                                                                  |
| `OPTIMISTIC`                                                           | Label                  | The optimistic habitable zone, recent Venus to early Mars (Kopparapu et al. 2013), beside `HABITABLE ZONE`, the conservative one: the orbit map's toggle and annulus label, and the host readout's row                                   |
| `READINGS`                                                             | Region                 | The selected body's readout as a scrolling region, its position and total under it (`1-13 of 26`)                                                                                                                                        |
| `NOT RESOLVED`                                                         | Data state             | A section the granted detail level withholds                                                                                                                                                                                             |
| `NOT YET MODELLED`                                                     | Data state             | A quantity this generator version does not compute; never "none"                                                                                                                                                                         |
| `NOT YET FORMED`                                                       | State                  | A system, star or planet not yet born at the time shown                                                                                                                                                                                  |
| `CONTACT ONLY`, `MASS AND ORBIT ONLY`, `TO BULK`, `TO SURFACE`, `FULL` | Detail level           | What the granted detail level shows of a body                                                                                                                                                                                            |
| `DETAIL`                                                               | Label                  | The granted detail level                                                                                                                                                                                                                 |
| `ARCH`                                                                 | Label                  | A host's architecture class                                                                                                                                                                                                              |
| `SINCE`                                                                | Label                  | When a body's state began (`DESTROYED`, `UNBOUND`)                                                                                                                                                                                       |
| `PARENT`                                                               | Label                  | What a body orbits: a star, a pair, the barycentre or a planet                                                                                                                                                                           |
| `PAIR /0 /1`                                                           | Designation            | A pair of stars, by their body indices                                                                                                                                                                                                   |
| `COLLIDED`                                                             | Cause                  | The body collided with a heavier neighbour whose orbit its own crossed after a supernova, and merged into it                                                                                                                             |
| `SMA`                                                                  | Abbreviation           | Semi-major axis                                                                                                                                                                                                                          |
| `ECC`                                                                  | Abbreviation           | Eccentricity                                                                                                                                                                                                                             |
| `INC`                                                                  | Abbreviation           | Inclination                                                                                                                                                                                                                              |
| `T EQ`                                                                 | Abbreviation           | Equilibrium temperature                                                                                                                                                                                                                  |
| `T EFF`                                                                | Abbreviation           | Effective temperature                                                                                                                                                                                                                    |
| `LUM`                                                                  | Abbreviation           | Luminosity                                                                                                                                                                                                                               |
| `KICK`                                                                 | Label                  | A remnant's natal kick speed                                                                                                                                                                                                             |
| `NEBULA RADIUS`                                                        | Label                  | The radius of a star's planetary nebula                                                                                                                                                                                                  |
| `M(V)`                                                                 | Abbreviation           | Absolute visual magnitude, in `mag`                                                                                                                                                                                                      |
| `WD`                                                                   | Abbreviation           | White dwarf, in census lines (`ACCRETING WD PENDING`)                                                                                                                                                                                    |
| `VEL`                                                                  | Abbreviation           | Velocity, with its `COREWARD`, `SPINWARD` and `NORTH` components                                                                                                                                                                         |
| `ORIGIN`                                                               | Label                  | Where and how an object formed: a system's birth population and placement (`OLD THIN DISC · DISPLACED REMNANT`) or feature (`GC 47 TUC · FIELD`), or a moon's origin (`CAPTURED`)                                                        |
| `FIELD`                                                                | Placement              | A system where its population places it                                                                                                                                                                                                  |
| `RETAINED`                                                             | Placement              | A remnant that stayed with its population after its kick                                                                                                                                                                                 |
| `DISPLACED REMNANT`                                                    | Placement              | A remnant whose kick carried it out of its population's volume                                                                                                                                                                           |
| `RUNAWAY`                                                              | Placement              | A star ejected fast, by a supernova in its binary or an encounter                                                                                                                                                                        |
| `WALKAWAY`                                                             | Placement              | A star ejected slowly, as a runaway is                                                                                                                                                                                                   |
| `EXTINCTION`                                                           | Map quantity           | Dimming by dust, as the galaxy map's second raster quantity                                                                                                                                                                              |
| `DUST OVERLAY`                                                         | Map overlay            | The column-density map dimmed by the extinction along each pixel                                                                                                                                                                         |
| `A(V)`                                                                 | Abbreviation           | Extinction in the visual band, in `mag`                                                                                                                                                                                                  |
| `A(K)`                                                                 | Abbreviation           | Extinction in the K band, in `mag`                                                                                                                                                                                                       |
| `E(B-V)`                                                               | Abbreviation           | Colour excess, B band minus V band, in `mag`                                                                                                                                                                                             |
| `N(H)`                                                                 | Abbreviation           | Hydrogen column density, in `/cm²`                                                                                                                                                                                                       |
| `FEATURES`                                                             | Map overlay            | The galaxy map's globulars, centre and bright shells, each kind switchable                                                                                                                                                               |
| `GC`                                                                   | Abbreviation           | Globular cluster                                                                                                                                                                                                                         |
| `OC`                                                                   | Abbreviation           | Open cluster                                                                                                                                                                                                                             |
| `OB`                                                                   | Abbreviation           | OB association                                                                                                                                                                                                                           |
| `SFR`                                                                  | Abbreviation           | Star-forming region                                                                                                                                                                                                                      |
| `MC`                                                                   | Abbreviation           | Molecular cloud                                                                                                                                                                                                                          |
| `CENTRE`                                                               | Feature                | The galactic centre and its black hole                                                                                                                                                                                                   |
| `SUPERBUBBLE`                                                          | Feature                | A cavity blown by many supernovae, drawn at its true radius                                                                                                                                                                              |
| `STREAM`                                                               | Feature                | A tidal stream of stars stripped from a globular or a dwarf galaxy                                                                                                                                                                       |
| `DWARF CORE`                                                           | Feature                | The surviving core of a dwarf galaxy                                                                                                                                                                                                     |
| `H II`                                                                 | Abbreviation           | Emission class: ionised hydrogen region                                                                                                                                                                                                  |
| `SNR`                                                                  | Abbreviation           | Emission class: supernova remnant shell                                                                                                                                                                                                  |
| `PWN`                                                                  | Abbreviation           | Emission class: pulsar wind nebula                                                                                                                                                                                                       |
| `REFLECTION`                                                           | Emission class         | Reflection nebula                                                                                                                                                                                                                        |
| `DARK CLOUD`                                                           | Emission class         | Dark cloud                                                                                                                                                                                                                               |
| `OBS`                                                                  | Abbreviation           | Observed: as the light arriving now shows it                                                                                                                                                                                             |
| `LIGHT AGE`                                                            | Label                  | How long the light from a thing has travelled to the observer                                                                                                                                                                            |
| `TRANSIENT`                                                            | Event kind             | A detected event, such as a nova, whose source may not yet be resolved                                                                                                                                                                   |
| `NOW`                                                                  | Mode                   | A display showing present positions                                                                                                                                                                                                      |
| `OBSERVED FROM`                                                        | Mode                   | A display showing what an observer's light shows, followed by the observer                                                                                                                                                               |
| `AS OBSERVED`                                                          | State                  | A readout showing a system as its light shows it                                                                                                                                                                                         |

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
