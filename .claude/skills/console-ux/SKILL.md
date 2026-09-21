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

Read the guide in full the first time you do UI work in a session. It is about 300 lines, and
every rule carries weight. Then look at how the existing code does things, and reuse its pieces
rather than inventing parallel ones:

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
  for display titles only. Labels are UPPER CASE (three words or fewer); sentences are mixed case.
- **Voice**: terse, literal, impersonal, as in `NO CARRIER`. No "I", "please", "you", `!`,
  "Loading…" or "thinking…". Use only abbreviations on the nomenclature list.
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
  or labelled `NOT TO SCALE`. Fast graphics go on a canvas, and text meant to be read stays in the
  DOM.

## When the guide doesn't cover it

New displays will hit gaps: a new unit, a new glyph, a new kind of display. The guide is the
owner's to change. Ask the user rather than inventing a convention. There is one exception: a
plan task that specifies a guide edit (as P05.T2 does). Make exactly that edit, and keep items
marked "needs the owner's confirmation" in a commit of their own. Never edit the guide to make
code pass a check.

## Checks

Run these before calling UI work done. The `ux-reviewer` agent runs them too.

```bash
python3 ${CLAUDE_SKILL_DIR}/scripts/ux_lint.py                     # changed renderer files; or pass paths, or --all
python3 ${CLAUDE_SKILL_DIR}/scripts/contrast.py                    # every token pairing the guide requires
python3 ${CLAUDE_SKILL_DIR}/scripts/contrast.py text-muted surface-2  # one pairing (tokens or #hex)
python3 ${CLAUDE_SKILL_DIR}/scripts/glyphs.py "☉ ↑ µ"              # can the bundled B612 and B612 Mono draw these?
```

- `ux_lint.py` is a heuristic and exits 1 on any error. An `error` breaks a must-or-never rule as
  written. A `check` needs judgement: a `50%` radius may be a contact symbol, and a hover handler
  may have a keyboard twin. Fix and re-run until it is clean, or until every remaining line is a
  justified exception with a code comment giving the reason. The guide lets a "should" rule be
  broken with a stated reason.
- Run `contrast.py` whenever a token changes or a new foreground and background meet. Text needs
  6:1 and control outlines 3:1.
- Run `glyphs.py` for any character outside ASCII before it reaches the screen. A glyph B612 lacks
  falls back to a system font, so draw it as an inline SVG. For `☉` the galaxy-generation plan
  specifies this in P05.T2.d, and P05.T3.b builds `SunGlyph` and `SolarMassUnit`; reuse them once
  they exist.

The scripts cannot see layout, data states or command flow. When the change is visible, run the
client (`just server` and `just client`) and check it at 1920×1080 and 1280×720 if you can take a
screenshot. If you can't, say that the visual check is still to do. Component tests that query by
role and accessible name also confirm that controls have names.
