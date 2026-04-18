# Refactoring Opportunities

A scan of the tree for dead code, duplication, and cleanup candidates. Each item is a discussion starter, not a commitment — some will turn out to be intentional structure, others are quick wins.

Scan date: 2026-04-18. Compiler warnings at the time of writing: 0.

## Duplication

### 1. `create_test_config` copied across 8 files

Eight near-identical test config builders exist:

- `src/config.rs:910` — `create_test_game_config` (the canonical one, `pub`)
- `src/blueprint.rs:441`
- `src/compiler.rs:531`
- `src/api/blueprints.rs:458`
- `src/api/factions.rs:71`
- `src/api/teams.rs:394`
- `src/api/ships.rs:137`
- `tests/integration_tests.rs:34`

Every time a field is added to `GameConfig`, all eight need updating. `config::create_test_game_config` is already `pub` — the other seven should import it (plus a `with_*` builder for per-test overrides) and disappear.

### 2. Shield→hull→status-effect damage application

`src/simulation/systems.rs`:

- `damage_system` at lines 180–191
- `beam_weapon_system` at lines 274–285

The two blocks are byte-for-byte identical except for scaling `damage * delta_time` in the beam case. Extract a helper like `apply_damage_result(&mut shield, &mut ship_data, effects, result)` so the two systems share a single application path. A bug fix in one won't silently miss the other.

### 3. `WarpDriveComponent` vs `JumpDriveComponent`

`src/simulation/components.rs:696` and `:743`. Both carry startup/cooldown timers, a `disabled` flag, and `can_*` / progress methods with matching semantics. Candidates:

- Share a `DriveTimer { startup, startup_progress, cooldown, cooldown_progress, disabled }` sub-struct and compose.
- Or keep them separate but extract a `DriveState` trait with `tick(dt)`, `can_engage()`, `is_cooling_down()`.

Worth doing before a third drive type (impulse boost? phase drive?) lands and triples the copy count.

### 4. `get_*` / `get_*_mut` / `get_*_by_name` / `get_all_*` fourfold across entities

`src/state.rs` repeats this shape for players, teams, blueprints, ships, stations (lines 181, 272, 391, 433, 485). Five entity types × ~4 accessors = ~20 near-identical methods. Options:

- A `declare_entity_accessors!` macro keyed on field name + type.
- A generic `Registry<T: HasId>` that exposes the four accessors once.

Not urgent, but any new entity type (factions? projectiles?) will re-pay the tax.

### 5. `ok_or(Status::NotFound)` scattered across 28 API handler sites

28 occurrences across `src/api/*` (heaviest in `blueprints.rs` with 9). A thin extension trait or helper — `fn find_blueprint(world, id) -> Result<&mut ShipBlueprint, Status>` — would centralize the "not found" path and make it easy to, e.g., add logging or a consistent JSON error body later.

### 6. Validation method skeleton in `blueprint.rs`

`validate_players_and_roles`, `validate_required_modules`, `validate_max_allowed`, etc. (lines 218–375 of `src/blueprint.rs`) all share the pattern "iterate collection, push to `errors: Vec<ValidationError>`". Not the worst duplication, but a `Validator` builder (or even just a single `for_each_*_error(&self, &mut Vec<ValidationError>)` signature) would make adding new validation kinds trivial.

## Dead code

### 7. `ModuleSlot` legacy struct in `src/models.rs:31`

Marked "Legacy export for backward compatibility". A grep for `models::ModuleSlot` returns zero hits — every consumer uses `config::ModuleSlot` instead. Delete.

### 8. `#[allow(dead_code)]` pragmas

Three survivors, each a candidate for deletion or re-integration:

- `src/config.rs:459` — generic `load_directory()` helper, unused.
- `src/config.rs:607` — `load_weapons()`, superseded by `ModuleVariants` migration (see TODO on line 254).
- `src/api/modules.rs:111` — `determine_category()` with a special-case for "maneuvering thrusters" suggesting a half-finished refactor.

### 9. Unused `_blueprint` parameter in `compiler::initialize_ship_status`

`src/compiler.rs:237`. The underscore prefix suppresses the warning, but the parameter is wired through the call site and fed a real `&ShipBlueprint`. Either the function should use it (most likely — initialization probably wants blueprint-level config) or the caller should stop threading it. Current state is the worst of both: looks used, isn't.

### 10. `get_player_ships` stub

`src/state.rs:456` — `_player_id` unused, returns empty `Vec`. Anything that calls this today is silently getting nothing back. Either implement it or mark it `todo!()` so callers fail loudly.

### 11. Commented-out ship test block

`src/state.rs:1122–1170`, gated by "Re-enable ship tests once Ship::new() constructor is implemented". If the constructor has landed, uncomment; if not, file an issue and delete the comment block — commented tests rot silently.

## Parallel systems mid-migration

### 12. `weapon_definitions` → `ModuleVariants`

`src/config.rs:254` has a TODO: "Remove weapon_definitions once weapon migration to ModuleVariants is complete." Until that happens the tree carries two sources of truth for weapon data, and `load_weapons` (item 8) is the stranded half. Worth finishing the migration and deleting the legacy path in one pass.

### 13. Simulation TODOs cluster in `simulation/systems.rs`

~15 TODOs in `src/simulation/systems.rs` covering power/cooling enforcement, ship spawn events, targeting, ammunition, area-of-effect damage, point defense, repair, scanning, inter-ship messaging. Individually each is a feature gap, but collectively they mean the simulation crate advertises systems that are stubs. Tracking these in a single "simulation completeness" epic (instead of inline TODOs) would make it easier to see what actually runs.

## Suggested order of attack

1. **Item 1** (test config) — biggest maintenance pain, mechanical to fix, touches no runtime logic.
2. **Item 7** (`ModuleSlot` legacy) — pure deletion, zero risk.
3. **Item 2** (damage helper) — small extraction, correctness payoff (keeps the two systems in sync).
4. **Item 9** (`_blueprint`) — quick decision: use it or drop it.
5. **Item 12** (weapon migration) — unblocks items 8 and reduces config surface area.

Everything else (macros for state accessors, drive-component abstraction, validation builder) is worth doing but should wait until the surrounding code is a bit more settled, so we're not abstracting over a moving target.
