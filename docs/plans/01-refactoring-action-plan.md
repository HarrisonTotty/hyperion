# Refactoring Action Plan

A phased rollout of the findings in [`../brainstorming/refactoring-opportunities.md`](../brainstorming/refactoring-opportunities.md). Item numbers below reference that document.

Ordering principle: ship cheap/low-risk deletions first, then test infrastructure that unlocks future work, then correctness fixes, then the in-flight migration, and finally the larger structural abstractions. Each phase is independently mergeable and passes `just check`.

Plan date: 2026-04-18.

---

## Phase 1 — Dead code & stubs (zero-risk deletions)

**Goal:** Remove code that no caller depends on, and make stub behavior explicit.

**Items:** 7, 10, 11.

**Scope:**

- **Item 7** — Delete the `ModuleSlot` struct at `src/models.rs:31`. No `pub use` for it exists, so only the definition needs to go. Re-confirm zero hits for `models::ModuleSlot` before removing.
- **Item 10** — `src/state.rs:456` `get_player_ships` currently returns an empty `Vec` for any caller. There is one live caller at `src/websocket.rs:305` that silently iterates the empty result. Decide: implement it (preferred — walk `ships` for matches on `player_roles`), or replace the body with `todo!("not yet implemented")` so the websocket path fails loudly.
- **Item 11** — `src/state.rs:1122–1158` commented-out tests (`test_ship_registration`, `test_ship_queries`). If `Ship::new()` exists now, uncomment and fix compile errors. If not, delete the block and open a tracking issue rather than leaving dead comments in the tree.

**Validation:** `just check` passes. For item 10, whichever path is chosen must not silently change behavior for live callers.

**Dependencies:** None. Can ship as a single PR.

**Estimated size:** < 100 LOC net deletion.

**Carry-over (item 11):** Landed in 95a4473 by deletion. `Ship::new()` does not exist, so `test_ship_registration` and `test_ship_queries` have no constructor to exercise. When a `Ship` constructor lands, re-add coverage for: registering a ship and retrieving it via `get_ship` / `ship_exists`; querying by team via `get_team_ships`; querying by player via `get_player_ships` (the latter now has a real implementation per item 10 and deserves direct test coverage). Reference text of the original block lives in the git history at 95a4473^:src/state.rs.

---

## Phase 2 — Test config consolidation

**Goal:** One source of truth for `create_test_game_config` so adding a `GameConfig` field is a one-file change.

**Items:** 1.

**Scope:**

- Make `config::create_test_game_config` the canonical builder (already `pub`).
- Introduce a small set of `with_*` helpers on `GameConfig` (or a `TestConfigBuilder`) for the per-test overrides currently baked into each copy. Audit each of the eight copies to find divergent fields.
- Delete the seven duplicates:
  - `src/blueprint.rs:441`
  - `src/compiler.rs:531`
  - `src/api/blueprints.rs:458`
  - `src/api/factions.rs:71`
  - `src/api/teams.rs:394`
  - `src/api/ships.rs:137`
  - `tests/integration_tests.rs:34`
- Update each call site to use the canonical version + builders.

**Validation:** `just test` — every test that previously used a local builder must still pass without behavior changes. Diff each replaced test's seed values against the canonical one and record any intentional differences.

**Dependencies:** None, but do this before Phase 3–7 so those phases don't have to touch eight copies.

**Estimated size:** 8 files touched, mostly deletions plus builder glue.

---

## Phase 3 — Correctness fixes

**Goal:** Close two small correctness gaps before they drift further.

**Items:** 2, 9.

**Scope:**

- **Item 2** — Extract a helper like `apply_damage_result(&mut shield, &mut ship_data, effects, result)` in `src/simulation/systems.rs` and call it from both `damage_system` (fn at line 146, duplication block at 180–191) and `beam_weapon_system` (fn at line 241, duplication block at 274–285). Preserve the `damage * delta_time` scaling by computing the scaled result at the call site. Verify tick-by-tick damage output is unchanged against existing tests.
- **Item 9** — `src/compiler.rs:235` `initialize_ship_status` takes `_blueprint: &ShipBlueprint` but never reads it. Decide whether initialization should use blueprint data (likely yes — check what fields are blueprint-driven). If yes, thread the relevant fields through. If no, drop the parameter and update callers.

**Validation:** `just check` + specifically the simulation systems tests. For item 2, consider adding a unit test that exercises both damage paths through the same helper to lock in the shared behavior.

**Dependencies:** None. Should follow Phase 2 to avoid test-config churn, but not strictly blocked.

**Estimated size:** Item 2 is ~30 LOC extraction. Item 9 is a small-but-decision-heavy change.

---

## Phase 4 — Finish the `weapon_definitions` → `ModuleVariants` migration

**Goal:** Eliminate the dual source of truth for weapon data.

**Items:** 12, plus the tail end of 8.

**Scope:**

- Complete the migration flagged by the TODO at `src/config.rs:254`. Identify every reader of `weapon_definitions` and move them to `ModuleVariants`.
- Delete `weapon_definitions` and the stranded `#[allow(dead_code)] load_weapons` at `src/config.rs:607`.
- While in the file, also delete `#[allow(dead_code)] load_directory` at `src/config.rs:459` if still unused, and audit `determine_category` at `src/api/modules.rs:111` — either finish the refactor it was part of or delete the special case. Note: `determine_category` has an active unit test at `src/api/modules.rs:225` (`test_determine_category`) — if the function is deleted, the test must go with it.

**Validation:** `just check`. Load a known weapon config in an integration test to confirm round-trip still works. Grep for any remaining `weapon_definitions` references post-change.

**Dependencies:** Should land after Phase 2 (one `create_test_config` is much easier to re-seed with the new weapon shape).

**Estimated size:** Medium. The migration itself is the bulk; the deletions are small.

---

## Phase 5 — API & validation helpers

**Goal:** Reduce repetition in handler code so future handlers (and error-response improvements) land in one place.

**Items:** 5, 6.

**Scope:**

- **Item 5** — Introduce a thin lookup layer for the 28 `ok_or(Status::NotFound)` sites in `src/api/*`. Options: a `WorldExt` extension trait (`fn find_blueprint(&self, id) -> Result<&ShipBlueprint, Status>`), or per-entity helpers. Pick whichever minimizes churn. Start with the heaviest file (`blueprints.rs`, 9 sites) to validate the shape before rolling out to the rest.
- **Item 6** — Introduce a `Validator` or a shared `for_each_*_error(&self, &mut Vec<ValidationError>)` signature in `src/blueprint.rs` so `validate_players_and_roles`, `validate_required_modules`, `validate_max_allowed`, etc. (lines 218–375) share a skeleton. Aim for "adding a new validation rule is one function, not one function plus boilerplate."

**Validation:** `just check`. For item 5, a smoke test that a missing entity still returns `404` with the same body as before.

**Dependencies:** Phase 2 (shared test config) makes the API tests easier to rewrite during the helper rollout.

**Estimated size:** Item 5 is ~28 call sites but mechanical. Item 6 is a design choice followed by a narrow rewrite of one module.

---

## Phase 6 — Structural abstractions

**Goal:** Reduce duplication in state accessors and drive components, but only once the surrounding code has settled.

**Items:** 3, 4.

**Scope:**

- **Item 4** — `src/state.rs` repeats `get_* / get_*_mut / get_*_by_name / get_all_*` for five entity types. Pick one of:
  - `declare_entity_accessors!` macro keyed on field name + type.
  - `Registry<T: HasId>` generic that exposes the four accessors once and is embedded for each entity.

  Either way, migrate entities one at a time so bisecting regressions stays easy.

- **Item 3** — `WarpDriveComponent` (`src/simulation/components.rs:696`) and `JumpDriveComponent` (`:743`) share timer/disabled state and `can_*` methods. Extract a `DriveTimer` sub-struct or a `DriveState` trait. Do this before a third drive type lands.

**Validation:** `just check`, plus specifically the state and simulation tests. For item 4, a before/after diff on entity accessor call sites should be mechanical.

**Dependencies:** Phase 2 (test config) and Phase 5 (API helpers) should ideally land first — both phases touch state and API code that this one restructures.

**Estimated size:** Larger. Worth scoping into two PRs (one per item).

---

## Phase 7 — Simulation completeness tracking

**Goal:** Make the state of simulation stubs legible at a glance.

**Items:** 13.

**Scope:**

- Audit the ~15 TODOs in `src/simulation/systems.rs` (power/cooling enforcement, ship spawn events, targeting, ammunition, AoE damage, point defense, repair, scanning, inter-ship messaging).
- Convert inline TODOs to a single tracking document (`docs/plans/simulation-completeness.md` or an issue tracker epic) with a line per system and its current implementation state.
- Replace inline `TODO:` markers with `// See plans/simulation-completeness.md` or leave a one-line note pointing to the tracking doc.

**Validation:** `just check`. Not a behavior change — just a documentation/visibility one.

**Dependencies:** None, but lowest priority — this is a hygiene task, not a correctness one.

**Estimated size:** Small, mostly prose.

---

## Phase ordering at a glance

| Phase | Items | Risk | Unblocks |
|-------|-------|------|----------|
| 1 | 7, 10, 11 | None | — |
| 2 | 1 | None | 3, 4, 5 |
| 3 | 2, 9 | Low | — |
| 4 | 12, tail of 8 | Medium | — |
| 5 | 5, 6 | Low | 6 |
| 6 | 3, 4 | Medium | — |
| 7 | 13 | None | — |

Phases 1, 2, 3, 7 can land in any order. Phase 4 benefits from Phase 2 landing first. Phase 6 should wait for Phases 2 and 5.
