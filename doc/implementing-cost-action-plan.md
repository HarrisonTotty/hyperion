# Implementing Credit Cost - Hyperion Action Plan

This document outlines the detailed implementation plan for adding credit-based costs to the Hyperion server. This complements the high-level overview in `/home/lain/gh/frigate/doc/implementing-cost.md`.

## Design Decisions

Based on requirements clarification:

1. **Build Points vs Credits**: Both systems are maintained. Build Points are scoped to a ship class (per-ship constraint), Credits are scoped to a team (shared resource).
2. **Refunds**: Ship destruction or module removal refunds at **100%** of original cost.
3. **Credit Earning**: Teams earn credits through quests and looting ships (out of scope for initial implementation).
4. **Module Slot Costs**: Module slots **do** have credit costs (in addition to their build point costs).

## Current State Analysis

### What Exists

| Component | Has Cost Field | Field Name | Value Type | Currently Exposed in API |
|-----------|----------------|-----------|-----------|--------------------------|
| Ship Classes | Partial | `maintenance_cost` | `Option<f32>` | No |
| Module Slots | Yes | `base_cost` | `i32` | Yes (`/v1/catalog/module-slots/<id>`) |
| Module Variants | Yes | `cost` | `i32` | Yes (`/v1/catalog/modules/<slot>/<id>`) |
| Ammunition | Yes | `cost` | `f32` | Yes (`/v1/catalog/ammo/<cat>/<id>`) |
| Teams | No | N/A | N/A | N/A |
| Game Config | Empty file | N/A | N/A | N/A |

### Key Observations

1. **Ship classes have no construction cost** - Only `maintenance_cost` exists (for lore purposes)
2. **All module slots and variants have `cost`/`base_cost` fields** - These represent **build points**, not credits
3. **Teams have no budget tracking** - Only id, name, faction, members
4. **`data/game.yaml` is empty** - Ready for global game parameters

### Important Distinction: Build Points vs Credits

- **Build Points**: Per-ship constraint limiting module complexity. Defined by ship class's `build_points` field. Module slots contribute via `base_cost`, variants contribute via `cost`. Currently used by frontend to track build point allocation.
- **Credits**: Team-wide resource used to purchase ships, modules, and consumables.

**Approach**: Since the existing `base_cost` and `cost` fields are actively used by the frontend for build point calculations, we must add **new `credit_cost` fields** for credit tracking:

| Component | Build Points Field | Credit Cost Field (New) |
|-----------|-------------------|------------------------|
| Ship Classes | `build_points` | `cost` (new) |
| Module Slots | `base_cost` | `credit_cost` (new) |
| Module Variants | `cost` | `credit_cost` (new) |
| Ammunition | N/A | `credit_cost` (new, or repurpose existing `cost`) |

**Note**: For ammunition, the existing `cost` field may be repurposed as credit cost since ammo doesn't contribute to build points.

---

## Phase 1: Game Configuration

### Task 1.1: Expand `data/game.yaml`

**File**: `data/game.yaml`

Add global game parameters:

```yaml
# Game Settings
# -------------

# Team Economy Settings
team_starting_credits: 1000000  # 1 million credits per team

# Economy Balance Multipliers (optional, for tuning)
economy:
  ship_class_cost_multiplier: 1.0
  module_slot_cost_multiplier: 1.0
  module_cost_multiplier: 1.0
  ammo_cost_multiplier: 1.0
```

### Task 1.2: Create Config Struct for Game Settings

**File**: `src/config/game_settings.rs` (new file)

```rust
//! Global game settings configuration

use serde::{Deserialize, Serialize};

/// Economy balance settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EconomyConfig {
    #[serde(default = "default_multiplier")]
    pub ship_class_cost_multiplier: f32,
    #[serde(default = "default_multiplier")]
    pub module_slot_cost_multiplier: f32,
    #[serde(default = "default_multiplier")]
    pub module_cost_multiplier: f32,
    #[serde(default = "default_multiplier")]
    pub ammo_cost_multiplier: f32,
}

fn default_multiplier() -> f32 { 1.0 }

/// Global game settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSettings {
    /// Starting credits for each new team
    #[serde(default = "default_starting_credits")]
    pub team_starting_credits: i64,

    /// Economy balance configuration
    #[serde(default)]
    pub economy: EconomyConfig,
}

fn default_starting_credits() -> i64 { 1_000_000 }

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            team_starting_credits: 1_000_000,
            economy: EconomyConfig::default(),
        }
    }
}
```

### Task 1.3: Integrate Game Settings into GameConfig

**File**: `src/config/mod.rs`

Add `game_settings` field to `GameConfig` struct and load from `data/game.yaml`.

---

## Phase 2: Ship Class Costs

### Task 2.1: Add `cost` Field to Ship Class YAML Files

**Files**: `data/ship-classes/*.yaml` (16 files)

Add `cost` field to each ship class based on the following scale:

| Ship Class | Size | Suggested Cost |
|------------|------|----------------|
| Scout | Small | 15,000 |
| Interceptor | Small | 18,000 |
| Courier | Small | 12,000 |
| Corvette | Small | 20,000 |
| Escort | Medium | 35,000 |
| Frigate | Medium | 40,000 |
| Destroyer | Medium | 50,000 |
| Battleship | Medium | 55,000 |
| Cruiser | Medium | 60,000 |
| Warship | Medium | 65,000 |
| Defender | Medium | 45,000 |
| Tanker | Medium | 30,000 |
| Transport | Medium | 25,000 |
| Freighter | Large | 70,000 |
| Dreadnought | Large | 90,000 |
| Hyperion | Large | 100,000 |

Example YAML addition:
```yaml
name: Scout
# ... existing fields ...
cost: 15000  # Credit cost to construct this ship class
maintenance_cost: 250.0  # Credits per day (existing field)
```

### Task 2.2: Update Ship Class Config Struct

**File**: `src/config/ship_class.rs`

Add `cost` field to `ShipClassConfig`:

```rust
/// Credit cost to construct a ship of this class
#[serde(default)]
pub cost: i64,
```

### Task 2.3: Update Ship Class API Responses

**File**: `src/api/ship_classes.rs`

Add `cost` to both `ShipClassResponse` and `ShipClassSummary`:

```rust
pub struct ShipClassSummary {
    // ... existing fields ...
    pub cost: i64,  // Add this
}

pub struct ShipClassResponse {
    // ... existing fields ...
    pub cost: i64,  // Add this
}
```

Update the endpoint handlers to include `cost` in responses.

---

## Phase 3: Module Slot Credit Costs

### Task 3.1: Add `credit_cost` to Module Slot YAML Files

**Files**: `data/module-slots/*.yaml` (18 files)

Add `credit_cost` field based on the following scale (1,000-10,000 range):

| Module Slot | Groups | Suggested Credit Cost |
|-------------|--------|----------------------|
| power-core | Support | 3,000 |
| shield-generator | Defense | 4,000 |
| cooling-system | Support | 2,500 |
| impulse-engine | Propulsion | 3,500 |
| warp-jump-core | Propulsion | 8,000 |
| kinetic-weapon | Offense | 5,000 |
| energy-weapon | Offense | 6,000 |
| torpedo-tube | Offense | 7,000 |
| missile-launcher | Offense | 6,500 |
| sensor-array | Support | 2,000 |
| electronic-warfare | Support | 4,500 |
| comms-array | Support | 1,500 |
| cargo-bay | Support | 1,000 |
| hull-plating | Defense | 2,000 |
| damage-control | Support | 2,500 |
| life-support | Support | 1,500 |
| shuttle-bay | Support | 5,000 |
| mining-laser | Support | 4,000 |

Example:
```yaml
id: power-core
# ... existing fields ...
base_cost: 40  # Build points (existing - DO NOT CHANGE)
credit_cost: 3000  # Credit cost (new)
```

### Task 3.2: Update Module Slot Config Struct

**File**: `src/config/module.rs`

Add to `ModuleSlot`:

```rust
/// Credit cost for this module slot
#[serde(default)]
pub credit_cost: i64,
```

### Task 3.3: Update Catalog API

**File**: `src/api/catalog.rs`

Ensure `credit_cost` is included in module slot responses.

---

## Phase 4: Module Variant Credit Costs

### Task 4.1: Add `credit_cost` to Module Variant YAML Files

**Files**: `data/modules/*/*.yaml` (many files)

Add `credit_cost` field based on the following scale (100-1,000 range):

General guidelines:
- Basic/starter variants: 100-200 credits
- Standard variants: 200-500 credits
- Advanced variants: 500-800 credits
- Elite/exotic variants: 800-1,000 credits

Example:
```yaml
id: "mark-iii-fission-reactor"
# ... existing fields ...
cost: 0  # Build points (existing - DO NOT CHANGE)
credit_cost: 150  # Credit cost (new)
```

### Task 4.2: Update Module Variant Config Struct

**File**: `src/config/module.rs`

Add to `ModuleVariant`:

```rust
/// Credit cost for this module variant
#[serde(default)]
pub credit_cost: i64,
```

---

## Phase 5: Ammunition Credit Costs

### Task 5.1: Review Ammo YAML Files

**Files**: `data/ammo/*/*.yaml`

Ammunition already has `cost` field. Since ammunition doesn't contribute to build points, we can **repurpose the existing `cost` field as credit cost** and rebalance values according to:

| Ammo Type | Credit Cost Range |
|-----------|-------------------|
| Kinetic Ammo | 1-10 credits |
| Missile Ammo | 10-100 credits |
| Torpedo Ammo | 10-100 credits |

### Task 5.2: Rebalance Ammo Cost Values

Review all ammunition files and adjust `cost` values to fit within the credit ranges above. The existing values may already be appropriate or may need scaling.

---

## Phase 6: Team Economy System

### Task 6.1: Update Team Model

**File**: `src/models/player.rs`

Add credits field to Team struct:

```rust
pub struct Team {
    pub id: String,
    pub name: String,
    pub faction: String,
    pub members: Vec<String>,
    /// Team's current credit balance
    pub credits: i64,
}
```

### Task 6.2: Update Team Creation

**File**: `src/state/game_world.rs`

When creating a team, initialize with starting credits:

```rust
pub fn create_team(&mut self, name: String, faction: String) -> Result<String, String> {
    // ... existing validation ...

    let team = Team {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        faction,
        members: Vec::new(),
        credits: self.game_settings.team_starting_credits,  // Initialize with starting credits
    };

    // ... rest of function ...
}
```

### Task 6.3: Update Team API

**File**: `src/api/teams.rs`

Update response structs to include credits:

```rust
pub struct TeamResponse {
    pub id: String,
    pub name: String,
    pub faction: String,
    pub members: Vec<String>,
    pub credits: i64,  // Add this
}

pub struct CreateTeamResponse {
    pub id: String,
    pub name: String,
    pub faction: String,
    pub credits: i64,  // Add this
}
```

### Task 6.4: Add Credit Transaction Endpoints

**File**: `src/api/teams.rs` (or new file `src/api/economy.rs`)

Add endpoints for credit operations:

```rust
/// GET /v1/teams/<id>/credits - Get team's credit balance
#[get("/v1/teams/<id>/credits")]
pub fn get_team_credits(...) -> ...

/// POST /v1/teams/<id>/credits/deduct - Deduct credits from team
#[post("/v1/teams/<id>/credits/deduct", data = "<request>")]
pub fn deduct_credits(...) -> ...

/// POST /v1/teams/<id>/credits/add - Add credits to team (for rewards)
#[post("/v1/teams/<id>/credits/add", data = "<request>")]
pub fn add_credits(...) -> ...
```

---

## Phase 7: Credit Transactions (Deduction and Refunds)

### Task 7.1: Add Credit Transaction Methods to GameWorld

**File**: `src/state/game_world.rs`

```rust
/// Deduct credits from a team
pub fn deduct_team_credits(&mut self, team_id: &str, amount: i64) -> Result<i64, String> {
    let team = self.teams.get_mut(team_id)
        .ok_or_else(|| format!("Team {} not found", team_id))?;

    if team.credits < amount {
        return Err(format!("Insufficient credits: have {}, need {}", team.credits, amount));
    }

    team.credits -= amount;
    Ok(team.credits)
}

/// Add credits to a team (for rewards, refunds, etc.)
pub fn add_team_credits(&mut self, team_id: &str, amount: i64) -> Result<i64, String> {
    let team = self.teams.get_mut(team_id)
        .ok_or_else(|| format!("Team {} not found", team_id))?;

    team.credits += amount;
    Ok(team.credits)
}

/// Refund credits to a team (100% refund rate)
pub fn refund_team_credits(&mut self, team_id: &str, amount: i64) -> Result<i64, String> {
    self.add_team_credits(team_id, amount)
}
```

### Task 7.2: Update Ship Creation Flow

When a player creates a ship (selects ship class), deduct ship class cost:

**File**: `src/api/ships.rs` or `src/api/blueprints.rs`

```rust
pub fn create_ship(
    world: &State<SharedGameWorld>,
    config: &State<GameConfig>,
    request: Json<CreateShipRequest>,
) -> Result<...> {
    let ship_class = config.get_ship_class(&request.ship_class_id)?;
    let team = world.get_team(&request.team_id)?;

    // Check if team has enough credits
    if team.credits < ship_class.cost {
        return Err("Insufficient credits");
    }

    // Deduct cost
    world.deduct_team_credits(&request.team_id, ship_class.cost)?;

    // Create ship
    // ...
}
```

### Task 7.3: Implement Refund on Ship Destruction/Deletion

When a ship is destroyed or deleted, refund the full cost:

```rust
pub fn delete_ship(
    world: &State<SharedGameWorld>,
    config: &State<GameConfig>,
    ship_id: &str,
) -> Result<...> {
    let ship = world.get_ship(ship_id)?;
    let ship_class = config.get_ship_class(&ship.class_id)?;

    // Calculate total refund: ship class + all module slots + all module variants
    let mut total_refund = ship_class.cost;

    for instance in &ship.module_instances {
        if let Some(slot) = config.get_module_slot(&instance.module_slot_id) {
            total_refund += slot.credit_cost;
        }
        if let Some(variant_id) = &instance.variant_id {
            if let Some(variant) = config.get_module_variant(variant_id) {
                total_refund += variant.credit_cost;
            }
        }
    }

    // Refund at 100%
    world.refund_team_credits(&ship.team_id, total_refund)?;

    // Delete ship
    world.delete_ship(ship_id)?;

    Ok(())
}
```

### Task 7.4: Implement Refund on Module Slot Removal

When a module slot is removed from a ship, refund slot + variant cost:

```rust
pub fn remove_module_instance(
    world: &State<SharedGameWorld>,
    config: &State<GameConfig>,
    ship_id: &str,
    instance_id: &str,
) -> Result<...> {
    let ship = world.get_ship(ship_id)?;
    let instance = ship.get_instance(instance_id)?;

    // Calculate refund
    let mut refund = 0i64;

    if let Some(slot) = config.get_module_slot(&instance.module_slot_id) {
        refund += slot.credit_cost;
    }
    if let Some(variant_id) = &instance.variant_id {
        if let Some(variant) = config.get_module_variant(variant_id) {
            refund += variant.credit_cost;
        }
    }

    // Refund at 100%
    world.refund_team_credits(&ship.team_id, refund)?;

    // Remove instance
    world.remove_module_instance(ship_id, instance_id)?;

    Ok(())
}
```

---

## Phase 8: Testing

### Task 8.1: Unit Tests for Economy System

- Test team creation with starting credits
- Test credit deduction
- Test credit addition
- Test insufficient credits error handling

### Task 8.2: Integration Tests

- Test ship creation deducts credits
- Test ship class API returns cost
- Test module slot API returns credit_cost
- Test module variant API returns credit_cost

### Task 8.3: YAML Validation

- Ensure all ship classes have valid cost values
- Ensure all module slots have credit_cost
- Ensure all module variants have credit_cost

---

## Implementation Order

1. **Phase 1**: Game Configuration (foundation for all other phases)
2. **Phase 6.1-6.2**: Team Model & Creation (foundation for credit tracking)
3. **Phase 2**: Ship Class Costs (YAML + Rust + API)
4. **Phase 3**: Module Slot Costs (YAML + Rust + API)
5. **Phase 4**: Module Variant Costs (YAML + Rust + API)
6. **Phase 5**: Ammunition Costs (rebalance existing `cost` values)
7. **Phase 6.3-6.4**: Team API Updates
8. **Phase 7**: Credit Transactions (deduction on creation, refunds on deletion)
9. **Phase 8**: Testing

---

## Files to Modify Summary

### New Files
- `src/config/game_settings.rs`

### Config/YAML Files
- `data/game.yaml` - Add `team_starting_credits`
- `data/ship-classes/*.yaml` (16 files) - Add `cost` field
- `data/module-slots/*.yaml` (18 files) - Add `credit_cost` field
- `data/modules/*/*.yaml` (many files) - Add `credit_cost` field
- `data/ammo/*/*.yaml` - Rebalance existing `cost` values

### Rust Source Files
- `src/config/mod.rs` - Integrate game settings
- `src/config/game_settings.rs` (new) - Game settings struct
- `src/config/ship_class.rs` - Add `cost: i64` field
- `src/config/module.rs` - Add `credit_cost: i64` to ModuleSlot and ModuleVariant
- `src/models/player.rs` - Add `credits: i64` to Team struct
- `src/state/game_world.rs` - Credit transaction methods, team creation with starting credits
- `src/api/ship_classes.rs` - Expose `cost` in responses
- `src/api/catalog.rs` - Expose `credit_cost` in responses
- `src/api/teams.rs` - Expose `credits` in responses, add credit transaction endpoints
- `src/api/ships.rs` or `src/api/blueprints.rs` - Credit deduction on ship creation, refund on deletion

---

## Resolved Design Questions

1. **Build Points vs Credits**: ✅ Both systems maintained. Build Points are per-ship (scoped to ship class), Credits are per-team (shared resource).

2. **Refunds**: ✅ Ship destruction or module removal refunds at **100%** of original cost.

3. **Credit Earning**: ✅ Teams earn credits through quests and looting ships (out of scope for initial implementation).

4. **Module Slot Credit Costs**: ✅ Module slots **do** have credit costs in addition to build point costs.

---

## Open Questions (Future Consideration)

1. **Persistence**: How should team credit balances be persisted? (Database, file, in-memory only?) - Current approach uses in-memory state.

2. **Transaction Logging**: Should credit transactions be logged for audit/replay purposes?

3. **Partial Module Variant Changes**: When swapping a module variant for another, should it refund old + charge new, or just charge the difference?
