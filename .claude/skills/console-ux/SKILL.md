---
name: console-ux
description: Builds and checks HYPERION bridge-console UI to the project's flight-hardware standard in docs/frontend/ux-guidelines.md. It covers colour tokens and reserved status colours, B612 typography and glyph coverage, units and number and time formats, data states, alerts, commanding, spatial displays, motion and voice, with lint, contrast and glyph-coverage scripts. Use whenever writing or changing React components, CSS, displayed strings, value formatting or canvas drawing in apps/hyperion/src/renderer, even for a small UI tweak.
paths:
  - "apps/hyperion/src/renderer/**"
  - "docs/frontend/ux-guidelines.md"
---

# Console UX

HYPERION's consoles should look like flight hardware that an agency certified, not like a game
menu. The usual instincts for web and "sci-fi" UI break this standard: glows, gradients,
spinners, toasts, green for OK, colour-coding everything. `docs/frontend/ux-guidelines.md` is the
standard. This skill is how to apply it and check the result.

## Before building

Read the guide in full the first time you do UI work in a session; every rule carries weight.
Then look at how the existing code does things, and reuse its pieces rather than inventing
parallel ones:

- `styles.css` holds the tokens (`:root`) and the shared classes: `.console*` (the fixed frame),
  `.panel` and `.panel__title`, `.field`, `.readout` with `.readout__missing`, `.annunciator`.
- `components/ConsoleFrame.tsx` is the header strip, work area and navigation bar.
  `LinkStatus.tsx` is an annunciator that pairs state with text. `UtcClock.tsx` is a labelled
  time. `ConnectionPanel.tsx` is a panel of readouts.

## Where changes usually go wrong

Each item below is in the guide. They are listed here because they are the defaults a model
reaches for.

- **Honest data**: every number on screen is a real simulated quantity with its unit and state.
  No placeholder readings, no hard-coded statuses, no numbers the simulation doesn't have. Scripts
  can't catch these, so check each value's source yourself.

- **Colour**: tokens only (`var(--…)`). A status colour appears only for the state it names:
  never for hover, decoration or chart series. A nominal reading is plain `--text`, not green. No
  glow, shadow, gradient, blur or transparency on chrome.
- **State** is never shown by colour alone. Pair it with text, a symbol, a position or a shape.
- **Numbers**: a unit on every value; B612 Mono with tabular figures; a fixed field width; the same
  precision for a quantity everywhere. A missing value is `—` in `--text-muted`, never `0`, `NaN`
  or blank. A stale value is muted with a trailing `S`. An estimated one is prefixed `~`.
- **Times** carry their time system label. Bearings are `000°` to `359°`. Countdowns run from
  `T-` to `T+`.
- **Type**: at least `0.875rem`, sized in rem, at most four sizes on a console. No italics. Bold
  for display titles only. Upper case "for display titles, panel titles, field labels, button
  labels and status annunciations of three words or fewer"; mixed case for sentences.
- **Characters**: only what both B612 faces hold, a space being U+0020 or the no-break U+00A0. A
  quantity B612 can't set is named in words (`phase shift`, not `Δφ` or `delta phi`). A formula
  keeps its case and its digits on the line, in an upper-case label too: `CO2`, never `CO(2)`
  or `CO₂`, and `Co` is cobalt. An isotope is `He-3`, never `³He`.
- **Substances** are named through `substanceName` (`lib/substances.ts`, P14.T49.f), never by a
  registry key or any other code name. The phase is a word beside the name (`liquid CH4`, never
  `CH4(l)`), an ion is singly charged with its sign on the line (`Fe+`, `H−`), and a fraction is a named quantity
  (`mass fraction 75 %`, never `75 wt%`, `% w/w`, `% by mass` or `ppmv`).
- **Signs and temperatures** (decision-ux-numbers-names): a number's sign is the hyphen-minus `-`;
  the minus sign U+2212 is an ion's charge or an operator. A temperature is in kelvin, never °C.
- **Voice**: terse, literal, impersonal, as in `NO CARRIER`. No "I", "please", "you", `!`,
  "Loading…" or "thinking…". The guide allows only abbreviations on a ship-wide nomenclature list,
  which doesn't exist yet: bring a new abbreviation to the owner rather than coining one.
- **Alerts** come from the server, never from a console. Each names the system, what is wrong and
  what to do. Only emergency and warning flash or use reverse video. Use `role="alert"` for
  emergency and warning and `role="status"` for the rest; live values go in `output` or a live
  region.
- **Never**: spinners, skeleton loaders, toasts, hamburger menus, emoji, or a modal dialog for
  anything short of `ARM`/`EXECUTE`.
- **Commands** are closed-loop: `PENDING`, then the server's result. Never update optimistically.
  A hazardous command takes `ARM`, then `EXECUTE`. Controls that change the ship look different
  from controls that only change the view.
- **Motion** shows a change of state and nothing else: 80–150 ms, no idle animation, and honour
  `prefers-reduced-motion`. Live readouts update at about 4 Hz with no tweened digits.
- **Input**: everything works from the keyboard, focus shows as a 2px `--accent` outline, targets
  are at least 2rem, and nothing depends on hover or a right click alone.
- **Spatial displays** always show scale, orientation and reference frame. They are true to scale,
  or say what isn't (`BODIES NOT TO SCALE`). Fast graphics go on a canvas, and text meant to be read
  stays in the DOM.

## When the guide doesn't cover it

New displays will hit gaps: a new unit, a new glyph, a new kind of display. The guide is the
owner's to change. Ask the user rather than inventing a convention. There is one exception: a
plan task that specifies a guide edit (as galaxy-generation's P05.T2 does). Make exactly that
edit, and keep items marked "needs the owner's confirmation" in a commit of their own. Never edit
the guide to make code pass a check.

## Checks

Run these before calling UI work done. The `ux-reviewer` agent runs them too.

Run them from the repository root:

```bash
python3 .claude/skills/console-ux/scripts/ux_lint.py            # changed renderer files; or pass paths, or --all
python3 .claude/skills/console-ux/scripts/contrast.py           # every token pairing the guide requires
python3 .claude/skills/console-ux/scripts/contrast.py text-muted surface-2          # one text pairing (6:1)
python3 .claude/skills/console-ux/scripts/contrast.py line-strong surface-2 --min 3  # an outline (3:1)
python3 .claude/skills/console-ux/scripts/glyphs.py "☉ ↑ µ"     # can the bundled B612 fonts draw these?
```

- `ux_lint.py` is a heuristic. It exits 1 on any error, and 2 when it couldn't run fully (a bad
  path, or the fonts missing, which `pnpm install` fixes). `glyphs.py` and `contrast.py` use the
  same exit codes. An `error` breaks a must-or-never rule as written. A `check` needs judgement: a
  `50%` radius may be a contact symbol, and a hover handler may have a keyboard twin. Fix and re-run
  until it is clean, or until every remaining line is a justified exception with a code comment
  giving the reason. The guide lets a "should" rule be broken with a stated reason.
- Its typography and substance rules read displayed strings. Errors: a formula's count in
  parentheses or subscripts, a raised isotope, ion sign or charge number, a state symbol, a
  qualified per cent and a snake_case key. Checks: a formula whose case was changed (`MG2SIO4`,
  `Co2`), `Fe3+`, `H-` with a hyphen, `3He`, a per cent with no space, a symbol spelled by its
  letters' names, `10^`, `x 10-6`, `1e-3`, `c_p`, `<=`, U+2212 before a digit, `°C`, and a
  `${…material}`, `${…species}` or `${…substance}` not passed to `substanceName`. They skip
  developer text: the smoke pages, provenance and identifier fields (`source`, `paper`, `basis`,
  `key`, ...) and shader sources. The descent spike (`view/spike/`) is a display, and is read.
- Run `contrast.py` whenever a token changes or a new foreground and background meet. Text needs
  6:1 and control outlines 3:1.
- Run `glyphs.py` for any character outside ASCII before it reaches the screen, spaces
  included: it skips only the ASCII whitespace that separates its arguments, so
  `glyphs.py '\u2009'` (a thin space) fails.
  A glyph B612 lacks falls back to a system font, so name the quantity in words (guide,
  Typography); only a unit symbol or a key's own sign is drawn as an inline SVG. Look in
  `components/` and the plan for an existing drawn glyph first (galaxy-generation's P05.T3.b plans
  `SunGlyph` for `☉`).
- After changing a script, run its tests (no step of `just ci` runs them):
  `python3 -B .claude/skills/console-ux/scripts/test_ux_lint.py`, and `test_glyphs.py` beside it.

The scripts cannot see layout, data states or command flow. When the change is visible, run the
client (`just server` and `just client`) and check it at 1920×1080 and 1280×720 if you can take a
screenshot. If you can't, say that the visual check is still to do. Component tests that query by
role and accessible name also confirm that controls have names.
