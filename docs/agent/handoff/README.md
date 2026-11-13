# Handoff: where development stands

Written 2026-10-28 by the orchestrating agent, when the owner moved development to another machine.
It is for the next orchestrating agent. Read it first, then the plans it points to. The plans'
records are the source of truth: this file only says where things stand and what comes next.

## 1. Start here

- **Integration branch:** `rendering-and-planets`, at `d43678e3` when this was written. `main` is the
  repository's default branch; the work happens on `rendering-and-planets`.
- **History is linear.** Land a lane by cherry-picking its non-merge commits onto the tip. Never
  make a merge commit on the integration branch, and never force-push.
- **The gate:** `just smart-ci origin/rendering-and-planets` (add `--hook-checks` for Rust lanes).
  Push only after the gate's log ends `smart-ci exit 0`, checked in its own step; then
  `git push --no-verify`, since the gate already ran the hooks' checks. The owner runs the full
  `just ci` elsewhere.
- **Landing shortcuts** (no second gate): the picked tree equals the gated tree; or the lane's
  files are disjoint from the integration branch's newer commits, the diff is equal, and nothing
  in those newer commits imports or is imported by what the lane changes (grep both ways for
  shared types, APIs, wire shapes and generated bindings); or the only difference is docs that
  pass Prettier. Anything else gates the picked tree.
- **Commits:** conventional, `type(scope): <task ID> summary`, ending with exactly
  `Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>`. Lanes never use
  `--no-verify`. Never put Claude session links or `Claude-Session` trailers anywhere.
- **Finding tasks:** `python3 -I .claude/skills/implement-task/scripts/plan_task.py --list R08`
  lists a plan's tasks and marks those with commits; `<ID> --context` and `<ID> --records` print a
  task's text and its as-built records. Implement with the `implement-task` skill; review with
  `review-changes`.
- **Record files:** plans R09 and R10 keep each task's as-built records in
  `<plan-stem>/records/<KEY>.md`; every other plan still keeps them at the end of its Risks
  section. The tooling reads both layouts.

## 2. The owner's standing rules

- **Features and critical bug fixes come first.** Record figure mismatches and land, unless one is
  a blocker. A blocker is only: a wrong state or class; a crash or non-finite value; a broken
  contract; a visibly wrong picture; a determinism failure; a physically absurd result. Cost alone
  is not a blocker. At most one ruling per figure.
- **Decisions are delegated.** Rulings and sign-offs go to dedicated decision agents (a
  general-purpose agent when the ruling writes files). Only security, hardware or fundamental
  project changes go to the owner.
- **The owner's display is never touched.** No `xset`, `xrandr` or `xdotool`. Electron and the
  smoke harness run hidden and offscreen (`show: false`); nothing is shown, focused, mapped, moved
  or resized. A GPU run may set `DISPLAY` only for Chromium's GPU initialisation.
- **Every agent run is memory-capped** (on Linux, a `systemd-run --user --scope` with
  `MemoryMax`). Long or heavy runs are serialised; timed GPU measurements happen only in quiet
  windows with every other lane paused.
- **Linux, macOS and Windows** must all build and lint.
- **The repository is public.** No personal details, home paths, host names or session links in
  commits, issues or files.
- **Generation must stay deterministic** and bit-identical across native and WebAssembly. Goldens
  are re-blessed by test name, never with a bare `just bless`, and `golden_diff.py` must say
  "extended only" unless a version bump is under way.

## 3. Versions

- `GENERATOR_VERSION` is **23**; `PROTOCOL_VERSION` is **3**.
- **Version 24 is built and closed.** Its members sit on the integration branch behind sixteen
  holds (`*_VERSION` = 24 constants with tripwires): RAYLEIGH_FLOOR, ESCAPE, PRIMORDIAL_HELIUM,
  UNLIT_MEAN, MOON_BULK, COLD_TRAPS, READ_DENSITIES, COMPRESSED_LIQUIDS, ROW_DARKENING,
  AIR_SECTIONS, OCEAN_FLOOR, TENUOUS, SMALL_BODY_TYPES, OCEAN, BELT_POPULATION and
  WEATHERED_AREA_MIX, each switched in tests by `with_every_24_hold`. P14.T54.b's batches B1–B3
  (subtasks a–k) have landed, and the last blocker (P14.T53.a's thick-CO₂ surfaces) is fixed.
- **The next task is P14.T54.b.l, "B4", the bump to 24** (plan 14, `galaxy-generation/14-…`).
  It deletes every 24 hold with its tripwire and switch; re-blesses the goldens and the protocol
  fixtures at 24, keeping `with_every_25_hold` (which wraps the 24 helper) and
  `STEAM_ENVELOPE_VERSION` = 25 working; splits the six fast-tier tests that run over 120 s under 24's rules (an
  invariance test may split only into disjoint sample chunks that each keep the full per-record
  check; an order-independence test is never split and gets a slow tier instead); and writes the
  bless note. Its inputs (the deletion list of 124 tests, the re-bless list, the bless note's
  contents, and the gotchas) are in the local records (§9): the B4 handoff and the p14-escape
  handoff's B4 section.
- **After the bump:** split plan 14's records into per-task files (the owner approved; it waits
  for a quiet plan 14), then version 25.
- **Version 25's train** (on the integration branch behind 25 holds, or on lane branches):
  - P14.T53.b part 3, the steam envelope (`STEAM_ENVELOPE_VERSION`, `with_every_25_hold`),
    already landed. 25's bump must add back the floor-test exemption for world 0900, and a
    test-only follow-up restates its condensation-cap slope criterion.
  - Galaxy-side holds are allowed through a `GalaxyRules` value that the `Galaxy` itself
    carries (never a thread-local switch): P09.T28.b.b (`KEPLER_REGIME_VERSION`) and P09.T28.d's
    secular term. Stellar members (P06.T19.e, P15.T3.c, P11.T4.l–o, the A2/A4 swaps, P15.T7.b's
    swap, P06.T28.g with T24.b) stay on lane branches until their train.
  - Riders: P14.T31.c (storms only on giants with a water deck; the storm clock's period floored at
    10 years), P14.T49.p (the molten refractories' optics and the molten salts' densities, a
    precondition of R08.T16.a), and the molten-O₂ surface follow-up for thick airs.
  - The trains after it: a 25-hour production fit run, then 26, 27 and 28 (plan 09's catalogue
    classes wire in at 26–27).

## 4. RM4: atmospheres (plan R08)

- **The frame budget** (high, 1080p, RTX 3080): target 15.67 ms p95, blocker 16.67 ms.
  QW33 (the last timed run, on the RTX 3080 under Electron 45, two runs judged by the worse p95; a row
  stalled in one run is judged from the other, and one stalled in both by its medians) left one row
  over the blocker: the slab Earth at 10⁷ m, x4 (p95 18.32 and 16.59 ms, medians 16.42–16.63). 86 of
  92 rows meet 15.67 ms. **The next R08 task is the edged one-band march for that row**: a march
  variant compiled for media with one slice and one band but layer edges, exact, from the tables
  alone, adding two kernels to R01.T15's bounded cache, checked byte for byte in smoke part 8.
  Its untimed estimate is −1.15 to −1.95 ms. Its brief is in the R08.T11.f handoff (§9). TEST
  SATURN's x4 row meets (16.61 ms); its lever, a sliced edge-free march, is recorded only.
- **Landed:** R08.T11.c (QW29's measurement); T11.d (the march over each air's screen area with
  one kernel, and a march specialised for one-band, edge-free media; the tile cull kept behind a
  switch, off); T11.f as a docs-only record (its disc correction met its accuracy but cost the RTX
  more than it saved, and was dropped); T13 entire (the regime boundary and its call sites: Earth,
  Mars and Saturn-class airs now carry `ATMOSPHERE: APPROXIMATE` until the bake routes them); T14.a,
  T14.b, T14.c (the thick read path, not yet routed) and T14.e (the polarisation correction); and
  T10.c–f (the APPROXIMATE plate's reasons, with no paper citations on screen).
- **Next:** R08.T14.f (the correction's read path and the vector gates), then R08.T14.d (the
  gates that route `thickScattering` worlds through the bake). T14.d has five recorded
  preconditions: the bake holds azimuthal mode m ≥ 2 (m = 3 if needed); Mars is routed only if its
  gate passes (its forward-peaked dust is worse through the bake today); the surface lighting reads
  the same multiple scattering as the sky; routing meets the 16.67 ms blocker (with the thick read
  forced, the class map at 10⁷ m x4 sat at 16.45–16.51 ms; two levers are recorded: the altitude
  row found once per sample, and T14.f's summed m = 0..2 table); and the rest as plan 08 records.
- **Then:** T15 (the cloud-deck split), T16 (gas giants; T16.a waits on P14.T49.p, on a review of
  plan 14's deck model's sedimentation share, and on two recorded opacity bounds), T18 (thermal
  emission), T4.f, T5.e, and T17 (the verification pass).
- **Visual checks still to do by eye** (hidden captures): the APPROXIMATE plate's reason lines at
  1920×1080 and 1280×720, and TEST SATURN's datum in both styles, equator-on and pole-on.

## 5. RM5: generated worlds (plans R09 and R10)

- R10.T16.g (the datum stroked inside the figure) and R02.T19 (the datum on the body's spheroid)
  have landed; RM4's close no longer waits on them.
- Still open: R09.T7.g, R09.T7.k, R09.T13.b–d and R10.T15.b (the served-world pass, after 24).
  R09's follow-ups (valley depth, C_c, neighbours' trunks, playa base level, has_sea) ride a later
  bump.

## 6. Galaxy generation close-out (plans P01–P15)

- **Plan 09, the catalogue-class chain** (the close-out's longest pole):
  - P09.T17 (DeathMarks, `claims`, `SupernovaInterval`) and P09.T32 (the class grid:
    `ClassProcess`, `ClassLookup`, `resolve` with its visitors, `cells_in_sphere`, the headroom
    check) have landed. Their signatures are the interfaces T33–T38 build on. Every kind of
    candidate takes its velocity from one function.
  - Next: P09.T33.a, which registers the first class, wires `placement::resolve`'s dispatch and
    adds `SystemOrigin::CatalogueSystem`; then T33.b, T34.a, T43, and T35 (wired at 26).
    T32–T35 and T43 need not wait for the galactic centre; only T36.b, T37 and T38's centre
    volumes do.
- **Plan 09, the centre:** P09.T28.b.a (the pure Kepler regime; its propagator's bisection
  fallback) has landed. P09.T29 (the range query at the centre:
  `CentreMemberSource`, not yet registered; a per-cell orbit cache; `Galaxy::centre()`) has landed.
  Its benchmark still needs a quiet window: a probe measured about 100 ms warm against a 10 ms
  target, and 17.6 s cold, nearly all of it member placement. Then P09.T28.b.b behind its 25 hold, P09.T30.b and P09.T31.
- **Plan 10:** re-validated against the code. P10.T3.d.a (pure) and P10.T7.a (the dwarf-core
  model, with the ruled metallicity spread) are ready.
- **Plan 06:** P06.T28.c (magnetar rates, ruled), T28.e and T28.f are ready; P06.T41 is held;
  T42 and T43 split the accepted asks into pure halves and swaps.
- **Plan 11:** P11.T4.l → T4.m → T4.n → T4.o ride 25 on a lane branch; T8.a–d follow plan 09's
  chain.
- **Plan 15:** P15.T5.a's 10⁷ run (before the 1,000 km/s truncation; an unlocked, capped fit run
  outside any bless window).

## 7. The laptop issue lane

A second machine works owner-authored `agent-task` GitHub issues through the `work-issue` skill.
The orchestrator writes briefs with `brief-issue`, lands them with `land-issue`, reads the queue
with `survey-issues`, and posts status to the coordination discussion with `post-status`. Briefs
must be self-contained, since the worker sees only the issue and the repository. That machine can't
pass a gate that runs `just test-render`, so test-render work stays on the main machine. A sim gate
there takes about two hours; label such issues `heavy`. No issue is open now; the lane was told to
stop.

## 8. Open questions for the owner

- Whether to set pnpm's `minimumReleaseAgeStrict: true`. pnpm 12 silently adds release-age
  exemptions when installing a version younger than a day.
- Electron 45.1.0's release-age exemption in `pnpm-workspace.yaml` is temporary: remove it after
  2026-10-28 19:57 UTC, then run `pnpm install` and check that the lockfile doesn't change.
- On the laptop, a permission guard refuses the `review-changes` skill (it calls `git -C` on the
  shared checkout); the worker runs the reviewers directly.
- rust-analyzer's flycheck runs an uncapped `cargo check --workspace` (about 2 GB).
- The Earth-class bake ruling amended the rendering brainstorm (the analytic term is replaced
  wherever it drifts, not only at Venus-class depths). It was judged not fundamental; the owner
  may override.

## 9. The local records (carried separately)

The orchestration records lived under the old machine's `.git/` directory and are not in the
repository. The owner carries them as an archive. Its layout:

- `rm45-orchestration/`: the current initiative.
  - `status.md`: the dated log of everything.
  - `decision-*.md`: every ruling by name. The plans cite them as "decision-<name>".
  - `brief-notes-24.md`: the serve-gate counts and must-fixes for 24.
  - `prebless-24-list.md`: what to re-bless at 24.
  - `bump-25-list.md`: what 25 must carry.
  - `docs-batch-24-queue.md`: queued docs and infra items, each marked TAKEN once briefed.
  - `common.md`: the lane rules.
  - `handoff/handoff-<lane>.md`: one per lane, with its next task. B4's handoff is
    `handoff-p14-wire-24.md`, plus the B4 section of `handoff-p14-escape.md`.
  - `galaxy-ready-2026-10-27.md`: the close-out survey, with what's ready and what's blocked.
- `rm23-orchestration/`: `deferred-corrections.md`, and the machine's run-slot scripts
  (`bin/build-slot`, `bin/gate-slot`). These are machine-specific; a new machine needs its own
  equivalents.
- `rm1-orchestration/`: the first initiative's records.
- `scratch/`: the small text inputs the handoffs cite, for example B4's deletion list and the
  frame-timing harness scripts.
- `memory/`: the previous agent's memory files: the owner's preferences and lessons.

## 10. Infra items queued

- SwiftShader's `exit_code=512` in test-render is Chromium's "hung" code: the GPU watchdog ending a
  long synchronous pipeline compile under load. Options: a longer watchdog for the software adapter
  only, or asynchronous pipeline creation in the smoke harness. A rerun recovers it.
- nextest may lower a test's `threads-required` to the test-thread cap, so tests with their own
  pools can oversubscribe a small machine. Check and cap them.
- `plan_split.py`'s `--keep` doesn't persist between runs.
- `brief_issue.py`'s task-ID pattern rejects sub-subtask IDs such as `P09.T28.b.a`.
- test-render's longest part is part 7, at about 122 s against the 180 s watchdog.
