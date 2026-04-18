# Simulation Completeness Tracking

Status of the ECS systems in [`src/simulation/systems.rs`](../../src/simulation/systems.rs). Each entry captures what is currently implemented, what is stubbed or hard-coded, and the gap that separates the two. Inline `TODO:` markers in that file have been replaced with a pointer back to this document.

This is a visibility doc, not a plan: it does not prescribe an order of implementation. Pick items off it as related features come online (ship parent-child relationships, blueprint-driven module configuration, crew assignments, etc.), since most of these stubs are blocked on infrastructure rather than on the systems themselves.

Plan date: 2026-04-18. Derived from Phase 7 of [`01-refactoring-action-plan.md`](01-refactoring-action-plan.md).

---

## Systems

### `power_system`

- **Current**: Iterates each `PowerGrid` and computes `total_allocated()` into a discarded binding. No enforcement.
- **Gap**: If modules allocate more power than the grid produces, nothing throttles or brownouts them. Needs a redistribution pass (priority-weighted shedding) and a signal back to affected modules.
- **Blocked on**: Module priority metadata and a brownout concept on `PowerGrid`.

### `cooling_system`

- **Current**: Mirrors `power_system` — sums `total_allocated()` and discards it. No overheating consequences.
- **Gap**: Modules with insufficient cooling should accumulate heat and eventually take damage or disable. Needs a per-module heat accumulator and failure thresholds.
- **Blocked on**: Heat/temperature state on modules and damage hooks.

### `shield_system`

- **Current**: Fully implemented for regeneration. Scales regen by `available_power / power_draw` and clamps to `max_strength`.
- **Gap**: None at this layer. Shield *damage absorption* lives in `damage_system` / `beam_weapon_system`.

### `weapon_cooldown_system`

- **Current**: Fully implemented. Delegates to `WeaponComponent::update_cooldown`.
- **Gap**: None.

### `weapon_fire_system`

- **Current**: Auto-fires weapons when `is_automatic && is_active && can_fire()`. Spawns a kinetic projectile with the weapon's own entity as owner.
- **Gap**:
  - No **targeting**: projectiles are fired with no `target`, so `damage_system` never matches them against a ship.
  - No **ammunition check**: weapons that should draw from a ship magazine fire freely.
  - No **parent ship context**: uses the weapon entity as the projectile owner because weapons aren't children of ships yet.
- **Blocked on**: Parent-child relationships between ships and their weapons, targeting component on ships, ammunition inventory on ships.

### `damage_system`

- **Current**: Distance-based collision against `projectile.target`. Applies weapon-tag damage to shield, spills remainder to hull, applies status effects, despawns the projectile.
- **Gap**: Collision distance is a hard-coded `10.0` stand-in for real ship/projectile radii. Should read radius from `ShipData` / `ProjectileComponent` (or a dedicated `Collider` component).

### `projectile_system`

- **Current**: Advances lifetime and despawns expired projectiles. Leaves the velocity update to `movement_system`.
- **Gap**: `ProjectileType::Missile { thrust, turn_rate }` is matched but both fields are ignored — missiles fly in a straight line. Needs homing logic that rotates velocity toward the target at `turn_rate` under `thrust`.
- **Blocked on**: Nothing; `target` is already on `ProjectileComponent`.

### `beam_weapon_system`

- **Current**: Continuous `base_damage * delta_time` application while target is within a hard-coded 5000.0 range. Otherwise mirrors `damage_system`.
- **Gap**: Range is a literal, not pulled from the beam's weapon configuration. Also shares the duplicated "apply to shield then hull, then status effects" block with `damage_system` (see refactoring plan Phase 3 / item 2).

### `explosion_system`

- **Current**: Empty. Single-target damage for missiles/torpedoes happens via `damage_system`.
- **Gap**: No area-of-effect damage. Missiles and torpedoes should emit an explosion event on impact that radiates damage to ships within a blast radius (likely with falloff).
- **Blocked on**: A way for `damage_system` to signal "projectile X detonated at position P with AoE profile Y" — probably an `Events<Explosion>` channel.

### `status_effect_system`

- **Current**: Fully implemented. Ticks down durations and removes expired effects.
- **Gap**: None.

### `countermeasure_system`

- **Current**: Point-defense weapons despawn the first in-range missile/torpedo they find each tick.
- **Gap**:
  - No **friend/foe filtering**: a ship's own PD can intercept its own missiles.
  - **Range** is a hard-coded `1000.0` instead of being drawn from the PD weapon stats.
  - No **accuracy roll**: interception is always successful if in range.
  - No **ammunition/cooldown enforcement** on the PD weapon beyond `can_fire()`.
- **Blocked on**: Parent-child relationships (for owner filtering) and per-weapon range/accuracy stats.

### `repair_system`

- **Current**: Empty placeholder.
- **Gap**: Damaged modules should regain health over time when a crew member is assigned to repair them, at a rate driven by crew skill and available parts.
- **Blocked on**: Crew assignment model and module damage state.

### `scanning_system`

- **Current**: Empty placeholder.
- **Gap**: Science officers should be able to initiate scans on targets and, over time, reveal ship data (modules, crew, faction, status effects). Needs a `ScanProgress` component and a `RevealedInfo` fog-of-war layer.
- **Blocked on**: Target acquisition pipeline and the fog-of-war / knowledge model.

### `communication_system`

- **Current**: Empty placeholder. `CommunicationState::can_communicate()` already answers the "am I jammed?" question; there's nothing consuming it.
- **Gap**: No actual message delivery between ships. Needs an events/inbox model gated by `can_communicate()` and possibly range/relay checks.
- **Blocked on**: A message type and an inter-ship event bus.

### `warp_system`

- **Current**: Fully implemented. Handles Tachyon disable, startup progress, cooldown, and velocity scaling by warp factor.
- **Gap**: None at the system level. `WarpDriveComponent` and `JumpDriveComponent` share state that's flagged for extraction in refactoring plan Phase 6 / item 3, but that's a structural change, not a missing behavior.

### `jump_system`

- **Current**: Fully implemented. Handles Tachyon disable, charging, instantaneous teleport on completion, and cooldown.
- **Gap**: None at the system level (see note on `warp_system` re: shared state).

### `movement_system`

- **Current**: Fully implemented. Applies linear velocity and angular velocity via `UnitQuaternion::from_scaled_axis`.
- **Gap**: None.

---

## Cross-cutting dependencies

Several gaps above collapse into a handful of missing primitives. Listing them here so the tracking entries don't have to repeat themselves:

- **Ship ↔ weapon parent-child relationships.** Unblocks owner filtering in `countermeasure_system`, targeting and ammunition in `weapon_fire_system`, and per-ship PD/ammo stats.
- **Event channels** (`Events<Explosion>`, `Events<ShipMessage>`, etc.). Unblocks `explosion_system` and `communication_system` cleanly without over-fetching queries.
- **Fog-of-war / knowledge model.** Unblocks `scanning_system` and makes `communication_system` more than a pure delivery mechanism.
- **Crew assignment model.** Unblocks `repair_system` and eventually feeds into scanning and damage control.
- **Per-module heat/priority metadata.** Unblocks meaningful `power_system` and `cooling_system` enforcement.
