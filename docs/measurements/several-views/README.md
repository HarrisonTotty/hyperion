# Several views results

The recorded runs of plan R07's by-hand checks of several views
([`07-lit-bodies-styles-and-main-screen.md`](../../agent/plans/rendering-and-planets/07-lit-bodies-styles-and-main-screen.md)):
T20, a photorealistic primary and two wireframe instruments in `VIEW`, and T21, an instrument view
in a child window on a second monitor. R12 folds them into `docs/measurements/rendering/`.

## For the owner: the runs

Each is one command on a quiet machine: no other agent, lane, build or test running, and the load
average under 1 when it starts (README, "Measurements on a quiet machine"). Commit what a run
writes here.

### R07.T20, on the RTX 3080 and on the UHD 620

```sh
just views-check                  # the RTX 3080: the high setting, a 1920 × 1080 px window
just views-check --setting low    # the UHD 620: the low setting, a 1280 × 720 px window
```

The run takes about four minutes and drives itself; no server is needed. `VIEW` opens on the kept
`PHASE TEST` scene and goes through five phases, each settled for 5 s and then traced, its 20 s
window measured from a second after the trace starts:

1. the photorealistic primary alone;
2. the primary with both instruments open in the wireframe (T20's cockpit); after it the primary's
   stage narrows to 80% and 65% of its width and widens again, a step every 2 s;
3. the primary in the wireframe, with two wireframe instruments;
4. instrument 1 photorealistic beside a wireframe instrument 2;
5. the wireframe primary alone.

Then the cockpit is set up again and a dialog asks whether every view is the right way up.
Checklist:

- [ ] Leave the window alone while the phases run (about three and a half minutes); don't cover
      it, move it or resize it.
- [ ] Watch the resize after phase 2: only the primary changes size; the instruments keep theirs
      and nothing flickers or tears.
- [ ] At the dialog, click each view in turn and hold an arrow key a moment: as a camera turns up
      the bodies move down, and as it turns right they move left, in every view, and nothing is
      upside down or mirrored. Then answer **Yes** or **No** (**Skip** leaves it unanswered). The
      dialog waits three minutes.
- [ ] The run ends with `views check: exit 0`. Commit the two files it names.

On the UHD 620, R07.T17 has not yet made `VIEW`'s quality setting reach its sky and its
photorealistic frame: `--setting low` gives the budgets the low setting (one photorealistic view,
the photorealistic primary at 30 Hz) and the 1280 × 720 window, but the sky and the frame draw at
`high` until T17 lands; the record says so and is provisional. Whether to run it before T17 at
all is pending your ruling (R07's Risks); a run before it is to be repeated after.

`--hidden` runs the same phases offscreen, with no presentation times and no question;
`--smoke` runs them in 2 s windows, hidden, and writes under `target/views-check/`. Neither is a
T20 result.

### R07.T21, with a second display

Connect a second display and extend the desktop onto it (ideally at another refresh rate than the
first, so that the pacing can tell the two apart), then:

```sh
just child-window-check --seconds 60
```

It opens a window on the primary display and, a moment later, a child window on the second
display; the child is resized after 30 s, closes at 60 s, and the main window draws on for 2 s.
With one display it refuses (exit 2) and opens nothing. Checklist:

- [ ] The child window stands on the second display and draws a green marker in its top left.
- [ ] The main window draws throughout, before and after the child closes.
- [ ] The record's `T21 no GPU-process exit` line reads 0: the run is R07's on-screen check of a
      child window under the Vulkan surface (R07's Risks, "Hidden-window resizes restart the GPU
      process").
- [ ] The run ends with `child window check: exit 0`. Commit the record it names.

`--hidden` puts an offscreen child on the one display instead, to prove the harness; it writes
under `target/views-check/`.

## Files

A T20 run is two files, written by the client's main process (`apps/hyperion/src/main/
viewsCheckResults.ts`):

- `<date>-<machine>-<setting>.json`: the results file, schema `hyperion.views-check.results`,
  version 1, a few tens of kilobytes. A second run of the same day, machine and setting gets `-2`,
  and so on; a smoke run's name ends `-smoke`.
- `<date>-<machine>-<setting>.md`: its summary: T20's checks with their verdicts, the phases, the
  views, and the by-eye items.

A T21 run writes `<date>-<machine>-child-window.md` (`-hidden` for the hidden variant): the
machine, the displays, then every check of the harness with its figures, the GPU process's exits
among them. `<machine>` is the host name,
lower-cased, with anything but `a`–`z`, `0`–`9` and `-` dropped. The PNG of each phase's last
frame goes to `target/views-check/captures-<time>/` and is never committed.

## What a T20 results file holds

Every figure is `{ "value": …, "reason": null }`, or `{ "value": null, "reason": "…" }` with the
reason it is missing, as the descent spike's are.

| Part                         | Holds                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `run`                        | The machine (CPU, threads, memory, governor, load average at the start, Chromium's GPU and driver), the versions, the platform and launch mode, the setting, every switch applied, whether the window was shown, its size, the device-pixel ratio, the pass timer, the display's rate and vsync period, the captures' directory, and whether the run is provisional                                                                       |
| `phases`                     | Per phase: whether it held its configuration, the primary's rate and T, each view's style, canvas, draws, GPU time and submission time a draw, its passes' labels and its scene target's scale; the intervals between animation frames, between the primary's draws and between presentations; the main thread's and the GPU's time a frame; the GPU process's time a frame and its copy slices; Design note 21's frame and headroom rows |
| `findings.rightWayUp`        | The person's answer at the dialog                                                                                                                                                                                                                                                                                                                                                                                                         |
| `findings.copies`            | The views' passes named as copies (none is the pass), and the GPU process's copy slices a frame by phase                                                                                                                                                                                                                                                                                                                                  |
| `findings.resize`            | Every texture made or destroyed during the resize, by the view it belongs to, and each view's canvas at each step: the pass is the primary's attachments made again and none of the instruments'                                                                                                                                                                                                                                          |
| `findings.frameTime`         | T20's cockpit against Design note 21: the low setting's 33 ms on the UHD 620                                                                                                                                                                                                                                                                                                                                                              |
| `findings.lowCases`          | The two wireframe-primary phases against Design note 21 at 60 Hz, with what a miss does (T20; decision-r07-t18, items 4 and 5)                                                                                                                                                                                                                                                                                                            |
| `findings.perCanvasOverhead` | Per pair of phases (a primary alone, then with the two instruments): the GPU process's main-thread time a primary draw, without and with, and the difference per canvas; the instruments' own GPU time a draw beside it; and the larger pair's figure, an upper bound, against the client's `PER_CANVAS_OVERHEAD_MS` (0.3 ms, provisional)                                                                                                |
| `findings.passTimer`         | The pass timer's console warnings ("dropping some", "not timed") and the resolves it dropped: none is the pass                                                                                                                                                                                                                                                                                                                            |
| `findings.faults`            | The engine's faults and the GPU process's exits                                                                                                                                                                                                                                                                                                                                                                                           |

How the figures are read:

- **T** is the display's vsync period from its refresh rate (`Display.displayFrequency`), or twice
  it for a photorealistic primary on the low setting, which draws at 30 Hz. The 95th-percentile
  row is T + 1 ms at 60 Hz and 35 ms at 30 Hz (R05 Design note 21). A hidden run has no window, so
  T and the presentation figures are missing with "no window shown".
- **The pass timer** is lifted to `full` by the client's `--hyperion-gpu-timing` switch, which a
  check launch applies on the Vulkan path; elsewhere it may be `quantized` or `absent`, and then
  the GPU rows say so.
- **The windows:** every trace figure is clipped to the phase's measured window, which the page
  marks with a `performance.measure` span, so the trace's figures and the page's frames cover one
  span; the trace records R05's five categories for timed runs, without `gpu`.
- **The primary** fills `VIEW`'s stage beside its side columns, not the whole window: the record
  gives every canvas's size. The window asks for its size in device pixels; a tiling window
  manager may give it its tile instead, and the record reads the size it ended at.
- **The per-canvas overhead** is an upper bound: the GPU process's main-thread time a primary draw
  that each canvas adds, the decoding of the instruments' own commands as well as their
  presentation, which the pass timer cannot see. Which figure replaces `PER_CANVAS_OVERHEAD_MS` is
  the owner's ruling (R07's Risks, "Deviations in the T20 and T21 harnesses").
- **A quiet machine:** a run started with the load average at or above 1, a hidden run, a smoke
  run and, until R07.T17, a low-setting run are marked provisional; none of the first three is a
  T20 result.
